//! The input stream, receiver to channel (`neoteletext-common/INPUT.md`
//! section 5, REQ-INT-101 to REQ-INT-104): raw UTF-8 text, and events that
//! begin with `0xFF`, which never occurs in UTF-8.
//!
//! [`write()`] encodes one item and checks it with the reader, so a sender can
//! never write what a channel refuses; [`read`] decodes one item from bytes
//! that may end early, which [`read_message`] and [`StreamDecoder`] build on.

use crate::wire::WireError;

/// The byte that begins every event.
pub const EVENT: u8 = 0xFF;

/// Core event types (section 5.4).
pub const KEY: u8 = 0x01;
/// A mouse button or move.
pub const MOUSE: u8 = 0x02;
/// The wheel.
pub const WHEEL: u8 = 0x03;
/// A paste.
pub const PASTE: u8 = 0x04;
/// The receiver has the focus.
pub const FOCUS_IN: u8 = 0x05;
/// The receiver lost the focus.
pub const FOCUS_OUT: u8 = 0x06;
/// The page's size.
pub const RESIZE: u8 = 0x07;
/// An input method's preedit.
pub const COMPOSE: u8 = 0x08;
/// The session's first event.
pub const HELLO: u8 = 0x09;
/// The page the viewer has open.
pub const PAGE: u8 = 0x0A;
/// The last core type defined; core types beyond it, to `0x3F`, are refused.
const LAST_CORE: u8 = PAGE;
/// The core types' range.
const CORE_LAST_TYPE: u8 = 0x3F;
/// Extension types: a length, then a payload a receiver may skip.
pub const FIRST_EXTENSION: u8 = 0x40;
/// The last extension type.
pub const LAST_EXTENSION: u8 = 0x7F;

/// KEY's flag bits (section 5.5).
const ACTION: u8 = 0b11;
const FUNCTIONAL: u8 = 1 << 2;
const HAS_MODS: u8 = 1 << 3;
const HAS_TEXT: u8 = 1 << 4;
const HAS_BASE: u8 = 1 << 5;
const KEY_RESERVED: u8 = 0b1100_0000;
/// MOUSE's and WHEEL's flag bits (section 5.4).
const HAS_PIXELS: u8 = 1 << 2;
const POINTER_RESERVED: u8 = 0b1111_0000;

/// The bytes that cannot begin text: UTF-8 never uses them (RFC 3629).
const NEVER_UTF8: [u8; 2] = [0xC0, 0xC1];
const NEVER_UTF8_FROM: u8 = 0xF5;

/// Modifier bits of `mods` (section 5.5), as the Kitty keyboard protocol.
pub const SHIFT: u8 = 1;
/// Alt, which macOS calls Option.
pub const ALT: u8 = 2;
/// Control.
pub const CONTROL: u8 = 4;
/// Super: Command, or the Windows key.
pub const SUPER: u8 = 8;
/// Hyper.
pub const HYPER: u8 = 16;
/// Meta.
pub const META: u8 = 32;
/// Caps Lock on.
pub const CAPS_LOCK: u8 = 64;
/// Num Lock on.
pub const NUM_LOCK: u8 = 128;

/// A key's action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Pressed.
    Press,
    /// Repeating while held.
    Repeat,
    /// Released.
    Release,
}

/// Which key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyCode {
    /// A character key: the active layout's character without Shift.
    Character(char),
    /// A functional code ([`crate::keys`]).
    Functional(u32),
}

/// A KEY event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Key<'a> {
    /// Press, repeat or release.
    pub action: Action,
    /// Which key.
    pub key: KeyCode,
    /// The modifier bits held.
    pub mods: u8,
    /// The text the press produced (`ALL_KEYS` only).
    pub text: Option<&'a str>,
    /// The United States layout's character for the same physical key
    /// (`BASE_KEY` only).
    pub base: Option<char>,
}

/// A mouse button's action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseAction {
    /// A button pressed.
    Press,
    /// A button released.
    Release,
    /// The pointer moved.
    Move,
}

