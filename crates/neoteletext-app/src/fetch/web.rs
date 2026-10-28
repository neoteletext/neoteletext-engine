//! The web agent's loop (area WEB): a service or a channel's folder over
//! the std transport, its pages and notices sent to the decoder.

use super::decoder_link::{page_request, send};
use super::transport::{READ_CHUNK, StdTransport, SystemRandom, web_address};
use super::wakes::{Wake, Wakes};
use super::{CHANNEL_PREFIX, clock, neottx};
use crate::pipe::{self, Frame, Notice};
use neoteletext_core::types::Instant;
use neoteletext_web::agent::{
    Config, Event, Failure, Input, InputMessage, Output, PageBytes, Source as Came, Storage, UserAgent,
};
use neoteletext_web::descriptor::Descriptor;
use neoteletext_web::http::MediaType;
use neoteletext_web::port::{ConnectionId, Pump};
use neoteletext_web::proxy::Proxy;
use std::cell::Cell;
use std::net::IpAddr;
use std::sync::mpsc::Sender;
use std::time::Instant as StdInstant;

/// Cookie slots and bytes of the jar: REQ-WEB-006's default total.
const COOKIE_SLOTS: usize = 100;
const COOKIE_BYTES: usize = 32 * 1024;

/// The agent the loop runs.
type Agent<'a> = UserAgent<'a, SystemRandom, COOKIE_SLOTS, COOKIE_BYTES>;

