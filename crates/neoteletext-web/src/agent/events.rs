//! The agent's vocabulary: what the application gives it ([`Input`]),
//! what it answers with ([`Output`]), and the events and failures it
//! reports.

use neoteletext_core::types::{Instant, PageAddress, SubCode};

use crate::cookie::CookieRejected;
use crate::descriptor::{DescriptorError, PushMethod};
use crate::http::{HttpError, MediaType, Method};
use crate::port::{ConnectionId, NetInput, NetOutput, Security};
use crate::records::index::IndexRecord;
use crate::records::links::LinksRecord;
use crate::records::subpages::SubPagesRecord;
use crate::sse::SseError;
use crate::url::UrlError;
use crate::ws::WsError;
use neoteletext_formats::log::LogLine;

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
    /// On a channel, a sub-page of a page whose catalogue last answered
    /// ([`Input::FetchSubPages`]) is fetched from its own file,
    /// `NNN-SSSS.t42`, and nothing else; any other request asks for the
    /// page's file, `NNN.tti` or `NNN.t42` (REQ-WEB-019), and so does
    /// sub-code 3F7F, "any sub-page".
    FetchPage {
        /// The page.
        page: PageAddress,
        /// The sub-page, if given.
        subcode: Option<SubCode>,
    },
    /// Fetch the catalogue of a page of the open channel, `NNN.sub`
    /// (REQ-WEB-034), conditionally when its validators are held,
    /// answered by [`Event::SubPages`], [`Event::SubPagesCurrent`],
    /// [`Event::SubPagesMissing`] or [`Event::SubPagesFailed`].
    FetchSubPages {
        /// The page.
        page: PageAddress,
    },
    /// Change the push subscription.
    Subscribe(Subscription<'a>),
    /// Erase every cookie (REQ-SEC-013).
    WipeCookies,
    /// Calendar time now, in seconds since 1970, so cookie `Expires` dates
    /// can be compared and cookies saved.
    WallClock(u64),
    /// One message of the input profile (area INT, decision E.90), sent as
    /// a binary WebSocket message on the push connection: only while it is
    /// open and the channel's descriptor declares `push.input`, and only up
    /// to [`crate::consts::INPUT_MESSAGE_MAX`] bytes. Anything else is
    /// dropped, with [`Event::InputDropped`]: what it sends never changes
    /// what is fetched.
    Send(InputMessage<'a>),
    /// Measure the push connection's round trip, by a WebSocket ping at
    /// most once in [`crate::consts::ROUND_TRIP_EVERY_US`], answered by
    /// [`Event::RoundTrip`] (REQ-RCH-016): on while keys go to the channel,
    /// off otherwise.
    MeasureRoundTrip(bool),
}

/// One message of the input profile for the push WebSocket
/// ([`Input::Send`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InputMessage<'a> {
    /// The encoded message: one event, or a run of text.
    pub bytes: &'a [u8],
    /// The contact whose move it is, alone (0 the mouse's, 1 to 9 TOUCH's),
    /// below [`crate::consts::PUSH_MOVE_SLOTS`]; `None` for any other
    /// message. A move is written only while the push socket's unsent
    /// bytes are under [`crate::consts::PUSH_UNSENT_BOUND`], and otherwise
    /// held, the contact's latest replacing it, until they fall under it
    /// or any other message is sent, which writes the held moves first
    /// (REQ-RCH-035).
    pub move_of: Option<u8>,
    /// More messages, waiting at the same moment, are given at once after
    /// this one: they are written together, as one send, when the last
    /// comes (REQ-RCH-010). Nothing is ever held back for one that does
    /// not come: whatever input is handled next writes what waits.
    pub more: bool,
}

