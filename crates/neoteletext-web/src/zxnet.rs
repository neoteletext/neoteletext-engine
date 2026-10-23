//! The live WebSocket feed served to the zxnet online teletext viewer
//! (REQ-INP-022), as a sans-IO state machine over this crate's WebSocket
//! layer and the port's transport, encryption included.
//!
//! The protocol is undocumented (Appendix D.2). What is implemented here
//! is what REQ-INP-022 records from the viewer's behaviour and from probes
//! of the live servers (research/answers-teletext-sources.md section 2.1):
//!
//! - client messages are text frames of comma-separated values: `service`
//!   and the channel identifier, `ttx,true`, `pagesearch` requests,
//!   `keepalive` every 5 seconds, and `apiver,1` in reply to the server's
//!   `apiver`;
//! - server messages are JSON arrays named by their first element.
//!   `header`, `row` and `bsdp` carry the 40 raw bytes of packet bytes 6 to
//!   45 in base64, and are rebuilt here into complete 42-byte packets with
//!   a Hamming-coded address. Unknown messages and extra fields are
//!   ignored. Everything received is untrusted (REQ-SEC-003).

mod channel;
mod events;
mod feed;
mod message;
mod walk;

pub use channel::{Channel, ChannelError};
pub use events::{HeaderField, LiveEvent, LiveInput, LiveOutput, Malformed};

use neoteletext_core::types::{Instant, PageAddress, SubCode};
use walk::page_index;

use crate::consts::{DEFAULT_HEADER_LIMIT, DEFAULT_PUSH_LIMIT, ZXNET_MAX_PAGES};
use crate::http::HeadCollector;
use crate::port::RandomSource;
use crate::url::IpAddress;
use crate::ws::{FrameReader, HandshakeKey};

/// The longest channel identifier accepted.
const MAX_IDENTIFIER: usize = 512;
/// The longest service name in an identifier.
const MAX_SERVICE: usize = 64;
/// The longest outgoing client message.
const MAX_CLIENT_MESSAGE: usize = MAX_IDENTIFIER + 16;
/// Characters of the header clock (header bytes 38 to 45).
const CLOCK_BYTES: usize = 8;
/// The highest packet number a `row` message may carry.
const MAX_PACKET_NUMBER: u8 = 31;
/// Bytes of the page bitmap: one bit per page of `ZXNET_MAX_PAGES`.
const PAGE_BITMAP_BYTES: usize = ZXNET_MAX_PAGES / 8;
/// Page numbers per magazine.
const PAGES_PER_MAGAZINE: u16 = 256;

/// The feeds known on 1 October 2026 (REQ-INP-022): short name and
/// channel identifier. The identifiers are those the zxnet viewer's
/// settings give; each is base64 of `[service, host, path]`.
pub const KNOWN_CHANNELS: [(&str, &str); 6] = [
    ("teefax", "WyJ0ZWVmYXgiLCJ6eG5ldC5jby51ayIsIi93cy90ZWVmYXgiXQ=="),
    ("nms-ceefax", "WyJubXMtY2VlZmF4IiwiaW50ZXJuYWwubmF0aGFubWVkaWFzZXJ2aWNlcy5jby51ayIsIi93ZWJzb2NrZXRzL2NlZWZheCJd"),
    ("chunkytext", "WyJjaHVua3l0ZXh0IiwienhuZXQuY28udWsiLCIvd3MvY2h1bmt5dGV4dCJd"),
    ("spark", "WyJzcGFyayIsInp4bmV0LmNvLnVrIiwiL3dzL3NwYXJrIl0="),
    ("ch4", "WyJjaGFubmVsNC1jYXB0dXJlIiwienhuZXQuY28udWsiLCIvd3MvY2g0Il0="),
    ("test", "WyJ0ZXN0IiwienhuZXQuY28udWsiLCIvd3MvdGVzdCJd"),
];

/// The feed's buffers, supplied by the caller.
#[derive(Debug)]
pub struct LiveBuffers<'m> {
    /// The handshake response head.
    pub head: &'m mut [u8],
    /// Outgoing handshake and frames.
    pub request: &'m mut [u8],
    /// One server message; its length is the message limit.
    pub message: &'m mut [u8],
}

