//! The input system (area INT, `neoteletext-common/INPUT.md`, protocol
//! version 3): a receiver connected to a channel's push WebSocket sends the
//! viewer's input as one stream of UTF-8 text and small events, and the
//! channel answers in teletext, with records that place a cursor and
//! acknowledge input. There are no input modes: everything is sent
//! (section 6.3, decision E.125).
//!
//! - [`event`]: the input stream, receiver to channel, over [`wire`]'s
//!   primitives, with [`keys`]' functional codes and [`names`]' W3C table.
//! - [`caps`]: the capabilities a receiver offers and a channel agrees.
//! - [`cursor`] and [`control`]: the channel's records, of the audio
//!   profile's frame on the same independent data lines.
//! - [`session`]: a receiver's side of one session, the rate limit with it,
//!   and the messages kept to send again when a lost connection's session
//!   is resumed (REQ-RCH-013).
//! - [`pace`]: the pointer's moves at 60 a second, one slot a contact
//!   (REQ-RCH-031 to REQ-RCH-033); [`contacts`]: the numbers of a touch
//!   screen's contacts (REQ-RCH-043).
//!
//! Everything here is `no_std` and allocation-free, so a microcontroller
//! can be a receiver or a channel.
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
// REQ-SEC-003: everything here reads untrusted input.
#![deny(
    clippy::indexing_slicing,
    clippy::string_slice,
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

pub mod caps;
mod carriage;
pub mod contacts;
pub mod control;
pub mod cursor;
pub mod digits;
pub mod event;
pub mod held;
pub mod keys;
mod limits;
pub mod names;
pub mod pace;
pub mod session;
pub mod wire;

pub use carriage::is_input_packet;
pub use limits::*;
