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
//! Two connections are used: [`ConnectionId::Fetch`](crate::port::ConnectionId::Fetch) carries the
//! descriptor, page and form requests one at a time (REQ-WEB-014), and
//! [`ConnectionId::Push`](crate::port::ConnectionId::Push) carries the push stream (REQ-WEB-005).
//!
//! Push, form submission and cookies are compile-time features (decision
//! E.61). The agent's types are the same in every build. A build without a
//! feature answers whatever needs it with a value that says so, and every
//! branch that would reach the missing part tests a constant, so the
//! optimiser and the linker drop that part's code.

use crate::user_agent::UserAgentString;
use neoteletext_core::types::{Instant, PageAddress, SubCode};

use crate::bytes::TextBuf;
use crate::consts::{
    CACHE_ENTRIES, CONFIRMED_ORIGINS, MAX_HOST_TEXT_LENGTH, MAX_SUBSCRIBED_PAGES, MAX_VALIDATOR_LENGTH,
    MICROSECONDS_PER_SECOND, PAGE_QUEUE, PUSH_RETRY_FIRST_US, SECURE_HOSTS,
};
use crate::cookie::{CookieJar, Cookies, Site};
use crate::descriptor::{Descriptor, PushMethod};
use crate::http::{BodyDecoder, HeadCollector, Method, ResponseHead};
use crate::port::RandomSource;
use crate::sse::SseParser;
use crate::url::{Origin, Url};
use crate::ws::{FrameReader, HandshakeKey};

mod config;
mod cookies;
mod events;
mod fetch;
mod form;
mod input;
mod page;
mod push;
mod push_data;
mod push_messages;
mod request;
mod response;
mod service;

pub use config::{Buffers, Config, Storage};
pub use events::{BoundChange, Event, Failure, Input, Output, PageBytes, PushProblem, Sink, Source, Subscription};

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
}
