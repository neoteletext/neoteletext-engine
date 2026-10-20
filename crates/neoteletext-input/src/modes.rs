//! Negotiation and modes (`neoteletext-common/INPUT.md` section 6,
//! REQ-INT-106, REQ-INT-107, REQ-INT-114): the capability bits of HELLO and
//! WELCOME, the mode bits, and the receiver's mode stack with the open
//! page's own modes.

/// Mode: every key as KEY with its text; no raw text.
pub const ALL_KEYS: u32 = 1;
/// Mode: repeat and release actions too.
pub const KEY_RELEASE: u32 = 2;
/// Mode: `base` when it differs from `key`.
pub const BASE_KEY: u32 = 4;
/// Mode: MOUSE press and release, and WHEEL.
pub const MOUSE_BUTTONS: u32 = 8;
/// Mode: moves while a button is held.
pub const MOUSE_DRAG: u32 = 16;
/// Mode: every move over the page.
pub const MOUSE_ALL: u32 = 32;
/// Mode: pixels as well as the cell.
pub const MOUSE_PIXELS: u32 = 64;
/// Mode: COMPOSE preedit.
pub const COMPOSE: u32 = 128;
/// Every mode bit defined.
pub const ALL_MODES: u32 =
    ALL_KEYS | KEY_RELEASE | BASE_KEY | MOUSE_BUTTONS | MOUSE_DRAG | MOUSE_ALL | MOUSE_PIXELS | COMPOSE;
/// The modes that send pointer events.
pub const MOUSE_MODES: u32 = MOUSE_BUTTONS | MOUSE_DRAG | MOUSE_ALL;
/// Bits 0 to 15 of `caps` are modes; 8 to 15 are reserved for future ones.
pub const MODE_BITS: u32 = 0xFFFF;

/// Capability: the receiver shows teletext pages and sends PAGE.
pub const PAGE_CAP: u32 = 1 << 16;
/// Capability: the receiver draws the channel's cursor record.
pub const CURSOR_CAP: u32 = 1 << 17;
/// Capability: the receiver draws provisional echo.
pub const ECHO_CAP: u32 = 1 << 18;
/// Capability: HELLO carries the user agent string (HELLO only).
pub const AGENT: u32 = 1 << 19;
/// Every capability defined.
pub const ALL_CAPS: u32 = ALL_MODES | PAGE_CAP | CURSOR_CAP | ECHO_CAP | AGENT;

/// The fewest entries a receiver keeps on its mode stack.
pub const STACK_DEPTH: usize = 16;

/// The receiver's modes: the session's stack, and the open page's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modes {
    stack: [u32; STACK_DEPTH],
    depth: usize,
    /// The modes both sides agreed on in WELCOME; nothing outside them
    /// applies.
    agreed: u32,
    /// The open page and its own modes (section 6.4).
    page: Option<(u16, u32)>,
}

impl Modes {
    /// No modes, and nothing agreed yet.
    #[must_use]
    pub const fn new() -> Self {
        Self { stack: [0; STACK_DEPTH], depth: 0, agreed: 0, page: None }
    }

    /// WELCOME's capabilities: only these modes may apply.
    pub fn agree(&mut self, caps: u32) {
        self.agreed = caps & ALL_MODES;
    }

    /// `PUSH_MODE`: `modes` becomes the top; the oldest entry goes when the
    /// stack is full.
    pub fn push(&mut self, modes: u32) {
        if self.depth == STACK_DEPTH {
            self.stack.copy_within(1.., 0);
            self.depth = self.depth.saturating_sub(1);
        }
        if let Some(slot) = self.stack.get_mut(self.depth) {
            *slot = modes;
            self.depth = self.depth.saturating_add(1);
        }
    }

    /// `POP_MODE`: removes `count` entries, or all there are.
    pub fn pop(&mut self, count: u32) {
        self.depth = self.depth.saturating_sub(usize::try_from(count).unwrap_or(usize::MAX));
    }

    /// `PAGE_MODES`, or a page's `IM` line: `page`'s own modes, 0 to clear.
    pub fn set_page_modes(&mut self, page: u16, modes: u32) {
        self.page = (modes != 0).then_some((page, modes));
    }

    /// The modes in force on `open`, the page the viewer has open: the
    /// stack's top and that page's own, within what was agreed.
    #[must_use]
    pub fn on(&self, open: Option<u16>) -> u32 {
        let top = self.depth.checked_sub(1).and_then(|top| self.stack.get(top)).copied().unwrap_or(0);
        let page = self.page.filter(|(page, _)| Some(*page) == open).map_or(0, |(_, modes)| modes);
        (top | page) & self.agreed
    }

    /// A new session: the stack empty, the page's modes gone, nothing
    /// agreed (REQ-INT-106).
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-INT-107.
    #[test]
    fn modes_stack_and_apply_only_when_agreed() {
        let mut modes = Modes::new();
        modes.push(MOUSE_BUTTONS);
        assert_eq!(modes.on(None), 0, "nothing agreed");
        modes.agree(MOUSE_BUTTONS | KEY_RELEASE | PAGE_CAP);
        assert_eq!(modes.on(None), MOUSE_BUTTONS);
        modes.push(KEY_RELEASE | MOUSE_ALL);
        assert_eq!(modes.on(None), KEY_RELEASE, "MOUSE_ALL was not agreed");
        modes.pop(1);
        assert_eq!(modes.on(None), MOUSE_BUTTONS, "the program before is restored");
        modes.pop(5);
        assert_eq!(modes.on(None), 0, "an empty stack is no modes");
    }

    // REQ-INT-107.
    #[test]
    fn a_full_stack_drops_its_oldest() {
        let mut modes = Modes::new();
        modes.agree(ALL_MODES);
        for _ in 0..STACK_DEPTH {
            modes.push(ALL_KEYS);
        }
        modes.push(COMPOSE);
        assert_eq!(modes.on(None), COMPOSE);
        modes.pop(1);
        assert_eq!(modes.on(None), ALL_KEYS);
    }

    // REQ-INT-114.
    #[test]
    fn a_pages_own_modes_come_and_go_with_the_page() {
        let mut modes = Modes::new();
        modes.agree(MOUSE_BUTTONS | KEY_RELEASE);
        modes.push(KEY_RELEASE);
        modes.set_page_modes(0x300, MOUSE_BUTTONS);
        assert_eq!(modes.on(Some(0x300)), KEY_RELEASE | MOUSE_BUTTONS, "added to the stack's top");
        assert_eq!(modes.on(Some(0x301)), KEY_RELEASE, "another page has none of them");
        modes.set_page_modes(0x300, 0);
        assert_eq!(modes.on(Some(0x300)), KEY_RELEASE, "cleared");
        modes.reset();
        assert_eq!(modes.on(Some(0x300)), 0);
    }
}
