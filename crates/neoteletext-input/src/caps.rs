//! Capabilities (`neoteletext-common/INPUT.md` section 6.2, REQ-INT-106):
//! what a receiver can do, offered in HELLO and agreed in WELCOME. There are
//! no input modes: every event is sent and a channel ignores what it does
//! not use (section 6.3, decision E.125).

/// Bits 0 to 15 were the input modes until decision E.125: reserved, never
/// set, and ignored when a peer still sets them, so an older channel's
/// WELCOME is not refused for asking for a mode.
pub const RETIRED_MODE_BITS: u32 = 0xFFFF;

/// Capability: the receiver shows teletext pages and sends PAGE.
pub const PAGE_CAP: u32 = 1 << 16;
/// Capability: the receiver draws the channel's cursor record.
pub const CURSOR_CAP: u32 = 1 << 17;
/// Capability: the receiver draws provisional echo.
pub const ECHO_CAP: u32 = 1 << 18;
/// Capability: HELLO carries the user agent string (HELLO only).
pub const AGENT: u32 = 1 << 19;
/// Every capability defined.
pub const ALL_CAPS: u32 = PAGE_CAP | CURSOR_CAP | ECHO_CAP | AGENT;
