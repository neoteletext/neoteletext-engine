//! The input control record (`neoteletext-common/INPUT.md` section 7.2,
//! REQ-INT-111): WELCOME, `PUSH_MODE`, `POP_MODE` and `PAGE_MODES`, a channel's
//! answer to HELLO and its requests for input modes. It is a record of the
//! audio profile's frame (REQ-AUD-003) of type [`TYPE_INPUT_CONTROL`], one
//! fragment on data channel 12 as the cursor record is (decision E.91),
//! whose body is one control message encoded as the input stream encodes
//! its events: `0xFF`, a type, its fields.

pub use neoteletext_audio::idl::FRAGMENT_DATA;
use neoteletext_audio::idl::{fragment, packet};
pub use neoteletext_audio::record::TYPE_INPUT_CONTROL;
use neoteletext_audio::record::{RecordError, framed_length, record_length, split, write};
use neoteletext_core::packet::Packet;

use crate::event::{EVENT, FIRST_EXTENSION, LAST_EXTENSION};
use crate::is_page;
use crate::modes::{ALL_CAPS, ALL_MODES, MODE_BITS};
use crate::wire::{Reader, WireError, Writer};

/// WELCOME's type.
pub const WELCOME: u8 = 0x01;
/// `PUSH_MODE`'s type.
pub const PUSH_MODE: u8 = 0x02;
/// `POP_MODE`'s type.
pub const POP_MODE: u8 = 0x03;
/// `PAGE_MODES`'s type.
pub const PAGE_MODES: u8 = 0x04;

/// The most a control message's body may take, so the record is one
/// fragment.
pub const MAX_BODY: usize = FRAGMENT_DATA - record_length(0);

/// A control message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control<'a> {
    /// The channel's answer to HELLO.
    Welcome {
        /// The version it speaks.
        version: u32,
        /// The capabilities it will use, a subset of the receiver's.
        caps: u32,
    },
    /// Push a mode set.
    PushMode(u32),
    /// Pop this many entries.
    PopMode(u32),
    /// A page's own modes while it is open; 0 clears them.
    PageModes {
        /// The page.
        page: u16,
        /// Its modes.
        modes: u32,
    },
    /// An extension, skipped by a receiver that does not know it.
    Extension {
        /// `0x40` to `0x7F`.
        kind: u8,
        /// Its payload.
        payload: &'a [u8],
    },
}

/// Why a control record was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlError {
    /// The frame: length, CRC or version (REQ-AUD-003).
    Frame(RecordError),
    /// A record of another type.
    NotControl(u8),
    /// A primitive was malformed.
    Wire(WireError),
    /// A type not defined.
    Type(u8),
    /// A field outside its range, and which.
    BadValue(&'static str),
}

impl From<WireError> for ControlError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}

/// Reads a control message's body.
///
/// # Errors
/// Anything section 7.2 refuses, or bytes left over.
pub fn read_body(body: &[u8]) -> Result<Control<'_>, ControlError> {
    let mut reader = Reader::new(body);
    if reader.byte()? != EVENT {
        return Err(ControlError::BadValue("no event byte"));
    }
    let kind = reader.byte()?;
    let control = match kind {
        WELCOME => {
            let version = reader.uv()?;
            let caps = reader.uv()?;
            if caps & !ALL_CAPS != 0 {
                return Err(ControlError::BadValue("caps"));
            }
            Control::Welcome { version, caps }
        }
        PUSH_MODE => {
            let modes = reader.uv()?;
            if modes & !MODE_BITS != 0 || modes & !ALL_MODES != 0 {
                return Err(ControlError::BadValue("modes"));
            }
            Control::PushMode(modes)
        }
        POP_MODE => Control::PopMode(reader.uv()?),
        PAGE_MODES => {
            let page = u16::try_from(reader.uv()?).map_err(|_| ControlError::BadValue("page"))?;
            if !is_page(page) {
                return Err(ControlError::BadValue("page"));
            }
            let modes = reader.uv()?;
            if modes & !ALL_MODES != 0 {
                return Err(ControlError::BadValue("modes"));
            }
            Control::PageModes { page, modes }
        }
        FIRST_EXTENSION..=LAST_EXTENSION => {
            let length = usize::try_from(reader.uv()?).map_err(|_| ControlError::BadValue("length"))?;
            Control::Extension { kind, payload: reader.take(length)? }
        }
        other => return Err(ControlError::Type(other)),
    };
    if reader.used() != body.len() {
        return Err(ControlError::BadValue("bytes after the message"));
    }
    Ok(control)
}

