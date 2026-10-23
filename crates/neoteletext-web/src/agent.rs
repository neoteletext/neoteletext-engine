//! The user agent (ARCHITECTURE.md layer L7): one sans-IO state machine
//! that ties the address parser, HTTP, WebSocket, server-sent events, the
//! cookie store, the descriptor and form submission together.
//!
//! The application calls [`UserAgent::handle`] with an [`Input`]: a user
//! request, a network event from the transport, or a timer tick. The agent
//! answers through a sink with [`Output`]s: connections to open, bytes to
//! send, connections to close, the next timer, page bytes (TTI or T42) for
//! the format readers, and events for the viewer. Data in an output is
//! borrowed from the agent's buffers and is valid only during the call.
//!
//! Two connections are used: [`ConnectionId::Fetch`] carries the
//! descriptor, page and form requests one at a time (REQ-WEB-014), and
//! [`ConnectionId::Push`] carries the push stream (REQ-WEB-005).
//!
//! Push, form submission and cookies are compile-time features (decision
//! E.61). The agent's types are the same in every build. A build without a
//! feature answers whatever needs it with a value that says so, and every
//! branch that would reach the missing part tests a constant, so the
//! optimiser and the linker drop that part's code.

use crate::user_agent::UserAgentString;
use neoteletext_core::types::{Instant, PageAddress, SubCode};

use crate::bytes::{Overflow, TextBuf, Writer, copy_prefix};
use crate::channel::{self, ChannelFile};
use crate::consts::{
    ACCEPT_PAGES, CACHE_ENTRIES, CONFIRMED_ORIGINS, CRLF, IDENTITY, MAX_HOST_TEXT_LENGTH, MAX_REDIRECTS,
    MAX_RETRY_AFTER_US, MAX_SUBSCRIBED_PAGES, MAX_VALIDATOR_LENGTH, MICROSECONDS_PER_SECOND, PAGE_QUEUE,
    PUSH_RETRY_FIRST_US, SECURE_HOSTS, STATUS_FOUND, STATUS_GONE, STATUS_MOVED_PERMANENTLY, STATUS_NOT_FOUND,
    STATUS_NOT_MODIFIED, STATUS_OK, STATUS_PERMANENT_REDIRECT, STATUS_SEE_OTHER, STATUS_SUCCESS_FIRST,
    STATUS_TEMPORARY_REDIRECT, header, media,
};
use crate::cookie::{CookieJar, Cookies, Site};
use crate::descriptor::{Descriptor, PushMethod};
use crate::form::FormRefused;
use crate::http::{self, BodyDecoder, HeadCollector, HttpError, MediaType, Method, ResponseHead};
use crate::port::{ConnectionId, NetInput, RandomSource, Security};
use crate::sse::SseParser;
use crate::url::{IpAddress, Origin, Url, UrlError};
use crate::ws::{FrameReader, HandshakeKey};

mod config;
mod events;

pub use config::{Buffers, Config, Storage};

use cookies::{cookie_store, store_response_cookies, write_cookie_header};
pub use events::{BoundChange, Event, Failure, Input, Output, PageBytes, PushProblem, Sink, Source, Subscription};

mod cookies;
mod form;
mod push;
mod push_data;

/// The bytes of one T42 packet (REQ-INP-002).
const T42_PACKET: usize = neoteletext_core::consts::PACKET_BYTES;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Link {
    Closed,
    Connecting,
    Open,
}

/// Which file a page request asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PageFile {
    /// The descriptor's page address template (REQ-WEB-003).
    Template,
    /// A channel's page file (REQ-WEB-019): T42 or TTI, and whether it is
    /// the second of the two tried.
    Channel { t42: bool, second: bool },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Purpose {
    Descriptor {
        via_link: bool,
        channel: bool,
    },
    FrontDoor,
    Page {
        page: PageAddress,
        subcode: Option<SubCode>,
        file: PageFile,
    },
    Form,
    Contents,
    /// Registers a device-bound session (REQ-WEB-017).
    #[cfg_attr(not(feature = "dbsc"), allow(dead_code, reason = "made only with the feature `dbsc`"))]
    Register,
    /// Refreshes the device-bound session in `slot`; `retried` after the
    /// server's 403 and a fresh challenge.
    #[cfg_attr(not(feature = "dbsc"), allow(dead_code, reason = "made only with the feature `dbsc`"))]
    Refresh {
        slot: u8,
        retried: bool,
    },
}

#[derive(Clone, Copy, Debug)]
struct Exchange {
    purpose: Purpose,
    url: Url,
    method: Method,
    redirects: u8,
    site: Site,
    deadline: Instant,
    sent: bool,
    /// The request went out on a connection already used once.
    reused: bool,
    /// One retry after a kept-alive connection closed unanswered.
    retried: bool,
    /// When this request was made, for the exchange's report (REQ-WEB-033).
    started: Instant,
}

#[derive(Clone, Copy, Debug)]
struct Target {
    host: TextBuf<MAX_HOST_TEXT_LENGTH>,
    port: u16,
    secure: bool,
}

impl Target {
    fn of(url: &Url) -> Self {
        let mut host = TextBuf::new();
        // The host always fits: the address parser bounds it.
        let _ = host.set(url.host_text().as_bytes());
        Self { host, port: url.port(), secure: url.is_secure() }
    }

    fn matches(&self, url: &Url) -> bool {
        self.host.as_str() == url.host_text() && self.port == url.port() && self.secure == url.is_secure()
    }
}

#[derive(Clone, Copy, Debug)]
struct FetchConnection {
    link: Link,
    target: Option<Target>,
    deadline: Option<Instant>,
    /// A response has completed on this connection.
    used: bool,
    exchange: Option<Exchange>,
    head: HeadCollector,
    response: Option<ResponseHead>,
    body: Option<BodyDecoder>,
    body_len: usize,
    /// Response bytes have arrived for the current exchange.
    started: bool,
}

