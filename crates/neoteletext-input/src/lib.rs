//! The input profile (area INT, `docs/spec/design/input.md`): a receiver
//! connected to a channel's WebSocket sends the viewer's keys back as a
//! terminal does, one small record a key, and the channel answers by
//! pushing what changed, with an optional cursor record that places a
//! cursor, asks for local echo and acknowledges keys.
//!
//! - [`key`]: the key record, at most [`key::MAX_RECORD`] bytes.
//! - [`names`]: the numbered tables of W3C key names and physical keys.
//! - [`cursor`]: the cursor record, a record of the audio profile's frame
//!   carried on the same independent data lines.
//!
//! Everything here is `no_std` and allocation-free, so a microcontroller
//! can send keys and read cursors.
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

pub mod cursor;
pub mod key;
pub mod names;

/// The highest page number a record may name: magazine 8 (written 0 on
/// air), page FF. Pages with hexadecimal digits are addressable though no
/// viewer types them (EN 300 706 clause 7.1.2), so a channel may use them.
pub const LAST_ADDRESSABLE_PAGE: u16 = 0x8FF;

/// Whether `page` is a page number in the form records carry: magazine 1
/// to 8 then two hexadecimal page digits.
#[must_use]
pub const fn is_page(page: u16) -> bool {
    page >= neoteletext_core::consts::FIRST_PAGE && page <= LAST_ADDRESSABLE_PAGE
}