/// Writes a control message's body into `out`, checked by the reader.
///
/// # Errors
/// When a field is outside its range or `out` is too small.
pub fn write_body(control: &Control<'_>, out: &mut [u8]) -> Result<usize, ControlError> {
    let mut writer = Writer::new(out);
    writer.byte(EVENT)?;
    match control {
        Control::Welcome { version, caps } => {
            writer.byte(WELCOME)?;
            writer.uv(*version)?;
            writer.uv(*caps)?;
        }
        Control::PushMode(modes) => {
            writer.byte(PUSH_MODE)?;
            writer.uv(*modes)?;
        }
        Control::PopMode(count) => {
            writer.byte(POP_MODE)?;
            writer.uv(*count)?;
        }
        Control::PageModes { page, modes } => {
            writer.byte(PAGE_MODES)?;
            writer.uv(u32::from(*page))?;
            writer.uv(*modes)?;
        }
        Control::Extension { kind, payload } => {
            writer.byte(*kind)?;
            writer.uv(u32::try_from(payload.len()).map_err(|_| ControlError::BadValue("length"))?)?;
            writer.bytes(payload)?;
        }
    }
    let length = writer.len();
    if length > MAX_BODY {
        return Err(ControlError::BadValue("longer than one fragment"));
    }
    match read_body(out.get(..length).unwrap_or(&[])) {
        Ok(back) if back == *control => Ok(length),
        Ok(_) => Err(ControlError::BadValue("does not read back")),
        Err(error) => Err(error),
    }
}

/// The 4/31 packet carrying `control` as record `record_id` of user-defined
/// audio service `service`.
///
/// # Errors
/// As [`write_body`].
pub fn packet_of(control: &Control<'_>, service: u8, record_id: u8) -> Result<Packet, ControlError> {
    let mut body = [0_u8; MAX_BODY];
    let length = write_body(control, &mut body)?;
    let mut record = [0_u8; FRAGMENT_DATA];
    let written = write(TYPE_INPUT_CONTROL, body.get(..length).unwrap_or(&[]), &mut record)
        .ok_or(ControlError::BadValue("record"))?;
    Ok(packet(service, record_id, 0, true, record.get(..written).unwrap_or(&[])))
}

/// The control message `record` holds, frame and CRC included.
///
/// # Errors
/// A frame that fails REQ-AUD-003, a record of another type, or anything
/// section 7.2 refuses.
pub fn read(record: &[u8]) -> Result<Control<'_>, ControlError> {
    let (kind, body) = split(record).map_err(ControlError::Frame)?;
    if kind != TYPE_INPUT_CONTROL {
        return Err(ControlError::NotControl(kind));
    }
    read_body(body)
}

/// The control message `packet` carries, read from a copy in `buffer`, or
/// `None` when it is not a one-fragment record of type
/// [`TYPE_INPUT_CONTROL`] on the audio profile's data channel.
#[must_use]
pub fn from_packet<'b>(
    packet: &Packet,
    buffer: &'b mut [u8; FRAGMENT_DATA],
) -> Option<Result<Control<'b>, ControlError>> {
    let fragment = fragment(packet)?.ok()?;
    if !(fragment.first && fragment.last && fragment.data.get(1) == Some(&TYPE_INPUT_CONTROL)) {
        return None;
    }
    *buffer = fragment.data;
    let length = framed_length(buffer).ok()?;
    Some(read(buffer.get(..length).unwrap_or(&[])))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use super::*;
    use crate::modes::{MOUSE_BUTTONS, PAGE_CAP};

    // REQ-INT-111 and REQ-INT-114.
    #[test]
    fn every_control_message_round_trips_through_a_packet() {
        for control in [
            Control::Welcome { version: 2, caps: PAGE_CAP | MOUSE_BUTTONS },
            Control::PushMode(MOUSE_BUTTONS),
            Control::PopMode(1),
            Control::PageModes { page: 0x300, modes: MOUSE_BUTTONS },
            Control::PageModes { page: 0x300, modes: 0 },
            Control::Extension { kind: 0x40, payload: &[9, 9] },
        ] {
            let packet = packet_of(&control, 3, 77).unwrap();
            let mut buffer = [0; FRAGMENT_DATA];
            assert_eq!(from_packet(&packet, &mut buffer), Some(Ok(control)), "{control:?}");
            assert!(crate::cursor::from_packet(&packet).is_none(), "not a cursor");
        }
    }

    // REQ-INT-111.
    #[test]
    fn what_section_7_2_refuses_is_refused() {
        assert_eq!(read_body(&[0xFF, 0x05]), Err(ControlError::Type(5)));
        assert!(read_body(&[0xFF, PUSH_MODE, 0x80, 0x02]).is_err(), "a mode bit not defined");
        assert!(read_body(&[0xFF, PAGE_MODES, 0x80, 0x01, 0x08]).is_err(), "page 0x080");
        assert!(read_body(&[0xFF, POP_MODE, 0x01, 0x00]).is_err(), "bytes after");
        assert!(read_body(&[0x01, WELCOME, 2, 0]).is_err(), "no event byte");
    }
}
