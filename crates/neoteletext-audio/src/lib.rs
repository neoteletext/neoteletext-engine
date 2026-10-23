//! The teletext audio profile (area AUD, `docs/spec/design/audio.md`):
//! sounds carried as records over EN 300 708 independent data lines, a
//! network or a file; a store that keeps them compressed; and control by
//! channels, where the latest command wins and a sound can belong to a page.
//!
//! The crate decides what plays and when; it mixes nothing and drives no
//! hardware. Its output is voice operations on a [`port::VoiceSink`], which
//! the port supplies (REQ-AUD-018): a software mixer on a desktop, or the
//! tellyboy driver on a microcontroller.
//!
//! Everything here is `no_std` and allocation-free; every buffer has a size
//! fixed by its type. Only the `encode` feature, for publishing tools,
//! allocates.
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

#[cfg(feature = "encode")]
extern crate alloc;
#[cfg(test)]
extern crate std;

mod codec;
pub mod control;
pub mod crc;
#[cfg(feature = "encode")]
pub mod encode;
#[cfg(feature = "idl")]
pub mod idl;
#[cfg(feature = "ima")]
pub mod ima;
pub mod port;
mod profile;
pub mod receiver;
pub mod record;
mod serial;
pub mod store;

pub use codec::*;
pub use profile::*;
pub use serial::serial_newer;
