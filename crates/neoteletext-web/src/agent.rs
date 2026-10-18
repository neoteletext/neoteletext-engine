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

use neoteletext_core::policy;
use neoteletext_core::types::{Instant, PageAddress, SubCode};

use crate::bytes::{Overflow, TextBuf, Writer, copy_prefix};
use crate::channel::{self, ChannelFile};
use crate::consts::{
    ACCEPT_EVENT_STREAM, ACCEPT_PAGES, ALLOWED_LOCAL, CACHE_ENTRIES, CONFIRMED_ORIGINS, CRLF,
    DEFAULT_CONNECT_TIMEOUT_US, DEFAULT_COOKIES_PER_HOST, DEFAULT_COOKIES_TOTAL, DEFAULT_DESCRIPTOR_LIMIT,
    DEFAULT_FETCH_INTERVAL_US, DEFAULT_HEADER_LIMIT, DEFAULT_PAGE_LIMIT, DEFAULT_PUSH_LIMIT,
    DEFAULT_RESPONSE_TIMEOUT_US, FONT_QUEUE, IDENTITY, MAX_HOST_TEXT_LENGTH, MAX_REDIRECTS, MAX_RETRY_AFTER_US,
    MAX_SUBSCRIBED_PAGES, MAX_URL_LENGTH, MAX_VALIDATOR_LENGTH, MICROSECONDS_PER_SECOND, OPCODE_PONG, OPCODE_TEXT,
    PAGE_QUEUE, PUSH_RETRY_FIRST_US, PUSH_RETRY_MAX_US, SECURE_HOSTS, STATUS_CLIENT_ERROR_FIRST, STATUS_FOUND,
    STATUS_GONE, STATUS_MOVED_PERMANENTLY, STATUS_NOT_FOUND, STATUS_NOT_MODIFIED, STATUS_OK, STATUS_PERMANENT_REDIRECT,
    STATUS_REDIRECT_FIRST, STATUS_SEE_OTHER, STATUS_SUCCESS_FIRST, STATUS_TEMPORARY_REDIRECT, SUBSCRIBE, SUBSCRIBE_ALL,
    SUBSCRIBE_QUERY, header, media,
};
use crate::cookie::{CookieJar, CookieRejected, Site};
use crate::descriptor::{Descriptor, DescriptorError, PushMethod};
use crate::form::{self, FormMethod, FormRefused, FormSubmission};
use crate::http::{self, BodyDecoder, HeadCollector, HttpError, MediaType, Method, ResponseHead};
use crate::port::{ConnectionId, CookieStorage, NetInput, NetOutput, RandomSource, Security};
use crate::sse::{SseError, SseEvent, SseParser};
use crate::url::{IpAddress, Origin, Url, UrlError};
use crate::ws::{self, FrameReader, HandshakeKey, WsError, WsMessage};

/// The bytes of one T42 packet (REQ-INP-002).
const T42_PACKET: usize = neoteletext_core::consts::PACKET_BYTES;

/// The agent's configuration. Every security setting defaults to the safer
/// choice (REQ-SEC-020).
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent yes-or-no setting named by a requirement, not a state"
)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Largest page response body (REQ-WEB-008), at most the body buffer.
    pub page_limit: usize,
    /// Largest push message (REQ-WEB-008), at most the push buffer.
    pub push_limit: usize,
    /// Largest descriptor (REQ-WEB-008), at most the body buffer.
    pub descriptor_limit: usize,
    /// Largest response head (REQ-WEB-008), at most the head buffers.
    pub header_limit: usize,
    /// Shortest interval between fetches (REQ-WEB-004).
    pub fetch_interval_us: u64,
    /// Time limit for opening a connection (REQ-SEC-011).
    pub connect_timeout_us: u64,
    /// Time limit for a request and its response (REQ-SEC-011).
    pub response_timeout_us: u64,
    /// The port can offer TLS. Without it, HTTPS addresses are reported
    /// unreachable (REQ-WEB-008).
    pub tls_available: bool,
    /// Local-network addresses explicitly allowed (REQ-SEC-006). None by
    /// default.
    pub allowed_local: [Option<IpAddress>; ALLOWED_LOCAL],
    /// Ask before the first submission to each origin (REQ-SEC-009). The
    /// plain-HTTP confirmation of REQ-SEC-008 cannot be switched off.
    pub confirm_first_submission: bool,
    /// Cookies kept per host (REQ-WEB-006).
    pub cookies_per_host: usize,
    /// Cookies kept in total (REQ-WEB-006).
    pub cookies_total: usize,
    /// The port saves cookies between runs (REQ-SEC-013). Off by default.
    pub persist_cookies: bool,
    /// Refuse redirects of descriptor, page, bundle and contents requests
    /// that leave the channel, reporting [`Event::RedirectToOtherChannel`]
    /// instead, so that a browser can show its tuning screen before another
    /// channel's content (REQ-WEB-021). An upgrade from `http` to `https`
    /// on the same host and folder, between the default ports, keeps the
    /// channel. For a service opened by its descriptor, the channel is its
    /// origin. Off by default, which keeps `neottx`'s behaviour.
    pub redirects_within_channel: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            page_limit: DEFAULT_PAGE_LIMIT,
            push_limit: DEFAULT_PUSH_LIMIT,
            descriptor_limit: DEFAULT_DESCRIPTOR_LIMIT,
            header_limit: DEFAULT_HEADER_LIMIT,
            fetch_interval_us: DEFAULT_FETCH_INTERVAL_US,
            connect_timeout_us: DEFAULT_CONNECT_TIMEOUT_US,
            response_timeout_us: DEFAULT_RESPONSE_TIMEOUT_US,
            tls_available: true,
            allowed_local: [None; ALLOWED_LOCAL],
            confirm_first_submission: true,
            cookies_per_host: DEFAULT_COOKIES_PER_HOST,
            cookies_total: DEFAULT_COOKIES_TOTAL,
            persist_cookies: false,
            redirects_within_channel: false,
        }
    }
}

/// The byte buffers the agent works in, supplied by the caller
/// (REQ-GEN-003). Their lengths bound the corresponding limits.
#[derive(Debug)]
pub struct Buffers<'m> {
    /// Response heads on the fetch connection.
    pub fetch_head: &'m mut [u8],
    /// Requests on the fetch connection, form bodies included.
    pub fetch_request: &'m mut [u8],
    /// Page, descriptor and form response bodies.
    pub body: &'m mut [u8],
    /// The push connection's handshake response head.
    pub push_head: &'m mut [u8],
    /// Requests and frames on the push connection.
    pub push_request: &'m mut [u8],
    /// One push message.
    pub push_message: &'m mut [u8],
    /// The encoded values of the form being submitted.
    pub form: &'m mut [u8],
}

