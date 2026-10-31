//! Decoding the input stream: one item from bytes that may end early,
//! one whole message, and a byte stream split anywhere (section 5.1).

use super::LOST;
use super::encode::RESUME_PAYLOAD;
use super::{
    ACTION, Action, COMPOSE, CORE_LAST_TYPE, DEVICE_MASK, DEVICE_SHIFT, Device, EVENT, Event, FIRST_EXTENSION,
    FOCUS_IN, FOCUS_OUT, FUNCTIONAL, HAS_BASE, HAS_MODS, HAS_PIXELS, HAS_TEXT, HELLO, HOVER, HOVER_LEAVE, InputError,
    Item, KEY, KEY_RESERVED, Key, KeyCode, LAST_CORE, LAST_EXTENSION, MOUSE, MOUSE_RESERVED, Mouse, MouseAction,
    NEVER_UTF8, NEVER_UTF8_FROM, PASTE, Point, RESIZE, RESUME, Resume, SELECT, Select, Stamp, TOUCH, TOUCH_RESERVED,
    Touch, WHEEL, WHEEL_RESERVED, Wheel, check_text, is_page_parameters, printable,
};
use crate::caps::{AGENT, RETIRED_PAGE_BIT};
use crate::keys::is_code;
use crate::wire::{Reader, WireError};
use crate::{
    MAX_AGENT, MAX_COLUMNS, MAX_COMPOSE, MAX_KEY_TEXT, MAX_MESSAGE, MAX_PASTE, MAX_ROWS, MAX_TOUCH_ID, TOKEN_BYTES,
};

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

/// The stamp every input event begins with (section 5.4, REQ-1555).
fn read_stamp(reader: &mut Reader<'_>) -> Result<Stamp, InputError> {
    let narrow = |value: u32| u16::try_from(value).map_err(|_| InputError::BadValue("stamp"));
    let (page, sub) = (narrow(reader.uv()?)?, narrow(reader.uv()?)?);
    Stamp::new(page, sub).ok_or(InputError::BadValue("stamp"))
}

/// A pointer's cell, its column and row, and its dot when known.
type Cell = (u32, u32, Option<(u32, u32)>);

/// A cell, `col` and `row`, and the dot when `flags` says it follows.
fn read_cell(reader: &mut Reader<'_>, flags: u8) -> Result<Cell, InputError> {
    let column = reader.uv()?;
    let row = reader.uv()?;
    if column >= MAX_COLUMNS || row >= MAX_ROWS {
        return Err(InputError::BadValue("cell"));
    }
    let pixels = if flags & HAS_PIXELS == 0 { None } else { Some((reader.uv()?, reader.uv()?)) };
    Ok((column, row, pixels))
}

fn read_point(reader: &mut Reader<'_>, flags: u8) -> Result<Point, InputError> {
    let (column, row, pixels) = read_cell(reader, flags)?;
    let mods = if flags & HAS_MODS == 0 { 0 } else { reader.byte()? };
    Ok(Point { column, row, pixels, mods })
}

/// A pointer's action, MOUSE's or TOUCH's: all four codes are defined.
const fn read_action(flags: u8) -> MouseAction {
    match flags & ACTION {
        0 => MouseAction::Press,
        1 => MouseAction::Release,
        2 => MouseAction::Move,
        _ => MouseAction::Gone,
    }
}

fn read_event<'a>(reader: &mut Reader<'a>) -> Result<Event<'a>, InputError> {
    let kind = reader.byte()?;
    Ok(match kind {
        KEY => Event::Key(read_key(reader)?),
        MOUSE => {
            let on = read_stamp(reader)?;
            let flags = reader.byte()?;
            if flags & MOUSE_RESERVED != 0 {
                return Err(InputError::Flags);
            }
            // REQ-RCH-042: device 3 is reserved, and a protocol error.
            let device = match (flags & DEVICE_MASK) >> DEVICE_SHIFT {
                0 => Device::Mouse,
                1 => Device::Touch,
                2 => Device::Pen,
                _ => return Err(InputError::BadValue("mouse device")),
            };
            let button = reader.byte()?;
            Event::Mouse(Mouse { on, action: read_action(flags), button, at: read_point(reader, flags)?, device })
        }
        WHEEL => {
            let on = read_stamp(reader)?;
            let flags = reader.byte()?;
            if flags & (WHEEL_RESERVED | ACTION) != 0 {
                return Err(InputError::Flags);
            }
            let dx = reader.sv()?;
            let dy = reader.sv()?;
            Event::Wheel(Wheel { on, dx, dy, at: read_point(reader, flags)? })
        }
        PASTE => {
            let on = read_stamp(reader)?;
            Event::Paste { on, text: reader.str(MAX_PASTE)? }
        }
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
            let on = read_stamp(reader)?;
            let cursor = reader.uv()?;
            let text = reader.str(MAX_COMPOSE)?;
            check_text(text)?;
            if usize::try_from(cursor).map_or(true, |cursor| cursor > text.chars().count()) {
                return Err(InputError::BadValue("compose cursor"));
            }
            Event::Compose { on, cursor, text }
        }
        HELLO => {
            let version = reader.uv()?;
            let caps = reader.uv()?;
            if caps & RETIRED_PAGE_BIT != 0 {
                return Err(InputError::BadValue("caps"));
            }
            let agent = if caps & AGENT == 0 { None } else { Some(reader.str(MAX_AGENT)?) };
            if agent.is_some_and(|agent| !agent.bytes().all(|byte| byte.is_ascii_graphic() || byte == b' ')) {
                return Err(InputError::BadValue("agent"));
            }
            Event::Hello { version, caps, agent }
        }
        FIRST_EXTENSION..=LAST_EXTENSION => {
            let length = usize::try_from(reader.uv()?).map_err(|_| InputError::Message)?;
            if length > MAX_MESSAGE {
                return Err(InputError::Message);
            }
            let payload = reader.take(length)?;
            // A payload is whole once taken: a field it cuts short is a
            // fault of the payload, never a wait for more of the stream.
            let whole = |error| match error {
                InputError::Wire(WireError::Incomplete) => InputError::BadValue("extension cut short"),
                other => other,
            };
            match kind {
                HOVER => read_hover(payload).map_err(whole)?,
                TOUCH => read_touch(payload).map_err(whole)?,
                RESUME => read_resume(payload).map_err(whole)?,
                SELECT => read_select(payload).map_err(whole)?,
                _ => Event::Extension { kind, payload },
            }
        }
        other if other > LAST_CORE && other <= CORE_LAST_TYPE => return Err(InputError::Type(other)),
        other => return Err(InputError::Type(other)),
    })
}

