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

use neoteletext_core::consts::{CONTROL_MASK, DATA_BYTES, DATA_START, PACKET_HEADER, PACKET_SERVICE, SERVICE_MAGAZINE};
use neoteletext_core::packet::{self, Decoded, Packet};
use neoteletext_core::types::{Instant, Magazine, PageAddress, PageNumber, SubCode};

use crate::base64;
use crate::bytes::{TextBuf, Writer};
use crate::consts::{
    DEFAULT_CONNECT_TIMEOUT_US, DEFAULT_HEADER_LIMIT, DEFAULT_PUSH_LIMIT, DEFAULT_RESPONSE_TIMEOUT_US, OPCODE_PONG,
    OPCODE_TEXT, ZXNET_API_VERSION, ZXNET_DISPLAY_SLOT, ZXNET_INACTIVITY_US, ZXNET_KEEPALIVE_US, ZXNET_MAX_ATTEMPTS,
    ZXNET_MAX_PAGES, ZXNET_RECONNECT_US, ZXNET_WALK_SETTLE_US, ZXNET_WALK_TIMEOUT_US,
};
use crate::http::{self, HeadCollector};
use crate::json::{JsonReader, Token, integer};
use crate::port::{ConnectionId, NetInput, NetOutput, RandomSource, Security};
use crate::url::{IpAddress, Url};
use crate::ws::{self, FrameReader, HandshakeKey, WsMessage};

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

/// Why a channel could not be used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelError {
    /// Not base64.
    Base64,
    /// Not a JSON array of three non-empty strings.
    Json,
    /// The host and path do not form a valid `wss` address.
    Address,
    /// Too long to keep.
    TooLong,
    /// No feed has this short name.
    UnknownName,
}

/// A feed: its identifier, service name and address.
#[derive(Clone, Copy, Debug)]
pub struct Channel {
    identifier: TextBuf<MAX_IDENTIFIER>,
    service: TextBuf<MAX_SERVICE>,
    address: Url,
}

impl Channel {
    /// Decodes a channel identifier: base64 of a JSON array of the service
    /// name, the host and the path.
    pub fn from_identifier(identifier: &str) -> Result<Self, ChannelError> {
        let mut decoded = [0u8; MAX_IDENTIFIER];
        let len = base64::decode(identifier.as_bytes(), &mut decoded).map_err(|_| ChannelError::Base64)?;
        let json = decoded.get(..len).unwrap_or(&[]);
        let mut reader = JsonReader::new(json).map_err(|_| ChannelError::Json)?;
        if reader.next_token() != Ok(Some(Token::BeginArray)) {
            return Err(ChannelError::Json);
        }
        let mut parts: [TextBuf<MAX_IDENTIFIER>; 3] = [TextBuf::new(); 3];
        for part in &mut parts {
            let Ok(Some(Token::Str(value))) = reader.next_token() else { return Err(ChannelError::Json) };
            let mut buffer = [0u8; MAX_IDENTIFIER];
            let len = value.decode_into(&mut buffer).map_err(|_| ChannelError::TooLong)?;
            let text = buffer.get(..len).unwrap_or(&[]);
            if text.is_empty() || text.iter().any(|&b| !(0x21..=0x7E).contains(&b) || b == b',') {
                return Err(ChannelError::Json);
            }
            *part = TextBuf::from_bytes(text).ok_or(ChannelError::TooLong)?;
        }
        if reader.next_token() != Ok(Some(Token::EndArray)) || reader.next_token() != Ok(None) {
            return Err(ChannelError::Json);
        }
        let [service, host, path] = parts;
        if host.as_bytes().iter().any(|&b| b == b'/' || b == b'?') || path.as_bytes().first() != Some(&b'/') {
            return Err(ChannelError::Address);
        }
        let mut text = [0u8; crate::consts::MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut text);
        let built =
            writer.put(b"wss://").and_then(|()| writer.put(host.as_bytes())).and_then(|()| writer.put(path.as_bytes()));
        built.map_err(|_| ChannelError::TooLong)?;
        let address = core::str::from_utf8(writer.written()).map_err(|_| ChannelError::Address)?;
        let address = Url::parse(address).map_err(|_| ChannelError::Address)?;
        Ok(Self {
            identifier: TextBuf::from_bytes(identifier.as_bytes()).ok_or(ChannelError::TooLong)?,
            service: TextBuf::from_bytes(service.as_bytes()).ok_or(ChannelError::TooLong)?,
            address,
        })
    }

