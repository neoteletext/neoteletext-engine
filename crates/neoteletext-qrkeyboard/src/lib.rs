//! A phone as a keyboard, through qrkeyboard's relay (decision E.94, T42
//! Browser's `interaction.md` section 9). The screen shows a QR; the phone
//! that scans it pairs with the screen through the relay, end to end
//! encrypted, and sends its keys.
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
pub mod json;
pub mod message;
pub mod net;
pub mod pairing;
pub mod phone;
pub mod qr;
pub mod screen;

/// qrkeyboard's protocol version this crate speaks (`PROTOCOL_VERSION`).
pub const PROTOCOL_VERSION: u32 = 3;
/// The relay's limit on one binary WebSocket frame (`MAX_FRAME_BYTES`).
pub const MAX_FRAME_BYTES: usize = 1024;
/// The keepalive the relay answers itself (`KEEPALIVE`).
pub const KEEPALIVE: &str = "k";
/// What a screen sends when it goes away for good (`LEAVE`).
pub const LEAVE: &str = "leave";
/// How often a keepalive goes: below the idle timeout of about 30 seconds
/// common on NATs and mobile radios (`KEEPALIVE_INTERVAL_MS`).
pub const KEEPALIVE_INTERVAL_MS: u64 = 25_000;
/// The relay's public address.
pub const RELAY: &str = "https://qrkeyboard.com";

/// Why something failed.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The network: a name that did not resolve, a refused or broken
    /// connection, a time limit.
    Network(String),
    /// TLS failed, a certificate among the causes: a hard stop.
    Tls(String),
    /// The relay answered with this status.
    Status(u16),
    /// The relay broke HTTP, WebSocket or qrkeyboard's protocol, and how.
    Protocol(&'static str),
    /// The relay closed the WebSocket with this code (`CLOSE` in
    /// `protocol.ts`): 4001 to 4011.
    Closed(u16),
    /// An address that is not `http` or `https`.
    Address,
    /// Something this side was asked for is outside a limit.
    Limit(&'static str),
}

impl core::fmt::Display for Error {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Network(why) => write!(formatter, "network: {why}"),
            Self::Tls(why) => write!(formatter, "TLS: {why}"),
            Self::Status(status) => write!(formatter, "the relay answered {status}"),
            Self::Protocol(what) => write!(formatter, "the relay broke the protocol: {what}"),
            Self::Closed(code) => write!(formatter, "the relay closed the connection ({code})"),
            Self::Address => write!(formatter, "the relay's address must be http or https"),
            Self::Limit(what) => write!(formatter, "outside a limit: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// Random bytes from the operating system.
///
/// # Errors
/// When the operating system has none to give.
pub fn random(bytes: &mut [u8]) -> Result<(), Error> {
    getrandom::getrandom(bytes).map_err(|error| Error::Network(format!("no random numbers: {error}")))
}