/// HOVER's payload: the stamp, `flags` u8, `col` uv and `row` uv, the
/// control's first cell, and nothing after.
fn read_hover(payload: &[u8]) -> Result<Event<'static>, InputError> {
    let mut reader = Reader::new(payload);
    let on = read_stamp(&mut reader)?;
    let flags = reader.byte()?;
    if flags & !HOVER_LEAVE != 0 {
        return Err(InputError::BadValue("hover flags"));
    }
    let (column, row) = (reader.uv()?, reader.uv()?);
    if column >= MAX_COLUMNS || row >= MAX_ROWS || reader.used() != payload.len() {
        return Err(InputError::BadValue("hover"));
    }
    Ok(Event::Hover { on, leave: flags & HOVER_LEAVE != 0, column, row })
}

/// TOUCH's payload (REQ-RCH-043): the stamp, `flags` u8, `id` u8 of 1 to
/// [`MAX_TOUCH_ID`], `col` uv, `row` uv, the dot when flagged, and nothing
/// after.
fn read_touch(payload: &[u8]) -> Result<Event<'static>, InputError> {
    let mut reader = Reader::new(payload);
    let on = read_stamp(&mut reader)?;
    let flags = reader.byte()?;
    if flags & TOUCH_RESERVED != 0 {
        return Err(InputError::BadValue("touch flags"));
    }
    let id = reader.byte()?;
    if !(1..=MAX_TOUCH_ID).contains(&id) {
        return Err(InputError::BadValue("touch id"));
    }
    let (column, row, pixels) = read_cell(&mut reader, flags)?;
    if reader.used() != payload.len() {
        return Err(InputError::BadValue("touch"));
    }
    Ok(Event::Touch(Touch { on, action: read_action(flags), id, column, row, pixels }))
}

/// SELECT's payload (REQ-INT-140, DEC-1527, DEC-1715): `page` uv and `sub`
/// uv, then the page parameters to its end; every bound of [`Select::new`]
/// and of [`is_page_parameters`] a protocol error.
fn read_select(payload: &[u8]) -> Result<Event<'_>, InputError> {
    let refused = InputError::BadValue("select");
    let mut reader = Reader::new(payload);
    let (page, sub) = (reader.uv()?, reader.uv()?);
    let narrow = |value: u32| u16::try_from(value).map_err(|_| refused);
    let select = Select::new(narrow(page)?, narrow(sub)?).ok_or(refused)?;
    let parameters = payload.get(reader.used()..).unwrap_or(&[]);
    let parameters = core::str::from_utf8(parameters).map_err(|_| refused)?;
    if is_page_parameters(select, parameters) { Ok(Event::Select(select, parameters)) } else { Err(refused) }
}

/// RESUME's payload (REQ-RCH-013): the token and the acknowledgement, big-endian, exactly.
fn read_resume(payload: &[u8]) -> Result<Event<'static>, InputError> {
    if payload.len() != RESUME_PAYLOAD {
        return Err(InputError::BadValue("resume"));
    }
    let (token, acked) = payload.split_at_checked(TOKEN_BYTES).ok_or(InputError::BadValue("resume"))?;
    let token: [u8; TOKEN_BYTES] = token.try_into().map_err(|_| InputError::BadValue("resume"))?;
    let acked: [u8; 2] = acked.try_into().map_err(|_| InputError::BadValue("resume"))?;
    Ok(Event::Resume(Resume { token, acked: u16::from_be_bytes(acked) }))
}

fn read_key<'a>(reader: &mut Reader<'a>) -> Result<Key<'a>, InputError> {
    let on = read_stamp(reader)?;
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
    let lost = flags & LOST != 0;
    if lost && action != Action::Release {
        return Err(InputError::BadValue("lost, and not a release"));
    }
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
    Ok(Key { on, action, key, mods, text, base, lost })
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

/// The most bytes one item takes: a message, which a SELECT with the longest
/// page parameters can fill (DEC-1715).
pub const MAX_STREAM_ITEM: usize = MAX_MESSAGE;

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