    /// A known feed by its short name (REQ-INP-022's table). The short
    /// name is only a label: the `ch4` feed's service name is
    /// `channel4-capture`.
    pub fn from_short_name(name: &str) -> Result<Self, ChannelError> {
        let (_, identifier) =
            KNOWN_CHANNELS.iter().find(|(short, _)| *short == name).ok_or(ChannelError::UnknownName)?;
        Self::from_identifier(identifier)
    }

    /// A feed given by its `wss` address and service name; the identifier
    /// is built from them.
    pub fn from_address(address: &str, service: &str) -> Result<Self, ChannelError> {
        let url = Url::parse(address).map_err(|_| ChannelError::Address)?;
        if url.scheme() != crate::url::Scheme::Wss || url.query().is_some() {
            return Err(ChannelError::Address);
        }
        let printable = |text: &str| {
            !text.is_empty() && text.bytes().all(|b| (0x21..=0x7E).contains(&b) && !matches!(b, b'"' | b'\\' | b','))
        };
        if !printable(service) {
            return Err(ChannelError::Json);
        }
        let mut json = [0u8; MAX_IDENTIFIER];
        let mut writer = Writer::new(&mut json);
        let built = (|| {
            writer.put(b"[\"")?;
            writer.put(service.as_bytes())?;
            writer.put(b"\",\"")?;
            writer.put(url.authority())?;
            writer.put(b"\",\"")?;
            writer.put(url.path().as_bytes())?;
            writer.put(b"\"]")
        })();
        built.map_err(|_| ChannelError::TooLong)?;
        let mut identifier = [0u8; MAX_IDENTIFIER];
        let len = base64::encode(writer.written(), &mut identifier).map_err(|_| ChannelError::TooLong)?;
        Self::from_identifier(
            core::str::from_utf8(identifier.get(..len).unwrap_or(&[])).map_err(|_| ChannelError::Base64)?,
        )
    }

    /// The identifier, sent to the server as it is.
    #[must_use]
    pub fn identifier(&self) -> &str {
        self.identifier.as_str()
    }

    /// The service name: the identifier's first string.
    #[must_use]
    pub fn service(&self) -> &str {
        self.service.as_str()
    }

    /// The feed's address.
    #[must_use]
    pub const fn address(&self) -> &Url {
        &self.address
    }
}

/// The header fields cross-checked against the decoder (REQ-INP-022).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeaderField {
    /// The page number.
    Page,
    /// The sub-code.
    SubCode,
    /// The control bits C4 to C14.
    Control,
}

/// Why a server message was ignored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Malformed {
    /// Not a JSON array starting with a name.
    Json,
    /// A known message with fields of the wrong type or range.
    Fields,
    /// Packet data that was not 40 bytes of base64.
    PacketData,
    /// A binary frame, which the feed never sends.
    Binary,
    /// A message longer than the message buffer, skipped unread.
    TooLong,
}

/// An event from the live feed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiveEvent<'a> {
    /// The WebSocket is open and the client messages were sent.
    Connected,
    /// The server's protocol version.
    ApiVersion(i64),
    /// The server serves a different service: reconnection stops.
    WrongName,
    /// The connection was lost; another attempt follows after the delay.
    Reconnecting {
        /// Failed attempts so far.
        attempt: u8,
        /// Microseconds until the attempt.
        in_us: u64,
    },
    /// Five attempts failed: the feed stopped.
    GaveUp,
    /// The peer address is in a blocked range (REQ-SEC-006): the feed
    /// stopped.
    Blocked,
    /// A header's fields as the server decoded them disagree with the
    /// packet decoder.
    HeaderMismatch {
        /// Which field.
        field: HeaderField,
        /// The server's value.
        server: u16,
        /// The decoder's value, `None` when it was uncorrectable.
        decoded: Option<u16>,
    },
    /// A page the service has (`pageExists`).
    PageExists(PageAddress),
    /// The service's initial page and status message (`initialpage`).
    InitialPage {
        /// The page.
        page: PageAddress,
        /// Its sub-code.
        subcode: SubCode,
        /// The status message bytes.
        status: &'a [u8],
    },
    /// The header clock characters, raw with parity.
    Clock(&'a [u8]),
    /// The page walk moved to this page.
    WalkPage(PageAddress),
    /// The page walk finished with a page.
    WalkPageDone {
        /// The page.
        page: PageAddress,
        /// Row packets received for it.
        rows: u16,
        /// It was left because the timeout passed.
        timed_out: bool,
    },
    /// The walk reached the last known page.
    WalkComplete,
    /// A server message was ignored.
    Ignored(Malformed),
}

