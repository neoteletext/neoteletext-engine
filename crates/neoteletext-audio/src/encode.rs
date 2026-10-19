//! Writing sounds, for publishing tools (REQ-AUD-020): IMA ADPCM encoding,
//! records, and records cut into broadcast packets. This module allocates.

use alloc::vec::Vec;

use neoteletext_core::consts::{
    ADDRESS_BYTE_0, ADDRESS_BYTE_1, IDL_AUDIO_CONTROL_BYTE, IDL_AUDIO_DATA_BYTES, IDL_AUDIO_DATA_START,
    IDL_AUDIO_SERVICE_BYTE, IDL_AUDIO_SERVICE_MASK, IDL_AUDIO_USER_DEFINED, IDL_CHANNEL_INDEPENDENT_AUDIO,
    IDL_DESIGNATION, PACKET_BYTES,
};
use neoteletext_core::ecc::hamming84_encode;
use neoteletext_core::packet::Packet;

use crate::crc::{crc16_idl, crc32};
use crate::idl::{CONTROL_FIRST, CONTROL_LAST, FRAGMENT_DATA};
use crate::ima::{State, encode as encode_code};
use crate::record::{DATA_FIXED, TAG_LABEL, TYPE_CONTROL, TYPE_DATA, TYPE_OBJECT};
use crate::{Codec, MAX_BODY, MAX_RECORD, PROFILE_VERSION, SERVICE_SCOPE, rate_code};

/// A sound ready to publish.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sound {
    /// The handle commands name.
    pub handle: u8,
    /// Its generation.
    pub generation: u16,
    /// Stable across generations.
    pub id: u32,
    /// The codec.
    pub codec: Codec,
    /// Samples a second: one of [`crate::SAMPLE_RATES`].
    pub rate_hz: u32,
    /// Bytes in each whole block.
    pub block_bytes: u16,
    /// Samples in the whole sound.
    pub samples: u32,
    /// The encoded blocks, back to back.
    pub data: Vec<u8>,
    /// A label, at most [`crate::record::MAX_LABEL`] bytes.
    pub label: Option<Vec<u8>>,
}

/// Why a sound could not be written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncodeError {
    /// A rate the profile does not carry.
    Rate(u32),
    /// A block size the codec does not allow.
    BlockSize(u16),
    /// No samples, or more blocks than a sound may have.
    Length,
    /// A label longer than the bound.
    Label,
}

impl Sound {
    /// Encodes mono 16-bit samples.
    ///
    /// # Errors
    /// A rate or block size the profile does not allow, or a length it
    /// cannot carry.
    pub fn new(
        handle: u8,
        generation: u16,
        codec: Codec,
        rate_hz: u32,
        block_bytes: u16,
        pcm: &[i16],
    ) -> Result<Self, EncodeError> {
        rate_code(rate_hz).ok_or(EncodeError::Rate(rate_hz))?;
        if !codec.allows_block(block_bytes) {
            return Err(EncodeError::BlockSize(block_bytes));
        }
        let samples = u32::try_from(pcm.len()).map_err(|_| EncodeError::Length)?;
        let per_block = codec.samples_per_block(block_bytes);
        if samples == 0 || u16::try_from(samples.div_ceil(per_block)).is_err() {
            return Err(EncodeError::Length);
        }
        let data = match codec {
            Codec::Pcm16 => pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect(),
            Codec::ImaAdpcm => ima_blocks(pcm, usize::try_from(per_block).map_err(|_| EncodeError::Length)?),
        };
        Ok(Self { handle, generation, id: u32::from(handle), codec, rate_hz, block_bytes, samples, data, label: None })
    }

    /// Blocks in the whole sound.
    #[must_use]
    pub fn blocks(&self) -> u16 {
        u16::try_from(self.samples.div_ceil(self.codec.samples_per_block(self.block_bytes))).unwrap_or(u16::MAX)
    }

    /// The OBJECT record that announces the sound.
    ///
    /// # Errors
    /// A label longer than the bound.
    pub fn object_record(&self) -> Result<Vec<u8>, EncodeError> {
        let mut body = Vec::new();
        body.push(self.handle);
        body.extend_from_slice(&self.generation.to_be_bytes());
        body.extend_from_slice(&self.id.to_be_bytes());
        body.push(self.codec.code());
        body.push(rate_code(self.rate_hz).ok_or(EncodeError::Rate(self.rate_hz))?);
        body.extend_from_slice(&self.block_bytes.to_be_bytes());
        body.extend_from_slice(&self.samples.to_be_bytes());
        body.extend_from_slice(&self.blocks().to_be_bytes());
        if let Some(label) = &self.label {
            if label.len() > crate::record::MAX_LABEL {
                return Err(EncodeError::Label);
            }
            body.push(TAG_LABEL);
            body.push(u8::try_from(label.len()).map_err(|_| EncodeError::Label)?);
            body.extend_from_slice(label);
        }
        Ok(frame(TYPE_OBJECT, &body))
    }

    /// The DATA records that carry the sound: as many whole blocks in each
    /// as fit a record.
    #[must_use]
    pub fn data_records(&self) -> Vec<Vec<u8>> {
        let per_record =
            MAX_BODY.saturating_sub(DATA_FIXED).checked_div(usize::from(self.block_bytes)).unwrap_or(1).max(1);
        let block = usize::from(self.block_bytes);
        let mut records = Vec::new();
        let mut first: u16 = 0;
        for chunk in self.data.chunks(block.saturating_mul(per_record)) {
            let mut body = Vec::with_capacity(DATA_FIXED.saturating_add(chunk.len()));
            body.push(self.handle);
            body.extend_from_slice(&self.generation.to_be_bytes());
            body.extend_from_slice(&first.to_be_bytes());
            body.extend_from_slice(chunk);
            records.push(frame(TYPE_DATA, &body));
            first = first.saturating_add(u16::try_from(per_record).unwrap_or(u16::MAX));
        }
        records
    }
}

