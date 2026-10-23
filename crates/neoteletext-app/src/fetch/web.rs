//! The web agent's loop (area WEB): a service or a channel's folder over
//! the std transport, its pages and notices sent to the decoder.

use super::decoder_link::{page_request, send};
use super::transport::{READ_CHUNK, StdTransport, SystemRandom, web_address};
use super::wakes::{Wake, Wakes};
use super::{CHANNEL_PREFIX, clock, neottx};
use crate::pipe::{self, Frame, Notice};
use neoteletext_core::types::Instant;
use neoteletext_web::port::{ConnectionId, Pump};
use neoteletext_web::proxy::Proxy;
use std::net::IpAddr;
use std::sync::mpsc::Sender;
use std::time::Instant as StdInstant;

// One event loop; splitting it would scatter the agent's sink and state.
#[allow(clippy::too_many_lines, reason = "one event loop; splitting it would scatter the agent's sink and state")]
pub(super) fn run_web(
    address: &str,
    wakes: &mut Wakes,
    wake: Sender<Wake>,
    allow_local: &[IpAddr],
    proxy: Option<Proxy>,
) {
    use neoteletext_web::agent::{Event, Failure, Input, Output, Source as Came, Storage, UserAgent};
    use neoteletext_web::form::{FormMethod, FormSubmission};
    use neoteletext_web::http::MediaType;
    /// Cookie slots and bytes of the jar: REQ-WEB-006's default total.
    const COOKIE_SLOTS: usize = 100;
    const COOKIE_BYTES: usize = 32 * 1024;
    let mut storage = Box::new(Storage::default());
    let mut config = neoteletext_web::agent::Config::default();
    let channel = address.strip_prefix(CHANNEL_PREFIX);
    // An address naming a channel's folder keeps redirects inside it
    // (REQ-WEB-021).
    config.redirects_within_channel = channel.is_some();
    config.product = neottx();
    config.proxied = proxy.is_some();
    for (slot, &address) in config.allowed_local.iter_mut().zip(allow_local) {
        *slot = Some(web_address(address));
    }
    let mut agent: Box<UserAgent<'_, SystemRandom, COOKIE_SLOTS, COOKIE_BYTES>> =
        Box::new(UserAgent::new(config, storage.buffers(), SystemRandom));
    let mut pump = Pump::new(StdTransport::new(allow_local, wake, proxy));
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = std::cell::Cell::new(false);
    let service_ready = std::cell::Cell::new(false);
    // The push connection opened (true) or was lost (false), for the
    // decoder to know when keys may go (area INT).
    let push_state: std::cell::Cell<Option<bool>> = std::cell::Cell::new(None);
    let mut next_tick: Option<Instant> = Some(Instant(0));
    let sink = |output: Output<'_>, pump: &mut Pump<StdTransport>, next_tick: &mut Option<Instant>| {
        if let Some(net) = output.net() {
            pump.execute(net);
            return;
        }
        let result = match output {
            Output::Timer { at } => {
                *next_tick = Some(next_tick.map_or(at, |current| current.min(at)));
                Ok(())
            }
            Output::PageBytes(page) => {
                let data = pipe::PageData {
                    secure: page.secure,
                    origin: match page.source {
                        Came::Page { .. } => pipe::Origin::Page,
                        Came::Push => pipe::Origin::Push,
                        Came::Form => pipe::Origin::Form,
                        Came::FrontDoor => pipe::Origin::FrontDoor,
                    },
                    url: page.url.to_string(),
                    bytes: page.bytes.to_vec(),
                };
                match page.media {
                    MediaType::T42 => send(&Frame::T42(data)),
                    _ => send(&Frame::Tti(data)),
                }
            }
            Output::Event(Event::ServiceReady { initial_page, name, .. }) => {
                service_ready.set(true);
                send(&Frame::Notice(Notice::ServiceName(name.to_string())))
                    .and_then(|()| send(&Frame::Notice(Notice::InitialPage(initial_page.mpp()))))
            }
            Output::Event(Event::RedirectToOtherChannel { location }) => {
                send(&Frame::Notice(Notice::RedirectToOtherChannel(location.to_string())))
            }
            Output::Event(Event::BundleFailed(failure)) => {
                send(&Frame::Notice(Notice::Message(format!("index.t42 was not used: {failure:?}"))))
            }
            Output::Event(Event::Unencrypted { .. }) => send(&Frame::Notice(Notice::Unencrypted)),
            Output::Event(Event::PushConnected(_)) => {
                push_state.set(Some(true));
                Ok(())
            }
            // A bad message keeps the connection and says so with no wait.
            Output::Event(Event::PushLost { retry_in_us, .. }) if retry_in_us > 0 => {
                push_state.set(Some(false));
                Ok(())
            }
            Output::Event(Event::ConfirmSubmission { host, plain_http, first_to_origin, .. }) => {
                send(&Frame::Notice(Notice::Confirm(host.to_string(), plain_http, first_to_origin)))
            }
            Output::Event(Event::PageCurrent { page, .. }) => send(&Frame::Notice(Notice::Current(page.mpp()))),
            Output::Event(Event::FetchFailed { page, failure: Failure::Network | Failure::Timeout, .. }) => {
                send(&Frame::Notice(Notice::Offline(page.mpp())))
            }
            Output::Event(Event::SubmissionSucceeded { .. }) => send(&Frame::Notice(Notice::Submitted(true))),
            Output::Event(Event::SubmissionFailed { .. }) => send(&Frame::Notice(Notice::Submitted(false))),
            Output::Event(Event::FetchFailed { page, failure, .. }) => send(&Frame::Notice(Notice::Message(format!(
                "Page {} not used: {failure:?}",
                String::from_utf8_lossy(&page.ascii())
            )))),
            Output::Event(
                event @ (Event::DescriptorRejected(_)
                | Event::DescriptorFailed(_)
                | Event::Blocked { .. }
                | Event::DowngradeRefused { .. }
                | Event::SubmissionRefused(_)
                | Event::LinkUnknown),
            ) => send(&Frame::Notice(Notice::Message(format!("{event:?}")))),
            _ => Ok(()),
        };
        if result.is_err() {
            broken.set(true);
        }
    };
    let open = channel.map_or(Input::OpenService(address), Input::OpenChannel);
    agent.handle(clock(start), open, &mut |output| sink(output, &mut pump, &mut next_tick));
    while !broken.get() && !wakes.ended() {
        wakes.turn();
        let mut busy = false;
        for connection in [ConnectionId::Fetch, ConnectionId::Push] {
            if let Some(input) = pump.poll(connection, &mut buffer) {
                busy = true;
                agent.handle(clock(start), Input::Net(input), &mut |output| sink(output, &mut pump, &mut next_tick));
            }
        }
        if let Some(open) = push_state.take()
            && agent.descriptor().is_some_and(neoteletext_web::descriptor::Descriptor::input)
            && send(&Frame::Notice(Notice::Input(open))).is_err()
        {
            break;
        }
        if service_ready.replace(false) {
            // Tell the decoder about the (new) service's linked services
            // before any of its pages (Appendix E).
            let names: Vec<String> = agent
                .descriptor()
                .map(|descriptor| {
                    descriptor
                        .services()
                        .map(|service| String::from_utf8_lossy(service.short_name.as_bytes()).into_owned())
                        .collect()
                })
                .unwrap_or_default();
            let aliases: Vec<(u16, String)> = agent
                .descriptor()
                .map(|descriptor| {
                    descriptor
                        .aliases()
                        .map(|alias| (alias.page.mpp(), descriptor.alias_target(alias).to_string()))
                        .collect()
                })
                .unwrap_or_default();
            // A service that pushes keeps its pages current itself.
            let refresh = agent
                .descriptor()
                .filter(|descriptor| descriptor.push().is_none())
                .and_then(|descriptor| descriptor.refresh_seconds);
            if let Some(seconds) = refresh
                && send(&Frame::Notice(Notice::Refresh(seconds))).is_err()
            {
                break;
            }
            if send(&Frame::Notice(Notice::Service(names))).is_err()
                || send(&Frame::Notice(Notice::Aliases(aliases))).is_err()
            {
                break;
            }
        }
        while let Some(frame) = wakes.next_request() {
            busy = true;
            let now = clock(start);
            let mut out = |output: Output<'_>| sink(output, &mut pump, &mut next_tick);
            match frame {
                Frame::Request(mpp, sub) => {
                    if let Some((page, subcode)) = page_request(mpp, sub) {
                        agent.handle(now, Input::FetchPage { page, subcode }, &mut out);
                    }
                }
                Frame::Submit(submission) => {
                    let fields: Vec<(&str, &str)> =
                        submission.fields.iter().map(|(name, value)| (name.as_str(), value.as_str())).collect();
                    let form = FormSubmission {
                        page_url: &submission.page_url,
                        method: if submission.post { FormMethod::Post } else { FormMethod::Get },
                        target: &submission.target,
                        fields: &fields,
                        token: submission.token.as_deref(),
                        action: submission.action.as_deref(),
                    };
                    agent.handle(now, Input::Submit(form), &mut out);
                }
                Frame::Confirm(yes) => agent.handle(now, Input::ConfirmSubmission(yes), &mut out),
                Frame::Input(message) => agent.handle(now, Input::Send(&message), &mut out),
                Frame::Follow(service, mpp, sub) => {
                    if let Some((page, subcode)) = page_request(mpp, sub) {
                        agent.handle(now, Input::FollowLink { service: &service, page, subcode }, &mut out);
                    }
                }
                _ => {}
            }
        }
        if next_tick.is_some_and(|at| clock(start) >= at) {
            // What a tick asks for, a reconnection above all, is taken up
            // on the next turn, not after another wait.
            busy = true;
            next_tick = None;
            agent.handle(clock(start), Input::Tick, &mut |output| sink(output, &mut pump, &mut next_tick));
        }
        if !busy {
            wakes.wait(clock(start), next_tick);
        }
    }
}