/// An input to the feed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiveInput<'a> {
    /// Time has passed.
    Tick,
    /// Something happened on the connection.
    Net(NetInput<'a>),
    /// Connect.
    Start,
    /// Disconnect and stay disconnected.
    Stop,
    /// Show a page: request it on slot 0.
    Request {
        /// The page.
        page: PageAddress,
        /// The sub-page; `None` for any (3F7F).
        subcode: Option<SubCode>,
    },
    /// Start or stop walking every page the service lists.
    Walk(bool),
}

/// An output of the feed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiveOutput<'a> {
    /// Open the connection.
    Connect {
        /// The host.
        host: &'a str,
        /// The port.
        port: u16,
    },
    /// Send bytes.
    Send(&'a [u8]),
    /// Close the connection.
    Close,
    /// The next time a [`LiveInput::Tick`] is needed.
    Timer(Instant),
    /// A rebuilt 42-byte packet for acquisition.
    Packet(Packet),
    /// An event.
    Event(LiveEvent<'a>),
}

impl<'a> LiveOutput<'a> {
    /// The output as a network action for [`crate::port::Pump`], if it is
    /// one.
    #[must_use]
    pub const fn net(&self) -> Option<NetOutput<'a>> {
        match *self {
            Self::Connect { host, port } => {
                Some(NetOutput::Connect { connection: ConnectionId::Live, host, port, security: Security::Tls })
            }
            Self::Send(bytes) => Some(NetOutput::Send(ConnectionId::Live, bytes)),
            Self::Close => Some(NetOutput::Close(ConnectionId::Live)),
            _ => None,
        }
    }
}

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

/// A magazine as the feed writes it: 1 to 8, or 0 for 8.
fn magazine(value: i64) -> Option<Magazine> {
    match value {
        0 => Some(Magazine::from_bits(0)),
        _ => Magazine::new(u8::try_from(value).ok()?),
    }
}

fn page_index(page: PageAddress) -> usize {
    page.magazine.index().saturating_mul(usize::from(PAGES_PER_MAGAZINE)).saturating_add(usize::from(page.page.0))
}

/// Pages walk in the order 100 to 8FF: magazine 8 (index 0) last.
fn walk_order(page: PageAddress) -> usize {
    let magazine = usize::from(page.magazine.number().saturating_sub(1));
    magazine.saturating_mul(usize::from(PAGES_PER_MAGAZINE)).saturating_add(usize::from(page.page.0))
}

fn page_from_walk_order(order: usize) -> Option<PageAddress> {
    let per_magazine = usize::from(PAGES_PER_MAGAZINE);
    let magazine = u8::try_from(order.checked_div(per_magazine)?).ok()?.checked_add(1)?;
    let page = u8::try_from(order.checked_rem(per_magazine)?).ok()?;
    Some(PageAddress { magazine: Magazine::new(magazine)?, page: PageNumber(page) })
}

/// Reads the next token as an integer.
fn next_integer(reader: &mut JsonReader<'_>) -> Option<i64> {
    match reader.next_token().ok()? {
        Some(Token::Number(number)) => integer(number),
        _ => None,
    }
}

/// Decodes the next token as base64 packet data of exactly 40 bytes.
fn next_packet_data(reader: &mut JsonReader<'_>) -> Result<[u8; DATA_BYTES], Malformed> {
    let Ok(Some(Token::Str(value))) = reader.next_token() else { return Err(Malformed::Fields) };
    let text = value.plain().ok_or(Malformed::PacketData)?;
    let mut data = [0u8; DATA_BYTES + 2];
    match base64::decode(text, &mut data) {
        Ok(DATA_BYTES) => {
            let mut exact = [0u8; DATA_BYTES];
            exact.copy_from_slice(data.get(..DATA_BYTES).unwrap_or(&[0; DATA_BYTES]));
            Ok(exact)
        }
        _ => Err(Malformed::PacketData),
    }
}

