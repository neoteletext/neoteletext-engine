//! The feed's vocabulary (REQ-INP-022): its inputs, its outputs and the
//! events it reports.

use neoteletext_core::packet::Packet;
use neoteletext_core::types::{Instant, PageAddress, SubCode};

use crate::port::{ConnectionId, NetInput, NetOutput, Security};

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