/// Buffers of the default sizes, for callers that do not need to choose.
#[derive(Debug)]
pub struct Storage {
    fetch_head: [u8; DEFAULT_HEADER_LIMIT],
    fetch_request: [u8; crate::consts::DEFAULT_REQUEST_LIMIT],
    body: [u8; DEFAULT_PAGE_LIMIT],
    push_head: [u8; DEFAULT_HEADER_LIMIT],
    push_request: [u8; crate::consts::DEFAULT_REQUEST_LIMIT],
    push_message: [u8; DEFAULT_PUSH_LIMIT],
    form: [u8; crate::consts::DEFAULT_FORM_LIMIT],
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage {
    /// Zeroed buffers. Large: the caller places them in static memory or
    /// on the heap.
    #[must_use]
    // The arrays are the buffers themselves; building them is the point.
    #[allow(clippy::large_stack_arrays)]
    pub const fn new() -> Self {
        Self {
            fetch_head: [0; DEFAULT_HEADER_LIMIT],
            fetch_request: [0; crate::consts::DEFAULT_REQUEST_LIMIT],
            body: [0; DEFAULT_PAGE_LIMIT],
            push_head: [0; DEFAULT_HEADER_LIMIT],
            push_request: [0; crate::consts::DEFAULT_REQUEST_LIMIT],
            push_message: [0; DEFAULT_PUSH_LIMIT],
            form: [0; crate::consts::DEFAULT_FORM_LIMIT],
        }
    }

    /// The buffers, borrowed for an agent.
    pub fn buffers(&mut self) -> Buffers<'_> {
        Buffers {
            fetch_head: &mut self.fetch_head,
            fetch_request: &mut self.fetch_request,
            body: &mut self.body,
            push_head: &mut self.push_head,
            push_request: &mut self.push_request,
            push_message: &mut self.push_message,
            form: &mut self.form,
        }
    }
}

/// Which pages the push connection should carry (REQ-WEB-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Subscription<'a> {
    /// The whole service.
    All,
    /// Only these pages.
    Pages(&'a [PageAddress]),
}

/// An input to the agent.
#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    /// Time has passed: deadlines and the rate limit are checked.
    Tick,
    /// Something happened on a connection.
    Net(NetInput<'a>),
    /// Open the service whose descriptor is at this address.
    OpenService(&'a str),
    /// Open the channel whose folder, or one of whose files, is at this
    /// address (REQ-WEB-018): its `channel.json`, or the defaults when it
    /// has none (404 or 410); then its bundle `index.t42`; then pages from
    /// `NNN.tti` or `NNN.t42` in its folder.
    OpenChannel(&'a str),
    /// Fetch the open channel's `contents.txt` (REQ-WEB-025), answered by
    /// [`Event::Contents`], [`Event::ContentsMissing`] or
    /// [`Event::ContentsFailed`].
    FetchContents,
    /// Fetch a font, or a font library's `fonts.txt` when `index` is set
    /// (REQ-FNT-003, REQ-FNT-004), at this absolute address, answered by
    /// [`Event::Font`] or [`Event::FontFailed`]. No service need be open.
    /// The request carries no cookies and none are stored from its answer;
    /// whether the address may be fetched at all is the caller's decision
    /// (REQ-WEB-014), and the local-network and downgrade rules apply as to
    /// every request (REQ-SEC-006, REQ-SEC-007).
    FetchFont {
        /// The address.
        address: &'a str,
        /// A library index rather than a font.
        index: bool,
        /// The `ETag` of the copy the caller holds, for the conditional
        /// request of a font's daily check (REQ-FNT-009, REQ-WEB-004).
        etag: Option<&'a str>,
        /// The `Last-Modified` of that copy.
        last_modified: Option<&'a str>,
    },
    /// Follow a link to a page of another service named in the current
    /// descriptor (Appendix E). The request is a cross-service navigation
    /// for cookies (REQ-WEB-006).
    FollowLink {
        /// The service's short name.
        service: &'a str,
        /// The page.
        page: PageAddress,
        /// The sub-page, if given.
        subcode: Option<SubCode>,
    },
    /// Fetch a page of the current service (REQ-WEB-003, REQ-WEB-007).
    FetchPage {
        /// The page.
        page: PageAddress,
        /// The sub-page, if given.
        subcode: Option<SubCode>,
    },
    /// Change the push subscription.
    Subscribe(Subscription<'a>),
    /// Submit a form (REQ-WEB-011).
    Submit(FormSubmission<'a>),
    /// The viewer's answer to [`Event::ConfirmSubmission`].
    ConfirmSubmission(bool),
    /// Erase every cookie (REQ-SEC-013).
    WipeCookies,
    /// Calendar time now, in seconds since 1970, so cookie `Expires` dates
    /// can be compared and cookies saved.
    WallClock(u64),
}

/// Where page bytes came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// A page fetch.
    Page {
        /// The page asked for.
        page: PageAddress,
        /// The sub-page asked for.
        subcode: Option<SubCode>,
    },
    /// The push connection.
    Push,
    /// The response to a form submission (REQ-WEB-011 item 3).
    Form,
    /// A channel's bundle, `index.t42`: it may hold any number of the
    /// channel's pages (REQ-WEB-019).
    FrontDoor,
}

/// Page bytes for the format readers (TTI or T42), with what is known
/// about them. The caller decides the format from `media`, detecting it
/// for `text/plain` and `application/octet-stream` (REQ-INP-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PageBytes<'a> {
    /// Where they came from.
    pub source: Source,
    /// The declared media type.
    pub media: MediaType,
    /// The address they came from, after redirects: the base for a form
    /// on the page, and its origin.
    pub url: &'a str,
    /// True when they came over an encrypted connection; false means the
    /// security indicator must show (REQ-SEC-008).
    pub secure: bool,
    /// The bytes.
    pub bytes: &'a [u8],
}

/// Why a request failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Failure {
    /// The connection could not be made or broke.
    Network,
    /// A time limit passed (REQ-SEC-011).
    Timeout,
    /// The response broke the HTTP profile.
    Http(HttpError),
    /// An unexpected status code.
    Status(u16),
    /// More than five redirects (REQ-WEB-003).
    TooManyRedirects,
    /// A redirect that leaves the channel, refused under
    /// [`Config::redirects_within_channel`].
    RedirectToOtherChannel,
    /// A channel's file did not pass the check for the format its name
    /// says (REQ-WEB-027).
    Content,
    /// A redirect without a usable `Location`.
    BadRedirect,
    /// An address failed the strict parser.
    Address(UrlError),
    /// A media type outside the profile (REQ-WEB-014).
    MediaType,
    /// A blocked local-network address (REQ-SEC-006).
    Blocked,
    /// A downgrade from HTTPS to plain HTTP (REQ-SEC-007).
    Downgrade,
    /// An HTTPS address, and the port offers no TLS (REQ-WEB-008).
    TlsUnavailable,
    /// No service is open.
    NoService,
    /// Too many page requests are waiting.
    QueueFull,
    /// The request did not fit in the request buffer.
    RequestTooLarge,
}

/// What went wrong on the push connection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PushProblem {
    /// The connection or its set-up failed.
    Failure(Failure),
    /// A WebSocket rule was broken.
    WebSocket(WsError),
    /// A server-sent event was too big.
    EventStream(SseError),
    /// Pushed T42 data was not whole 42-byte packets.
    PacketLength,
    /// Pushed packets were not valid base64.
    Base64,
}