/// Buffers of the default sizes.
#[derive(Debug)]
pub struct LiveStorage {
    head: [u8; DEFAULT_HEADER_LIMIT],
    request: [u8; 1024],
    message: [u8; DEFAULT_PUSH_LIMIT],
}

impl Default for LiveStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveStorage {
    /// Zeroed buffers. Large: the caller places them in static memory or
    /// on the heap.
    #[must_use]
    // The arrays are the buffers themselves; building them is the point.
    #[allow(clippy::large_stack_arrays)]
    pub const fn new() -> Self {
        Self { head: [0; DEFAULT_HEADER_LIMIT], request: [0; 1024], message: [0; DEFAULT_PUSH_LIMIT] }
    }

    /// The buffers, borrowed for a feed.
    pub fn buffers(&mut self) -> LiveBuffers<'_> {
        LiveBuffers { head: &mut self.head, request: &mut self.request, message: &mut self.message }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Stopped,
    Waiting { until: Instant },
    Connecting { deadline: Instant },
    Handshake { deadline: Instant },
    Open,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Walk {
    current: Option<PageAddress>,
    started: Instant,
    header_seen: bool,
    rows: u16,
    last_row: Option<Instant>,
}

/// The output sink.
pub type LiveSink<'s> = dyn FnMut(LiveOutput<'_>) + 's;

fn event(sink: &mut LiveSink<'_>, event: LiveEvent<'_>) {
    sink(LiveOutput::Event(event));
}

/// The zxnet live feed reader.
#[derive(Debug)]
pub struct ZxnetFeed<'m, R: RandomSource> {
    channel: Channel,
    buffers: LiveBuffers<'m>,
    random: R,
    allowed_local: Option<IpAddress>,
    /// The `User-Agent` of the upgrade (REQ-WEB-031).
    user_agent: crate::user_agent::UserAgentString,
    /// The transport goes through a proxy (REQ-WEB-032).
    proxied: bool,
    phase: Phase,
    key: HandshakeKey,
    head: HeadCollector,
    frames: FrameReader,
    attempts: u8,
    last_activity: Instant,
    next_keepalive: Instant,
    display: Option<(PageAddress, Option<SubCode>)>,
    pages: [u8; PAGE_BITMAP_BYTES],
    walk: Option<Walk>,
}

impl<'m, R: RandomSource> ZxnetFeed<'m, R> {
    /// A feed for `channel`, not yet connected.
    pub fn new(channel: Channel, buffers: LiveBuffers<'m>, random: R) -> Self {
        Self {
            channel,
            buffers,
            random,
            allowed_local: None,
            user_agent: crate::user_agent::UserAgentString::default(),
            proxied: false,
            phase: Phase::Stopped,
            key: [0; crate::consts::WEBSOCKET_KEY_TEXT],
            head: HeadCollector::default(),
            frames: FrameReader::skipping_oversized(),
            attempts: 0,
            last_activity: Instant(0),
            next_keepalive: Instant(0),
            display: None,
            pages: [0; PAGE_BITMAP_BYTES],
            walk: None,
        }
    }

    /// Allows one local-network address explicitly (REQ-SEC-006), for a
    /// test server.
    pub fn allow_local(&mut self, address: IpAddress) {
        self.allowed_local = Some(address);
    }

    /// The transport goes through a proxy (REQ-WEB-032): the peer it
    /// reports is the proxy's, and a local name is refused before the proxy
    /// is asked.
    pub fn through_proxy(&mut self) {
        self.proxied = true;
    }

    /// The program the feed names itself as in its upgrade's `User-Agent`
    /// (REQ-WEB-031); without one, the engine.
    pub fn name_product(&mut self, product: Option<&crate::user_agent::Product>) {
        self.user_agent = crate::user_agent::UserAgentString::of(product);
    }

    /// The channel.
    #[must_use]
    pub const fn channel(&self) -> &Channel {
        &self.channel
    }

    /// True when the service listed `page` in a `pageExists` message.
    #[must_use]
    pub fn page_exists(&self, page: PageAddress) -> bool {
        let index = page_index(page);
        self.pages.get(index / 8).is_some_and(|byte| byte & (1 << (index % 8)) != 0)
    }
}