#[derive(Clone, Copy, Debug)]
struct CacheEntry {
    page: PageAddress,
    subcode: Option<SubCode>,
    etag: Option<TextBuf<MAX_VALIDATOR_LENGTH>>,
    last_modified: Option<TextBuf<MAX_VALIDATOR_LENGTH>>,
    fresh_until: Option<Instant>,
    used: u64,
    /// For a channel, the page file that answered is T42.
    t42: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FormPhase {
    Idle,
    Confirming { record_origin: bool },
    Queued,
    InFlight,
}

#[derive(Clone, Copy, Debug)]
struct FormState {
    phase: FormPhase,
    target: Option<Url>,
    method: Method,
    body_len: usize,
    site: Site,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PushPhase {
    Idle,
    Backoff {
        until: Instant,
    },
    Connecting {
        deadline: Instant,
    },
    Handshake {
        deadline: Instant,
    },
    /// Open, last heard from at `heard`, and pinged at `pinged` with no
    /// answer yet (REQ-WEB-005's heartbeat).
    Open {
        heard: Instant,
        pinged: Option<Instant>,
    },
}

#[derive(Clone, Copy, Debug)]
struct PushConnection {
    phase: PushPhase,
    url: Option<Url>,
    method: PushMethod,
    key: HandshakeKey,
    head: HeadCollector,
    body: Option<BodyDecoder>,
    sse: SseParser,
    frames: FrameReader,
    delay_us: u64,
    healthy: bool,
}

/// The web user agent: `SLOTS` cookies in `COOKIE_BYTES` bytes, random
/// numbers from `R`.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag belongs to a different part of the agent: navigation, the fetch connection, push, the channel"
)]
// Not `Debug`: the derive would need `dyn Cookies` and `dyn DeviceKeys` to be
// `Debug`, and a trait object's table then carries `core::fmt` into every
// build, which REQ-GEN-016 keeps out of a small one (the size check found
// its panic paths).
pub struct UserAgent<'m, R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> {
    config: Config,
    /// The `User-Agent` sent on every request and upgrade, composed once
    /// (REQ-WEB-031).
    named_as: UserAgentString,
    buffers: Buffers<'m>,
    random: R,
    jar: CookieJar<SLOTS, COOKIE_BYTES>,
    /// The port's cookies, kept outside the agent, used instead of `jar`
    /// when given (decision E.104).
    port_cookies: Option<&'m mut dyn Cookies>,
    wall_clock: Option<(Instant, u64)>,
    descriptor: Option<Descriptor>,
    pending_descriptor: Option<(Url, bool, bool)>,
    /// The open channel's folder (REQ-WEB-018), when a channel is open.
    channel: Option<Url>,
    /// The page file extension to try first in the channel: the one that
    /// last answered (REQ-WEB-019).
    t42_first: bool,
    /// A page whose other file is to be tried next.
    second_try: Option<(PageAddress, Option<SubCode>, bool)>,
    /// The channel's bundle to fetch after its settings.
    front_door: Option<Url>,
    /// The channel's contents file to fetch.
    contents: Option<Url>,
    navigation_pending: bool,
    secure_hosts: [Option<TextBuf<MAX_HOST_TEXT_LENGTH>>; SECURE_HOSTS],
    fetch: FetchConnection,
    /// Close the fetch connection when the current response completes.
    close_fetch_after: bool,
    queue: [Option<(PageAddress, Option<SubCode>)>; PAGE_QUEUE],
    next_fetch_at: Instant,
    /// The deadline the port was last told, so it is told only of a change.
    timer_told: Option<Instant>,
    cache: [Option<CacheEntry>; CACHE_ENTRIES],
    cache_tick: u64,
    form: FormState,
    confirmed: [Option<Origin>; CONFIRMED_ORIGINS],
    push: PushConnection,
    subscribe_all: bool,
    subscribed: [Option<PageAddress>; MAX_SUBSCRIBED_PAGES],
    /// The port's keys that never leave the device (REQ-WEB-017).
    #[cfg(feature = "dbsc")]
    device_keys: Option<&'m mut dyn crate::dbsc::DeviceKeys>,
    /// Device-bound sessions and the exchanges they wait for.
    #[cfg(feature = "dbsc")]
    bound: bound::Bound,
}

#[cfg(feature = "dbsc")]
mod bound;

/// The longest session instructions a bound session's exchange reads.
#[cfg(feature = "dbsc")]
const BOUND_INSTRUCTIONS_LIMIT: usize = crate::dbsc::MAX_INSTRUCTIONS_BYTES;
#[cfg(not(feature = "dbsc"))]
const BOUND_INSTRUCTIONS_LIMIT: usize = 0;