/// IMA ADPCM blocks of `per_block` samples: each block's header holds its
/// first sample and the step index the encoder had reached, so it decodes
/// on its own.
fn ima_blocks(pcm: &[i16], per_block: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut state = State { predictor: 0, index: 0 };
    for block in pcm.chunks(per_block) {
        let Some((&first, rest)) = block.split_first() else { continue };
        state.predictor = first;
        out.extend_from_slice(&first.to_le_bytes());
        out.push(state.index);
        out.push(0);
        // Two codes a byte, low nibble first; an odd count is padded.
        for pair in rest.chunks(2) {
            let low = encode_code(&mut state, pair.first().copied().unwrap_or(0));
            let high = match pair.get(1) {
                Some(&sample) => encode_code(&mut state, sample),
                None => 0,
            };
            out.push(low | (high << 4));
        }
    }
    out
}

/// A command, written as a CONTROL record (REQ-AUD-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Command {
    /// 0 to 3.
    pub channel: u8,
    /// Newer than the channel's last command.
    pub sequence: u16,
    /// 1 play, 2 stop, 3 gain.
    pub operation: u8,
    /// The sound's handle, for a play.
    pub handle: u8,
    /// The sound's generation, for a play.
    pub generation: u16,
    /// 0 to 100.
    pub gain: u8,
    /// Milliseconds.
    pub fade_ms: u16,
    /// The flags of [`crate::record`].
    pub flags: u8,
    /// 0 to 6.
    pub duck: u8,
    /// A page, or `None` for the whole service.
    pub page: Option<u16>,
}

impl Command {
    /// The CONTROL record.
    #[must_use]
    pub fn record(&self) -> Vec<u8> {
        let mut body = Vec::new();
        body.push(self.channel);
        body.extend_from_slice(&self.sequence.to_be_bytes());
        body.push(self.operation);
        body.push(self.handle);
        body.extend_from_slice(&self.generation.to_be_bytes());
        body.push(self.gain);
        body.extend_from_slice(&self.fade_ms.to_be_bytes());
        body.push(self.flags);
        body.push(self.duck);
        body.extend_from_slice(&self.page.unwrap_or(SERVICE_SCOPE).to_be_bytes());
        frame(TYPE_CONTROL, &body)
    }
}

/// A record: header, body and CRC-32 (REQ-AUD-003).
#[must_use]
pub fn frame(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(body.len().saturating_add(8).min(MAX_RECORD));
    bytes.push(PROFILE_VERSION);
    bytes.push(kind);
    bytes.extend_from_slice(&u16::try_from(body.len()).unwrap_or(u16::MAX).to_be_bytes());
    bytes.extend_from_slice(body);
    let crc = crc32(&bytes);
    bytes.extend_from_slice(&crc.to_be_bytes());
    bytes
}

/// Cuts records into the broadcast packets of audio service `service`
/// (REQ-AUD-007), numbering them from `record_id`. Returns the packets and
/// the next record id.
#[must_use]
pub fn packets(service: u8, mut record_id: u8, records: &[Vec<u8>]) -> (Vec<Packet>, u8) {
    let mut out = Vec::new();
    for record in records {
        let count = record.len().div_ceil(FRAGMENT_DATA);
        for (index, chunk) in record.chunks(FRAGMENT_DATA).enumerate() {
            let mut bytes = [0_u8; PACKET_BYTES];
            let mut set = |at: usize, value: u8| {
                if let Some(byte) = bytes.get_mut(at) {
                    *byte = value;
                }
            };
            set(ADDRESS_BYTE_0, hamming84_encode(IDL_CHANNEL_INDEPENDENT_AUDIO));
            set(ADDRESS_BYTE_1, hamming84_encode(IDL_DESIGNATION));
            set(IDL_AUDIO_SERVICE_BYTE, hamming84_encode(IDL_AUDIO_USER_DEFINED | (service & IDL_AUDIO_SERVICE_MASK)));
            let mut control = 0;
            if index == 0 {
                control |= CONTROL_FIRST;
            }
            if index.saturating_add(1) == count {
                control |= CONTROL_LAST;
            }
            set(IDL_AUDIO_CONTROL_BYTE, hamming84_encode(control));
            if let Some(audio) = bytes.get_mut(IDL_AUDIO_DATA_START..) {
                if let Some(id) = audio.get_mut(0) {
                    *id = record_id;
                }
                if let Some(place) = audio.get_mut(1) {
                    *place = u8::try_from(index).unwrap_or(u8::MAX);
                }
                if let Some(target) = audio.get_mut(2..2usize.saturating_add(chunk.len())) {
                    target.copy_from_slice(chunk);
                }
                let checked = IDL_AUDIO_DATA_BYTES.saturating_sub(2);
                let crc = crc16_idl(audio.get(..checked).unwrap_or(&[]));
                if let Some(target) = audio.get_mut(checked..) {
                    target.copy_from_slice(&crc.to_be_bytes());
                }
            }
            out.push(Packet { bytes });
        }
        record_id = record_id.wrapping_add(1);
    }
    (out, record_id)
}