/// Where on the page a pointer event happened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Point {
    /// The cell's column, 0 to [`MAX_COLUMNS`](crate::MAX_COLUMNS) − 1.
    pub column: u32,
    /// The cell's row, 0 to [`MAX_ROWS`](crate::MAX_ROWS) − 1.
    pub row: u32,
    /// Pixels of the page image, in `MOUSE_PIXELS`.
    pub pixels: Option<(u32, u32)>,
    /// The modifier bits held.
    pub mods: u8,
}

/// A MOUSE event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mouse {
    /// Press, release or move.
    pub action: MouseAction,
    /// On a press or release the button (0 left, 1 middle, 2 right, 3 back,
    /// 4 forward); on a move the bit mask of buttons held.
    pub button: u8,
    /// Where.
    pub at: Point,
}

/// A WHEEL event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Wheel {
    /// Across, in 1/120ths of a line, positive right.
    pub dx: i32,
    /// Down, in 1/120ths of a line, positive down.
    pub dy: i32,
    /// Where.
    pub at: Point,
}

/// An event: the core events of section 5.4 (REQ-INT-103), or an
/// extension.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event<'a> {
    /// A key.
    Key(Key<'a>),
    /// A mouse button or move.
    Mouse(Mouse),
    /// The wheel.
    Wheel(Wheel),
    /// A paste, at most [`MAX_PASTE`](crate::MAX_PASTE) bytes; data, never keystrokes.
    Paste(&'a str),
    /// The receiver has the focus.
    FocusIn,
    /// The receiver lost the focus: the channel releases what is held.
    FocusOut,
    /// The page's text area in cells and its image in pixels (0 unknown).
    Resize {
        /// Columns.
        columns: u32,
        /// Rows.
        rows: u32,
        /// Pixels across.
        width: u32,
        /// Pixels down.
        height: u32,
    },
    /// An input method's preedit; empty ends composition.
    Compose {
        /// A code-point offset into the text.
        cursor: u32,
        /// The preedit, at most [`MAX_COMPOSE`](crate::MAX_COMPOSE) bytes.
        text: &'a str,
    },
    /// The session's first event.
    Hello {
        /// The protocol's version.
        version: u32,
        /// The receiver's capabilities.
        caps: u32,
        /// The user agent string, present with the AGENT capability.
        agent: Option<&'a str>,
    },
    /// The page the viewer has open, or 0 for none.
    Page(u16),
    /// An extension, skipped by a receiver that does not know it.
    Extension {
        /// `0x40` to `0x7F`.
        kind: u8,
        /// Its payload.
        payload: &'a [u8],
    },
}

/// One item of the stream.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item<'a> {
    /// A run of text: whole characters, no control characters.
    Text(&'a str),
    /// An event.
    Event(Event<'a>),
}

/// Why bytes were refused (REQ-INT-102): a protocol error, which closes the
/// connection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputError {
    /// A primitive was malformed.
    Wire(WireError),
    /// A byte that cannot begin text or an event.
    Byte(u8),
    /// A reserved type, or a core type not defined.
    Type(u8),
    /// A reserved flag bit set.
    Flags,
    /// A control character in text.
    Control,
    /// A field outside its range, and which.
    BadValue(&'static str),
    /// A message that ends inside an item, or is over [`MAX_MESSAGE`](crate::MAX_MESSAGE).
    Message,
}

impl From<WireError> for InputError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

/// Whether a character may appear in text: no C0 or C1 control (section
/// 5.3).
#[must_use]
pub fn printable(character: char) -> bool {
    !character.is_control()
}

fn check_text(text: &str) -> Result<(), InputError> {
    if text.chars().all(printable) { Ok(()) } else { Err(InputError::Control) }
}

/// How much of the input count an item is (INPUT.md section 7.1): a
/// character of text counts one, and every event counts one.
#[must_use]
pub fn units(item: &Item<'_>) -> u32 {
    match item {
        Item::Text(text) => u32::try_from(text.chars().count()).unwrap_or(u32::MAX),
        Item::Event(_) => 1,
    }
}

mod decode;
mod encode;

pub use decode::{MAX_STREAM_ITEM, StreamDecoder, read, read_message};
pub use encode::write;

#[cfg(test)]
mod tests;