/// A server message, parsed and checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Message {
    ApiVersion(i64),
    WrongName,
    Header { slot: u8, data: [u8; DATA_BYTES], control: u16, magazine: Magazine, subcode: u16, page: u8 },
    Row { slot: u8, magazine: Magazine, number: u8, data: [u8; DATA_BYTES] },
    Bsdp([u8; DATA_BYTES]),
    Clock([u8; CLOCK_BYTES], usize),
    PageExists(PageAddress),
    InitialPage { page: PageAddress, subcode: SubCode, status: [u8; STATUS_BYTES], len: usize },
}

/// Room for the decoded `initialpage` status message.
const STATUS_BYTES: usize = neoteletext_core::consts::STATUS_MESSAGE_BYTES + 2;

fn next_u8(reader: &mut JsonReader<'_>) -> Result<u8, Malformed> {
    next_integer(reader).and_then(|v| u8::try_from(v).ok()).ok_or(Malformed::Fields)
}

fn next_u16(reader: &mut JsonReader<'_>) -> Result<u16, Malformed> {
    next_integer(reader).and_then(|v| u16::try_from(v).ok()).ok_or(Malformed::Fields)
}

fn next_magazine(reader: &mut JsonReader<'_>) -> Result<Magazine, Malformed> {
    next_integer(reader).and_then(magazine).ok_or(Malformed::Fields)
}

/// Decodes the next token as base64 into `out`, returning the length.
fn next_base64(reader: &mut JsonReader<'_>, out: &mut [u8]) -> Result<usize, Malformed> {
    let Ok(Some(Token::Str(value))) = reader.next_token() else { return Err(Malformed::Fields) };
    let text = value.plain().ok_or(Malformed::PacketData)?;
    base64::decode(text, out).map_err(|_| Malformed::PacketData)
}