impl<'a> InputMessage<'a> {
    /// A message alone, written at once: not a move, nothing more waiting.
    #[must_use]
    pub const fn alone(bytes: &'a [u8]) -> Self {
        Self { bytes, move_of: None, more: false }
    }
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
    /// The address they came from, after redirects, and so their origin.
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
    /// [`Config::redirects_within_channel`](super::Config::redirects_within_channel).
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
    /// A request on the fetch connection was answered (REQ-WEB-033): for a
    /// developer's view of the channel's network activity, each hop of a
    /// redirect its own.
    Exchanged {
        /// The request's method.
        method: Method,
        /// The address asked for.
        url: &'a str,
        /// The response's status code.
        status: u16,
        /// The body's bytes.
        bytes: usize,
        /// Microseconds from the request to the end of its response.
        elapsed_us: u64,
    },
    /// An [`Input::Send`] was not sent: no push WebSocket is open, the
    /// channel does not take input, or the message is too long.
    InputDropped,
    /// The push WebSocket's round trip, from a ping to its pong
    /// (REQ-RCH-016), while [`Input::MeasureRoundTrip`] is on.
    RoundTrip {
        /// Microseconds.
        microseconds: u64,
    },
    /// The descriptor names a push address, but this build has no push
    /// (the `push` feature, decision E.61): pages are fetched on request
    /// only.
    PushNotCompiledIn(PushMethod),
    /// Packets the push connection carried that were invalid
    /// (neoteletext-common's REQ-1462): a packet whose address does not
    /// decode or whose kind the stream may not carry
    /// ([`neoteletext_core::packet::is_valid_stream_packet`]), a binary
    /// message that is not whole packets (counted as its length divided by
    /// 42, rounded up), a text message (counted as one), or a channel
    /// record that breaks its layout (each of its packets). Each was
    /// discarded with nothing of it applied, and the connection kept.
    /// Given each time the count rises: the port reports it at most once
    /// in [`crate::consts::INVALID_PACKET_REPORT_SECONDS`].
    InvalidPackets {
        /// The count since the push connection opened.
        total: u64,
    },
    /// A log line for the channel's author from a LOG record on the push
    /// connection (`INPUT.md` section 7.6, REQ-INP-026): the same line a
    /// TTI `LG` line gives as `Item::Log`, shown only in a developer's
    /// view. At most `MAX_LOG_LINES_PER_PAGE` for each page frame, and as
    /// many before the first.
    Log {
        /// The level and the checked text.
        line: LogLine<'a>,
        /// The page it concerns, if any.
        page: Option<PageAddress>,
    },
    /// A part of a live channel's index (`INPUT.md` section 7.5): its
    /// pages are shown at once, added or replacing what was there;
    /// [`IndexRecord::entries`] gives them as `contents.txt`'s entries
    /// and [`IndexRecord::pages`] with their sub-page counts. Nothing is
    /// removed until [`Event::IndexComplete`].
    Index(IndexRecord<'a>),
    /// Every part of an index sending has arrived: the pages and
    /// magazines it listed are the whole index, and every other is
    /// removed (`INPUT.md` section 7.5).
    IndexComplete {
        /// The sending's generation.
        generation: u16,
    },
    /// A sub-page's TTI Link Extension links from a LINKS record on the
    /// push connection (`INPUT.md` section 7.8, REQ-1494), given as a TTI
    /// file's `LN`, `LK` and `LT` lines give them
    /// ([`LinksRecord::key_links`], [`LinksRecord::number_links`],
    /// [`LinksRecord::text_links`]), for the same store. Part 0 replaces
    /// the links held for that page and sub-code; later parts add to
    /// them. A sub-page sent again without one has no such links.
    Links(LinksRecord<'a>),
    /// A part of a page's list of sub-pages from a SUBPAGES record on the
    /// push connection (`INPUT.md` section 7.9, REQ-1503), for the
    /// sub-page list, its count, the view's place and stepping; never the
    /// sub-pages themselves. A record of another `version` of the page
    /// begins a new list; a list is whole when every part has arrived,
    /// and a partial one removes nothing.
    SubPageList(SubPagesRecord<'a>),
    /// A live channel (its `channel.json` names a push connection,
    /// REQ-WEB-018) is opening its connection: it asks for no bundle, page
    /// file or `contents.txt`, and is connecting, never a page not found,
    /// until [`Event::ChannelLive`].
    ChannelConnecting,
    /// A live channel's first page arrived on its connection.
    ChannelLive,
    /// The push connection was lost; it is retried after this delay.
    PushLost {
        /// Microseconds until the next attempt.
        retry_in_us: u64,
        /// What went wrong, if known.
        problem: Option<PushProblem>,
    },
    /// A link named a service the descriptor does not list: the link is
    /// null (Appendix E.2 rule 2).
    LinkUnknown,
    /// A request was redirected out of the channel and, under
    /// [`Config::redirects_within_channel`](super::Config::redirects_within_channel), not followed. The viewer
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
    /// A page's catalogue of sub-pages, `NNN.sub`, unchecked: the caller
    /// reads it with [`crate::subpages::read`]. From now on the page's
    /// sub-pages are fetched from their own files (REQ-WEB-019).
    SubPages {
        /// The page.
        page: PageAddress,
        /// The catalogue.
        bytes: &'a [u8],
    },
    /// A page's catalogue is unchanged (304), or still fresh by its cache
    /// lifetime: the copy the caller holds is current.
    SubPagesCurrent {
        /// The page.
        page: PageAddress,
    },
    /// The page has no catalogue (404 or 410): its sub-pages, if any, are
    /// in its page file, which is where they are fetched from.
    SubPagesMissing {
        /// The page.
        page: PageAddress,
    },
    /// A page's catalogue could not be fetched or used.
    SubPagesFailed {
        /// The page.
        page: PageAddress,
        /// Why.
        failure: Failure,
    },
    /// A `Set-Cookie` header was ignored.
    CookieRejected(CookieRejected),
    /// Not every matching cookie fitted in a request.
    CookiesTruncated,
    /// A device-bound session changed (REQ-WEB-017, feature `dbsc`).
    BoundSession {
        /// The session's identifier, empty when a registration failed
        /// before the server named one.
        session: &'a str,
        /// What happened.
        change: BoundChange,
    },
}

/// What happened to a device-bound session (REQ-WEB-017).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoundChange {
    /// The server registered it: its cookies are kept bound to a key on
    /// this device.
    Registered,
    /// Its cookies were refreshed with a proof of the key.
    Refreshed,
    /// It ended: the server said so, refused it, it lapsed, or its site's
    /// data was cleared. Its key is deleted.
    Ended,
    /// A registration the server asked for was not made, or not accepted.
    RegistrationFailed,
    /// A refresh did not bring new cookies; the request it held went on.
    RefreshFailed,
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