/// An event for the viewer and the application.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event<'a> {
    /// The descriptor was read and checked; the service is open.
    ServiceReady {
        /// The service name.
        name: &'a str,
        /// Its initial page.
        initial_page: PageAddress,
        /// The default character set region, if given (decision E.4).
        region: Option<u8>,
        /// The descriptor came over HTTPS.
        secure: bool,
    },
    /// The descriptor was fetched but is not valid (REQ-WEB-002).
    DescriptorRejected(DescriptorError),
    /// The descriptor could not be fetched.
    DescriptorFailed(Failure),
    /// A page is unchanged (304), or still fresh by its cache lifetime
    /// (REQ-WEB-004): the stored copy is current.
    PageCurrent {
        /// The page.
        page: PageAddress,
        /// The sub-page asked for.
        subcode: Option<SubCode>,
    },
    /// The server has no such page (404, REQ-WEB-003).
    PageNotInService {
        /// The page.
        page: PageAddress,
        /// The sub-page asked for.
        subcode: Option<SubCode>,
    },
    /// A page fetch failed; stored pages are not disturbed (REQ-WEB-003).
    FetchFailed {
        /// The page.
        page: PageAddress,
        /// The sub-page asked for.
        subcode: Option<SubCode>,
        /// Why.
        failure: Failure,
    },
    /// A connection was made without encryption (REQ-WEB-008).
    Unencrypted {
        /// Which connection.
        connection: ConnectionId,
        /// The host.
        host: &'a str,
    },
    /// A connection was refused because of where it led (REQ-SEC-006).
    Blocked {
        /// Which connection.
        connection: ConnectionId,
        /// The host asked for.
        host: &'a str,
    },
    /// A downgrade to plain HTTP was refused (REQ-SEC-007).
    DowngradeRefused {
        /// The host.
        host: &'a str,
    },
    /// The push connection is open.
    PushConnected(PushMethod),
    /// The descriptor names a push address, but this build has no push
    /// (the `push` feature, decision E.61): pages are fetched on request
    /// only.
    PushNotCompiledIn(PushMethod),
    /// The push connection was lost; it is retried after this delay.
    PushLost {
        /// Microseconds until the next attempt.
        retry_in_us: u64,
        /// What went wrong, if known.
        problem: Option<PushProblem>,
    },
    /// The viewer must confirm a form submission (REQ-SEC-008, REQ-SEC-009)
    /// and answer with [`Input::ConfirmSubmission`].
    ConfirmSubmission {
        /// The destination host.
        host: &'a str,
        /// The destination port.
        port: u16,
        /// The submission is encrypted.
        secure: bool,
        /// The values will travel unencrypted: the warning of REQ-SEC-008.
        plain_http: bool,
        /// The first submission to this origin (REQ-SEC-009).
        first_to_origin: bool,
    },
    /// The form was not submitted.
    SubmissionRefused(FormRefused),
    /// The viewer declined to confirm.
    SubmissionCancelled,
    /// The server accepted the form (2xx without a page): show the `FP`
    /// success page.
    SubmissionSucceeded {
        /// The status code.
        status: u16,
    },
    /// The submission failed: show the `FP` error page and keep the values.
    SubmissionFailed {
        /// The status code, when the server answered.
        status: Option<u16>,
        /// The failure, when it did not.
        failure: Option<Failure>,
    },
    /// The server answered the form with a TTI page, delivered as
    /// [`PageBytes`] with [`Source::Form`].
    SubmissionPage,
    /// A link named a service the descriptor does not list: the link is
    /// null (Appendix E.2 rule 2).
    LinkUnknown,
    /// A request was redirected out of the channel and, under
    /// [`Config::redirects_within_channel`], not followed. The browser
    /// confirms the new channel before going there.
    RedirectToOtherChannel {
        /// The address the redirect leads to.
        location: &'a str,
    },
    /// The channel has no `channel.json`: the defaults apply
    /// (REQ-WEB-018).
    ChannelDefaults,
    /// The channel's `index.t42` could not be used: it answered with
    /// something other than 200, 404 or 410, or failed its checks. The
    /// channel works without it (REQ-WEB-018).
    BundleFailed(Failure),
    /// The channel's `contents.txt`, unchecked: the caller reads it with
    /// [`crate::contents::read`].
    Contents(&'a [u8]),
    /// The channel has no `contents.txt` (404 or 410).
    ContentsMissing,
    /// The channel's `contents.txt` could not be fetched or used.
    ContentsFailed(Failure),
    /// A font or a library index, unchecked: the caller reads it in a
    /// confined process (REQ-FNT-003).
    Font {
        /// A library index rather than a font.
        index: bool,
        /// The address it came from, after any redirects.
        url: &'a str,
        /// The bytes.
        body: &'a [u8],
        /// Its `ETag`, kept for the next conditional request.
        etag: Option<&'a str>,
        /// Its `Last-Modified`.
        last_modified: Option<&'a str>,
    },
    /// The copy the caller holds is current: the server answered 304 to
    /// the conditional request (REQ-FNT-009).
    FontCurrent {
        /// A library index rather than a font.
        index: bool,
    },
    /// A font or a library index could not be fetched.
    FontFailed {
        /// A library index rather than a font.
        index: bool,
        /// Why.
        failure: Failure,
    },
    /// A `Set-Cookie` header was ignored.
    CookieRejected(CookieRejected),
    /// Not every matching cookie fitted in a request.
    CookiesTruncated,
}

/// An output of the agent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output<'a> {
    /// Open a connection.
    Connect {
        /// Which connection.
        connection: ConnectionId,
        /// The host, without brackets.
        host: &'a str,
        /// The port.
        port: u16,
        /// Plain or TLS.
        security: Security,
    },
    /// Send bytes.
    Send {
        /// Which connection.
        connection: ConnectionId,
        /// The bytes.
        bytes: &'a [u8],
    },
    /// Close a connection.
    Close {
        /// Which connection.
        connection: ConnectionId,
    },
    /// The next time the agent needs an [`Input::Tick`].
    Timer {
        /// When.
        at: Instant,
    },
    /// Page bytes for the format readers.
    PageBytes(PageBytes<'a>),
    /// An event.
    Event(Event<'a>),
}

impl<'a> Output<'a> {
    /// The output as a network action for [`crate::port::Pump`], if it is
    /// one.
    #[must_use]
    pub const fn net(&self) -> Option<NetOutput<'a>> {
        match *self {
            Self::Connect { connection, host, port, security } => {
                Some(NetOutput::Connect { connection, host, port, security })
            }
            Self::Send { connection, bytes } => Some(NetOutput::Send(connection, bytes)),
            Self::Close { connection } => Some(NetOutput::Close(connection)),
            _ => None,
        }
    }
}

/// The output sink.
pub type Sink<'s> = dyn FnMut(Output<'_>) + 's;

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
    Descriptor { via_link: bool, channel: bool },
    FrontDoor,
    Page { page: PageAddress, subcode: Option<SubCode>, file: PageFile },
    Form,
    Contents,
    Font { index: bool },
}

/// A copy's validators (REQ-WEB-004).
#[derive(Clone, Copy, Debug, Default)]
struct Validators {
    etag: Option<TextBuf<MAX_VALIDATOR_LENGTH>>,
    last_modified: Option<TextBuf<MAX_VALIDATOR_LENGTH>>,
}

