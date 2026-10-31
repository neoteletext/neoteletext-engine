//! The input control record (`neoteletext-common/INPUT.md` section 7.2,
//! REQ-INT-111): WELCOME, a channel's answer to HELLO. It is a record of the
//! audio profile's frame (REQ-AUD-003) of type [`TYPE_INPUT_CONTROL`], one
//! fragment on data channel 12 as the cursor record is (decision E.91),
//! whose body is one control message encoded as the input stream encodes
//! its events: `0xFF`, a type, its fields.

pub use neoteletext_audio::idl::FRAGMENT_DATA;
use neoteletext_audio::idl::packet;
pub use neoteletext_audio::record::TYPE_INPUT_CONTROL;
use neoteletext_audio::record::{RecordError, framed_length, record_length, split, write};
use neoteletext_core::packet::Packet;

use crate::TOKEN_BYTES;
use crate::caps::{ALL_CAPS, RESUME_CAP, RETIRED_ECHO_BIT, RETIRED_MODE_BITS};
use crate::carriage::input_record;
use crate::event::{EVENT, FIRST_EXTENSION, LAST_EXTENSION};
use crate::wire::{Reader, WireError, Writer};

/// WELCOME's type.
pub const WELCOME: u8 = 0x01;
/// The first of the withdrawn types, once `PUSH_MODE`, `POP_MODE` and
/// `PAGE_MODES` (decision E.125): reserved, and a record of one is read as
/// [`Control::Withdrawn`] and ignored, so an older channel still works.
pub const FIRST_WITHDRAWN: u8 = 0x02;
/// The last withdrawn type.
pub const LAST_WITHDRAWN: u8 = 0x04;

/// The most a control message's body may take, so the record is one
/// fragment.
pub const MAX_BODY: usize = FRAGMENT_DATA - record_length(0);

/// WELCOME's session state byte, with RESUME: a new session.
const SESSION_NEW: u8 = 0;
/// WELCOME's session state byte, with RESUME: the session asked for is
/// resumed, and the input count it had taken follows.
const SESSION_RESUMED: u8 = 1;

/// What WELCOME says of the session when both sides set RESUME (INPUT.md
/// section 6.4, REQ-RCH-013).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Resumption {
    /// The token for the next reconnection, new in every WELCOME.
    pub token: [u8; TOKEN_BYTES],
    /// For a resumed session, the input count the channel had taken of it
    /// when its connection was lost, modulo 2¹⁶: the receiver sends again,
    /// in order, what it sent after that. `None` for a new session.
    pub taken: Option<u16>,
}

