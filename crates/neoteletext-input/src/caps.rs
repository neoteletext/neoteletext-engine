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
/// Bit 18 was the capability of provisional echo until decision E.134,
/// which withdrew echo: reserved, never set, and ignored when a peer still
/// sets it, so an older channel's WELCOME is not refused for agreeing it.
pub const RETIRED_ECHO_BIT: u32 = 1 << 18;
/// Capability: HELLO carries the user agent string (HELLO only).
pub const AGENT: u32 = 1 << 19;
/// Capability: touch (REQ-RCH-041 to REQ-RCH-043): MOUSE's device bits and
/// the TOUCH extension are sent; without it touch goes as the mouse, the
/// first contact alone.
pub const TOUCH_CAP: u32 = 1 << 20;
/// Capability: a lost connection's session may be resumed (REQ-RCH-013):
/// WELCOME carries a token, and a reconnection's first message carries
/// RESUME after HELLO (INPUT.md section 6.4).
pub const RESUME_CAP: u32 = 1 << 21;
/// Capability: the receiver sends SELECT, the connection's selection
/// (INPUT.md section 5.4, REQ-INT-140); a channel that sets it sends the
/// one sub-page selected, never a page's sub-pages together (DEC-1500).
pub const SELECT_CAP: u32 = 1 << 22;
/// Every capability defined.
pub const ALL_CAPS: u32 = PAGE_CAP | CURSOR_CAP | AGENT | TOUCH_CAP | RESUME_CAP | SELECT_CAP;
// A retired bit is never a capability.
const _: () = assert!(ALL_CAPS & (RETIRED_MODE_BITS | RETIRED_ECHO_BIT) == 0);
