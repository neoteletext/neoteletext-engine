//! Records: the one form every carriage delivers (REQ-AUD-003 to
//! REQ-AUD-006). Integers are big-endian; a record is at most
//! [`MAX_RECORD`] bytes and is checked against every bound before anything
//! in it is used.

use crate::crc::crc32_parts;
use crate::{
    CHANNELS, Codec, DUCK_PERCENT, MAX_BODY, MAX_FADE_MS, MAX_GAIN, MAX_RECORD, PROFILE_VERSION, RECORD_CRC,
    RECORD_HEADER, SERVICE_SCOPE, sample_rate,
};

/// Record type: announce a sound.
pub const TYPE_OBJECT: u8 = 1;
/// Record type: blocks of a sound.
pub const TYPE_DATA: u8 = 2;
/// Record type: a command.
pub const TYPE_CONTROL: u8 = 3;

/// OBJECT's fixed fields: handle, generation, id, codec, rate, block bytes,
/// sample count, block count.
pub const OBJECT_FIXED: usize = 1 + 2 + 4 + 1 + 1 + 2 + 4 + 2;
/// DATA's fixed fields: handle, generation, first block.
pub const DATA_FIXED: usize = 1 + 2 + 2;
/// CONTROL's fields: channel, sequence, operation, handle, generation, gain,
/// fade, flags, duck, page.
pub const CONTROL_BYTES: usize = 1 + 2 + 1 + 1 + 2 + 1 + 2 + 1 + 1 + 2;
/// The longest label, in bytes of UTF-8 (REQ-AUD-004).
pub const MAX_LABEL: usize = 64;
/// The longest language tag.
pub const MAX_LANGUAGE: usize = 16;
/// Extension tag of a label.
pub const TAG_LABEL: u8 = 1;
/// Extension tag of a language.
pub const TAG_LANGUAGE: u8 = 2;

/// Command flag: start before the whole sound has arrived (REQ-AUD-013).
pub const FLAG_PROGRESSIVE: u8 = 1;
/// Command flag: repeat until stopped.
pub const FLAG_LOOP: u8 = 2;
/// Command flag: wait for the channel's current sound to end.
pub const FLAG_AFTER_CURRENT: u8 = 4;
const ALL_FLAGS: u8 = FLAG_PROGRESSIVE | FLAG_LOOP | FLAG_AFTER_CURRENT;

/// The lowest and highest page numbers a command may name (EN 300 706
/// clause 7.1.2).
const FIRST_PAGE: u16 = 0x100;
const LAST_PAGE: u16 = 0x8FF;

/// Why a record was refused. A refused record is discarded whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordError {
    /// Shorter than its header and CRC, or than its length says.
    Truncated,
    /// Longer than its length says.
    TrailingBytes,
    /// A body longer than [`MAX_BODY`].
    TooLong,
    /// A version this receiver does not read.
    Version(u8),
    /// A record type that does not exist.
    UnknownType(u8),
    /// The CRC-32 does not match.
    Crc,
    /// A field outside its range, and which.
    BadValue(&'static str),
}

/// An OBJECT record: a sound's definition (REQ-AUD-004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Object<'a> {
    /// The short name DATA and CONTROL use.
    pub handle: u8,
    /// Newer when the sound or its definition changes.
    pub generation: u16,
    /// Stable across generations within a service.
    pub id: u32,
    /// The codec, or `None` for a reserved code: the object is ignored.
    pub codec: Option<Codec>,
    /// Samples a second.
    pub rate_hz: u32,
    /// Bytes in each whole block.
    pub block_bytes: u16,
    /// Samples in the whole sound; 0 for a live sound.
    pub samples: u32,
    /// DATA blocks in the whole sound; 0 for a live sound.
    pub blocks: u16,
    /// The label, as sent, not yet cleaned for display.
    pub label: Option<&'a [u8]>,
    /// The BCP 47 language tag, as sent.
    pub language: Option<&'a [u8]>,
}

/// A DATA record: whole blocks of a sound (REQ-AUD-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Data<'a> {
    /// The sound's handle.
    pub handle: u8,
    /// Its generation.
    pub generation: u16,
    /// The index of the first block.
    pub first_block: u16,
    /// The blocks, back to back.
    pub blocks: &'a [u8],
}