/// A control message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control<'a> {
    /// The channel's answer to HELLO.
    Welcome {
        /// The version it speaks.
        version: u32,
        /// The capabilities it will use, a subset of the receiver's.
        caps: u32,
        /// The session's token and state, present exactly when `caps` has
        /// [`RESUME_CAP`].
        resume: Option<Resumption>,
    },
    /// A withdrawn type, `0x02` to `0x04`: ignored, and never written.
    Withdrawn(u8),
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
/// Anything section 7.2 refuses: a body longer than [`MAX_BODY`], which
/// would not be one fragment, or bytes left over.
pub fn read_body(body: &[u8]) -> Result<Control<'_>, ControlError> {
    if body.len() > MAX_BODY {
        return Err(ControlError::BadValue("longer than one fragment"));
    }
    let mut reader = Reader::new(body);
    if reader.byte()? != EVENT {
        return Err(ControlError::BadValue("no event byte"));
    }
    let kind = reader.byte()?;
    let control = match kind {
        WELCOME => {
            let version = reader.uv()?;
            // An older channel may still agree the input modes' bits and
            // echo's.
            let caps = reader.uv()? & !(RETIRED_MODE_BITS | RETIRED_ECHO_BIT);
            if caps & !ALL_CAPS != 0 {
                return Err(ControlError::BadValue("caps"));
            }
            let resume = if caps & RESUME_CAP == 0 { None } else { Some(read_resumption(&mut reader)?) };
            Control::Welcome { version, caps, resume }
        }
        // Its fields are not read: the record is ignored whatever it holds.
        FIRST_WITHDRAWN..=LAST_WITHDRAWN => return Ok(Control::Withdrawn(kind)),
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

/// WELCOME's session, with RESUME: the state, the token and, for a resumed
/// session, the count taken, big-endian.
fn read_resumption(reader: &mut Reader<'_>) -> Result<Resumption, ControlError> {
    let state = reader.byte()?;
    let mut token = [0_u8; TOKEN_BYTES];
    for byte in &mut token {
        *byte = reader.byte()?;
    }
    let count = match state {
        SESSION_NEW => None,
        SESSION_RESUMED => Some(u16::from_be_bytes([reader.byte()?, reader.byte()?])),
        _ => return Err(ControlError::BadValue("session state")),
    };
    Ok(Resumption { token, taken: count })
}

/// Writes a control message's body into `out`, checked by the reader.
///
/// # Errors
/// When a field is outside its range or `out` is too small.
pub fn write_body(control: &Control<'_>, out: &mut [u8]) -> Result<usize, ControlError> {
    let mut writer = Writer::new(out);
    writer.byte(EVENT)?;
    match control {
        Control::Welcome { version, caps, resume } => {
            if resume.is_some() != (caps & RESUME_CAP != 0) {
                return Err(ControlError::BadValue("resume without its capability"));
            }
            writer.byte(WELCOME)?;
            writer.uv(*version)?;
            writer.uv(*caps)?;
            if let Some(resume) = resume {
                writer.byte(if resume.taken.is_some() { SESSION_RESUMED } else { SESSION_NEW })?;
                writer.bytes(&resume.token)?;
                if let Some(taken) = resume.taken {
                    writer.bytes(&taken.to_be_bytes())?;
                }
            }
        }
        Control::Withdrawn(_) => return Err(ControlError::BadValue("a withdrawn type")),
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
    let fragment = input_record(packet, |kind| kind == TYPE_INPUT_CONTROL)?;
    *buffer = fragment.data;
    let length = framed_length(buffer).ok()?;
    Some(read(buffer.get(..length).unwrap_or(&[])))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
    use super::*;
    use crate::caps::CURSOR_CAP;

    // REQ-INT-111.
    #[test]
    fn every_control_message_round_trips_through_a_packet() {
        for control in [
            Control::Welcome { version: 3, caps: CURSOR_CAP, resume: None },
            Control::Extension { kind: 0x40, payload: &[9, 9] },
        ] {
            let packet = packet_of(&control, 3, 77).unwrap();
            let mut buffer = [0; FRAGMENT_DATA];
            assert_eq!(from_packet(&packet, &mut buffer), Some(Ok(control)), "{control:?}");
            assert!(crate::cursor::from_packet(&packet).is_none(), "not a cursor");
        }
    }

    // REQ-INT-111, decision E.125: an older channel's mode records and mode
    // bits are ignored, never refused, and never written.
    #[test]
    fn the_withdrawn_mode_records_and_bits_are_ignored() {
        assert_eq!(read_body(&[0xFF, 0x02, 0x08]), Ok(Control::Withdrawn(0x02)), "PUSH_MODE");
        assert_eq!(read_body(&[0xFF, 0x03, 0x01]), Ok(Control::Withdrawn(0x03)), "POP_MODE");
        assert_eq!(read_body(&[0xFF, 0x04, 0x80, 0x06, 0x08]), Ok(Control::Withdrawn(0x04)), "PAGE_MODES");
        // WELCOME agreeing PAGE and the old MOUSE_BUTTONS (8): the mode bit is dropped.
        assert_eq!(
            read_body(&[0xFF, WELCOME, 3, 0x88, 0x80, 0x08]),
            Ok(Control::Welcome { version: 3, caps: CURSOR_CAP, resume: None })
        );
        assert!(write_body(&Control::Withdrawn(0x02), &mut [0; MAX_BODY]).is_err());
    }

    // Decision E.134: an older channel agreeing echo (bit 18) is read, the
    // bit dropped.
    #[test]
    fn the_withdrawn_echo_bit_is_ignored() {
        assert_eq!(
            read_body(&[0xFF, WELCOME, 3, 0x80, 0x80, 0x18]),
            Ok(Control::Welcome { version: 3, caps: CURSOR_CAP, resume: None })
        );
    }

    // REQ-INT-111.
    #[test]
    fn what_section_7_2_refuses_is_refused() {
        assert_eq!(read_body(&[0xFF, 0x05]), Err(ControlError::Type(5)));
        assert!(read_body(&[0xFF, WELCOME, 3, 0x80, 0x80, 0x80, 0x01]).is_err(), "a capability not defined");
        // REQ-1556: bit 65,536 was PAGE's capability, a protocol error now.
        assert!(read_body(&[0xFF, WELCOME, 3, 0x80, 0x80, 0x04]).is_err(), "PAGE's capability");
        assert!(read_body(&[0xFF, WELCOME, 2, 0, 0]).is_err(), "bytes after");
        assert!(read_body(&[0x01, WELCOME, 2, 0]).is_err(), "no event byte");
    }

    /// REQ-INT-111, REQ-INT-125, REQ-RCH-013, decision E.133: with RESUME, WELCOME carries the
    /// session's state, its token and, resumed, the count taken; the
    /// longest, every capability agreed, is still one fragment.
    #[test]
    fn welcome_carries_the_session_with_resume() {
        let token = [7_u8; TOKEN_BYTES];
        for taken in [None, Some(0xFFFF), Some(1)] {
            let welcome = Control::Welcome { version: 3, caps: ALL_CAPS, resume: Some(Resumption { token, taken }) };
            let mut out = [0_u8; MAX_BODY];
            let length = write_body(&welcome, &mut out).unwrap();
            assert_eq!(read_body(&out[..length]), Ok(welcome));
            let packet = packet_of(&welcome, 3, 1).unwrap();
            let mut buffer = [0; FRAGMENT_DATA];
            assert_eq!(from_packet(&packet, &mut buffer), Some(Ok(welcome)), "one fragment");
        }
        // Resumed from 0x1234: state 1, the token, then the count, big-endian.
        let welcome =
            Control::Welcome { version: 3, caps: RESUME_CAP, resume: Some(Resumption { token, taken: Some(0x1234) }) };
        let mut out = [0_u8; MAX_BODY];
        let length = write_body(&welcome, &mut out).unwrap();
        assert_eq!(&out[..7], &[0xFF, WELCOME, 3, 0x80, 0x80, 0x80, 0x01]);
        assert_eq!(out[7], SESSION_RESUMED);
        assert_eq!(&out[8..24], &token);
        assert_eq!(&out[24..length], &[0x12, 0x34]);
        // A state that is neither, a session missing, or one without RESUME.
        let mut bad = out;
        bad[7] = 2;
        assert_eq!(read_body(&bad[..length]), Err(ControlError::BadValue("session state")));
        assert!(read_body(&out[..7]).is_err(), "RESUME agreed and no session");
        let without =
            Control::Welcome { version: 3, caps: CURSOR_CAP, resume: Some(Resumption { token, taken: None }) };
        assert!(write_body(&without, &mut out).is_err(), "a session without RESUME is never written");
    }

    /// The fuzz target `control_records` found an extension of 78 bytes
    /// read but then not written: a body is at most one fragment's.
    #[test]
    fn a_body_longer_than_a_fragment_is_refused() {
        let mut long = std::vec![0xFF, 0x4E, 0x4E];
        long.extend(core::iter::repeat_n(0x4E, 0x4E));
        assert_eq!(read_body(&long), Err(ControlError::BadValue("longer than one fragment")));
        let mut fits = std::vec![0xFF, 0x40, u8::try_from(MAX_BODY - 3).unwrap()];
        fits.extend(core::iter::repeat_n(0, MAX_BODY - 3));
        let read = read_body(&fits).unwrap();
        let mut out = [0_u8; MAX_BODY];
        assert_eq!(write_body(&read, &mut out), Ok(MAX_BODY), "the longest body reads and writes back");
    }
}
