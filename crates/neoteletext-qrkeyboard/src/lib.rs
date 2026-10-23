//! A phone as a keyboard, through qrkeyboard's relay (REQ-INT-007,
//! decision E.93, NeoTeletext's `interaction.md` section 9). The screen
//! shows a QR; the phone that scans it pairs with the screen through the
//! relay, end to end encrypted, and sends its keys.
//!
//! This crate is the screen's side, protocol 3 of qrkeyboard (its
//! `relay/src/protocol.ts`, `crypto.ts` and `pairing.ts`):
//!
//! - [`config`]: the screen's configuration and its hash, as the phone
//!   checks it;
//! - [`crypto`]: the directional AES-256-GCM keys, sealed frames and
//!   SPAKE2 for a PIN;
//! - [`pairing`]: one pairing run, sans input and output;
//! - [`message`]: the application messages;
//! - [`net`]: HTTP and WebSocket connections to the relay, with TLS;
//! - [`screen`]: the screen: create, show the QR, pair, receive keys;
//! - [`phone`]: the phone's side, for tests and for checking a relay;
//! - [`qr`]: the QR code, and as teletext mosaics.
//!
//! Everything the relay and the phone send is untrusted: every length is
//! bounded before use, and nothing here panics on input.
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

pub mod config;
pub mod crypto;
mod error;
pub mod json;
pub mod message;
pub mod net;
pub mod pairing;
pub mod phone;
mod protocol;
pub mod qr;
pub mod screen;

pub use crypto::random;
pub use error::Error;
pub use protocol::*;
