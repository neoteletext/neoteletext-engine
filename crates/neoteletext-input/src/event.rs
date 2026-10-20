//! The input stream, receiver to channel (`neoteletext-common/INPUT.md`
//! section 5, REQ-INT-101 to REQ-INT-104): raw UTF-8 text, and events that
//! begin with `0xFF`, which never occurs in UTF-8.
//!
//! [`write`] encodes one item and checks it with the reader, so a sender can
//! never write what a channel refuses; [`read`] decodes one item from bytes
//! that may end early, which [`read_message`] and [`StreamDecoder`] build on.

use crate::keys::is_code;
use crate::modes::AGENT;
use crate::wire::{Reader, WireError, Writer};
use crate::{MAX_AGENT, MAX_COLUMNS, MAX_COMPOSE, MAX_KEY_TEXT, MAX_MESSAGE, MAX_PASTE, MAX_ROWS, is_page};

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
    /// The cell's column, 0 to [`MAX_COLUMNS`] − 1.
    pub column: u32,
    /// The cell's row, 0 to [`MAX_ROWS`] − 1.
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
    /// A paste, at most [`MAX_PASTE`] bytes; data, never keystrokes.
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
        /// The preedit, at most [`MAX_COMPOSE`] bytes.
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
    /// A message that ends inside an item, or is over [`MAX_MESSAGE`].
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

/// Reads one item from `bytes`. `Ok(None)` means `bytes` is empty;
/// [`InputError::Wire`] of [`WireError::Incomplete`] means more is needed.
///
/// # Errors
/// Anything section 5 refuses.
pub fn read(bytes: &[u8]) -> Result<Option<(Item<'_>, usize)>, InputError> {
    let Some(&first) = bytes.first() else { return Ok(None) };
    if first != EVENT {
        return read_text(bytes).map(Some);
    }
    let mut reader = Reader::new(bytes);
    reader.byte()?;
    let event = read_event(&mut reader)?;
    Ok(Some((Item::Event(event), reader.used())))
}

fn read_text(bytes: &[u8]) -> Result<(Item<'_>, usize), InputError> {
    let end = bytes.iter().position(|&byte| byte == EVENT).unwrap_or(bytes.len());
    let run = bytes.get(..end).unwrap_or(&[]);
    if let Some(&bad) = run.iter().find(|byte| NEVER_UTF8.contains(byte) || **byte >= NEVER_UTF8_FROM) {
        return Err(InputError::Byte(bad));
    }
    let text = match core::str::from_utf8(run) {
        Ok(text) => text,
        // A character cut by the end of the bytes so far: the whole ones
        // before it, or wait for more.
        Err(error) if error.error_len().is_none() => {
            let whole = run.get(..error.valid_up_to()).unwrap_or(&[]);
            if whole.is_empty() {
                return Err(InputError::Wire(WireError::Incomplete));
            }
            core::str::from_utf8(whole).map_err(|_| InputError::Wire(WireError::Utf8))?
        }
        Err(_) => return Err(InputError::Wire(WireError::Utf8)),
    };
    check_text(text)?;
    Ok((Item::Text(text), text.len()))
}

fn read_point(reader: &mut Reader<'_>, flags: u8) -> Result<Point, InputError> {
    let column = reader.uv()?;
    let row = reader.uv()?;
    if column >= MAX_COLUMNS || row >= MAX_ROWS {
        return Err(InputError::BadValue("cell"));
    }
    let pixels = if flags & HAS_PIXELS == 0 { None } else { Some((reader.uv()?, reader.uv()?)) };
    let mods = if flags & HAS_MODS == 0 { 0 } else { reader.byte()? };
    Ok(Point { column, row, pixels, mods })
}

