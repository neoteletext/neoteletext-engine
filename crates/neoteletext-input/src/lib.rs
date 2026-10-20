//! The input system (area INT, `neoteletext-common/INPUT.md`, protocol
//! version 2): a receiver connected to a channel's push WebSocket sends the
//! viewer's input as one stream of UTF-8 text and small events, and the
//! channel answers in teletext, with records that place a cursor,
//! acknowledge input and ask for input modes.
//!
//! - [`event`]: the input stream, receiver to channel, over [`wire`]'s
//!   primitives, with [`keys`]' functional codes and [`names`]' W3C table.
//! - [`modes`]: capabilities, modes and the mode stack.
//! - [`cursor`] and [`control`]: the channel's records, of the audio
//!   profile's frame on the same independent data lines.
//! - [`session`]: a receiver's side of one session, the rate limit with it.
//!
//! Everything here is `no_std` and allocation-free, so a microcontroller
//! can be a receiver or a channel.
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
// REQ-SEC-003: everything here reads untrusted input.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

#[cfg(test)]
extern crate std;

pub mod control;
pub mod cursor;
pub mod digits;
pub mod event;
pub mod held;
pub mod keys;
pub mod modes;
pub mod names;
pub mod session;
pub mod wire;

/// The protocol version HELLO and WELCOME carry (section 6.1).
pub const VERSION: u32 = 2;
/// The most bytes in one binary message (section 5.1, REQ-INT-101).
pub const MAX_MESSAGE: usize = 4096;
/// The most bytes of a PASTE's text (section 5.4, REQ-INT-103).
pub const MAX_PASTE: usize = 4000;
/// The most bytes of a COMPOSE preedit (section 5.4).
pub const MAX_COMPOSE: usize = 256;
/// The most bytes of a KEY's text (section 5.5).
pub const MAX_KEY_TEXT: usize = 32;
/// The most bytes of HELLO's user agent string (section 5.4).
pub const MAX_AGENT: usize = 64;
/// Cells across a page with both side panels (section 5.4): 40 + 2 × 16.
pub const MAX_COLUMNS: u32 = 72;
/// Rows of a page: header, 23 rows and the Fastext row (section 5.4).
pub const MAX_ROWS: u32 = 25;
/// The most input units a second (section 8, REQ-INT-109): qrkeyboard's
/// relay limit.
pub const MAX_PER_SECOND: u32 = 120;
/// How long a receiver waits for WELCOME (section 6.1).
pub const WELCOME_TIMEOUT_MS: u32 = 10_000;

// A PASTE, its event byte, type and length, fits a message.
const _: () = assert!(MAX_PASTE + 2 + wire::MAX_VARINT <= MAX_MESSAGE);
// Section 7.2: a control message is at most 26 bytes, so its record, with
// the frame's 8, is the 34 bytes of one fragment.
const _: () = assert!(control::MAX_BODY == 26);

/// The highest page number a record may name: magazine 8 (written 0 on
/// air), page FF. Pages with hexadecimal digits are addressable though no
/// viewer types them (EN 300 706 clause 7.1.2), so a channel may use them.
pub const LAST_ADDRESSABLE_PAGE: u16 = 0x8FF;

/// Whether `packet` carries one of the channel's records for its input: a
/// cursor or control record, one fragment on the audio profile's data
/// channel (section 7). What it says is read by [`cursor::from_packet`] and
/// [`control::from_packet`].
#[must_use]
pub fn is_input_packet(packet: &neoteletext_core::packet::Packet) -> bool {
    matches!(
        neoteletext_audio::idl::fragment(packet),
        Some(Ok(fragment)) if fragment.first
            && fragment.last
            && fragment.data.get(1).is_some_and(|&kind| neoteletext_audio::record::is_input_type(kind))
    )
}

/// Whether `page` is a page number in the form records carry: magazine 1
/// to 8 then two hexadecimal page digits.
#[must_use]
pub const fn is_page(page: u16) -> bool {
    page >= neoteletext_core::consts::FIRST_PAGE && page <= LAST_ADDRESSABLE_PAGE
}