/// Parses one server message. `Ok(None)` for messages that are ignored:
/// `channelSettings`, `secondTick` and unknown names. Fields after the
/// known ones are ignored (REQ-INP-022).
fn parse_message(text: &[u8]) -> Result<Option<Message>, Malformed> {
    let mut reader = JsonReader::new(text).map_err(|_| Malformed::Json)?;
    if reader.next_token() != Ok(Some(Token::BeginArray)) {
        return Err(Malformed::Json);
    }
    let Ok(Some(Token::Str(name))) = reader.next_token() else { return Err(Malformed::Json) };
    // No message name has escapes; one that does is unknown.
    let Some(name) = name.plain() else { return Ok(None) };
    let message = match name {
        b"apiver" => Message::ApiVersion(next_integer(&mut reader).ok_or(Malformed::Fields)?),
        b"wrongname" => Message::WrongName,
        b"header" => {
            let slot = next_u8(&mut reader)?;
            let data = next_packet_data(&mut reader)?;
            let Ok(Some(Token::Bool(_))) = reader.next_token() else { return Err(Malformed::Fields) };
            let control = next_u16(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let subcode = next_u16(&mut reader)?;
            let page = next_u8(&mut reader)?;
            Message::Header { slot, data, control, magazine, subcode, page }
        }
        b"row" => {
            let slot = next_u8(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let number = next_u8(&mut reader)?;
            if !(1..=MAX_PACKET_NUMBER).contains(&number) {
                return Err(Malformed::Fields);
            }
            Message::Row { slot, magazine, number, data: next_packet_data(&mut reader)? }
        }
        b"bsdp" => Message::Bsdp(next_packet_data(&mut reader)?),
        b"clock" => {
            next_u8(&mut reader)?;
            let mut decoded = [0u8; CLOCK_BYTES + 2];
            let len = next_base64(&mut reader, &mut decoded)?;
            let mut clock = [0u8; CLOCK_BYTES];
            let len = len.min(CLOCK_BYTES);
            clock.get_mut(..len).unwrap_or(&mut []).copy_from_slice(decoded.get(..len).unwrap_or(&[]));
            Message::Clock(clock, len)
        }
        b"pageExists" => {
            next_u8(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let page = next_u8(&mut reader)?;
            Message::PageExists(PageAddress { magazine, page: PageNumber(page) })
        }
        b"initialpage" => {
            next_u8(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let page = next_u8(&mut reader)?;
            let subcode = SubCode::new(next_u16(&mut reader)?);
            let mut status = [0u8; STATUS_BYTES];
            let len = next_base64(&mut reader, &mut status)?;
            Message::InitialPage { page: PageAddress { magazine, page: PageNumber(page) }, subcode, status, len }
        }
        _ => return Ok(None),
    };
    Ok(Some(message))
}

/// Compares the header fields the server decoded with the packet decoder's
/// reading of the same bytes, reporting each disagreement (REQ-INP-022).
fn cross_check(packet: &Packet, control: u16, subcode: u16, page: u8, sink: &mut LiveSink<'_>) {
    let Ok(Decoded::Header(header)) = packet::decode(packet) else { return };
    let checks = [
        (HeaderField::Page, u16::from(page), header.page.map(|p| u16::from(p.0))),
        (HeaderField::SubCode, SubCode::new(subcode).value(), header.subcode.map(SubCode::value)),
        (HeaderField::Control, control & CONTROL_MASK, header.control.map(|c| c.0 & CONTROL_MASK)),
    ];
    for (field, server, decoded) in checks {
        if decoded != Some(server) {
            event(sink, LiveEvent::HeaderMismatch { field, server, decoded });
        }
    }
}

/// Builds a complete packet: the Hamming-coded address, then the 40 bytes.
fn rebuild(magazine: Magazine, number: u8, data: &[u8; DATA_BYTES]) -> Packet {
    let mut packet = packet::encode::address(magazine, number);
    if let Some(target) = packet.bytes.get_mut(DATA_START..) {
        target.copy_from_slice(data);
    }
    packet
}

impl<'m, R: RandomSource> ZxnetFeed<'m, R> {
    /// A feed for `channel`, not yet connected.
    pub fn new(channel: Channel, buffers: LiveBuffers<'m>, random: R) -> Self {
        Self {
            channel,
            buffers,
            random,
            allowed_local: None,
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

    /// Handles one input. Outputs go to `sink` before this returns.
    pub fn handle(&mut self, now: Instant, input: LiveInput<'_>, sink: &mut LiveSink<'_>) {
        match input {
            LiveInput::Tick => self.tick(now, sink),
            LiveInput::Start => {
                if matches!(self.phase, Phase::Stopped | Phase::Waiting { .. }) {
                    self.attempts = 0;
                    self.connect(now, sink);
                }
            }
            LiveInput::Stop => {
                if !matches!(self.phase, Phase::Stopped | Phase::Waiting { .. }) {
                    sink(LiveOutput::Close);
                }
                self.phase = Phase::Stopped;
            }
            LiveInput::Request { page, subcode } => {
                self.display = Some((page, subcode));
                self.walk = None;
                if self.phase == Phase::Open {
                    self.page_search(page, subcode, sink);
                }
            }
            LiveInput::Walk(on) => {
                self.walk =
                    on.then_some(Walk { current: None, started: now, header_seen: false, rows: 0, last_row: None });
                if on {
                    self.walk_next(now, None, sink);
                }
            }
            LiveInput::Net(net) => match net {
                NetInput::Connected(ConnectionId::Live, peer) => self.connected(now, peer, sink),
                NetInput::ConnectFailed(ConnectionId::Live) | NetInput::Closed(ConnectionId::Live) => {
                    if !matches!(self.phase, Phase::Stopped | Phase::Waiting { .. }) {
                        self.dropped(now, false, sink);
                    }
                }
                NetInput::Received(ConnectionId::Live, bytes) => self.received(now, bytes, sink),
                _ => {}
            },
        }
        if let Some(at) = self.next_deadline() {
            sink(LiveOutput::Timer(at));
        }
    }

    fn connect(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        if let Some(ip) = self.channel.address.ip()
            && ip.is_blocked()
            && self.allowed_local != Some(ip)
        {
            self.phase = Phase::Stopped;
            event(sink, LiveEvent::Blocked);
            return;
        }
        self.phase = Phase::Connecting { deadline: Instant(now.0.saturating_add(DEFAULT_CONNECT_TIMEOUT_US)) };
        let address = self.channel.address;
        sink(LiveOutput::Connect { host: self.channel.address.host_text(), port: address.port() });
    }

    fn connected(&mut self, now: Instant, peer: IpAddress, sink: &mut LiveSink<'_>) {
        if !matches!(self.phase, Phase::Connecting { .. }) {
            return;
        }
        if peer.is_blocked() && self.allowed_local != Some(peer) {
            sink(LiveOutput::Close);
            self.phase = Phase::Stopped;
            event(sink, LiveEvent::Blocked);
            return;
        }
        self.key = ws::new_key(&mut self.random);
        self.head.reset();
        // The server sends messages the feed does not use, such as
        // "channelSettings" with the channel's clock face at about 88 KB;
        // they are skipped, not taken as a broken connection.
        self.frames = FrameReader::skipping_oversized();
        let mut writer = Writer::new(self.buffers.request);
        let written = ws::write_handshake_headers(&mut writer, &self.channel.address, &self.key)
            .and_then(|()| writer.put(crate::consts::CRLF));
        if written.is_err() {
            self.dropped(now, true, sink);
            return;
        }
        self.phase = Phase::Handshake { deadline: Instant(now.0.saturating_add(DEFAULT_RESPONSE_TIMEOUT_US)) };
        let len = writer.len();
        sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
    }

    /// The connection dropped or failed: wait 10 seconds and try again, at
    /// most five times (REQ-INP-022).
    fn dropped(&mut self, now: Instant, close: bool, sink: &mut LiveSink<'_>) {
        if close {
            sink(LiveOutput::Close);
        }
        self.attempts = self.attempts.saturating_add(1);
        if self.attempts >= ZXNET_MAX_ATTEMPTS {
            self.phase = Phase::Stopped;
            event(sink, LiveEvent::GaveUp);
            return;
        }
        self.phase = Phase::Waiting { until: Instant(now.0.saturating_add(ZXNET_RECONNECT_US)) };
        event(sink, LiveEvent::Reconnecting { attempt: self.attempts, in_us: ZXNET_RECONNECT_US });
    }

    /// Sends one client message as a text frame.
    fn send_text(&mut self, message: &[u8], sink: &mut LiveSink<'_>) {
        let mut writer = Writer::new(self.buffers.request);
        if ws::write_frame(&mut writer, OPCODE_TEXT, message, &mut self.random).is_ok() {
            let len = writer.len();
            sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
        }
    }

    /// `pagesearch` on slot 0: slot, magazine, page number, sub-code (16255
    /// for any) and three flags, as the viewer sends for the displayed page.
    fn page_search(&mut self, page: PageAddress, subcode: Option<SubCode>, sink: &mut LiveSink<'_>) {
        let mut text = [0u8; MAX_CLIENT_MESSAGE];
        let mut writer = Writer::new(&mut text);
        let subcode = subcode.unwrap_or(SubCode::ANY).value();
        let built = (|| {
            writer.put(b"pagesearch,")?;
            writer.decimal(u64::from(ZXNET_DISPLAY_SLOT))?;
            writer.byte(b',')?;
            writer.decimal(u64::from(page.magazine.number()))?;
            writer.byte(b',')?;
            writer.decimal(u64::from(page.page.0))?;
            writer.byte(b',')?;
            writer.decimal(u64::from(subcode))?;
            writer.put(b",true,false,false")
        })();
        if built.is_ok() {
            let len = writer.len();
            self.send_text(text.get(..len).unwrap_or(&[]), sink);
        }
    }

    fn opened(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        self.phase = Phase::Open;
        self.last_activity = now;
        self.next_keepalive = Instant(now.0.saturating_add(ZXNET_KEEPALIVE_US));
        let mut text = [0u8; MAX_CLIENT_MESSAGE];
        let mut writer = Writer::new(&mut text);
        if writer.put(b"service,").and_then(|()| writer.put(self.channel.identifier.as_bytes())).is_ok() {
            let len = writer.len();
            self.send_text(text.get(..len).unwrap_or(&[]), sink);
        }
        self.send_text(b"ttx,true", sink);
        let wanted = self.walk.and_then(|walk| walk.current.map(|page| (page, None))).or(self.display);
        if let Some((page, subcode)) = wanted {
            self.page_search(page, subcode, sink);
        }
        event(sink, LiveEvent::Connected);
    }

    fn received(&mut self, now: Instant, mut input: &[u8], sink: &mut LiveSink<'_>) {
        while !input.is_empty() {
            match self.phase {
                Phase::Handshake { .. } => {
                    let head_buffer = &mut *self.buffers.head;
                    let Ok((used, complete)) = self.head.feed(head_buffer, input) else {
                        self.dropped(now, true, sink);
                        return;
                    };
                    input = input.get(used..).unwrap_or(&[]);
                    if complete {
                        let head = self.buffers.head.get(..self.head.len()).unwrap_or(&[]);
                        let accepted = http::parse_head(head, &mut |_| {})
                            .is_ok_and(|head| ws::check_handshake(&head, &self.key).is_ok());
                        if !accepted {
                            self.dropped(now, true, sink);
                            return;
                        }
                        self.opened(now, sink);
                    }
                }
                Phase::Open => {
                    let (used, done) = match self.frames.feed(self.buffers.message, input) {
                        Ok(result) => result,
                        Err(error) => {
                            let mut writer = Writer::new(self.buffers.request);
                            if ws::write_close(&mut writer, error.close_code(), &mut self.random).is_ok() {
                                let len = writer.len();
                                sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
                            }
                            self.dropped(now, true, sink);
                            return;
                        }
                    };
                    input = input.get(used..).unwrap_or(&[]);
                    match done {
                        None | Some(WsMessage::Pong) => {}
                        Some(WsMessage::Text(len)) => self.message(now, len, sink),
                        Some(WsMessage::Skipped) => event(sink, LiveEvent::Ignored(Malformed::TooLong)),
                        Some(WsMessage::Binary(_)) => event(sink, LiveEvent::Ignored(Malformed::Binary)),
                        Some(WsMessage::Ping) => {
                            let mut payload = [0u8; crate::consts::WS_CONTROL_MAX];
                            let ping = self.frames.control_payload();
                            let len = ping.len().min(payload.len());
                            payload.get_mut(..len).unwrap_or(&mut []).copy_from_slice(ping.get(..len).unwrap_or(&[]));
                            let mut writer = Writer::new(self.buffers.request);
                            if ws::write_frame(
                                &mut writer,
                                OPCODE_PONG,
                                payload.get(..len).unwrap_or(&[]),
                                &mut self.random,
                            )
                            .is_ok()
                            {
                                let len = writer.len();
                                sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
                            }
                        }
                        Some(WsMessage::Close(code)) => {
                            let mut writer = Writer::new(self.buffers.request);
                            let reply = code.unwrap_or(crate::consts::CLOSE_NORMAL);
                            if ws::write_close(&mut writer, reply, &mut self.random).is_ok() {
                                let len = writer.len();
                                sink(LiveOutput::Send(self.buffers.request.get(..len).unwrap_or(&[])));
                            }
                            self.dropped(now, true, sink);
                            return;
                        }
                    }
                    if self.phase != Phase::Open {
                        return;
                    }
                }
                Phase::Stopped | Phase::Waiting { .. } | Phase::Connecting { .. } => return,
            }
        }
    }

    /// Handles one server message held in the message buffer.
    fn message(&mut self, now: Instant, len: usize, sink: &mut LiveSink<'_>) {
        let parsed = parse_message(self.buffers.message.get(..len).unwrap_or(&[]));
        let message = match parsed {
            Ok(Some(message)) => message,
            Ok(None) => return,
            Err(malformed) => {
                event(sink, LiveEvent::Ignored(malformed));
                return;
            }
        };
        match message {
            Message::ApiVersion(version) => {
                event(sink, LiveEvent::ApiVersion(version));
                let mut text = [0u8; 16];
                let mut writer = Writer::new(&mut text);
                if writer.put(b"apiver,").and_then(|()| writer.decimal(u64::from(ZXNET_API_VERSION))).is_ok() {
                    let len = writer.len();
                    self.send_text(text.get(..len).unwrap_or(&[]), sink);
                }
            }
            Message::WrongName => {
                sink(LiveOutput::Close);
                self.phase = Phase::Stopped;
                event(sink, LiveEvent::WrongName);
            }
            Message::Header { slot, data, control, magazine, subcode, page } => {
                self.last_activity = now;
                // A healthy connection: failed attempts count from zero.
                self.attempts = 0;
                let packet = rebuild(magazine, PACKET_HEADER, &data);
                sink(LiveOutput::Packet(packet));
                cross_check(&packet, control, subcode, page, sink);
                let address = PageAddress { magazine, page: PageNumber(page) };
                if slot == ZXNET_DISPLAY_SLOT
                    && let Some(walk) = &mut self.walk
                    && walk.current == Some(address)
                {
                    walk.header_seen = true;
                }
            }
            Message::Row { slot, magazine, number, data } => {
                sink(LiveOutput::Packet(rebuild(magazine, number, &data)));
                if slot == ZXNET_DISPLAY_SLOT
                    && let Some(walk) = &mut self.walk
                    && walk.header_seen
                {
                    walk.rows = walk.rows.saturating_add(1);
                    walk.last_row = Some(now);
                }
            }
            Message::Bsdp(data) => {
                self.last_activity = now;
                let service = Magazine::new(SERVICE_MAGAZINE).unwrap_or(Magazine::from_bits(0));
                sink(LiveOutput::Packet(rebuild(service, PACKET_SERVICE, &data)));
            }
            Message::Clock(clock, len) => {
                self.last_activity = now;
                event(sink, LiveEvent::Clock(clock.get(..len).unwrap_or(&[])));
            }
            Message::PageExists(address) => {
                let index = page_index(address);
                if let Some(byte) = self.pages.get_mut(index / 8) {
                    *byte |= 1 << (index % 8);
                }
                event(sink, LiveEvent::PageExists(address));
            }
            Message::InitialPage { page, subcode, status, len } => {
                event(sink, LiveEvent::InitialPage { page, subcode, status: status.get(..len).unwrap_or(&[]) });
            }
        }
    }

    /// Moves the walk to the first listed page after `after`.
    fn walk_next(&mut self, now: Instant, after: Option<PageAddress>, sink: &mut LiveSink<'_>) {
        let start = after.map_or(0, |page| walk_order(page).saturating_add(1));
        let next = (start..ZXNET_MAX_PAGES).filter_map(page_from_walk_order).find(|&page| self.page_exists(page));
        let Some(walk) = &mut self.walk else { return };
        *walk = Walk { current: next, started: now, header_seen: false, rows: 0, last_row: None };
        match next {
            Some(page) => {
                event(sink, LiveEvent::WalkPage(page));
                if self.phase == Phase::Open {
                    self.page_search(page, None, sink);
                }
            }
            None if after.is_some() => {
                self.walk = None;
                event(sink, LiveEvent::WalkComplete);
            }
            None => {}
        }
    }

    fn tick(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        match self.phase {
            Phase::Waiting { until } if now >= until => self.connect(now, sink),
            Phase::Connecting { deadline } | Phase::Handshake { deadline } if now >= deadline => {
                self.dropped(now, true, sink);
            }
            Phase::Open => {
                if now.since(self.last_activity) >= ZXNET_INACTIVITY_US {
                    self.dropped(now, true, sink);
                    return;
                }
                if now >= self.next_keepalive {
                    self.next_keepalive = Instant(now.0.saturating_add(ZXNET_KEEPALIVE_US));
                    self.send_text(b"keepalive", sink);
                }
                self.tick_walk(now, sink);
            }
            _ => {}
        }
    }

    fn tick_walk(&mut self, now: Instant, sink: &mut LiveSink<'_>) {
        let Some(walk) = self.walk else { return };
        let Some(page) = walk.current else {
            // No pages were listed yet: try again as they arrive.
            self.walk_next(now, None, sink);
            return;
        };
        let settled = walk.last_row.is_some_and(|last| now.since(last) >= ZXNET_WALK_SETTLE_US);
        let timed_out = now.since(walk.started) >= ZXNET_WALK_TIMEOUT_US;
        if settled || timed_out {
            event(sink, LiveEvent::WalkPageDone { page, rows: walk.rows, timed_out: !settled });
            self.walk_next(now, Some(page), sink);
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Stopped => None,
            Phase::Waiting { until } => Some(until),
            Phase::Connecting { deadline } | Phase::Handshake { deadline } => Some(deadline),
            Phase::Open => {
                let inactive = Instant(self.last_activity.0.saturating_add(ZXNET_INACTIVITY_US));
                let walk = self.walk.and_then(|walk| {
                    walk.last_row
                        .map(|last| Instant(last.0.saturating_add(ZXNET_WALK_SETTLE_US)))
                        .or(Some(Instant(walk.started.0.saturating_add(ZXNET_WALK_TIMEOUT_US))))
                });
                [Some(inactive), Some(self.next_keepalive), walk].into_iter().flatten().min()
            }
        }
    }
}
