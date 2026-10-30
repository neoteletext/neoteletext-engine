//! Encoding the input stream: one item, read back before it is sent.

use super::decode::read;
use super::{
    Action, COMPOSE, DEVICE_SHIFT, Device, EVENT, Event, FIRST_EXTENSION, FOCUS_IN, FOCUS_OUT, FUNCTIONAL, HAS_BASE,
    HAS_MODS, HAS_PIXELS, HAS_TEXT, HELLO, HOVER, HOVER_LEAVE, InputError, Item, KEY, Key, KeyCode, LAST_EXTENSION,
    MOUSE, MouseAction, PAGE, PASTE, Point, RESIZE, RESUME, SELECT, TOUCH, Touch, WHEEL, check_text,
};
use crate::caps::AGENT;
use crate::wire::{MAX_VARINT, ONE_BYTE_MAX, Writer};
use crate::{MAX_AGENT, MAX_COLUMNS, MAX_COMPOSE, MAX_KEY_TEXT, MAX_PASTE, MAX_ROWS, TOKEN_BYTES};

/// HOVER's payload at its longest: the flags and two varints.
const HOVER_PAYLOAD: usize = 1 + 2 * MAX_VARINT;
/// TOUCH's payload at its longest: the flags, the id and four varints.
pub(super) const TOUCH_PAYLOAD: usize = 2 + 4 * MAX_VARINT;
/// SELECT's payload at its longest: two varints.
const SELECT_PAYLOAD: usize = 2 * MAX_VARINT;
/// RESUME's payload: the token and the acknowledgement, big-endian.
pub(super) const RESUME_PAYLOAD: usize = TOKEN_BYTES + 2;
/// The bytes of a cell's column or row: every one is under 128, one
/// varint byte.
const CELL_BYTES: usize = 1;
const _: () = assert!(MAX_COLUMNS <= ONE_BYTE_MAX + 1 && MAX_ROWS <= ONE_BYTE_MAX + 1);
/// The most bytes a pointer's move takes as an event (REQ-RCH-035), what a
/// holder of one move needs: a MOUSE move, its event byte, type, flags and
/// button, the cell, the dot's two varints and the modifiers; or a TOUCH,
/// its event byte, type and one-byte length, its flags and id, the cell
/// and the dot; whichever is longer.
pub const MAX_MOVE_BYTES: usize = {
    let mouse = 4 + 2 * CELL_BYTES + 2 * MAX_VARINT + 1;
    let touch = 3 + 2 + 2 * CELL_BYTES + 2 * MAX_VARINT;
    if mouse > touch { mouse } else { touch }
};
// TOUCH's length is one varint byte.
const _: () = assert!(TOUCH_PAYLOAD <= ONE_BYTE_MAX as usize);

/// The code of a pointer's action (section 5.4).
const fn action_code(action: MouseAction) -> u8 {
    match action {
        MouseAction::Press => 0,
        MouseAction::Release => 1,
        MouseAction::Move => 2,
        MouseAction::Gone => 3,
    }
}

/// The code of MOUSE's device, bits 4 and 5 (REQ-RCH-042).
const fn device_code(device: Device) -> u8 {
    match device {
        Device::Mouse => 0,
        Device::Touch => 1,
        Device::Pen => 2,
    }
}

/// Writes an extension: its type, its payload's length, then the payload
/// `fill` writes, so the length can go before it.
fn write_extension<const N: usize>(
    kind: u8,
    writer: &mut Writer<'_>,
    fill: impl FnOnce(&mut Writer<'_>) -> Result<(), InputError>,
) -> Result<(), InputError> {
    let mut payload = [0_u8; N];
    let mut inner = Writer::new(&mut payload);
    fill(&mut inner)?;
    let length = inner.len();
    writer.byte(kind)?;
    writer.uv(u32::try_from(length).map_err(|_| InputError::Message)?)?;
    writer.bytes(payload.get(..length).unwrap_or(&[]))?;
    Ok(())
}

fn write_touch(touch: &Touch, writer: &mut Writer<'_>) -> Result<(), InputError> {
    write_extension::<TOUCH_PAYLOAD>(TOUCH, writer, |inner| {
        let pixels = if touch.pixels.is_some() { HAS_PIXELS } else { 0 };
        inner.bytes(&[action_code(touch.action) | pixels, touch.id])?;
        inner.uv(touch.column)?;
        inner.uv(touch.row)?;
        if let Some((x, y)) = touch.pixels {
            inner.uv(x)?;
            inner.uv(y)?;
        }
        Ok(())
    })
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

/// A key event: its flags, its code, then the modifiers, text and base
/// key it carries.
fn write_key(key: &Key<'_>, writer: &mut Writer<'_>) -> Result<(), InputError> {
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
    Ok(())
}

fn write_event(event: &Event<'_>, writer: &mut Writer<'_>) -> Result<(), InputError> {
    match event {
        Event::Key(key) => write_key(key, writer)?,
        Event::Mouse(mouse) => {
            let flags =
                action_code(mouse.action) | point_flags(&mouse.at) | (device_code(mouse.device) << DEVICE_SHIFT);
            writer.bytes(&[MOUSE, flags, mouse.button])?;
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
        Event::Hover { leave, column, row } => write_extension::<HOVER_PAYLOAD>(HOVER, writer, |inner| {
            inner.byte(if *leave { HOVER_LEAVE } else { 0 })?;
            inner.uv(*column)?;
            inner.uv(*row)?;
            Ok(())
        })?,
        Event::Touch(touch) => write_touch(touch, writer)?,
        Event::Resume(resume) => write_extension::<RESUME_PAYLOAD>(RESUME, writer, |inner| {
            inner.bytes(&resume.token)?;
            inner.bytes(&resume.acked.to_be_bytes())?;
            Ok(())
        })?,
        Event::Select(select) => write_extension::<SELECT_PAYLOAD>(SELECT, writer, |inner| {
            inner.uv(u32::from(select.page))?;
            inner.uv(u32::from(select.sub))?;
            Ok(())
        })?,
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
