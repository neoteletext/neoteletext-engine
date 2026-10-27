//! Decoding the input stream: one item from bytes that may end early,
//! one whole message, and a byte stream split anywhere (section 5.1).

use super::{
    ACTION, Action, COMPOSE, CORE_LAST_TYPE, EVENT, Event, FIRST_EXTENSION, FOCUS_IN, FOCUS_OUT, FUNCTIONAL, HAS_BASE,
    HAS_MODS, HAS_PIXELS, HAS_TEXT, HELLO, HOVER, HOVER_LEAVE, InputError, Item, KEY, KEY_RESERVED, Key, KeyCode,
    LAST_CORE, LAST_EXTENSION, MOUSE, Mouse, MouseAction, NEVER_UTF8, NEVER_UTF8_FROM, PAGE, PASTE, POINTER_RESERVED,
    Point, RESIZE, WHEEL, Wheel, check_text, printable,
};
use crate::caps::AGENT;
use crate::keys::is_code;
use crate::wire::{Reader, WireError};
use crate::{MAX_AGENT, MAX_COLUMNS, MAX_COMPOSE, MAX_KEY_TEXT, MAX_MESSAGE, MAX_PASTE, MAX_ROWS, is_page};

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
            let payload = reader.take(length)?;
            if kind == HOVER { read_hover(payload)? } else { Event::Extension { kind, payload } }
        }
        other if other > LAST_CORE && other <= CORE_LAST_TYPE => return Err(InputError::Type(other)),
        other => return Err(InputError::Type(other)),
    })
}

/// HOVER's payload: `flags` u8, `col` uv and `row` uv, the control's first
/// cell, and nothing after.
fn read_hover(payload: &[u8]) -> Result<Event<'static>, InputError> {
    let mut reader = Reader::new(payload);
    let flags = reader.byte()?;
    if flags & !HOVER_LEAVE != 0 {
        return Err(InputError::BadValue("hover flags"));
    }
    let (column, row) = (reader.uv()?, reader.uv()?);
    if column >= MAX_COLUMNS || row >= MAX_ROWS || reader.used() != payload.len() {
        return Err(InputError::BadValue("hover"));
    }
    Ok(Event::Hover { leave: flags & HOVER_LEAVE != 0, column, row })
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