fn read_event<'a>(reader: &mut Reader<'a>) -> Result<Event<'a>, InputError> {
    let kind = reader.byte()?;
    Ok(match kind {
        KEY => Event::Key(read_key(reader)?),
        MOUSE => {
            let flags = reader.byte()?;
            if flags & POINTER_RESERVED != 0 {
                return Err(InputError::Flags);
            }
            let action = match flags & ACTION {
                0 => MouseAction::Press,
                1 => MouseAction::Release,
                2 => MouseAction::Move,
                _ => return Err(InputError::BadValue("mouse action")),
            };
            let button = reader.byte()?;
            Event::Mouse(Mouse { action, button, at: read_point(reader, flags)? })
        }
        WHEEL => {
            let flags = reader.byte()?;
            if flags & (POINTER_RESERVED | ACTION) != 0 {
                return Err(InputError::Flags);
            }
            let dx = reader.sv()?;
            let dy = reader.sv()?;
            Event::Wheel(Wheel { dx, dy, at: read_point(reader, flags)? })
        }
        PASTE => Event::Paste(reader.str(MAX_PASTE)?),
        FOCUS_IN => Event::FocusIn,
        FOCUS_OUT => Event::FocusOut,
        RESIZE => {
            let (columns, rows, width, height) = (reader.uv()?, reader.uv()?, reader.uv()?, reader.uv()?);
            if columns == 0 || columns > MAX_COLUMNS || rows == 0 || rows > MAX_ROWS {
                return Err(InputError::BadValue("size"));
            }
            Event::Resize { columns, rows, width, height }
        }
        COMPOSE => {
            let cursor = reader.uv()?;
            let text = reader.str(MAX_COMPOSE)?;
            check_text(text)?;
            if usize::try_from(cursor).map_or(true, |cursor| cursor > text.chars().count()) {
                return Err(InputError::BadValue("compose cursor"));
            }
            Event::Compose { cursor, text }
        }
        HELLO => {
            let version = reader.uv()?;
            let caps = reader.uv()?;
            let agent = if caps & AGENT == 0 { None } else { Some(reader.str(MAX_AGENT)?) };
            if agent.is_some_and(|agent| !agent.bytes().all(|byte| byte.is_ascii_graphic() || byte == b' ')) {
                return Err(InputError::BadValue("agent"));
            }
            Event::Hello { version, caps, agent }
        }
        PAGE => {
            let page = u16::try_from(reader.uv()?).map_err(|_| InputError::BadValue("page"))?;
            if page != 0 && !is_page(page) {
                return Err(InputError::BadValue("page"));
            }
            Event::Page(page)
        }
        FIRST_EXTENSION..=LAST_EXTENSION => {
            let length = usize::try_from(reader.uv()?).map_err(|_| InputError::Message)?;
            if length > MAX_MESSAGE {
                return Err(InputError::Message);
            }
            Event::Extension { kind, payload: reader.take(length)? }
        }
        other if other > LAST_CORE && other <= CORE_LAST_TYPE => return Err(InputError::Type(other)),
        other => return Err(InputError::Type(other)),
    })
}

fn read_key<'a>(reader: &mut Reader<'a>) -> Result<Key<'a>, InputError> {
    let flags = reader.byte()?;
    if flags & KEY_RESERVED != 0 {
        return Err(InputError::Flags);
    }
    let action = match flags & ACTION {
        0 => Action::Press,
        1 => Action::Repeat,
        2 => Action::Release,
        _ => return Err(InputError::BadValue("key action")),
    };
    let value = reader.uv()?;
    let key = if flags & FUNCTIONAL == 0 {
        let character = char::from_u32(value).ok_or(InputError::BadValue("key"))?;
        if !printable(character) {
            return Err(InputError::BadValue("key"));
        }
        KeyCode::Character(character)
    } else if is_code(value) {
        KeyCode::Functional(value)
    } else {
        return Err(InputError::BadValue("functional code"));
    };
    let mods = if flags & HAS_MODS == 0 { 0 } else { reader.byte()? };
    if flags & HAS_MODS != 0 && mods == 0 {
        return Err(InputError::BadValue("mods present and empty"));
    }
    let text = if flags & HAS_TEXT == 0 { None } else { Some(reader.str(MAX_KEY_TEXT)?) };
    if let Some(text) = text {
        check_text(text)?;
    }
    let base = if flags & HAS_BASE == 0 {
        None
    } else {
        let base = char::from_u32(reader.uv()?).filter(|base| printable(*base)).ok_or(InputError::BadValue("base"))?;
        Some(base)
    };
    Ok(Key { action, key, mods, text, base })
}