/// What a command does (REQ-AUD-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operation {
    /// Start a sound.
    Play,
    /// Fade the channel out.
    Stop,
    /// Ramp the channel to a new gain.
    Gain,
}

/// A CONTROL record (REQ-AUD-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Control {
    /// 0 to 3.
    pub channel: u8,
    /// The channel's sequence number (REQ-AUD-010).
    pub sequence: u16,
    /// What to do.
    pub operation: Operation,
    /// The sound, for a play.
    pub handle: u8,
    /// The sound's generation, for a play.
    pub generation: u16,
    /// 0 to 100 per cent.
    pub gain: u8,
    /// The ramp, at least [`crate::MIN_FADE_MS`] once applied.
    pub fade_ms: u16,
    /// [`FLAG_PROGRESSIVE`], [`FLAG_LOOP`] and [`FLAG_AFTER_CURRENT`].
    pub flags: u8,
    /// 0 to 6: the attenuation of EN 300 708 Table 19 applied to the other
    /// channels while this sound plays.
    pub duck: u8,
    /// The page the command belongs to, or `None` for the whole service.
    pub page: Option<u16>,
}

/// One record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Record<'a> {
    /// A sound's definition.
    Object(Object<'a>),
    /// Blocks of a sound.
    Data(Data<'a>),
    /// A command.
    Control(Control),
}

/// The length of the record that starts `bytes`, once its header is there:
/// what a file reader needs to frame the next record.
///
/// # Errors
/// [`RecordError::Truncated`] before the header is complete, and
/// [`RecordError::TooLong`] for a length beyond the bound.
pub fn framed_length(bytes: &[u8]) -> Result<usize, RecordError> {
    let header = bytes.get(..RECORD_HEADER).ok_or(RecordError::Truncated)?;
    let body =
        usize::from(u16::from_be_bytes([header.get(2).copied().unwrap_or(0), header.get(3).copied().unwrap_or(0)]));
    if body > MAX_BODY {
        return Err(RecordError::TooLong);
    }
    Ok(RECORD_HEADER.saturating_add(body).saturating_add(RECORD_CRC))
}

/// Reads one whole record, exactly `bytes` long.
///
/// # Errors
/// Any failure of REQ-AUD-003 to REQ-AUD-006: the frame, the CRC, the type
/// or any field.
pub fn parse(bytes: &[u8]) -> Result<Record<'_>, RecordError> {
    if bytes.len() > MAX_RECORD {
        return Err(RecordError::TooLong);
    }
    let length = framed_length(bytes)?;
    if bytes.len() < length {
        return Err(RecordError::Truncated);
    }
    if bytes.len() > length {
        return Err(RecordError::TrailingBytes);
    }
    let (header, rest) = bytes.split_at(RECORD_HEADER);
    let (body, crc) = rest.split_at(rest.len().saturating_sub(RECORD_CRC));
    let crc: [u8; RECORD_CRC] = crc.try_into().map_err(|_| RecordError::Truncated)?;
    if crc32_parts(header, body) != u32::from_be_bytes(crc) {
        return Err(RecordError::Crc);
    }
    let version = header.first().copied().unwrap_or(0);
    if version != PROFILE_VERSION {
        return Err(RecordError::Version(version));
    }
    let mut fields = Fields { bytes: body };
    match header.get(1).copied().unwrap_or(0) {
        TYPE_OBJECT => object(&mut fields).map(Record::Object),
        TYPE_DATA => {
            let data = Data {
                handle: fields.u8()?,
                generation: fields.u16()?,
                first_block: fields.u16()?,
                blocks: fields.rest(),
            };
            if data.blocks.is_empty() {
                return Err(RecordError::BadValue("no blocks"));
            }
            Ok(Record::Data(data))
        }
        TYPE_CONTROL => {
            let control = control(&mut fields)?;
            fields.end()?;
            Ok(Record::Control(control))
        }
        other => Err(RecordError::UnknownType(other)),
    }
}