/// Without device-bound sessions (feature `dbsc`) nothing waits for one.
#[cfg(not(feature = "dbsc"))]
impl<R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'_, R, SLOTS, COOKIE_BYTES> {
    #[allow(clippy::unused_self, reason = "the same call as the feature's")]
    const fn bound_next(&self, _: &mut Sink<'_>) -> Option<(Purpose, Url, Method, Site)> {
        None
    }

    #[allow(clippy::unused_self, reason = "the same call as the feature's")]
    const fn bound_defer(&self, _: Instant, _: Purpose, _: &Url, _: Method, _: Site) -> bool {
        false
    }

    #[allow(clippy::unused_self, reason = "the same call as the feature's")]
    const fn bound_head(&self, _: Instant, _: &Exchange, _: &ResponseHead, _: &mut Sink<'_>) {}

    #[allow(clippy::unused_self, reason = "the same call as the feature's")]
    const fn bound_complete(&self, _: Instant, _: &Exchange, _: &ResponseHead, _: &mut Sink<'_>) {}

    #[allow(clippy::unused_self, reason = "the same call as the feature's")]
    const fn bound_failed(&self, _: Purpose, _: &mut Sink<'_>) {}
}

fn event(sink: &mut Sink<'_>, event: Event<'_>) {
    sink(Output::Event(event));
}

/// Checks a channel's page file or bundle (REQ-WEB-027): a media type
/// accepted for its extension, and content of the format its name says.
/// Returns the format, which the caller need not detect again.
fn channel_file_media(t42: bool, media: MediaType, body: &[u8]) -> Result<MediaType, Failure> {
    let (accepted, content, format) = if t42 {
        (
            matches!(media, MediaType::T42 | MediaType::OctetStream | MediaType::TextPlain),
            channel::is_t42(body),
            MediaType::T42,
        )
    } else {
        (
            matches!(media, MediaType::Tti | MediaType::TextPlain | MediaType::OctetStream),
            channel::is_tti(body),
            MediaType::Tti,
        )
    };
    if !accepted {
        return Err(Failure::MediaType);
    }
    if !content {
        return Err(Failure::Content);
    }
    Ok(format)
}

impl<'m, R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> UserAgent<'m, R, SLOTS, COOKIE_BYTES> {
    /// A new agent with no service open.
    pub fn new(config: Config, buffers: Buffers<'m>, random: R) -> Self {
        Self::with_store(config, buffers, random, None)
    }

    /// A new agent whose cookies the port keeps (decision E.104): every
    /// cookie it would send is asked of `cookies`, and every cookie a
    /// response sets is given to it; the agent's own jar stays empty, and
    /// [`Self::save_cookies`] and [`Self::load_cookies`] are the port's
    /// business, not the agent's.
    pub fn with_cookies(config: Config, buffers: Buffers<'m>, random: R, cookies: &'m mut dyn Cookies) -> Self {
        Self::with_store(config, buffers, random, Some(cookies))
    }

    fn with_store(config: Config, buffers: Buffers<'m>, random: R, port_cookies: Option<&'m mut dyn Cookies>) -> Self {
        let mut jar = CookieJar::new();
        jar.set_limits(config.cookies_per_host, config.cookies_total);
        Self {
            port_cookies,
            named_as: UserAgentString::of(config.product.as_ref()),
            config,
            buffers,
            random,
            jar,
            wall_clock: None,
            descriptor: None,
            pending_descriptor: None,
            channel: None,
            t42_first: false,
            second_try: None,
            front_door: None,
            contents: None,
            navigation_pending: false,
            secure_hosts: [None; SECURE_HOSTS],
            fetch: FetchConnection {
                link: Link::Closed,
                target: None,
                deadline: None,
                used: false,
                exchange: None,
                head: HeadCollector::default(),
                response: None,
                body: None,
                body_len: 0,
                started: false,
            },
            close_fetch_after: false,
            queue: [None; PAGE_QUEUE],
            next_fetch_at: Instant(0),
            timer_told: None,
            cache: [None; CACHE_ENTRIES],
            cache_tick: 0,
            form: FormState {
                phase: FormPhase::Idle,
                target: None,
                method: Method::Post,
                body_len: 0,
                site: Site::SameService,
            },
            confirmed: [None; CONFIRMED_ORIGINS],
            push: PushConnection {
                phase: PushPhase::Idle,
                url: None,
                method: PushMethod::WebSocket,
                key: [0; crate::consts::WEBSOCKET_KEY_TEXT],
                head: HeadCollector::default(),
                body: None,
                sse: SseParser::new(),
                frames: FrameReader::new(),
                delay_us: PUSH_RETRY_FIRST_US,
                healthy: false,
            },
            subscribe_all: true,
            subscribed: [None; MAX_SUBSCRIBED_PAGES],
            #[cfg(feature = "dbsc")]
            device_keys: None,
            #[cfg(feature = "dbsc")]
            bound: bound::Bound::new(),
        }
    }

    /// The open service's descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> Option<&Descriptor> {
        self.descriptor.as_ref()
    }

    /// The cookie store.
    #[must_use]
    pub const fn cookies(&self) -> &CookieJar<SLOTS, COOKIE_BYTES> {
        &self.jar
    }

    /// Seconds for the cookie store, and whether they are calendar time.
    fn seconds(&self, now: Instant) -> (u64, bool) {
        match self.wall_clock {
            Some((at, unix)) => (unix.saturating_add(now.since(at) / MICROSECONDS_PER_SECOND), true),
            None => (now.0 / MICROSECONDS_PER_SECOND, false),
        }
    }

    /// Handles one input. Outputs go to `sink` before this returns.
    pub fn handle(&mut self, now: Instant, input: Input<'_>, sink: &mut Sink<'_>) {
        let ticked = matches!(input, Input::Tick);
        match input {
            Input::Tick => self.tick(now, sink),
            Input::Net(net) => match net {
                NetInput::Connected(ConnectionId::Fetch, peer) => self.fetch_connected(now, peer, sink),
                NetInput::ConnectFailed(ConnectionId::Fetch) => self.fetch_lost(now, Failure::Network, sink),
                NetInput::Received(ConnectionId::Fetch, bytes) => self.fetch_received(now, bytes, sink),
                NetInput::Closed(ConnectionId::Fetch) => self.fetch_closed(now, sink),
                NetInput::Connected(ConnectionId::Push, peer) if cfg!(feature = "push") => {
                    self.push_connected(now, peer, sink);
                }
                NetInput::ConnectFailed(ConnectionId::Push) | NetInput::Closed(ConnectionId::Push)
                    if cfg!(feature = "push") =>
                {
                    let live = matches!(
                        self.push.phase,
                        PushPhase::Connecting { .. } | PushPhase::Handshake { .. } | PushPhase::Open { .. }
                    );
                    if live {
                        self.push_lost(now, None, false, sink);
                    }
                }
                NetInput::Received(ConnectionId::Push, bytes) if cfg!(feature = "push") => {
                    self.push_received(now, bytes, sink);
                }
                // The live feed has its own machine, and a build without
                // push never opens a push connection.
                NetInput::Connected(ConnectionId::Push | ConnectionId::Live, _)
                | NetInput::ConnectFailed(ConnectionId::Push | ConnectionId::Live)
                | NetInput::Received(ConnectionId::Push | ConnectionId::Live, _)
                | NetInput::Closed(ConnectionId::Push | ConnectionId::Live) => {}
            },
            Input::OpenService(address) => match Url::parse(address) {
                Ok(url) if !url.scheme().is_websocket() => {
                    self.leave_channel();
                    self.pending_descriptor = Some((url, false, false));
                }
                Ok(_) => event(sink, Event::DescriptorFailed(Failure::Address(UrlError::Scheme))),
                Err(error) => event(sink, Event::DescriptorFailed(Failure::Address(error))),
            },
            Input::OpenChannel(address) => match channel::parse_address(address) {
                Ok(found) => self.open_channel(&found.folder, false),
                Err(channel::ChannelError::Address(error)) => {
                    event(sink, Event::DescriptorFailed(Failure::Address(error)));
                }
                Err(_) => event(sink, Event::DescriptorFailed(Failure::Address(UrlError::Path))),
            },
            Input::FetchContents => self.fetch_contents(sink),
            Input::FollowLink { service, page, subcode } => self.follow_link(service, page, subcode, sink),
            Input::FetchPage { page, subcode } => self.enqueue(page, subcode, sink),
            Input::Subscribe(subscription) => self.subscribe(now, subscription, sink),
            Input::Submit(submission) => {
                if cfg!(feature = "forms") {
                    self.submit(&submission, sink);
                } else {
                    event(sink, Event::SubmissionRefused(FormRefused::NotCompiledIn));
                }
            }
            // Without forms nothing ever waits for a confirmation.
            Input::ConfirmSubmission(confirmed) => {
                if cfg!(feature = "forms") {
                    self.confirm(confirmed, sink);
                }
            }
            Input::WipeCookies => self.wipe_cookies(),
            Input::WallClock(unix) => self.wall_clock = Some((now, unix)),
            Input::Send(message) => self.send_input(message, sink),
        }
        self.dispatch(now, sink);
        // The port is told its timer when the deadline changes, and again
        // after a tick, which spent the one it had; not after every input,
        // which woke the port twice for each (decision E.103).
        let deadline = self.next_deadline();
        if ticked || deadline != self.timer_told {
            self.timer_told = deadline;
            if let Some(at) = deadline {
                sink(Output::Timer { at });
            }
        }
    }

    // -----------------------------------------------------------------
    // Requests
    // -----------------------------------------------------------------

    /// Forgets the open channel: what follows is a service opened by its
    /// descriptor.
    fn leave_channel(&mut self) {
        self.channel = None;
        self.front_door = None;
        self.contents = None;
        self.second_try = None;
    }

    /// Starts opening a channel: its settings first (REQ-WEB-018).
    fn open_channel(&mut self, folder: &Url, via_link: bool) {
        self.leave_channel();
        self.t42_first = false;
        // The folder is checked, so its settings' address always joins.
        if let Ok(settings) = channel::file_url(folder, ChannelFile::Settings) {
            self.pending_descriptor = Some((settings, via_link, true));
        }
        self.channel = Some(*folder);
    }

    /// The open service's folder: its channel's, or the folder holding its
    /// descriptor (REQ-WEB-018).
    fn folder(&self) -> Option<Url> {
        self.channel.or_else(|| self.descriptor.as_ref().and_then(|d| d.address().join(".").ok()))
    }

    fn fetch_contents(&mut self, sink: &mut Sink<'_>) {
        match self.folder().map(|folder| channel::file_url(&folder, ChannelFile::Contents)) {
            Some(Ok(url)) => self.contents = Some(url),
            Some(Err(error)) => event(sink, Event::ContentsFailed(Failure::Address(error))),
            None => event(sink, Event::ContentsFailed(Failure::NoService)),
        }
    }

    fn follow_link(&mut self, service: &str, page: PageAddress, subcode: Option<SubCode>, sink: &mut Sink<'_>) {
        let linked =
            self.descriptor.as_ref().and_then(|d| d.service(service).map(|s| (d.service_address(s), s.channel)));
        let address = linked.map(|(address, _)| address);
        let to_channel = linked.is_some_and(|(_, channel)| channel);
        match address.map(Url::parse) {
            Some(Ok(url)) => {
                if to_channel {
                    self.open_channel(&url, true);
                } else {
                    self.leave_channel();
                    self.pending_descriptor = Some((url, true, false));
                }
                self.queue = [None; PAGE_QUEUE];
                if let Some(slot) = self.queue.first_mut() {
                    *slot = Some((page, subcode));
                }
            }
            _ => event(sink, Event::LinkUnknown),
        }
    }

    fn enqueue(&mut self, page: PageAddress, subcode: Option<SubCode>, sink: &mut Sink<'_>) {
        if self.queue.iter().flatten().any(|&queued| queued == (page, subcode)) {
            return;
        }
        match self.queue.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => *slot = Some((page, subcode)),
            None => event(sink, Event::FetchFailed { page, subcode, failure: Failure::QueueFull }),
        }
    }

    fn take_queued(&mut self) -> Option<(PageAddress, Option<SubCode>)> {
        let first = self.queue.first_mut()?.take()?;
        self.queue.rotate_left(1);
        Some(first)
    }

    fn cache_entry(&self, page: PageAddress, subcode: Option<SubCode>) -> Option<&CacheEntry> {
        self.cache.iter().flatten().find(|entry| entry.page == page && entry.subcode == subcode)
    }

    /// The first refusal that applies to a request to `url`, before any
    /// connection is made.
    fn refusal(&self, now: Instant, url: &Url) -> Option<Failure> {
        if url.is_secure() && !self.config.tls_available {
            return Some(Failure::TlsUnavailable);
        }
        if self.is_downgrade(now, url) {
            return Some(Failure::Downgrade);
        }
        if url.ip().is_some_and(|ip| self.blocked(ip)) {
            return Some(Failure::Blocked);
        }
        // Through a proxy, names are resolved there: a local name is
        // refused before the proxy is asked (REQ-WEB-032, REQ-SEC-006).
        let allowed = url.ip().is_some_and(|ip| self.config.allowed_local.contains(&Some(ip)));
        if self.config.proxied && !allowed && crate::proxy::hidden_local(url.host_text()) {
            return Some(Failure::Blocked);
        }
        None
    }

    fn blocked(&self, address: IpAddress) -> bool {
        address.is_blocked() && !self.config.allowed_local.contains(&Some(address))
    }

    /// Whether the peer a connection reached is refused: never through a
    /// proxy, whose address it is (REQ-WEB-032).
    fn peer_blocked(&self, peer: IpAddress) -> bool {
        !self.config.proxied && self.blocked(peer)
    }

    fn report_refusal(connection: ConnectionId, url: &Url, failure: Failure, sink: &mut Sink<'_>) {
        match failure {
            Failure::Blocked => event(sink, Event::Blocked { connection, host: url.host_text() }),
            Failure::Downgrade => event(sink, Event::DowngradeRefused { host: url.host_text() }),
            _ => {}
        }
    }

    /// Starts the next request when the fetch connection is free.
    fn dispatch(&mut self, now: Instant, sink: &mut Sink<'_>) {
        while self.fetch.exchange.is_none() {
            let (purpose, url, method, site) = if let Some(bound) = self.bound_next(sink) {
                bound
            } else if let Some((url, via_link, channel)) = self.pending_descriptor.take() {
                let site = if via_link { Site::CrossServiceNavigation } else { Site::SameService };
                (Purpose::Descriptor { via_link, channel }, url, Method::Get, site)
            } else if let Some(url) = self.front_door.take() {
                (Purpose::FrontDoor, url, Method::Get, Site::SameService)
            } else if let Some((page, subcode, t42)) = self.second_try.take() {
                // The other file of a page whose first answered 404 or 410
                // (REQ-WEB-019), without waiting for the rate limit: it is
                // the same request, completed.
                let Some(folder) = self.channel else { continue };
                match channel::file_url(&folder, ChannelFile::Page { page, t42 }) {
                    Ok(url) => (
                        Purpose::Page { page, subcode, file: PageFile::Channel { t42, second: true } },
                        url,
                        Method::Get,
                        Site::SameService,
                    ),
                    Err(error) => {
                        event(sink, Event::FetchFailed { page, subcode, failure: Failure::Address(error) });
                        continue;
                    }
                }
            } else if let Some(url) = self.contents.take() {
                (Purpose::Contents, url, Method::Get, Site::SameService)
            } else if cfg!(feature = "forms") && self.form.phase == FormPhase::Queued {
                let Some(url) = self.form.target else {
                    self.form.phase = FormPhase::Idle;
                    continue;
                };
                self.form.phase = FormPhase::InFlight;
                (Purpose::Form, url, self.form.method, self.form.site)
            } else if self.queue.iter().any(Option::is_some) && now >= self.next_fetch_at {
                let Some((page, subcode)) = self.take_queued() else { return };
                let Some(descriptor) = &self.descriptor else {
                    event(sink, Event::FetchFailed { page, subcode, failure: Failure::NoService });
                    continue;
                };
                if self
                    .cache_entry(page, subcode)
                    .is_some_and(|entry| entry.fresh_until.is_some_and(|until| now < until))
                {
                    event(sink, Event::PageCurrent { page, subcode });
                    continue;
                }
                let (address, file) = match self.channel {
                    Some(folder) => {
                        // The file that answered last time is asked again,
                        // so its validators apply; otherwise the extension
                        // that last answered in the channel.
                        let t42 = self.cache_entry(page, subcode).map_or(self.t42_first, |entry| entry.t42);
                        (
                            channel::file_url(&folder, ChannelFile::Page { page, t42 }),
                            PageFile::Channel { t42, second: false },
                        )
                    }
                    None => (descriptor.page_url(page, subcode), PageFile::Template),
                };
                let url = match address {
                    Ok(url) => url,
                    Err(error) => {
                        event(sink, Event::FetchFailed { page, subcode, failure: Failure::Address(error) });
                        continue;
                    }
                };
                let site = if core::mem::take(&mut self.navigation_pending) {
                    Site::CrossServiceNavigation
                } else {
                    Site::SameService
                };
                (Purpose::Page { page, subcode, file }, url, Method::Get, site)
            } else {
                return;
            };
            // REQ-WEB-017: a request that would go without a bound
            // session's cookie waits for the session's refresh.
            if self.bound_defer(now, purpose, &url, method, site) {
                continue;
            }
            self.next_fetch_at = Instant(now.0.saturating_add(self.config.fetch_interval_us));
            if let Some(failure) = self.refusal(now, &url) {
                Self::report_refusal(ConnectionId::Fetch, &url, failure, sink);
                self.finish_purpose(purpose, Err(failure), sink);
                continue;
            }
            self.fetch.exchange = Some(Exchange {
                purpose,
                url,
                method,
                redirects: 0,
                site,
                deadline: Instant(now.0.saturating_add(self.config.response_timeout_us)),
                sent: false,
                reused: false,
                retried: false,
                started: now,
            });
            self.start_exchange(now, sink);
        }
    }

    /// Sends the current exchange's request, connecting first if needed.
    fn start_exchange(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let Some(exchange) = self.fetch.exchange else { return };
        let reusable =
            self.fetch.link == Link::Open && self.fetch.target.is_some_and(|target| target.matches(&exchange.url));
        if reusable {
            self.send_request(now, sink);
            return;
        }
        if self.fetch.link != Link::Closed {
            sink(Output::Close { connection: ConnectionId::Fetch });
        }
        let target = Target::of(&exchange.url);
        self.fetch.target = Some(target);
        self.fetch.link = Link::Connecting;
        self.fetch.used = false;
        self.fetch.deadline = Some(Instant(now.0.saturating_add(self.config.connect_timeout_us)));
        let security = if target.secure { Security::Tls } else { Security::Plain };
        sink(Output::Connect {
            connection: ConnectionId::Fetch,
            host: exchange.url.host_text(),
            port: target.port,
            security,
        });
    }

    fn write_request(&mut self, now: Instant, exchange: &Exchange, sink: &mut Sink<'_>) -> Result<usize, Overflow> {
        let (seconds, _) = self.seconds(now);
        // Only a form's request has a body; the bound sessions' requests
        // are posted empty (REQ-WEB-017).
        let body_len =
            if exchange.method == Method::Post && exchange.purpose == Purpose::Form { self.form.body_len } else { 0 };
        let mut writer = Writer::new(self.buffers.fetch_request);
        http::write_request_line(&mut writer, exchange.method, exchange.url.target())?;
        http::write_host(&mut writer, &exchange.url)?;
        http::write_header(&mut writer, header::USER_AGENT, self.named_as.as_bytes())?;
        http::write_header(&mut writer, header::ACCEPT, ACCEPT_PAGES)?;
        http::write_header(&mut writer, header::ACCEPT_ENCODING, IDENTITY)?;
        if let Purpose::Page { page, subcode, .. } = exchange.purpose
            && exchange.redirects == 0
            && let Some(entry) = self.cache.iter().flatten().find(|e| e.page == page && e.subcode == subcode)
        {
            if let Some(etag) = &entry.etag {
                http::write_header(&mut writer, header::IF_NONE_MATCH, etag.as_bytes())?;
            }
            if let Some(date) = &entry.last_modified {
                http::write_header(&mut writer, header::IF_MODIFIED_SINCE, date.as_bytes())?;
            }
        }
        if exchange.method == Method::Post {
            if exchange.purpose == Purpose::Form {
                http::write_header(&mut writer, header::CONTENT_TYPE, media::FORM_URLENCODED)?;
            }
            writer.put(header::CONTENT_LENGTH)?;
            writer.put(b": ")?;
            writer.decimal(body_len as u64)?;
            writer.put(CRLF)?;
        }
        #[cfg(feature = "dbsc")]
        bound::write_headers(&mut self.bound, self.device_keys.as_deref_mut(), exchange, &mut writer)?;
        let path = exchange.url.path();
        if write_cookie_header(
            cookie_store(&mut self.jar, &mut self.port_cookies),
            &mut writer,
            &exchange.url,
            path,
            exchange.site,
            seconds,
            body_len,
        )? {
            event(sink, Event::CookiesTruncated);
        }
        writer.put(CRLF)?;
        writer.put(self.buffers.form.get(..body_len).unwrap_or(&[]))?;
        Ok(writer.len())
    }

    fn send_request(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let Some(exchange) = self.fetch.exchange else { return };
        match self.write_request(now, &exchange, sink) {
            Ok(len) => {
                if let Some(current) = &mut self.fetch.exchange {
                    current.sent = true;
                    current.reused = self.fetch.used;
                }
                self.fetch.head.reset();
                self.fetch.response = None;
                self.fetch.body = None;
                self.fetch.body_len = 0;
                self.fetch.started = false;
                self.fetch.deadline = Some(exchange.deadline);
                let bytes = self.buffers.fetch_request.get(..len).unwrap_or(&[]);
                sink(Output::Send { connection: ConnectionId::Fetch, bytes });
            }
            Err(Overflow) => self.fail_exchange(Failure::RequestTooLarge, sink),
        }
    }

    // -----------------------------------------------------------------
    // Fetch connection events
    // -----------------------------------------------------------------

    fn fetch_connected(&mut self, now: Instant, peer: IpAddress, sink: &mut Sink<'_>) {
        if self.fetch.link != Link::Connecting {
            return;
        }
        let Some(target) = self.fetch.target else { return };
        if self.peer_blocked(peer) {
            sink(Output::Close { connection: ConnectionId::Fetch });
            self.fetch.link = Link::Closed;
            event(sink, Event::Blocked { connection: ConnectionId::Fetch, host: target.host.as_str() });
            self.fail_exchange(Failure::Blocked, sink);
            return;
        }
        self.fetch.link = Link::Open;
        if target.secure {
            self.remember_secure(target.host.as_str());
        } else {
            event(sink, Event::Unencrypted { connection: ConnectionId::Fetch, host: target.host.as_str() });
        }
        self.send_request(now, sink);
    }

    fn fetch_closed(&mut self, now: Instant, sink: &mut Sink<'_>) {
        self.fetch.link = Link::Closed;
        let Some(exchange) = self.fetch.exchange else { return };
        if let Some(body) = &mut self.fetch.body {
            match body.finish_at_close() {
                Ok(()) => {
                    self.complete(now, sink);
                    return;
                }
                Err(error) => {
                    self.fail_exchange(Failure::Http(error), sink);
                    return;
                }
            }
        }
        // A kept-alive connection the server closed before answering:
        // retry a GET once on a new connection (RFC 9112 section 9.3.1).
        // A form submission is never repeated without the viewer (RFC 9110
        // section 9.2.2).
        let idempotent = !matches!(exchange.purpose, Purpose::Form | Purpose::Register | Purpose::Refresh { .. });
        if idempotent && exchange.sent && exchange.reused && !exchange.retried && !self.fetch.started {
            if let Some(current) = &mut self.fetch.exchange {
                current.retried = true;
                current.sent = false;
            }
            self.start_exchange(now, sink);
            return;
        }
        self.fail_exchange(Failure::Network, sink);
    }

    fn fetch_lost(&mut self, _now: Instant, failure: Failure, sink: &mut Sink<'_>) {
        self.fetch.link = Link::Closed;
        self.fail_exchange(failure, sink);
    }

    fn fetch_received(&mut self, now: Instant, mut input: &[u8], sink: &mut Sink<'_>) {
        if self.fetch.exchange.is_none() || self.fetch.link != Link::Open {
            // Bytes nobody asked for: the connection cannot be trusted.
            if !input.is_empty() {
                sink(Output::Close { connection: ConnectionId::Fetch });
                self.fetch.link = Link::Closed;
            }
            return;
        }
        self.fetch.started = true;
        while !input.is_empty() {
            if self.fetch.response.is_none() {
                let limit = self.config.header_limit.min(self.buffers.fetch_head.len());
                let buffer = self.buffers.fetch_head.get_mut(..limit).unwrap_or(&mut []);
                match self.fetch.head.feed(buffer, input) {
                    Ok((used, complete)) => {
                        input = input.get(used..).unwrap_or(&[]);
                        if complete && !self.fetch_head_complete(now, sink) {
                            return;
                        }
                    }
                    Err(error) => {
                        self.abandon_fetch(Failure::Http(error), sink);
                        return;
                    }
                }
                continue;
            }
            let Some(body) = &mut self.fetch.body else { break };
            match body.feed(input) {
                Ok(step) => {
                    let end = self.fetch.body_len.saturating_add(step.data.len());
                    if let Some(target) = self.buffers.body.get_mut(self.fetch.body_len..end) {
                        copy_prefix(target, step.data);
                        self.fetch.body_len = end;
                    } else {
                        self.abandon_fetch(Failure::Http(HttpError::BodyTooLarge), sink);
                        return;
                    }
                    input = input.get(step.consumed..).unwrap_or(&[]);
                    if step.done {
                        break;
                    }
                    if step.consumed == 0 {
                        break;
                    }
                }
                Err(error) => {
                    self.abandon_fetch(Failure::Http(error), sink);
                    return;
                }
            }
        }
        if self.fetch.body.is_some_and(|body| body.is_done()) {
            if !input.is_empty() {
                // Bytes after the response: no pipelining is used, so the
                // connection is out of step and is closed.
                self.close_fetch_after = true;
            }
            self.complete(now, sink);
        }
    }

    /// Parses a complete head. Returns false when the exchange ended.
    fn fetch_head_complete(&mut self, now: Instant, sink: &mut Sink<'_>) -> bool {
        let Some(exchange) = self.fetch.exchange else { return false };
        let (seconds, calendar) = self.seconds(now);
        let head_len = self.fetch.head.len();
        let Self { buffers, jar, port_cookies, channel, .. } = self;
        let head_bytes = buffers.fetch_head.get(..head_len).unwrap_or(&[]);
        let host = exchange.url.host_text();
        let path = exchange.url.path();
        let secure = exchange.url.is_secure();
        // A channel's cookies stay in its folder (REQ-WEB-026).
        let scope = channel.as_ref().map(Url::path);
        let store = cookie_store(jar, port_cookies);
        let parsed = store_response_cookies(store, head_bytes, host, path, scope, secure, seconds, calendar, sink);
        let head = match parsed {
            Ok(head) => head,
            Err(error) => {
                self.abandon_fetch(Failure::Http(error), sink);
                return false;
            }
        };
        if head.status < STATUS_SUCCESS_FIRST {
            self.abandon_fetch(Failure::Status(head.status), sink);
            return false;
        }
        let limit = match exchange.purpose {
            Purpose::Descriptor { .. } => self.config.descriptor_limit,
            // A front door is bounded like a page: a bundled site larger
            // than the page limit publishes page files instead.
            Purpose::Page { .. } | Purpose::Form | Purpose::FrontDoor => self.config.page_limit,
            Purpose::Contents => crate::contents::MAX_BYTES,
            Purpose::Register | Purpose::Refresh { .. } => BOUND_INSTRUCTIONS_LIMIT,
        }
        .min(self.buffers.body.len());
        self.bound_head(now, &exchange, &head, sink);
        match BodyDecoder::new(head.framing, limit) {
            Ok(decoder) => {
                self.fetch.response = Some(head);
                self.fetch.body = Some(decoder);
                true
            }
            Err(error) => {
                self.abandon_fetch(Failure::Http(error), sink);
                false
            }
        }
    }

    /// Closes the fetch connection and fails the exchange.
    fn abandon_fetch(&mut self, failure: Failure, sink: &mut Sink<'_>) {
        sink(Output::Close { connection: ConnectionId::Fetch });
        self.fetch.link = Link::Closed;
        self.fail_exchange(failure, sink);
    }

    fn fail_exchange(&mut self, failure: Failure, sink: &mut Sink<'_>) {
        self.fetch.deadline = None;
        self.fetch.response = None;
        self.fetch.body = None;
        if let Some(exchange) = self.fetch.exchange.take() {
            self.finish_purpose(exchange.purpose, Err(failure), sink);
        }
    }

    fn finish_purpose(&mut self, purpose: Purpose, result: Result<(), Failure>, sink: &mut Sink<'_>) {
        let Err(failure) = result else { return };
        match purpose {
            Purpose::Descriptor { .. } => event(sink, Event::DescriptorFailed(failure)),
            Purpose::FrontDoor => event(sink, Event::BundleFailed(failure)),
            Purpose::Contents => event(sink, Event::ContentsFailed(failure)),
            Purpose::Page { page, subcode, .. } => event(sink, Event::FetchFailed { page, subcode, failure }),
            Purpose::Form => {
                self.form.phase = FormPhase::Idle;
                event(sink, Event::SubmissionFailed { status: None, failure: Some(failure) });
            }
            Purpose::Register | Purpose::Refresh { .. } => self.bound_failed(purpose, sink),
        }
    }

    /// A response is complete: act on it.
    /// REQ-WEB-033: each exchange reported as it completes, for a
    /// developer's view of the channel.
    fn report_exchange(&self, now: Instant, exchange: &Exchange, status: u16, sink: &mut Sink<'_>) {
        event(
            sink,
            Event::Exchanged {
                method: exchange.method,
                url: exchange.url.as_str(),
                status,
                bytes: self.fetch.body_len,
                elapsed_us: now.0.saturating_sub(exchange.started.0),
            },
        );
    }

    fn complete(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let (Some(exchange), Some(head)) = (self.fetch.exchange, self.fetch.response) else { return };
        self.fetch.deadline = None;
        self.fetch.used = true;
        self.fetch.body = None;
        self.fetch.response = None;
        if head.close || core::mem::take(&mut self.close_fetch_after) {
            sink(Output::Close { connection: ConnectionId::Fetch });
            self.fetch.link = Link::Closed;
        }
        if let Some(seconds) = head.retry_after {
            let delay = u64::from(seconds).saturating_mul(MICROSECONDS_PER_SECOND).min(MAX_RETRY_AFTER_US);
            self.next_fetch_at = self.next_fetch_at.max(Instant(now.0.saturating_add(delay)));
        }
        let status = head.status;
        self.report_exchange(now, &exchange, status, sink);
        // A bound session's exchange is never redirected (REQ-WEB-017).
        if matches!(exchange.purpose, Purpose::Register | Purpose::Refresh { .. }) {
            self.fetch.exchange = None;
            self.bound_complete(now, &exchange, &head, sink);
            return;
        }
        let redirect = matches!(
            status,
            STATUS_MOVED_PERMANENTLY
                | STATUS_FOUND
                | STATUS_SEE_OTHER
                | STATUS_TEMPORARY_REDIRECT
                | STATUS_PERMANENT_REDIRECT
        );
        if redirect {
            self.redirect(now, exchange, &head, sink);
            return;
        }
        self.fetch.exchange = None;
        if self.complete_channel(now, &exchange, &head, sink) {
            return;
        }
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        match exchange.purpose {
            // complete_channel handled every one of these, and
            // bound_complete the bound sessions'.
            Purpose::FrontDoor | Purpose::Contents | Purpose::Register | Purpose::Refresh { .. } => {}
            Purpose::Descriptor { via_link, channel } => {
                // A channel's settings may come as any type REQ-WEB-027
                // accepts for them; a service's descriptor only as JSON.
                let media_accepted = head.media == MediaType::Json
                    || channel && matches!(head.media, MediaType::TextPlain | MediaType::OctetStream);
                if status != STATUS_OK {
                    event(sink, Event::DescriptorFailed(Failure::Status(status)));
                } else if !media_accepted {
                    event(sink, Event::DescriptorFailed(Failure::MediaType));
                } else {
                    let parsed = if channel {
                        Descriptor::parse_channel(body, &exchange.url)
                    } else {
                        Descriptor::parse(body, &exchange.url)
                    };
                    match parsed {
                        Ok(descriptor) => {
                            self.open_descriptor(now, descriptor, via_link, sink);
                            if channel {
                                self.front_door = self
                                    .channel
                                    .and_then(|folder| channel::file_url(&folder, ChannelFile::Bundle).ok());
                            }
                        }
                        Err(error) => event(sink, Event::DescriptorRejected(error)),
                    }
                }
            }
            Purpose::Page { page, subcode, file } => {
                self.complete_page(now, &exchange, &head, (page, subcode, file), sink);
            }
            // Only a build with forms makes a form request.
            Purpose::Form if !cfg!(feature = "forms") => {}
            Purpose::Form => self.complete_form(&exchange, &head, sink),
        }
    }

    /// A page request is complete: the page's bytes, unchanged, missing
    /// (then, for a channel, its other file), or refused (REQ-WEB-019,
    /// REQ-WEB-027).
    fn complete_page(
        &mut self,
        now: Instant,
        exchange: &Exchange,
        head: &ResponseHead,
        (page, subcode, file): (PageAddress, Option<SubCode>, PageFile),
        sink: &mut Sink<'_>,
    ) {
        let status = head.status;
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        let secure = exchange.url.is_secure();
        let t42 = matches!(file, PageFile::Channel { t42: true, .. });
        let missing = status == STATUS_NOT_FOUND || (file != PageFile::Template && status == STATUS_GONE);
        if status == STATUS_NOT_MODIFIED {
            self.update_cache(now, page, subcode, head, t42);
            event(sink, Event::PageCurrent { page, subcode });
        } else if missing {
            match file {
                // REQ-WEB-019: `NNN.tti`, then `NNN.t42`, or the
                // other way round.
                PageFile::Channel { t42, second: false } => self.second_try = Some((page, subcode, !t42)),
                _ => event(sink, Event::PageNotInService { page, subcode }),
            }
        } else if status != STATUS_OK {
            event(sink, Event::FetchFailed { page, subcode, failure: Failure::Status(status) });
        } else {
            let checked = match file {
                PageFile::Template => {
                    if head.media.is_page() {
                        Ok(head.media)
                    } else {
                        Err(Failure::MediaType)
                    }
                }
                PageFile::Channel { t42, .. } => channel_file_media(t42, head.media, body),
            };
            match checked {
                Ok(media) => {
                    sink(Output::PageBytes(PageBytes {
                        source: Source::Page { page, subcode },
                        media,
                        url: exchange.url.as_str(),
                        secure,
                        bytes: body,
                    }));
                    if file != PageFile::Template {
                        self.t42_first = t42;
                    }
                    self.update_cache(now, page, subcode, head, t42);
                }
                Err(failure) => event(sink, Event::FetchFailed { page, subcode, failure }),
            }
        }
    }

    /// The responses that belong to a channel (REQ-WEB-018): settings that
    /// are missing mean the defaults, the bundle's pages are delivered,
    /// and the contents are passed on. Returns true when the response was
    /// one of these.
    fn complete_channel(
        &mut self,
        now: Instant,
        exchange: &Exchange,
        head: &ResponseHead,
        sink: &mut Sink<'_>,
    ) -> bool {
        let status = head.status;
        let missing = status == STATUS_NOT_FOUND || status == STATUS_GONE;
        let body = self.buffers.body.get(..self.fetch.body_len).unwrap_or(&[]);
        match exchange.purpose {
            Purpose::Descriptor { channel: true, via_link } if missing => {
                let Some(folder) = self.channel else { return true };
                event(sink, Event::ChannelDefaults);
                self.open_descriptor(now, Descriptor::channel_defaults(&folder), via_link, sink);
                self.front_door = channel::file_url(&folder, ChannelFile::Bundle).ok();
                true
            }
            Purpose::FrontDoor => {
                // A channel without a bundle is still a channel: its pages
                // are fetched one by one.
                if missing {
                } else if status != STATUS_OK {
                    event(sink, Event::BundleFailed(Failure::Status(status)));
                } else {
                    match channel_file_media(true, head.media, body) {
                        Ok(media) => sink(Output::PageBytes(PageBytes {
                            source: Source::FrontDoor,
                            media,
                            url: exchange.url.as_str(),
                            secure: exchange.url.is_secure(),
                            bytes: body,
                        })),
                        Err(failure) => event(sink, Event::BundleFailed(failure)),
                    }
                }
                true
            }
            Purpose::Contents => {
                if missing {
                    event(sink, Event::ContentsMissing);
                } else if status != STATUS_OK {
                    event(sink, Event::ContentsFailed(Failure::Status(status)));
                } else if !matches!(head.media, MediaType::TextPlain | MediaType::OctetStream) {
                    event(sink, Event::ContentsFailed(Failure::MediaType));
                } else {
                    event(sink, Event::Contents(body));
                }
                true
            }
            _ => false,
        }
    }

    fn redirect(&mut self, now: Instant, mut exchange: Exchange, head: &ResponseHead, sink: &mut Sink<'_>) {
        self.fetch.exchange = None;
        if exchange.redirects >= MAX_REDIRECTS {
            self.finish_purpose(exchange.purpose, Err(Failure::TooManyRedirects), sink);
            return;
        }
        let Some(location) = head.location else {
            self.finish_purpose(exchange.purpose, Err(Failure::BadRedirect), sink);
            return;
        };
        let next = match exchange.url.join(location.as_str()) {
            Ok(url) if !url.scheme().is_websocket() => url,
            Ok(_) => {
                self.finish_purpose(exchange.purpose, Err(Failure::Address(UrlError::Scheme)), sink);
                return;
            }
            Err(error) => {
                self.finish_purpose(exchange.purpose, Err(Failure::Address(error)), sink);
                return;
            }
        };
        // REQ-SEC-007: a redirect from HTTPS to plain HTTP is refused,
        // whichever host it names.
        let downgrade = exchange.url.is_secure() && !next.is_secure();
        let failure = if downgrade { Some(Failure::Downgrade) } else { self.refusal(now, &next) };
        if let Some(failure) = failure {
            Self::report_refusal(ConnectionId::Fetch, &next, failure, sink);
            self.finish_purpose(exchange.purpose, Err(failure), sink);
            return;
        }
        // RFC 9110 section 15.4: 303 always becomes GET, and 301 and 302
        // do for POST, as browsers do; 307 and 308 keep the method.
        let status = head.status;
        if status == STATUS_SEE_OTHER
            || ((status == STATUS_MOVED_PERMANENTLY || status == STATUS_FOUND) && exchange.method == Method::Post)
        {
            exchange.method = Method::Get;
            self.form.body_len = 0;
        }
        // Another channel is confirmed before anything from it is shown
        // (REQ-WEB-021). Forms keep their own rule below.
        if self.config.redirects_within_channel
            && exchange.purpose != Purpose::Form
            && !self.redirect_keeps_channel(&exchange.url, &next)
        {
            event(sink, Event::RedirectToOtherChannel { location: next.as_str() });
            self.finish_purpose(exchange.purpose, Err(Failure::RedirectToOtherChannel), sink);
            return;
        }
        // A redirect to another origin makes a form submission cross-site.
        if exchange.purpose == Purpose::Form && !next.origin().same(&exchange.url.origin()) {
            exchange.site = Site::CrossOriginForm;
        }
        exchange.url = next;
        exchange.redirects = exchange.redirects.saturating_add(1);
        exchange.sent = false;
        exchange.retried = false;
        exchange.started = now;
        self.fetch.exchange = Some(exchange);
        self.start_exchange(now, sink);
    }

    /// A redirect from `from` to `next` keeps the channel (REQ-WEB-021): for
    /// a channel, inside its folder; for a service opened by its descriptor,
    /// on the same origin. Either way, an upgrade from `http` to `https` on
    /// the same host between the default ports keeps it.
    fn redirect_keeps_channel(&self, from: &Url, next: &Url) -> bool {
        if let Some(folder) = &self.channel {
            return channel::redirect_keeps_channel(folder, next);
        }
        let upgrade = !from.is_secure()
            && next.is_secure()
            && next.host_text() == from.host_text()
            && from.port() == crate::url::Scheme::Http.default_port()
            && next.port() == crate::url::Scheme::Https.default_port();
        next.origin().same(&from.origin()) || upgrade
    }

    fn update_cache(
        &mut self,
        now: Instant,
        page: PageAddress,
        subcode: Option<SubCode>,
        head: &ResponseHead,
        t42: bool,
    ) {
        let index = self.cache.iter().position(|slot| slot.is_some_and(|e| e.page == page && e.subcode == subcode));
        if head.no_store {
            if let Some(slot) = index.and_then(|i| self.cache.get_mut(i)) {
                *slot = None;
            }
            return;
        }
        self.cache_tick = self.cache_tick.wrapping_add(1);
        let fresh_until = match head.max_age {
            Some(seconds) if !head.no_cache => {
                Some(Instant(now.0.saturating_add(u64::from(seconds).saturating_mul(MICROSECONDS_PER_SECOND))))
            }
            _ => None,
        };
        let previous = index.and_then(|i| self.cache.get(i).copied().flatten());
        let entry = CacheEntry {
            page,
            subcode,
            // A 304 may omit validators; keep the ones held.
            etag: head.etag.or(previous.and_then(|p| p.etag)),
            last_modified: head.last_modified.or(previous.and_then(|p| p.last_modified)),
            fresh_until,
            used: self.cache_tick,
            t42,
        };
        let slot = index.or_else(|| self.cache.iter().position(Option::is_none)).or_else(|| {
            self.cache.iter().enumerate().min_by_key(|(_, slot)| slot.map_or(0, |e| e.used)).map(|(i, _)| i)
        });
        if let Some(slot) = slot.and_then(|i| self.cache.get_mut(i)) {
            *slot = Some(entry);
        }
    }

    fn open_descriptor(&mut self, now: Instant, descriptor: Descriptor, via_link: bool, sink: &mut Sink<'_>) {
        self.cache = [None; CACHE_ENTRIES];
        self.navigation_pending = via_link;
        if self.push.phase != PushPhase::Idle {
            sink(Output::Close { connection: ConnectionId::Push });
        }
        self.push.phase = PushPhase::Idle;
        self.push.delay_us = PUSH_RETRY_FIRST_US;
        self.push.sse = SseParser::new();
        self.push.url = descriptor.push().map(|(url, _)| *url);
        self.push.method = descriptor.push().map_or(PushMethod::WebSocket, |(_, method)| *method);
        let descriptor = self.descriptor.insert(descriptor);
        event(
            sink,
            Event::ServiceReady {
                name: descriptor.name.as_str(),
                initial_page: descriptor.initial_page,
                region: descriptor.region,
                secure: descriptor.address().is_secure(),
            },
        );
        if self.push.url.is_some() {
            if cfg!(feature = "push") {
                self.push_connect(now, sink);
            } else {
                event(sink, Event::PushNotCompiledIn(self.push.method));
            }
        }
    }

    // -----------------------------------------------------------------
    // Time
    // -----------------------------------------------------------------

    fn tick(&mut self, now: Instant, sink: &mut Sink<'_>) {
        if let Some(deadline) = self.fetch.deadline
            && now >= deadline
        {
            self.abandon_fetch(Failure::Timeout, sink);
        }
        self.push_tick(now, sink);
    }

    fn next_deadline(&self) -> Option<Instant> {
        let push = match self.push.phase {
            PushPhase::Backoff { until } => Some(until),
            PushPhase::Connecting { deadline } | PushPhase::Handshake { deadline } => Some(deadline),
            PushPhase::Open { .. } => self.heartbeat_due(),
            PushPhase::Idle => None,
        };
        let queue =
            (self.fetch.exchange.is_none() && self.queue.iter().any(Option::is_some)).then_some(self.next_fetch_at);
        [self.fetch.deadline, push, queue].into_iter().flatten().min()
    }
}