/// A font or library index waiting to be fetched.
#[derive(Clone, Copy, Debug)]
struct FontFetch {
    url: Url,
    index: bool,
    validators: Validators,
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
    Backoff { until: Instant },
    Connecting { deadline: Instant },
    Handshake { deadline: Instant },
    Open,
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
#[derive(Debug)]
pub struct UserAgent<'m, R: RandomSource, const SLOTS: usize, const COOKIE_BYTES: usize> {
    config: Config,
    buffers: Buffers<'m>,
    random: R,
    jar: CookieJar<SLOTS, COOKIE_BYTES>,
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
    /// Fonts and library indexes to fetch, oldest first.
    fonts: [Option<FontFetch>; FONT_QUEUE],
    /// The validators of the font being fetched now.
    font_validators: Validators,
    navigation_pending: bool,
    secure_hosts: [Option<TextBuf<MAX_HOST_TEXT_LENGTH>>; SECURE_HOSTS],
    fetch: FetchConnection,
    /// Close the fetch connection when the current response completes.
    close_fetch_after: bool,
    queue: [Option<(PageAddress, Option<SubCode>)>; PAGE_QUEUE],
    next_fetch_at: Instant,
    cache: [Option<CacheEntry>; CACHE_ENTRIES],
    cache_tick: u64,
    form: FormState,
    confirmed: [Option<Origin>; CONFIRMED_ORIGINS],
    push: PushConnection,
    subscribe_all: bool,
    subscribed: [Option<PageAddress>; MAX_SUBSCRIBED_PAGES],
}

fn event(sink: &mut Sink<'_>, event: Event<'_>) {
    sink(Output::Event(event));
}

