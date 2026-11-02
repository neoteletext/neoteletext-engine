//! The web agent's loop (area WEB): a service or a channel's folder over
//! the std transport, its pages and notices sent to the decoder.

use super::decoder_link::{page_request, send};
use super::transport::{StdTransport, SystemRandom};
use super::wakes::{Wake, Wakes};
use super::{clock, neottx};
use crate::pipe::{self, Frame, Notice};
use neoteletext_core::types::Instant;
use neoteletext_transport::{SOCKET_READ, web_address};
use neoteletext_web::agent::{
    Config, Event, Failure, Input, InputMessage, Output, PageBytes, PushStop, Source as Came, Storage, UserAgent,
};
use neoteletext_web::channel::is_channel_address;
use neoteletext_web::consts::{INVALID_PACKET_REPORT_SECONDS, PUSH_SUBPROTOCOL};
use neoteletext_web::descriptor::Descriptor;
use neoteletext_web::http::MediaType;
use neoteletext_web::port::{ConnectionId, Pump};
use neoteletext_web::proxy::Proxy;
use neoteletext_web::records::redirect::RedirectRecord;
use neoteletext_web::subpages::PartNumber;
use std::cell::Cell;
use std::net::IpAddr;
use std::sync::mpsc::Sender;
use std::time::Instant as StdInstant;

/// Cookie slots and bytes of the jar: REQ-WEB-006's default total.
const COOKIE_SLOTS: usize = 100;
const COOKIE_BYTES: usize = 32 * 1024;

/// Microseconds in a second.
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
/// The window of a report of invalid packets, in the clock's microseconds.
const INVALID_WINDOW_US: u64 = INVALID_PACKET_REPORT_SECONDS * MICROSECONDS_PER_SECOND;

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
    let channel = is_channel_address(address);
    let config = agent_config(channel, allow_local, proxy.is_some());
    let mut agent: Box<Agent<'_>> = Box::new(UserAgent::new(config, storage.buffers(), SystemRandom));
    let mut pump = Pump::new(StdTransport::new(allow_local, wake, proxy));
    let start = StdInstant::now();
    let mut buffer = vec![0u8; SOCKET_READ];
    let broken = Cell::new(false);
    let service_ready = Cell::new(false);
    // The push connection opened (true) or was lost (false), for the
    // decoder to know when keys may go (area INT).
    let push_state: Cell<Option<bool>> = Cell::new(None);
    let invalid = Cell::new(InvalidReport::default());
    let mut next_tick: Option<Instant> = Some(Instant(0));
    let sink = |output: Output<'_>, pump: &mut Pump<StdTransport>, next_tick: &mut Option<Instant>| {
        if let Some(net) = output.net() {
            pump.execute(net);
            return;
        }
        if forward(output, next_tick, &service_ready, &push_state, &invalid, clock(start)).is_err() {
            broken.set(true);
        }
    };
    let open = if channel { Input::OpenChannel(address) } else { Input::OpenService(address) };
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
        let mut report = invalid.get();
        let line = report.due(clock(start));
        invalid.set(report);
        if line.is_some_and(|line| send(&Frame::Notice(Notice::Message(line))).is_err()) {
            break;
        }
        if !busy {
            let window_end = invalid.get().window_end;
            wakes.wait(clock(start), [next_tick, window_end].into_iter().flatten().min());
        }
    }
}

/// The push connection's invalid packets, for the warning line of REQ-1462
/// item 4: a rise opens a window of [`INVALID_PACKET_REPORT_SECONDS`], and at
/// its end one line gives what it gathered and the total. So at most one line
/// comes in any window, and none in a quiet one.
#[derive(Clone, Copy, Debug, Default)]
struct InvalidReport {
    total: u64,
    /// The total the last line gave.
    reported: u64,
    /// The end of the window the count rose in, while one is open.
    window_end: Option<Instant>,
}

impl InvalidReport {
    /// The count rose to `total`, at `now`. A new connection counts from 0.
    fn rose(&mut self, total: u64, now: Instant) {
        if total < self.total {
            self.reported = 0;
        }
        self.total = total;
        self.window_end.get_or_insert(Instant(now.0.saturating_add(INVALID_WINDOW_US)));
    }