fn object<'a>(fields: &mut Fields<'a>) -> Result<Object<'a>, RecordError> {
    let handle = fields.u8()?;
    let generation = fields.u16()?;
    let id = fields.u32()?;
    let codec = Codec::from_code(fields.u8()?);
    let rate_hz = sample_rate(fields.u8()?).ok_or(RecordError::BadValue("sample rate"))?;
    let block_bytes = fields.u16()?;
    let samples = fields.u32()?;
    let blocks = fields.u16()?;
    let mut object =
        Object { handle, generation, id, codec, rate_hz, block_bytes, samples, blocks, label: None, language: None };
    if let Some(codec) = codec {
        if !codec.allows_block(block_bytes) {
            return Err(RecordError::BadValue("block size"));
        }
        let per_block = codec.samples_per_block(block_bytes);
        let expected = samples.div_ceil(per_block);
        if u32::from(blocks) != expected {
            return Err(RecordError::BadValue("block count"));
        }
    }
    // Extensions: tag, length, value. Unknown tags are skipped.
    while !fields.bytes.is_empty() {
        let tag = fields.u8()?;
        let length = usize::from(fields.u8()?);
        let value = fields.take(length)?;
        match tag {
            TAG_LABEL if length <= MAX_LABEL => object.label = Some(value),
            TAG_LANGUAGE if length <= MAX_LANGUAGE => object.language = Some(value),
            TAG_LABEL | TAG_LANGUAGE => return Err(RecordError::BadValue("extension too long")),
            _ => {}
        }
    }
    Ok(object)
}

fn control(fields: &mut Fields<'_>) -> Result<Control, RecordError> {
    let channel = fields.u8()?;
    if channel >= CHANNELS {
        return Err(RecordError::BadValue("channel"));
    }
    let sequence = fields.u16()?;
    let operation = match fields.u8()? {
        1 => Operation::Play,
        2 => Operation::Stop,
        3 => Operation::Gain,
        _ => return Err(RecordError::BadValue("operation")),
    };
    let handle = fields.u8()?;
    let generation = fields.u16()?;
    let gain = fields.u8()?;
    if gain > MAX_GAIN {
        return Err(RecordError::BadValue("gain"));
    }
    let fade_ms = fields.u16()?;
    if fade_ms > MAX_FADE_MS {
        return Err(RecordError::BadValue("fade"));
    }
    let flags = fields.u8()?;
    if flags & !ALL_FLAGS != 0 {
        return Err(RecordError::BadValue("flags"));
    }
    let duck = fields.u8()?;
    if usize::from(duck) >= DUCK_PERCENT.len() {
        return Err(RecordError::BadValue("duck"));
    }
    let page = match fields.u16()? {
        SERVICE_SCOPE => None,
        page if (FIRST_PAGE..=LAST_PAGE).contains(&page) && page & 0xFF <= 0x99 && page & 0x0F <= 9 => Some(page),
        _ => return Err(RecordError::BadValue("page")),
    };
    Ok(Control { channel, sequence, operation, handle, generation, gain, fade_ms, flags, duck, page })
}

/// Reads fields from a body, failing on any short read.
struct Fields<'a> {
    bytes: &'a [u8],
}

