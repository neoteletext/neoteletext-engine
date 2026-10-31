//! Records: the one form every carriage delivers (REQ-AUD-003 to
//! REQ-AUD-006). Integers are big-endian; a record is at most
//! [`MAX_RECORD`](crate::MAX_RECORD) bytes and is checked against every bound before anything
//! in it is used.

use crate::{CHANNELS, Codec, DUCK_PERCENT, MAX_FADE_MS, MAX_GAIN, SERVICE_SCOPE, sample_rate};
use frame::Fields;

mod frame;

pub use frame::{RecordError, framed_length, record_length, split, write};

/// Record type: announce a sound.
pub const TYPE_OBJECT: u8 = 1;
/// Record type: blocks of a sound.
pub const TYPE_DATA: u8 = 2;
/// Record type: a command.
pub const TYPE_CONTROL: u8 = 3;
/// Record type: the input system's cursor record (area INT,
/// `neoteletext-common/INPUT.md` section 7.1), carried with this profile's records on
/// the same data channel because EN 300 708 Table 16 gives data channel 13
/// to Datavideo. It is always one fragment; this receiver leaves it alone.
pub const TYPE_CURSOR: u8 = 4;
/// Record type: the input profile's control record (WELCOME and the input
/// modes, `neoteletext-common/INPUT.md` section 7.2), carried as the cursor
/// record is, one fragment; this receiver leaves it alone.
pub const TYPE_INPUT_CONTROL: u8 = 5;

/// Record type: the channel's index, its magazines and pages
/// (`neoteletext-common/INPUT.md` section 7.5), read by the web agent; one
/// or more fragments; this receiver leaves it alone.
pub const TYPE_INDEX: u8 = 6;
/// Record type: a log line for the channel's author (`INPUT.md` section
/// 7.6, REQ-INP-026), read by the web agent; this receiver leaves it alone.
pub const TYPE_LOG: u8 = 7;
/// Record type: a cookie set while the connection is open (`INPUT.md`
/// section 7.7), read by the web agent; this receiver leaves it alone.
pub const TYPE_SET_COOKIE: u8 = 8;

/// Record type: a sub-page's TTI Link Extension links (`INPUT.md` section
/// 7.8, REQ-1494), read by the web agent; this receiver leaves it alone.
pub const TYPE_LINKS: u8 = 9;

/// Record type: a page's list of sub-pages (`INPUT.md` section 7.9,
/// REQ-1503), read by the web agent; this receiver leaves it alone.
pub const TYPE_SUBPAGES: u8 = 10;

/// Record type: a channel's redirect, to another page or another channel
/// (`INPUT.md` section 7.11, REQ-1688), read by the web agent; this receiver
/// leaves it alone.
pub const TYPE_REDIRECT: u8 = 11;

/// Whether a record type belongs to the input profile, which shares this
/// profile's carriage and is left alone by the audio receiver.
#[must_use]
pub const fn is_input_type(kind: u8) -> bool {
    kind == TYPE_CURSOR || kind == TYPE_INPUT_CONTROL
}

/// Whether a record type is one of the channel's records beside sound
/// (`INPUT.md` section 7): the input profile's, the index, LOG and
/// `SET_COOKIE`. The audio receiver leaves every one alone, and counts none
/// as refused.
#[must_use]
pub const fn is_channel_type(kind: u8) -> bool {
    is_input_type(kind)
        || kind == TYPE_INDEX
        || kind == TYPE_LOG
        || kind == TYPE_SET_COOKIE
        || kind == TYPE_LINKS
        || kind == TYPE_SUBPAGES
        || kind == TYPE_REDIRECT
}

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

/// Reads one whole record, exactly `bytes` long.
///
/// # Errors
/// Any failure of REQ-AUD-003 to REQ-AUD-006: the frame, the CRC, the type
/// or any field.
pub fn parse(bytes: &[u8]) -> Result<Record<'_>, RecordError> {
    let (kind, body) = split(bytes)?;
    let mut fields = Fields::new(body);
    match kind {
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
        if !codec.allows_rate(rate_hz) {
            return Err(RecordError::BadValue("sample rate"));
        }
        let per_block = codec.samples_per_block(block_bytes);
        let expected = samples.div_ceil(per_block);
        if u32::from(blocks) != expected {
            return Err(RecordError::BadValue("block count"));
        }
    }
    // Extensions: tag, length, value. Unknown tags are skipped.
    while !fields.is_empty() {
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

#[cfg(test)]
mod tests;