pub(super) fn run_web(
    address: &str,
    wakes: &mut Wakes,
    wake: Sender<Wake>,
    allow_local: &[IpAddr],
    proxy: Option<Proxy>,
) {
    let mut storage = Box::new(Storage::default());
    let channel = address.strip_prefix(CHANNEL_PREFIX);
    let config = agent_config(channel.is_some(), allow_local, proxy.is_some());
    let mut agent: Box<Agent<'_>> = Box::new(UserAgent::new(config, storage.buffers(), SystemRandom));
    let mut pump = Pump::new(StdTransport::new(allow_local, wake, proxy));
    let start = StdInstant::now();
    let mut buffer = vec![0u8; READ_CHUNK];
    let broken = Cell::new(false);
    let service_ready = Cell::new(false);
    // The push connection opened (true) or was lost (false), for the
    // decoder to know when keys may go (area INT).
    let push_state: Cell<Option<bool>> = Cell::new(None);
    let mut next_tick: Option<Instant> = Some(Instant(0));
    let sink = |output: Output<'_>, pump: &mut Pump<StdTransport>, next_tick: &mut Option<Instant>| {
        if let Some(net) = output.net() {
            pump.execute(net);
            return;
        }
        if forward(output, next_tick, &service_ready, &push_state).is_err() {
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
            && agent.descriptor().is_some_and(Descriptor::input)
            && send(&Frame::Notice(Notice::Input(open))).is_err()
        {
            break;
        }
        if service_ready.replace(false) && announce_service(agent.descriptor()).is_err() {
            break;
        }
        while let Some(frame) = wakes.next_request() {
            busy = true;
            let now = clock(start);
            let more = wakes.next_is_input();
            hand_frame(&mut agent, now, frame, more, &mut |output| sink(output, &mut pump, &mut next_tick));
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

/// The agent's configuration: this program's name, the proxy, the local
/// addresses it may reach and, for a channel, redirects kept inside it.
fn agent_config(channel: bool, allow_local: &[IpAddr], proxied: bool) -> Config {
    let mut config = Config {
        // An address naming a channel's folder keeps redirects inside it
        // (REQ-WEB-021).
        redirects_within_channel: channel,
        product: neottx(),
        proxied,
        ..Config::default()
    };
    for (slot, &address) in config.allowed_local.iter_mut().zip(allow_local) {
        *slot = Some(web_address(address));
    }
    config
}

/// Acts on one of the agent's outputs that is not for the network: a timer
/// moves the next tick, pages and notices go to the decoder, and a new
/// service or a push connection's change is noted for the loop.
fn forward(
    output: Output<'_>,
    next_tick: &mut Option<Instant>,
    service_ready: &Cell<bool>,
    push_state: &Cell<Option<bool>>,
) -> std::io::Result<()> {
    match output {
        Output::Timer { at } => {
            *next_tick = Some(next_tick.map_or(at, |current| current.min(at)));
            Ok(())
        }
        Output::PageBytes(page) => send_page(&page),
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
        Output::Event(Event::PageCurrent { page, .. }) => send(&Frame::Notice(Notice::Current(page.mpp()))),
        Output::Event(Event::FetchFailed { page, failure: Failure::Network | Failure::Timeout, .. }) => {
            send(&Frame::Notice(Notice::Offline(page.mpp())))
        }
        Output::Event(Event::FetchFailed { page, failure, .. }) => send(&Frame::Notice(Notice::Message(format!(
            "Page {} not used: {failure:?}",
            String::from_utf8_lossy(&page.ascii())
        )))),
        Output::Event(
            event @ (Event::DescriptorRejected(_)
            | Event::DescriptorFailed(_)
            | Event::Blocked { .. }
            | Event::DowngradeRefused { .. }
            | Event::LinkUnknown),
        ) => send(&Frame::Notice(Notice::Message(format!("{event:?}")))),
        _ => Ok(()),
    }
}

/// Sends a page the agent received to the decoder, with where it came
/// from and whether it came securely.
fn send_page(page: &PageBytes<'_>) -> std::io::Result<()> {
    let data = pipe::PageData {
        secure: page.secure,
        origin: match page.source {
            Came::Page { .. } => pipe::Origin::Page,
            Came::Push => pipe::Origin::Push,
            Came::FrontDoor => pipe::Origin::FrontDoor,
        },
        bytes: page.bytes.to_vec(),
    };
    match page.media {
        MediaType::T42 => send(&Frame::T42(data)),
        _ => send(&Frame::Tti(data)),
    }
}

/// Tells the decoder about a (new) service's refresh period, linked
/// services and aliases before any of its pages (Appendix E).
fn announce_service(descriptor: Option<&Descriptor>) -> std::io::Result<()> {
    let names: Vec<String> = descriptor
        .map(|descriptor| {
            descriptor
                .services()
                .map(|service| String::from_utf8_lossy(service.short_name.as_bytes()).into_owned())
                .collect()
        })
        .unwrap_or_default();
    let aliases: Vec<(u16, String)> = descriptor
        .map(|descriptor| {
            descriptor.aliases().map(|alias| (alias.page.mpp(), descriptor.alias_target(alias).to_string())).collect()
        })
        .unwrap_or_default();
    // A service that pushes keeps its pages current itself.
    let refresh =
        descriptor.filter(|descriptor| descriptor.push().is_none()).and_then(|descriptor| descriptor.refresh_seconds);
    if let Some(seconds) = refresh {
        send(&Frame::Notice(Notice::Refresh(seconds)))?;
    }
    send(&Frame::Notice(Notice::Service(names)))?;
    send(&Frame::Notice(Notice::Aliases(aliases)))
}

/// Gives the agent what the decoder asked for: a page, a key, or a link to
/// another service. `more` says another input message is already waiting,
/// to be written with this one (REQ-RCH-010).
fn hand_frame(agent: &mut Agent<'_>, now: Instant, frame: Frame, more: bool, out: &mut dyn FnMut(Output<'_>)) {
    match frame {
        Frame::Request(mpp, sub) => {
            if let Some((page, subcode)) = page_request(mpp, sub) {
                agent.handle(now, Input::FetchPage { page, subcode }, out);
            }
        }
        Frame::Input(message) => {
            agent.handle(now, Input::Send(InputMessage { bytes: &message, move_of: None, more }), out);
        }
        Frame::Follow(service, mpp, sub) => {
            if let Some((page, subcode)) = page_request(mpp, sub) {
                agent.handle(now, Input::FollowLink { service: &service, page, subcode }, out);
            }
        }
        _ => {}
    }
}
