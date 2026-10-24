//! WebSocket client framing (RFC 6455), written in the project (decision
//! E.38): the opening handshake and its accept check, client masking with
//! keys from the caller's random source, framing, fragmentation, control
//! frames and size limits.

use crate::consts::{CLOSE_INVALID_DATA, CLOSE_PROTOCOL_ERROR, CLOSE_TOO_BIG, WEBSOCKET_KEY_TEXT};

mod frame;
mod handshake;
mod reader;

pub use frame::{write_close, write_frame, write_frame_with_mask};
pub use handshake::{accept_for, check_handshake, new_key, write_handshake_headers};
pub use reader::FrameReader;

/// Why a WebSocket connection failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WsError {
    /// The handshake response was not a valid acceptance (RFC 6455
    /// section 4.1, client requirements 1 to 6).
    Handshake,
    /// A frame broke the framing rules: a masked server frame, reserved
    /// bits, an unknown opcode, a fragmented or oversized control frame, a
    /// continuation without a start, or a non-minimal length.
    Protocol,
    /// A message larger than the limit.
    TooBig,
    /// A text message that is not UTF-8 (RFC 6455 section 8.1).
    InvalidText,
}

impl WsError {
    /// The close status to send for this error (RFC 6455 section 7.4.1).
    #[must_use]
    pub const fn close_code(self) -> u16 {
        match self {
            Self::Handshake | Self::Protocol => CLOSE_PROTOCOL_ERROR,
            Self::TooBig => CLOSE_TOO_BIG,
            Self::InvalidText => CLOSE_INVALID_DATA,
        }
    }
}

/// The `Sec-WebSocket-Key` text: base64 of 16 random bytes.
pub type HandshakeKey = [u8; WEBSOCKET_KEY_TEXT];

/// What a completed frame or message is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WsMessage {
    /// A complete text message of this length in the message buffer,
    /// already checked to be UTF-8.
    Text(usize),
    /// A complete binary message of this length in the message buffer.
    Binary(usize),
    /// A ping; the payload is in [`FrameReader::control_payload`] and must
    /// be answered with a pong.
    Ping,
    /// A pong.
    Pong,
    /// The server closed the connection, with its status if it sent one.
    Close(Option<u16>),
    /// A data message longer than the message buffer, read to its end and
    /// discarded, from a reader made with [`FrameReader::skipping_oversized`].
    Skipped,
}

#[cfg(test)]
mod tests;
