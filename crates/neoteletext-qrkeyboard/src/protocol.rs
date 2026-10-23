//! qrkeyboard's protocol 3 as the relay defines it (`protocol.ts`): its
//! version, frame limit, keepalive and leave messages, and the public relay.

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