/// Writes one item, then reads it back: what does not read back as it was
/// written is not sent.
///
/// # Errors
/// When the item breaks a bound, or `out` is too small.
pub fn write(item: &Item<'_>, out: &mut [u8]) -> Result<usize, InputError> {
    let mut writer = Writer::new(out);
    match item {
        Item::Text(text) => {
            check_text(text)?;
            writer.bytes(text.as_bytes())?;
        }
        Item::Event(event) => {
            writer.byte(EVENT)?;
            write_event(event, &mut writer)?;
        }
    }
    let length = writer.len();
    let written = out.get(..length).unwrap_or(&[]);
    match read(written) {
        Ok(Some((back, used))) if used == length && back == *item => Ok(length),
        Ok(_) => Err(InputError::BadValue("does not read back")),
        Err(error) => Err(error),
    }
}

fn write_point(point: &Point, writer: &mut Writer<'_>) -> Result<(), InputError> {
    writer.uv(point.column)?;
    writer.uv(point.row)?;
    if let Some((x, y)) = point.pixels {
        writer.uv(x)?;
        writer.uv(y)?;
    }
    if point.mods != 0 {
        writer.byte(point.mods)?;
    }
    Ok(())
}

fn point_flags(point: &Point) -> u8 {
    (if point.pixels.is_some() { HAS_PIXELS } else { 0 }) | (if point.mods == 0 { 0 } else { HAS_MODS })
}

fn write_event(event: &Event<'_>, writer: &mut Writer<'_>) -> Result<(), InputError> {
    match event {
        Event::Key(key) => {
            let action = match key.action {
                Action::Press => 0,
                Action::Repeat => 1,
                Action::Release => 2,
            };
            let (functional, value) = match key.key {
                KeyCode::Character(character) => (0, u32::from(character)),
                KeyCode::Functional(code) => (FUNCTIONAL, code),
            };
            let flags = action
                | functional
                | (if key.mods == 0 { 0 } else { HAS_MODS })
                | (if key.text.is_some() { HAS_TEXT } else { 0 })
                | (if key.base.is_some() { HAS_BASE } else { 0 });
            writer.bytes(&[KEY, flags])?;
            writer.uv(value)?;
            if key.mods != 0 {
                writer.byte(key.mods)?;
            }
            if let Some(text) = key.text {
                writer.str(text, MAX_KEY_TEXT)?;
            }
            if let Some(base) = key.base {
                writer.uv(u32::from(base))?;
            }
        }
        Event::Mouse(mouse) => {
            let action = match mouse.action {
                MouseAction::Press => 0,
                MouseAction::Release => 1,
                MouseAction::Move => 2,
            };
            writer.bytes(&[MOUSE, action | point_flags(&mouse.at), mouse.button])?;
            write_point(&mouse.at, writer)?;
        }
        Event::Wheel(wheel) => {
            writer.bytes(&[WHEEL, point_flags(&wheel.at)])?;
            writer.sv(wheel.dx)?;
            writer.sv(wheel.dy)?;
            write_point(&wheel.at, writer)?;
        }
        Event::Paste(text) => {
            writer.byte(PASTE)?;
            writer.str(text, MAX_PASTE)?;
        }
        Event::FocusIn => writer.byte(FOCUS_IN)?,
        Event::FocusOut => writer.byte(FOCUS_OUT)?,
        Event::Resize { columns, rows, width, height } => {
            writer.byte(RESIZE)?;
            for value in [*columns, *rows, *width, *height] {
                writer.uv(value)?;
            }
        }
        Event::Compose { cursor, text } => {
            writer.byte(COMPOSE)?;
            writer.uv(*cursor)?;
            writer.str(text, MAX_COMPOSE)?;
        }
        Event::Hello { version, caps, agent } => {
            if agent.is_some() != (caps & AGENT != 0) {
                return Err(InputError::BadValue("agent without its capability"));
            }
            writer.byte(HELLO)?;
            writer.uv(*version)?;
            writer.uv(*caps)?;
            if let Some(agent) = agent {
                writer.str(agent, MAX_AGENT)?;
            }
        }
        Event::Page(page) => {
            writer.byte(PAGE)?;
            writer.uv(u32::from(*page))?;
        }
        Event::Extension { kind, payload } => {
            if !(FIRST_EXTENSION..=LAST_EXTENSION).contains(kind) {
                return Err(InputError::Type(*kind));
            }
            writer.byte(*kind)?;
            writer.uv(u32::try_from(payload.len()).map_err(|_| InputError::Message)?)?;
            writer.bytes(payload)?;
        }
    }
    Ok(())
}