impl<'a> Fields<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], RecordError> {
        if self.bytes.len() < count {
            return Err(RecordError::Truncated);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RecordError> {
        self.take(N)?.try_into().map_err(|_| RecordError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, RecordError> {
        Ok(u8::from_be_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, RecordError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, RecordError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn rest(&mut self) -> &'a [u8] {
        core::mem::take(&mut self.bytes)
    }

    fn end(&self) -> Result<(), RecordError> {
        if self.bytes.is_empty() { Ok(()) } else { Err(RecordError::TrailingBytes) }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-002, REQ-AUD-003, REQ-AUD-004 and REQ-AUD-006.
    use super::*;
    use crate::crc::crc32;
    use std::vec::Vec;

    fn frame(kind: u8, body: &[u8]) -> Vec<u8> {
        let mut bytes = std::vec![PROFILE_VERSION, kind];
        bytes.extend_from_slice(&u16::try_from(body.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(body);
        let crc = crc32(&bytes);
        bytes.extend_from_slice(&crc.to_be_bytes());
        bytes
    }

    fn object_body() -> Vec<u8> {
        // Handle 3, generation 1, id 9, IMA ADPCM, 16 kHz, 256-byte blocks,
        // 1 000 samples in 2 blocks of 505.
        let mut body = std::vec![3, 0, 1, 0, 0, 0, 9, 1, 2, 1, 0];
        body.extend_from_slice(&1000_u32.to_be_bytes());
        body.extend_from_slice(&2_u16.to_be_bytes());
        body
    }

    #[test]
    fn an_object_with_a_label_is_read() {
        let mut body = object_body();
        body.extend_from_slice(&[TAG_LABEL, 5]);
        body.extend_from_slice(b"Chime");
        body.extend_from_slice(&[0x7F, 1, 0]); // an unknown tag, skipped
        let bytes = frame(TYPE_OBJECT, &body);
        let Record::Object(object) = parse(&bytes).unwrap() else { panic!("not an object") };
        assert_eq!(object.codec, Some(Codec::ImaAdpcm));
        assert_eq!((object.rate_hz, object.samples, object.blocks), (16_000, 1000, 2));
        assert_eq!(object.label, Some(&b"Chime"[..]));
    }

    #[test]
    fn a_wrong_block_count_is_refused() {
        let mut body = object_body();
        let last = body.len() - 1;
        body[last] = 3;
        assert_eq!(parse(&frame(TYPE_OBJECT, &body)), Err(RecordError::BadValue("block count")));
    }

    #[test]
    fn a_reserved_codec_is_read_and_left_for_the_receiver_to_ignore() {
        let mut body = object_body();
        body[7] = 2;
        let bytes = frame(TYPE_OBJECT, &body);
        let Record::Object(object) = parse(&bytes).unwrap() else { panic!("not an object") };
        assert_eq!(object.codec, None);
    }

    #[test]
    fn controls_are_read_and_their_bounds_kept() {
        let body = [1, 0, 7, 1, 3, 0, 1, 80, 0, 200, FLAG_LOOP, 2, 0x01, 0x00];
        let Record::Control(control) = parse(&frame(TYPE_CONTROL, &body)).unwrap() else { panic!("not a control") };
        assert_eq!(control.page, Some(0x100));
        assert_eq!((control.channel, control.sequence, control.gain, control.fade_ms), (1, 7, 80, 200));
        for (index, value, error) in [
            (0, 4, "channel"),
            (3, 9, "operation"),
            (7, 101, "gain"),
            (10, 8, "flags"),
            (11, 7, "duck"),
            (12, 0x09, "page"),
        ] {
            let mut bad = body;
            bad[index] = value;
            assert_eq!(parse(&frame(TYPE_CONTROL, &bad)), Err(RecordError::BadValue(error)), "field {index}");
        }
        // A hexadecimal page, and a page for the whole service.
        let mut bad = body;
        bad[13] = 0x0A;
        assert_eq!(parse(&frame(TYPE_CONTROL, &bad)), Err(RecordError::BadValue("page")));
        let mut service = body;
        service[12..14].copy_from_slice(&SERVICE_SCOPE.to_be_bytes());
        assert!(matches!(parse(&frame(TYPE_CONTROL, &service)), Ok(Record::Control(Control { page: None, .. }))));
    }

    #[test]
    fn damaged_and_malformed_frames_are_refused() {
        let good = frame(TYPE_OBJECT, &object_body());
        assert!(parse(&good).is_ok());
        let mut damaged = good.clone();
        damaged[6] ^= 1;
        assert_eq!(parse(&damaged), Err(RecordError::Crc));
        assert_eq!(parse(&good[..good.len() - 1]), Err(RecordError::Truncated));
        let mut longer = good.clone();
        longer.push(0);
        assert_eq!(parse(&longer), Err(RecordError::TrailingBytes));
        assert_eq!(parse(&frame(9, &[])), Err(RecordError::UnknownType(9)));
        // Version 1 with a correct CRC.
        let mut version = std::vec![1, TYPE_CONTROL];
        version.extend_from_slice(&u16::try_from(CONTROL_BYTES).unwrap().to_be_bytes());
        version.extend_from_slice(&[0; CONTROL_BYTES]);
        let crc = crc32(&version);
        version.extend_from_slice(&crc.to_be_bytes());
        assert_eq!(parse(&version), Err(RecordError::Version(1)));
        let mut too_long = std::vec![PROFILE_VERSION, TYPE_DATA];
        too_long.extend_from_slice(&u16::try_from(MAX_BODY + 1).unwrap().to_be_bytes());
        assert_eq!(framed_length(&too_long), Err(RecordError::TooLong));
    }
}