/// Writes the `Cookie` header for a request to `url`, if any cookie
/// matches, keeping room for the empty line and `body_len` bytes of body.
/// Returns true when some matching cookies did not fit.
fn write_cookie_header<const SLOTS: usize, const BYTES: usize>(
    jar: &mut CookieJar<SLOTS, BYTES>,
    writer: &mut Writer<'_>,
    url: &Url,
    path: &str,
    site: Site,
    seconds: u64,
    body_len: usize,
) -> Result<bool, Overflow> {
    if !cfg!(feature = "cookies") {
        return Ok(false);
    }
    // Room for this header's line end, the empty line and the body.
    writer.set_reserve(CRLF.len().saturating_mul(2).saturating_add(body_len));
    let mark = writer.len();
    let started = writer.put(header::COOKIE).and_then(|()| writer.put(b": "));
    let (count, truncated) = match started {
        Ok(()) => jar.header_for(url.host_text(), path, url.is_secure(), site, seconds, writer),
        Err(Overflow) => (0, true),
    };
    writer.set_reserve(0);
    if count == 0 {
        writer.truncate(mark);
    } else {
        writer.put(CRLF)?;
    }
    Ok(truncated)
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
        let mut jar = CookieJar::new();
        jar.set_limits(config.cookies_per_host, config.cookies_total);
        Self {
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
            fonts: [None; FONT_QUEUE],
            font_validators: Validators::default(),
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

    /// Saves the cookies that may be kept (REQ-SEC-013), when the
    /// configuration allows it and calendar time is known. Returns the
    /// number saved. `scratch` holds the encoded data.
    pub fn save_cookies<S: CookieStorage>(
        &self,
        now: Instant,
        storage: &mut S,
        scratch: &mut [u8],
    ) -> Result<usize, Option<S::Error>> {
        let (seconds, calendar) = self.seconds(now);
        if !cfg!(feature = "cookies") || !self.config.persist_cookies || !calendar {
            return Ok(0);
        }
        let mut writer = Writer::new(scratch);
        let count = self.jar.export(seconds, &mut writer).map_err(|_| None)?;
        storage.save(writer.written()).map_err(Some)?;
        Ok(count)
    }

    /// Restores saved cookies, when the configuration allows it and
    /// calendar time is known. Returns the number restored.
    pub fn load_cookies<S: CookieStorage>(
        &mut self,
        now: Instant,
        storage: &mut S,
        scratch: &mut [u8],
    ) -> Result<usize, S::Error> {
        let (seconds, calendar) = self.seconds(now);
        if !cfg!(feature = "cookies") || !self.config.persist_cookies || !calendar {
            return Ok(0);
        }
        let len = storage.load(scratch)?;
        Ok(self.jar.import(scratch.get(..len).unwrap_or(&[]), seconds))
    }

    /// Handles one input. Outputs go to `sink` before this returns.
    pub fn handle(&mut self, now: Instant, input: Input<'_>, sink: &mut Sink<'_>) {
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
                        PushPhase::Connecting { .. } | PushPhase::Handshake { .. } | PushPhase::Open
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
            Input::FetchFont { address, index, etag, last_modified } => {
                let validators = Validators {
                    etag: etag.and_then(|value| TextBuf::from_bytes(value.as_bytes())),
                    last_modified: last_modified.and_then(|value| TextBuf::from_bytes(value.as_bytes())),
                };
                self.fetch_font(address, index, &validators, sink);
            }
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
            Input::WipeCookies => self.jar.wipe(),
            Input::WallClock(unix) => self.wall_clock = Some((now, unix)),
        }
        self.dispatch(now, sink);
        if let Some(at) = self.next_deadline() {
            sink(Output::Timer { at });
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

    fn fetch_font(&mut self, address: &str, index: bool, validators: &Validators, sink: &mut Sink<'_>) {
        let url = match Url::parse(address) {
            Ok(url) if !url.scheme().is_websocket() => url,
            Ok(_) => return event(sink, Event::FontFailed { index, failure: Failure::Address(UrlError::Scheme) }),
            Err(error) => return event(sink, Event::FontFailed { index, failure: Failure::Address(error) }),
        };
        match self.fonts.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => *slot = Some(FontFetch { url, index, validators: *validators }),
            None => event(sink, Event::FontFailed { index, failure: Failure::QueueFull }),
        }
    }

    /// The oldest font or index waiting, keeping the rest in order.
    fn take_font(&mut self) -> Option<FontFetch> {
        let first = self.fonts.first_mut()?.take()?;
        self.fonts.rotate_left(1);
        Some(first)
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
        None
    }

    fn blocked(&self, address: IpAddress) -> bool {
        address.is_blocked() && !self.config.allowed_local.contains(&Some(address))
    }

    /// REQ-SEC-007: the host was reached over HTTPS this session, or holds
    /// a live cookie set over HTTPS, and `url` is plain. Decided by the
    /// policy.
    fn is_downgrade(&self, now: Instant, url: &Url) -> bool {
        let host = url.host_text();
        let (seconds, _) = self.seconds(now);
        let reached = self.secure_hosts.iter().flatten().any(|known| known.as_str() == host)
            || cfg!(feature = "cookies") && self.jar.reached_securely(host, seconds);
        policy::is_downgrade(reached, url.origin().parts(), host)
    }

    fn remember_secure(&mut self, host: &str) {
        if self.secure_hosts.iter().flatten().any(|known| known.as_str() == host) {
            return;
        }
        if let Some(slot) = self.secure_hosts.iter_mut().find(|slot| slot.is_none()) {
            *slot = TextBuf::from_bytes(host.as_bytes());
        }
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
            let (purpose, url, method, site) = if let Some((url, via_link, channel)) = self.pending_descriptor.take() {
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
            } else if let Some(font) = self.take_font() {
                self.font_validators = font.validators;
                // The site is irrelevant: a font request carries no cookies.
                (Purpose::Font { index: font.index }, font.url, Method::Get, Site::SameService)
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
        let body_len = if exchange.method == Method::Post { self.form.body_len } else { 0 };
        let mut writer = Writer::new(self.buffers.fetch_request);
        http::write_request_line(&mut writer, exchange.method, exchange.url.target())?;
        http::write_host(&mut writer, &exchange.url)?;
        http::write_header(&mut writer, header::USER_AGENT, crate::consts::USER_AGENT.as_bytes())?;
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
        // REQ-FNT-009: a font's daily check is a conditional request.
        if matches!(exchange.purpose, Purpose::Font { .. }) && exchange.redirects == 0 {
            if let Some(etag) = &self.font_validators.etag {
                http::write_header(&mut writer, header::IF_NONE_MATCH, etag.as_bytes())?;
            }
            if let Some(date) = &self.font_validators.last_modified {
                http::write_header(&mut writer, header::IF_MODIFIED_SINCE, date.as_bytes())?;
            }
        }
        if exchange.method == Method::Post {
            http::write_header(&mut writer, header::CONTENT_TYPE, media::FORM_URLENCODED)?;
            writer.put(header::CONTENT_LENGTH)?;
            writer.put(b": ")?;
            writer.decimal(body_len as u64)?;
            writer.put(CRLF)?;
        }
        let path = exchange.url.path();
        // REQ-FNT-003, REQ-WEB-014: fonts and library indexes go without
        // cookies.
        let cookies = !matches!(exchange.purpose, Purpose::Font { .. });
        if cookies
            && write_cookie_header(&mut self.jar, &mut writer, &exchange.url, path, exchange.site, seconds, body_len)?
        {
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
        if self.blocked(peer) {
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
        let idempotent = exchange.purpose != Purpose::Form;
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
        let Self { buffers, jar, channel, .. } = self;
        let head_bytes = buffers.fetch_head.get(..head_len).unwrap_or(&[]);
        let host = exchange.url.host_text();
        let path = exchange.url.path();
        let secure = exchange.url.is_secure();
        // A channel's cookies stay in its folder (REQ-WEB-026).
        let scope = channel.as_ref().map(Url::path);
        // Nothing a font's answer sets is kept (REQ-FNT-003).
        let keep_cookies = !matches!(exchange.purpose, Purpose::Font { .. });
        let parsed = http::parse_head(head_bytes, &mut |value| {
            if !keep_cookies {
                return;
            }
            let stored = if cfg!(feature = "cookies") {
                jar.store_within(value, host, path, scope, secure, seconds, calendar)
            } else {
                Err(CookieRejected::NotCompiledIn)
            };
            if let Err(rejected) = stored {
                event(sink, Event::CookieRejected(rejected));
            }
        });
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
            Purpose::Font { index: false } => neoteletext_core::consts::FONT_MAX_FILE_BYTES,
            Purpose::Font { index: true } => crate::fonts::MAX_BYTES,
        }
        .min(self.buffers.body.len());
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
            Purpose::Font { index } => event(sink, Event::FontFailed { index, failure }),
            Purpose::Page { page, subcode, .. } => event(sink, Event::FetchFailed { page, subcode, failure }),
            Purpose::Form => {
                self.form.phase = FormPhase::Idle;
                event(sink, Event::SubmissionFailed { status: None, failure: Some(failure) });
            }
        }
    }

    /// A response is complete: act on it.
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
        let secure = exchange.url.is_secure();
        match exchange.purpose {
            // complete_channel handled every one of these.
            Purpose::FrontDoor | Purpose::Contents | Purpose::Font { .. } => {}
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
            Purpose::Form => {
                self.form.phase = FormPhase::Idle;
                let success = (STATUS_SUCCESS_FIRST..STATUS_REDIRECT_FIRST).contains(&status);
                if success && head.media == MediaType::Tti {
                    sink(Output::PageBytes(PageBytes {
                        source: Source::Form,
                        media: head.media,
                        url: exchange.url.as_str(),
                        secure,
                        bytes: body,
                    }));
                    event(sink, Event::SubmissionPage);
                } else if success {
                    event(sink, Event::SubmissionSucceeded { status });
                } else if status >= STATUS_CLIENT_ERROR_FIRST && head.media == MediaType::Tti {
                    // REQ-WEB-011 item 3: a TTI page is displayed whatever
                    // the status; the error is still reported.
                    sink(Output::PageBytes(PageBytes {
                        source: Source::Form,
                        media: head.media,
                        url: exchange.url.as_str(),
                        secure,
                        bytes: body,
                    }));
                    event(sink, Event::SubmissionFailed { status: Some(status), failure: None });
                } else {
                    event(sink, Event::SubmissionFailed { status: Some(status), failure: None });
                }
            }
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
            Purpose::Font { index } => {
                // A font may come as BDF's own type or a generic one; an
                // index is text, like `contents.txt`.
                let media_accepted = matches!(head.media, MediaType::TextPlain | MediaType::OctetStream)
                    || !index && head.media == MediaType::FontBdf;
                if status == STATUS_NOT_MODIFIED {
                    event(sink, Event::FontCurrent { index });
                } else if status != STATUS_OK {
                    event(sink, Event::FontFailed { index, failure: Failure::Status(status) });
                } else if !media_accepted {
                    event(sink, Event::FontFailed { index, failure: Failure::MediaType });
                } else {
                    event(
                        sink,
                        Event::Font {
                            index,
                            url: exchange.url.as_str(),
                            body,
                            etag: head.etag.as_ref().map(TextBuf::as_str),
                            last_modified: head.last_modified.as_ref().map(TextBuf::as_str),
                        },
                    );
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
        // A browser confirms another channel before showing anything from
        // it (REQ-WEB-021). Forms keep their own rule below.
        if self.config.redirects_within_channel
            && exchange.purpose != Purpose::Form
            && !matches!(exchange.purpose, Purpose::Font { .. })
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
    // Forms
    // -----------------------------------------------------------------

    fn submit(&mut self, submission: &FormSubmission<'_>, sink: &mut Sink<'_>) {
        if self.form.phase != FormPhase::Idle {
            event(sink, Event::SubmissionRefused(FormRefused::Busy));
            return;
        }
        let page = match Url::parse(submission.page_url) {
            Ok(page) => page,
            Err(error) => {
                event(sink, Event::SubmissionRefused(FormRefused::Address(error)));
                return;
            }
        };
        let permitted = self.descriptor.as_ref().map(Descriptor::form_origins).into_iter().flatten();
        let target = match form::destination(&page, submission.target, permitted) {
            Ok(target) => target,
            Err(refused) => {
                event(sink, Event::SubmissionRefused(refused));
                return;
            }
        };
        // A channel's form submits inside its folder, or to an origin its
        // settings list (REQ-WEB-026).
        if let Some(folder) = &self.channel {
            let listed = self.descriptor.as_ref().is_some_and(|d| d.form_origins().any(|o| o.same(&target.origin())));
            if !channel::is_inside(folder, &target) && !listed {
                event(sink, Event::SubmissionRefused(FormRefused::Origin));
                return;
            }
        }
        let mut writer = Writer::new(self.buffers.form);
        if form::encode(submission.fields, submission.token, submission.action, &mut writer).is_err() {
            event(sink, Event::SubmissionRefused(FormRefused::TooLarge));
            return;
        }
        let encoded_len = writer.len();
        let (url, method, body_len) = match submission.method {
            FormMethod::Post => (target, Method::Post, encoded_len),
            FormMethod::Get => {
                // The values replace the target's query.
                let mut buffer = [0u8; MAX_URL_LENGTH];
                let mut address = Writer::new(&mut buffer);
                let built = (|| -> Result<(), Overflow> {
                    address.put(target.scheme().name())?;
                    address.put(b"://")?;
                    address.put(target.authority())?;
                    address.put(target.path().as_bytes())?;
                    address.byte(b'?')?;
                    address.put(self.buffers.form.get(..encoded_len).unwrap_or(&[]))
                })();
                let parsed = built
                    .map_err(|_| FormRefused::TooLarge)
                    .and_then(|()| core::str::from_utf8(address.written()).map_err(|_| FormRefused::TooLarge))
                    .and_then(|text| Url::parse(text).map_err(FormRefused::Address));
                match parsed {
                    Ok(url) => (url, Method::Get, 0),
                    Err(refused) => {
                        event(sink, Event::SubmissionRefused(refused));
                        return;
                    }
                }
            }
        };
        let target_origin = url.origin();
        let site = if target_origin.same(&page.origin()) { Site::SameService } else { Site::CrossOriginForm };
        self.form = FormState { phase: FormPhase::Queued, target: Some(url), method, body_len, site };
        let plain_http = policy::needs_plain_http_warning(url.is_secure());
        let first_to_origin = self.config.confirm_first_submission
            && !self.confirmed.iter().flatten().any(|known| known.same(&target_origin));
        if plain_http || first_to_origin {
            self.form.phase = FormPhase::Confirming { record_origin: first_to_origin };
            event(
                sink,
                Event::ConfirmSubmission {
                    host: url.host_text(),
                    port: url.port(),
                    secure: url.is_secure(),
                    plain_http,
                    first_to_origin,
                },
            );
        }
    }

    fn confirm(&mut self, confirmed: bool, sink: &mut Sink<'_>) {
        let FormPhase::Confirming { record_origin } = self.form.phase else { return };
        if !confirmed {
            self.form = FormState {
                phase: FormPhase::Idle,
                target: None,
                method: Method::Post,
                body_len: 0,
                site: Site::SameService,
            };
            event(sink, Event::SubmissionCancelled);
            return;
        }
        if record_origin && let Some(target) = &self.form.target {
            let origin = target.origin();
            if let Some(slot) = self.confirmed.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(origin);
            } else {
                // Full: forget the oldest, so the newest is remembered.
                self.confirmed.rotate_left(1);
                if let Some(last) = self.confirmed.last_mut() {
                    *last = Some(origin);
                }
            }
        }
        self.form.phase = FormPhase::Queued;
    }

    // -----------------------------------------------------------------
    // Push
    // -----------------------------------------------------------------

    fn subscribe(&mut self, now: Instant, subscription: Subscription<'_>, sink: &mut Sink<'_>) {
        self.subscribed = [None; MAX_SUBSCRIBED_PAGES];
        match subscription {
            Subscription::All => self.subscribe_all = true,
            Subscription::Pages(pages) => {
                self.subscribe_all = false;
                for (slot, page) in self.subscribed.iter_mut().zip(pages) {
                    *slot = Some(*page);
                }
            }
        }
        if cfg!(feature = "push") && self.push.phase == PushPhase::Open {
            match self.push.method {
                PushMethod::WebSocket => self.send_subscribe(sink),
                PushMethod::ServerSentEvents => {
                    // The subscription is in the address: reconnect at once.
                    sink(Output::Close { connection: ConnectionId::Push });
                    self.push.phase = PushPhase::Idle;
                    self.push_connect(now, sink);
                }
            }
        }
    }

    /// Writes the subscription: `all`, or page numbers separated by
    /// `separator`.
    fn write_subscription(&self, writer: &mut Writer<'_>, separator: u8) -> Result<(), Overflow> {
        if self.subscribe_all {
            return writer.put(SUBSCRIBE_ALL);
        }
        for (index, page) in self.subscribed.iter().flatten().enumerate() {
            if index > 0 {
                writer.byte(separator)?;
            }
            writer.put(&page.ascii())?;
        }
        Ok(())
    }

    fn send_subscribe(&mut self, sink: &mut Sink<'_>) {
        let mut text = [0u8; 4 * MAX_SUBSCRIBED_PAGES + 16];
        let mut writer = Writer::new(&mut text);
        let built = writer
            .put(SUBSCRIBE)
            .and_then(|()| writer.byte(b' '))
            .and_then(|()| self.write_subscription(&mut writer, b' '));
        if built.is_err() {
            return;
        }
        let len = writer.len();
        let message = text.get(..len).unwrap_or(&[]);
        let mut frame = Writer::new(self.buffers.push_request);
        if ws::write_frame(&mut frame, OPCODE_TEXT, message, &mut self.random).is_ok() {
            let len = frame.len();
            let bytes = self.buffers.push_request.get(..len).unwrap_or(&[]);
            sink(Output::Send { connection: ConnectionId::Push, bytes });
        }
    }

    /// The push address with the server-sent events subscription query.
    fn push_address(&self) -> Option<Url> {
        let url = self.push.url?;
        if self.push.method == PushMethod::WebSocket {
            return Some(url);
        }
        let mut buffer = [0u8; MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut buffer);
        writer.put(url.as_str().as_bytes()).ok()?;
        writer.byte(if url.query().is_some() { b'&' } else { b'?' }).ok()?;
        writer.put(SUBSCRIBE_QUERY).ok()?;
        self.write_subscription(&mut writer, b',').ok()?;
        Url::parse(core::str::from_utf8(writer.written()).ok()?).ok()
    }

    fn push_connect(&mut self, now: Instant, sink: &mut Sink<'_>) {
        let Some(url) = self.push_address() else {
            self.push.phase = PushPhase::Idle;
            return;
        };
        if let Some(failure) = self.refusal(now, &url) {
            Self::report_refusal(ConnectionId::Push, &url, failure, sink);
            self.push_lost(now, Some(PushProblem::Failure(failure)), false, sink);
            return;
        }
        self.push.phase =
            PushPhase::Connecting { deadline: Instant(now.0.saturating_add(self.config.connect_timeout_us)) };
        self.push.healthy = false;
        let security = if url.is_secure() { Security::Tls } else { Security::Plain };
        sink(Output::Connect { connection: ConnectionId::Push, host: url.host_text(), port: url.port(), security });
    }

    fn push_connected(&mut self, now: Instant, peer: IpAddress, sink: &mut Sink<'_>) {
        if !matches!(self.push.phase, PushPhase::Connecting { .. }) {
            return;
        }
        let Some(url) = self.push_address() else { return };
        if self.blocked(peer) {
            event(sink, Event::Blocked { connection: ConnectionId::Push, host: url.host_text() });
            self.push_lost(now, Some(PushProblem::Failure(Failure::Blocked)), true, sink);
            return;
        }
        if url.is_secure() {
            self.remember_secure(url.host_text());
        } else {
            event(sink, Event::Unencrypted { connection: ConnectionId::Push, host: url.host_text() });
        }
        let (seconds, _) = self.seconds(now);
        self.push.head.reset();
        self.push.body = None;
        self.push.frames = FrameReader::new();
        self.push.sse.restart();
        // A channel's push connection may be anywhere on its origin, and
        // carries the channel's cookies, whose paths are its folder
        // (REQ-WEB-026).
        let folder = self.channel;
        let mut writer = Writer::new(self.buffers.push_request);
        if self.push.method == PushMethod::WebSocket {
            self.push.key = ws::new_key(&mut self.random);
        }
        let written = (|| -> Result<bool, Overflow> {
            match self.push.method {
                PushMethod::WebSocket => ws::write_handshake_headers(&mut writer, &url, &self.push.key)?,
                PushMethod::ServerSentEvents => {
                    http::write_request_line(&mut writer, Method::Get, url.target())?;
                    http::write_host(&mut writer, &url)?;
                    http::write_header(&mut writer, header::USER_AGENT, crate::consts::USER_AGENT.as_bytes())?;
                    http::write_header(&mut writer, header::ACCEPT, ACCEPT_EVENT_STREAM)?;
                    http::write_header(&mut writer, header::ACCEPT_ENCODING, IDENTITY)?;
                    let last = self.push.sse.last_event_id();
                    if !last.is_empty() {
                        http::write_header(&mut writer, header::LAST_EVENT_ID, last)?;
                    }
                }
            }
            // Cookies go with upgrades and event streams too (REQ-WEB-006).
            let path = folder.as_ref().map_or(url.path(), Url::path);
            let truncated = write_cookie_header(&mut self.jar, &mut writer, &url, path, Site::SameService, seconds, 0)?;
            writer.put(CRLF)?;
            Ok(truncated)
        })();
        if written == Ok(true) {
            event(sink, Event::CookiesTruncated);
        }
        if written.is_err() {
            self.push_lost(now, Some(PushProblem::Failure(Failure::RequestTooLarge)), true, sink);
            return;
        }
        self.push.phase =
            PushPhase::Handshake { deadline: Instant(now.0.saturating_add(self.config.response_timeout_us)) };
        let len = writer.len();
        let bytes = self.buffers.push_request.get(..len).unwrap_or(&[]);
        sink(Output::Send { connection: ConnectionId::Push, bytes });
    }

    /// The push connection failed or closed: wait, then reconnect with a
    /// doubling delay (REQ-WEB-005).
    fn push_lost(&mut self, now: Instant, problem: Option<PushProblem>, close: bool, sink: &mut Sink<'_>) {
        if self.push.url.is_none() || self.push.phase == PushPhase::Idle && problem.is_none() {
            return;
        }
        if close {
            sink(Output::Close { connection: ConnectionId::Push });
        }
        if self.push.healthy {
            self.push.delay_us = PUSH_RETRY_FIRST_US;
        }
        let delay = self.push.delay_us;
        self.push.delay_us = delay.saturating_mul(2).min(PUSH_RETRY_MAX_US);
        self.push.phase = PushPhase::Backoff { until: Instant(now.0.saturating_add(delay)) };
        self.push.healthy = false;
        event(sink, Event::PushLost { retry_in_us: delay, problem });
    }

    fn push_received(&mut self, now: Instant, mut input: &[u8], sink: &mut Sink<'_>) {
        while !input.is_empty() {
            match self.push.phase {
                PushPhase::Handshake { .. } => {
                    let limit = self.config.header_limit.min(self.buffers.push_head.len());
                    let buffer = self.buffers.push_head.get_mut(..limit).unwrap_or(&mut []);
                    match self.push.head.feed(buffer, input) {
                        Ok((used, complete)) => {
                            input = input.get(used..).unwrap_or(&[]);
                            if complete && !self.push_head_complete(now, sink) {
                                return;
                            }
                        }
                        Err(error) => {
                            self.push_lost(now, Some(PushProblem::Failure(Failure::Http(error))), true, sink);
                            return;
                        }
                    }
                }
                PushPhase::Open => {
                    let used = match self.push.method {
                        PushMethod::WebSocket => self.push_frames(now, input, sink),
                        PushMethod::ServerSentEvents => self.push_events(now, input, sink),
                    };
                    match used {
                        Some(used) if used > 0 => input = input.get(used..).unwrap_or(&[]),
                        _ => return,
                    }
                }
                PushPhase::Idle | PushPhase::Backoff { .. } | PushPhase::Connecting { .. } => return,
            }
        }
    }

    fn push_head_complete(&mut self, now: Instant, sink: &mut Sink<'_>) -> bool {
        let Some(url) = self.push_address() else { return false };
        let (seconds, calendar) = self.seconds(now);
        let head_len = self.push.head.len();
        let Self { buffers, jar, .. } = self;
        let head_bytes = buffers.push_head.get(..head_len).unwrap_or(&[]);
        let parsed = http::parse_head(head_bytes, &mut |value| {
            let stored = if cfg!(feature = "cookies") {
                jar.store(value, url.host_text(), url.path(), url.is_secure(), seconds, calendar)
            } else {
                Err(CookieRejected::NotCompiledIn)
            };
            if let Err(rejected) = stored {
                event(sink, Event::CookieRejected(rejected));
            }
        });
        let head = match parsed {
            Ok(head) => head,
            Err(error) => {
                self.push_lost(now, Some(PushProblem::Failure(Failure::Http(error))), true, sink);
                return false;
            }
        };
        let accepted = match self.push.method {
            PushMethod::WebSocket => ws::check_handshake(&head, &self.push.key).map_err(PushProblem::WebSocket),
            PushMethod::ServerSentEvents => {
                if head.status != STATUS_OK {
                    Err(PushProblem::Failure(Failure::Status(head.status)))
                } else if head.media != MediaType::EventStream {
                    Err(PushProblem::Failure(Failure::MediaType))
                } else {
                    // Any framing is accepted for a stream; its total size
                    // is unbounded, each event is limited by the parser.
                    BodyDecoder::new(head.framing, usize::MAX)
                        .map(|decoder| self.push.body = Some(decoder))
                        .map_err(|error| PushProblem::Failure(Failure::Http(error)))
                }
            }
        };
        if let Err(problem) = accepted {
            self.push_lost(now, Some(problem), true, sink);
            return false;
        }
        self.push.phase = PushPhase::Open;
        event(sink, Event::PushConnected(self.push.method));
        if self.push.method == PushMethod::WebSocket {
            self.send_subscribe(sink);
        }
        true
    }

    /// Reads WebSocket frames. Returns the bytes used, or `None` when the
    /// connection ended.
    fn push_frames(&mut self, now: Instant, input: &[u8], sink: &mut Sink<'_>) -> Option<usize> {
        let limit = self.config.push_limit.min(self.buffers.push_message.len());
        let message = self.buffers.push_message.get_mut(..limit).unwrap_or(&mut []);
        let (used, done) = match self.push.frames.feed(message, input) {
            Ok(result) => result,
            Err(error) => {
                let mut frame = Writer::new(self.buffers.push_request);
                if ws::write_close(&mut frame, error.close_code(), &mut self.random).is_ok() {
                    let len = frame.len();
                    sink(Output::Send {
                        connection: ConnectionId::Push,
                        bytes: self.buffers.push_request.get(..len).unwrap_or(&[]),
                    });
                }
                self.push_lost(now, Some(PushProblem::WebSocket(error)), true, sink);
                return None;
            }
        };
        let url = self.push.url;
        let url_text = url.as_ref().map_or("", Url::as_str);
        let secure = url.as_ref().is_some_and(Url::is_secure);
        match done {
            // The push reader is strict, so it never skips a message.
            None | Some(WsMessage::Skipped) => {}
            Some(WsMessage::Text(len)) => {
                self.push.healthy = true;
                let bytes = self.buffers.push_message.get(..len).unwrap_or(&[]);
                sink(Output::PageBytes(PageBytes {
                    source: Source::Push,
                    media: MediaType::Tti,
                    url: url_text,
                    secure,
                    bytes,
                }));
            }
            Some(WsMessage::Binary(len)) => {
                if len % T42_PACKET == 0 {
                    self.push.healthy = true;
                    let bytes = self.buffers.push_message.get(..len).unwrap_or(&[]);
                    sink(Output::PageBytes(PageBytes {
                        source: Source::Push,
                        media: MediaType::T42,
                        url: url_text,
                        secure,
                        bytes,
                    }));
                } else {
                    event(sink, Event::PushLost { retry_in_us: 0, problem: Some(PushProblem::PacketLength) });
                }
            }
            Some(WsMessage::Ping) => {
                let mut payload = [0u8; crate::consts::WS_CONTROL_MAX];
                let len = copy_prefix(&mut payload, self.push.frames.control_payload());
                let mut frame = Writer::new(self.buffers.push_request);
                if ws::write_frame(&mut frame, OPCODE_PONG, payload.get(..len).unwrap_or(&[]), &mut self.random).is_ok()
                {
                    let len = frame.len();
                    sink(Output::Send {
                        connection: ConnectionId::Push,
                        bytes: self.buffers.push_request.get(..len).unwrap_or(&[]),
                    });
                }
            }
            Some(WsMessage::Pong) => self.push.healthy = true,
            Some(WsMessage::Close(code)) => {
                // RFC 6455 section 5.5.1: answer with a close, then end.
                let mut frame = Writer::new(self.buffers.push_request);
                let reply = code.unwrap_or(crate::consts::CLOSE_NORMAL);
                if ws::write_close(&mut frame, reply, &mut self.random).is_ok() {
                    let len = frame.len();
                    sink(Output::Send {
                        connection: ConnectionId::Push,
                        bytes: self.buffers.push_request.get(..len).unwrap_or(&[]),
                    });
                }
                self.push_lost(now, None, true, sink);
                return None;
            }
        }
        Some(used)
    }

    /// Reads the event stream. Returns the bytes used, or `None` when the
    /// connection ended.
    fn push_events(&mut self, now: Instant, input: &[u8], sink: &mut Sink<'_>) -> Option<usize> {
        let body = self.push.body.as_mut()?;
        let step = match body.feed(input) {
            Ok(step) => step,
            Err(error) => {
                self.push_lost(now, Some(PushProblem::Failure(Failure::Http(error))), true, sink);
                return None;
            }
        };
        if step.done {
            self.push_lost(now, None, true, sink);
            return None;
        }
        let mut data = step.data;
        while !data.is_empty() {
            let limit = self.config.push_limit.min(self.buffers.push_message.len());
            let message = self.buffers.push_message.get_mut(..limit).unwrap_or(&mut []);
            let (used, dispatched) = match self.push.sse.feed(message, data) {
                Ok(result) => result,
                Err(error) => {
                    self.push_lost(now, Some(PushProblem::EventStream(error)), true, sink);
                    return None;
                }
            };
            data = data.get(used..).unwrap_or(&[]);
            self.deliver_event(dispatched, sink);
        }
        Some(step.consumed)
    }

    fn deliver_event(&mut self, dispatched: Option<SseEvent>, sink: &mut Sink<'_>) {
        let url = self.push.url;
        let url_text = url.as_ref().map_or("", Url::as_str);
        let secure = url.as_ref().is_some_and(Url::is_secure);
        match dispatched {
            Some(SseEvent::Page(len)) => {
                self.push.healthy = true;
                let bytes = self.buffers.push_message.get(..len).unwrap_or(&[]);
                sink(Output::PageBytes(PageBytes {
                    source: Source::Push,
                    media: MediaType::Tti,
                    url: url_text,
                    secure,
                    bytes,
                }));
            }
            Some(SseEvent::Packets(len)) => {
                let message = self.buffers.push_message.get_mut(..len).unwrap_or(&mut []);
                // Data lines are joined with line feeds; base64 has none.
                let mut kept = 0usize;
                for index in 0..message.len() {
                    let byte = message.get(index).copied().unwrap_or(b'\n');
                    if byte != b'\n'
                        && let Some(slot) = message.get_mut(kept)
                    {
                        *slot = byte;
                        kept = kept.saturating_add(1);
                    }
                }
                match crate::base64::decode_in_place(message, kept) {
                    Ok(decoded) if decoded % T42_PACKET == 0 => {
                        self.push.healthy = true;
                        let bytes = self.buffers.push_message.get(..decoded).unwrap_or(&[]);
                        sink(Output::PageBytes(PageBytes {
                            source: Source::Push,
                            media: MediaType::T42,
                            url: url_text,
                            secure,
                            bytes,
                        }));
                    }
                    Ok(_) => event(sink, Event::PushLost { retry_in_us: 0, problem: Some(PushProblem::PacketLength) }),
                    Err(_) => event(sink, Event::PushLost { retry_in_us: 0, problem: Some(PushProblem::Base64) }),
                }
            }
            Some(SseEvent::Other) | None => {}
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
        if !cfg!(feature = "push") {
            return;
        }
        match self.push.phase {
            PushPhase::Backoff { until } if now >= until => {
                self.push.phase = PushPhase::Idle;
                self.push_connect(now, sink);
            }
            PushPhase::Connecting { deadline } | PushPhase::Handshake { deadline } if now >= deadline => {
                self.push_lost(now, Some(PushProblem::Failure(Failure::Timeout)), true, sink);
            }
            _ => {}
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        let push = match self.push.phase {
            PushPhase::Backoff { until } => Some(until),
            PushPhase::Connecting { deadline } | PushPhase::Handshake { deadline } => Some(deadline),
            PushPhase::Idle | PushPhase::Open => None,
        };
        let queue =
            (self.fetch.exchange.is_none() && self.queue.iter().any(Option::is_some)).then_some(self.next_fetch_at);
        [self.fetch.deadline, push, queue].into_iter().flatten().min()
    }
}