/// Decodes one whole message (section 5.1): whole items only, at most
/// [`MAX_MESSAGE`] bytes, each passed to `each` in order.
///
/// # Errors
/// Any item section 5 refuses, an item cut by the message's end, or a
/// message over [`MAX_MESSAGE`].
pub fn read_message<'a>(message: &'a [u8], each: &mut dyn FnMut(Item<'a>)) -> Result<(), InputError> {
    if message.len() > MAX_MESSAGE {
        return Err(InputError::Message);
    }
    let mut rest = message;
    while let Some((item, used)) = read(rest).map_err(|error| match error {
        InputError::Wire(WireError::Incomplete) => InputError::Message,
        other => other,
    })? {
        each(item);
        rest = rest.get(used..).unwrap_or(&[]);
    }
    Ok(())
}

/// Decodes a byte stream that may split items anywhere, as a serial line
/// does (section 5.1), holding at most `N` bytes of an item not yet whole.
/// `N` must hold the largest event: [`MAX_STREAM_ITEM`].
pub struct StreamDecoder<const N: usize> {
    held: [u8; N],
    length: usize,
}

/// The most bytes one item takes: a PASTE of [`MAX_PASTE`] bytes with its
/// type and length.
pub const MAX_STREAM_ITEM: usize = MAX_PASTE + 8;

impl<const N: usize> Default for StreamDecoder<N> {
    fn default() -> Self {
        Self { held: [0; N], length: 0 }
    }
}

