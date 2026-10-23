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

mod carriage;
pub mod control;
pub mod cursor;
pub mod digits;
pub mod event;
pub mod held;
pub mod keys;
mod limits;
pub mod modes;
pub mod names;
pub mod session;
pub mod wire;

pub use carriage::is_input_packet;
pub use limits::*;