    /// The warning line, when the window has ended at `now`.
    fn due(&mut self, now: Instant) -> Option<String> {
        self.window_end.filter(|end| now >= *end)?;
        self.window_end = None;
        let gathered = self.total.saturating_sub(self.reported);
        self.reported = self.total;
        (gathered > 0).then(|| {
            format!(
                "{gathered} invalid packets discarded in the last {INVALID_PACKET_REPORT_SECONDS} seconds, {} since the connection opened",
                self.total
            )
        })
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

/// Why a push connection ended for good, in words.
fn stopped(stop: PushStop) -> String {
    match stop {
        PushStop::Closed(code) => format!("the channel closed the connection with stop code {code}"),
        PushStop::Refused(status) => format!("the server refused the connection with status {status}"),
        PushStop::WrongProtocol => format!("the server does not speak {PUSH_SUBPROTOCOL}"),
    }
}

/// Acts on one of the agent's outputs that is not for the network: a timer
/// moves the next tick, pages and notices go to the decoder, and a new
/// service or a push connection's change is noted for the loop.
fn forward(
    output: Output<'_>,
    next_tick: &mut Option<Instant>,
    service_ready: &Cell<bool>,
    push_state: &Cell<Option<bool>>,
    invalid: &Cell<InvalidReport>,
    now: Instant,
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
        Output::Event(Event::Redirect(RedirectRecord::Page { page, subcode, parameters })) => {
            send(&Frame::Notice(Notice::PageRedirect(page.mpp(), subcode.value(), parameters.map(str::to_owned))))
        }
        Output::Event(Event::Redirect(RedirectRecord::Address(address))) => {
            send(&Frame::Notice(Notice::RedirectToOtherChannel(address.to_string())))
        }
        Output::Event(Event::Denied(denied)) => {
            send(&Frame::Notice(Notice::Denied(denied.status, denied.place.to_owned())))
        }
        Output::Event(Event::BundleFailed(failure)) => {
            send(&Frame::Notice(Notice::Message(format!("index.t42 was not used: {failure:?}"))))
        }
        Output::Event(Event::Unencrypted { .. }) => send(&Frame::Notice(Notice::Unencrypted)),
        Output::Event(Event::PushConnected) => {
            push_state.set(Some(true));
            Ok(())
        }
        Output::Event(Event::PushStopped(stop)) => {
            push_state.set(Some(false));
            send(&Frame::Notice(Notice::PushStopped(stopped(stop))))
        }
        // A bad message keeps the connection and says so with no wait.
        Output::Event(Event::PushLost { retry_in_us, .. }) if retry_in_us > 0 => {
            push_state.set(Some(false));
            Ok(())
        }
        // REQ-INP-026: a channel's log line from a LOG record, as an LG
        // line's would be.
        Output::Event(Event::Log { line, page }) => {
            let page = page.map_or_else(|| "no page".to_owned(), |page| format!("page {:03X}", page.mpp()));
            send(&Frame::Notice(Notice::Message(format!("log {:?}, {page}: {}", line.level, line.text))))
        }
        Output::Event(Event::ChannelConnecting) => send(&Frame::Notice(Notice::Message("connecting".into()))),
        Output::Event(Event::InvalidPackets { total }) => {
            let mut report = invalid.get();
            report.rose(total, now);
            invalid.set(report);
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
        MediaType::Tti => send(&Frame::Tti(data)),
        // REQ-WEB-003: the declared type says nothing of the format.
        _ => send(&Frame::Undeclared(data)),
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
    send(&Frame::Notice(Notice::Region(descriptor.and_then(|descriptor| descriptor.region))))?;
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
                // The first part of a page's list says whether its sub-pages
                // are files in its folder, where the page's own file holds
                // the first alone (DOC-2038). The request after it goes when
                // it has answered, so one sub-page costs one more request.
                if subcode.is_some_and(|code| !code.is_any()) {
                    agent.handle(now, Input::FetchSubPages { page, part: PartNumber::FIRST }, out);
                }
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects, reason = "a test")]
mod report_tests {
    use super::*;

    /// A moment `seconds` after the clock's start.
    fn at(seconds: u64) -> Instant {
        Instant(seconds * MICROSECONDS_PER_SECOND)
    }

    // Verifies REQ-1462 item 4: nothing while the count is 0.
    #[test]
    fn no_line_while_the_count_is_0() {
        let mut report = InvalidReport::default();
        assert_eq!(report.due(at(1_000)), None);
    }

    // Verifies REQ-1462 item 4: one line after the window, with the total and
    // the interval's count, gathered from every rise in it.
    #[test]
    fn one_line_ends_the_window_with_its_count_and_the_total() {
        let mut report = InvalidReport::default();
        report.rose(1, at(5));
        report.rose(4, at(10));
        assert_eq!(report.due(at(24)), None, "the window is still open");
        let line = report.due(at(25)).unwrap();
        assert!(line.starts_with("4 invalid packets"), "{line}");
        assert!(line.contains("4 since the connection opened"), "{line}");
        assert_eq!(report.due(at(26)), None, "one line");
    }

    // Verifies REQ-1462 item 4: none in a quiet interval, at most one in any
    // 20 seconds, and the next line counts only its own window.
    #[test]
    fn the_next_window_counts_only_its_own() {
        let mut report = InvalidReport::default();
        report.rose(4, at(0));
        assert!(report.due(at(20)).is_some());
        assert_eq!(report.due(at(100)), None, "a quiet interval");
        report.rose(6, at(101));
        assert_eq!(report.due(at(120)), None, "fewer than 20 seconds after the rise");
        let line = report.due(at(121)).unwrap();
        assert!(line.starts_with("2 invalid packets") && line.contains("6 since"), "{line}");
    }

    // Verifies REQ-1462 item 4: the cumulative number is the connection's, so
    // a new connection counts from nothing, and its first rise opens a window.
    #[test]
    fn a_new_connection_counts_again() {
        let mut report = InvalidReport::default();
        report.rose(9, at(0));
        assert!(report.due(at(20)).is_some());
        report.rose(1, at(30));
        let line = report.due(at(50)).unwrap();
        assert!(line.starts_with("1 invalid packets") && line.contains("1 since"), "{line}");
    }
}