impl<const N: usize> StreamDecoder<N> {
    /// Takes `bytes`, passing each whole item to `each`, and keeps what is
    /// not yet whole.
    ///
    /// # Errors
    /// Any item section 5 refuses, or an item longer than `N`: either ends
    /// the stream.
    pub fn push(&mut self, mut bytes: &[u8], each: &mut dyn FnMut(Item<'_>)) -> Result<(), InputError> {
        while !bytes.is_empty() {
            let room = N.saturating_sub(self.length);
            if room == 0 {
                return Err(InputError::Message);
            }
            let (taken, rest) = bytes.split_at(room.min(bytes.len()));
            let end = self.length.saturating_add(taken.len());
            self.held.get_mut(self.length..end).ok_or(InputError::Message)?.copy_from_slice(taken);
            self.length = end;
            bytes = rest;
            let mut used = 0;
            loop {
                let pending = self.held.get(used..self.length).unwrap_or(&[]);
                match read(pending) {
                    Ok(Some((item, length))) => {
                        each(item);
                        used = used.saturating_add(length);
                    }
                    Ok(None) | Err(InputError::Wire(WireError::Incomplete)) => break,
                    Err(error) => return Err(error),
                }
            }
            self.held.copy_within(used..self.length, 0);
            self.length = self.length.saturating_sub(used);
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]
mod tests {
    use super::*;
    use crate::keys::code;
    use crate::modes::{ECHO_CAP, PAGE_CAP};
    use std::vec::Vec;

    fn bytes(item: &Item<'_>) -> Vec<u8> {
        let mut out = [0; MAX_MESSAGE];
        let length = write(item, &mut out).unwrap();
        out[..length].to_vec()
    }

    fn key(action: Action, key: KeyCode, mods: u8) -> Item<'static> {
        Item::Event(Event::Key(Key { action, key, mods, text: None, base: None }))
    }

    // REQ-INT-101.
    #[test]
    fn the_size_examples_of_section_5_8() {
        let functional = |name: &str| KeyCode::Functional(code(name).unwrap());
        assert_eq!(bytes(&Item::Text("a")), [0x61]);
        assert_eq!(bytes(&Item::Text("é")), [0xC3, 0xA9]);
        assert_eq!(bytes(&Item::Event(Event::FocusOut)), [0xFF, 0x06]);
        assert_eq!(bytes(&key(Action::Press, functional("Enter"), 0)), [0xFF, 0x01, 0x04, 0x02]);
        assert_eq!(bytes(&key(Action::Press, functional("ArrowUp"), 0)), [0xFF, 0x01, 0x04, 0x09]);
        assert_eq!(bytes(&key(Action::Press, functional("ColorF0Red"), 0)), [0xFF, 0x01, 0x04, 0x5C]);
        assert_eq!(bytes(&key(Action::Press, KeyCode::Character('c'), CONTROL)), [0xFF, 0x01, 0x08, 0x63, 0x04]);
        assert_eq!(bytes(&Item::Event(Event::Page(0x100))), [0xFF, 0x0A, 0x80, 0x02]);
        let at = Point { column: 10, row: 5, pixels: None, mods: 0 };
        assert_eq!(
            bytes(&Item::Event(Event::Mouse(Mouse { action: MouseAction::Press, button: 0, at }))),
            [0xFF, 0x02, 0x00, 0x00, 0x0A, 0x05]
        );
        assert_eq!(
            bytes(&Item::Event(Event::Wheel(Wheel { dx: 0, dy: 120, at }))),
            [0xFF, 0x03, 0x00, 0x00, 0xF0, 0x01, 0x0A, 0x05]
        );
    }

    // REQ-INT-103.
    #[test]
    fn every_event_round_trips() {
        let at = Point { column: 71, row: 24, pixels: Some((960, 1000)), mods: SHIFT | CAPS_LOCK };
        let events = [
            Event::Key(Key {
                action: Action::Release,
                key: KeyCode::Character('я'),
                mods: ALT,
                text: Some("Я"),
                base: Some('z'),
            }),
            Event::Key(Key {
                action: Action::Repeat,
                key: KeyCode::Functional(code("Dead").unwrap()),
                mods: 0,
                text: None,
                base: Some('\''),
            }),
            Event::Mouse(Mouse { action: MouseAction::Move, button: 0b101, at }),
            Event::Wheel(Wheel { dx: -240, dy: 1, at }),
            Event::Paste("line one\nline two\t\u{7}"),
            Event::FocusIn,
            Event::Resize { columns: 40, rows: 25, width: 480, height: 500 },
            Event::Compose { cursor: 2, text: "にほん" },
            Event::Compose { cursor: 0, text: "" },
            Event::Hello { version: 2, caps: PAGE_CAP | ECHO_CAP, agent: None },
            Event::Hello { version: 2, caps: AGENT, agent: Some("NeoTeletext/0.1 (macOS)") },
            Event::Page(0),
            Event::Page(0x8FF),
            Event::Extension { kind: 0x41, payload: &[1, 2, 3] },
        ];
        for event in events {
            let item = Item::Event(event);
            let encoded = bytes(&item);
            assert_eq!(read(&encoded), Ok(Some((item, encoded.len()))), "{event:?}");
        }
    }

    // REQ-INT-102.
    #[test]
    fn what_section_5_refuses_is_refused() {
        let refuse = |bytes: &[u8]| read(bytes).is_err();
        assert!(refuse(&[0x07]), "a control character as text");
        assert!(refuse(&[0xC0, 0x80]), "a byte UTF-8 never uses");
        assert!(refuse(&[0xF5]), "another");
        assert!(refuse(&[0xFF, 0x00]), "reserved type 0");
        assert!(refuse(&[0xFF, 0x0B]), "an undefined core type");
        assert!(refuse(&[0xFF, 0x80]), "reserved type 0x80");
        assert!(refuse(&[0xFF, 0x01, 0x40, 0x61]), "a reserved key flag");
        assert!(refuse(&[0xFF, 0x01, 0x03, 0x61]), "key action 3");
        assert!(refuse(&[0xFF, 0x01, 0x00, 0x0D]), "a control character as a key");
        assert!(refuse(&[0xFF, 0x01, 0x04, 0x38]), "a functional code not in use");
        assert!(refuse(&[0xFF, 0x01, 0x08, 0x61, 0x00]), "mods present and empty");
        assert!(refuse(&[0xFF, 0x02, 0x00, 0x00, 0x48, 0x00]), "column 72");
        assert!(refuse(&[0xFF, 0x03, 0x01, 0x00, 0x00, 0x00, 0x00]), "a wheel with an action");
        assert!(refuse(&[0xFF, 0x07, 0x00, 0x19, 0x00, 0x00]), "no columns");
        assert!(refuse(&[0xFF, 0x08, 0x05, 0x02, 0x61, 0x62]), "a cursor past the preedit");
        assert!(refuse(&[0xFF, 0x0A, 0x80, 0x01]), "page 0x080");
        let mut long = std::vec![0xFF, PASTE];
        long.extend_from_slice(&[0xA1, 0x1F]); // 4001
        long.extend(std::iter::repeat_n(b'a', 4001));
        assert!(refuse(&long), "a paste over its bound");
        assert_eq!(read(&[0xFF, 0x01]), Err(InputError::Wire(WireError::Incomplete)));
    }

    // REQ-INT-101.
    #[test]
    fn a_message_holds_whole_items_and_text_runs_to_the_next_event() {
        let mut message = Vec::new();
        message.extend_from_slice(b"*100#");
        message.extend_from_slice(&bytes(&key(Action::Press, KeyCode::Functional(code("Enter").unwrap()), 0)));
        message.extend_from_slice("ok é".as_bytes());
        let mut items = Vec::new();
        read_message(&message, &mut |item| items.push(std::format!("{item:?}"))).unwrap();
        assert_eq!(items.len(), 3);
        assert!(items[0].contains("*100#") && items[2].contains("ok é"));
        assert_eq!(read_message(&[0xFF, 0x01, 0x04], &mut |_| {}), Err(InputError::Message), "cut by the end");
        assert_eq!(read_message(&[b'a'; MAX_MESSAGE + 1], &mut |_| {}), Err(InputError::Message), "over the bound");
    }

    // REQ-INT-101.
    #[test]
    fn a_stream_split_anywhere_decodes_alike() {
        let mut stream = Vec::new();
        stream.extend_from_slice("héllo".as_bytes());
        stream.extend_from_slice(&bytes(&Item::Event(Event::Paste("pasted"))));
        stream.extend_from_slice(&bytes(&key(Action::Press, KeyCode::Character('x'), CONTROL)));
        for split in 0..stream.len() {
            let mut decoder = StreamDecoder::<MAX_STREAM_ITEM>::default();
            let mut text = std::string::String::new();
            let mut events = 0;
            let mut take = |item: Item<'_>| match item {
                Item::Text(run) => text.push_str(run),
                Item::Event(_) => events += 1,
            };
            decoder.push(&stream[..split], &mut take).unwrap();
            decoder.push(&stream[split..], &mut take).unwrap();
            assert_eq!((text.as_str(), events), ("héllo", 2), "split at {split}");
        }
    }

    // REQ-INT-110.
    #[test]
    fn the_input_count_counts_characters_and_events() {
        assert_eq!(units(&Item::Text("héllo")), 5);
        assert_eq!(units(&Item::Event(Event::Paste("a long paste"))), 1);
        assert_eq!(units(&Item::Event(Event::FocusIn)), 1);
    }
}
