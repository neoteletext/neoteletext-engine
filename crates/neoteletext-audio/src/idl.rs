//! Broadcast carriage (REQ-AUD-007): records cut into fragments on EN 300 708
//! independent data lines, data channel 12, as a user-defined
//! programme-independent audio service (clause 6.7.3, Table 21).
//!
//! Packet bytes 8 to 45 hold, in order: a record id (one byte, counting new
//! records modulo 256 as clause 6.5.5's continuity indicator counts packets),
//! the fragment's index, [`FRAGMENT_DATA`] bytes of the record, and a CRC-16
//! over the 36 bytes before it (clause 6.5.8.1).

use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::consts::{
    ADDRESS_BYTE_0, ADDRESS_BYTE_1, IDL_AUDIO_CONTROL_BYTE, IDL_AUDIO_DATA_BYTES, IDL_AUDIO_DATA_START,
    IDL_AUDIO_SERVICE_BYTE, IDL_AUDIO_SERVICE_MASK, IDL_AUDIO_USER_DEFINED, IDL_CHANNEL_INDEPENDENT_AUDIO,
    IDL_DESIGNATION,
};
use neoteletext_core::ecc::{hamming84, hamming84_encode};
use neoteletext_core::packet::Packet;

use crate::MAX_FRAMED_RECORD;
use crate::crc::crc16_idl;

/// Bytes of a record in one fragment: the 38 audio bytes less the record
/// id, the fragment index and the CRC-16.
pub const FRAGMENT_DATA: usize = IDL_AUDIO_DATA_BYTES - 2 - 2;
/// The most fragments of one record: enough for [`MAX_FRAMED_RECORD`].
pub const MAX_FRAGMENTS: usize = MAX_FRAMED_RECORD.div_ceil(FRAGMENT_DATA);
// REQ-AUD-007: a fragment's index is one byte.
const _: () = assert!(MAX_FRAGMENTS <= u8::MAX as usize + 1);
/// Control Byte message bit 1: the first fragment of a record.
pub const CONTROL_FIRST: u8 = 0b0001;
/// Control Byte message bit 2: the last fragment of a record.
pub const CONTROL_LAST: u8 = 0b0010;
/// Control Byte message bits 3 and 4 are 0 in this profile.
const CONTROL_RESERVED: u8 = 0b1100;

/// One fragment of a record, checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fragment {
    /// The audio service, 0 to 7: Service Byte message bits 1 to 3.
    pub service: u8,
    /// Counts new records of the service.
    pub record: u8,
    /// The fragment's place in the record.
    pub index: u8,
    /// The first fragment of its record.
    pub first: bool,
    /// The last fragment of its record.
    pub last: bool,
    /// The record's bytes in this fragment; the last is padded.
    pub data: [u8; FRAGMENT_DATA],
}

/// Why an audio packet was dropped. The transmitter's repeats fill the gap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IdlError {
    /// A Hamming 8/4 byte could not be corrected.
    Hamming,
    /// The Service Byte is not a user-defined service.
    NotUserDefined,
    /// Reserved Control Byte bits set, or flags that contradict the index.
    Control,
    /// A fragment index beyond [`MAX_FRAGMENTS`].
    Index,
    /// The CRC-16 does not match.
    Crc,
}

/// Whether `packet` is an independent data line of data channel 12, the
/// audio profile's channel: the data channel is the first address byte's
/// four message bits, and the designation code the second's (EN 300 708
/// clauses 6.4.1 and 6.4.2).
#[must_use]
pub fn is_audio(packet: &Packet) -> bool {
    hamming84(packet.byte(ADDRESS_BYTE_0)) == Some(IDL_CHANNEL_INDEPENDENT_AUDIO)
        && hamming84(packet.byte(ADDRESS_BYTE_1)) == Some(IDL_DESIGNATION)
}

/// The fragment `packet` carries, or `None` when it is not an audio packet.
///
/// # Errors
/// A packet of the audio channel that is damaged or malformed.
#[must_use]
pub fn fragment(packet: &Packet) -> Option<Result<Fragment, IdlError>> {
    is_audio(packet).then(|| read(packet))
}

/// The packet that carries fragment `index` of record `record` of audio
/// service `service`: `chunk` is at most [`FRAGMENT_DATA`] bytes of the
/// record, padded with zeros. It allocates nothing, so a microcontroller
/// can send a one-fragment record such as the input profile's cursor.
#[must_use]
pub fn packet(service: u8, record: u8, index: u8, last: bool, chunk: &[u8]) -> Packet {
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
    if last {
        control |= CONTROL_LAST;
    }
    set(IDL_AUDIO_CONTROL_BYTE, hamming84_encode(control));
    if let Some(audio) = bytes.get_mut(IDL_AUDIO_DATA_START..) {
        if let Some(id) = audio.get_mut(0) {
            *id = record;
        }
        if let Some(place) = audio.get_mut(1) {
            *place = index;
        }
        let chunk = chunk.get(..chunk.len().min(FRAGMENT_DATA)).unwrap_or(&[]);
        if let Some(target) = audio.get_mut(2..2usize.saturating_add(chunk.len())) {
            target.copy_from_slice(chunk);
        }
        let checked = IDL_AUDIO_DATA_BYTES.saturating_sub(2);
        let crc = crc16_idl(audio.get(..checked).unwrap_or(&[]));
        if let Some(target) = audio.get_mut(checked..) {
            target.copy_from_slice(&crc.to_be_bytes());
        }
    }
    Packet { bytes }
}

fn read(packet: &Packet) -> Result<Fragment, IdlError> {
    let service = hamming84(packet.byte(IDL_AUDIO_SERVICE_BYTE)).ok_or(IdlError::Hamming)?;
    if service & IDL_AUDIO_USER_DEFINED == 0 {
        return Err(IdlError::NotUserDefined);
    }
    let control = hamming84(packet.byte(IDL_AUDIO_CONTROL_BYTE)).ok_or(IdlError::Hamming)?;
    if control & CONTROL_RESERVED != 0 {
        return Err(IdlError::Control);
    }
    let bytes = packet.bytes.get(IDL_AUDIO_DATA_START..).ok_or(IdlError::Crc)?;
    let (checked, crc) = bytes.split_at(bytes.len().saturating_sub(2));
    let crc: [u8; 2] = crc.try_into().map_err(|_| IdlError::Crc)?;
    if crc16_idl(checked) != u16::from_be_bytes(crc) {
        return Err(IdlError::Crc);
    }
    let (record, index) = (checked.first().copied().unwrap_or(0), checked.get(1).copied().unwrap_or(0));
    if usize::from(index) >= MAX_FRAGMENTS {
        return Err(IdlError::Index);
    }
    let first = control & CONTROL_FIRST != 0;
    if first != (index == 0) {
        return Err(IdlError::Control);
    }
    let data = checked.get(2..).and_then(|data| data.try_into().ok()).ok_or(IdlError::Crc)?;
    Ok(Fragment {
        service: service & IDL_AUDIO_SERVICE_MASK,
        record,
        index,
        first,
        last: control & CONTROL_LAST != 0,
        data,
    })
}

mod reassembler;

pub use reassembler::{AUDIO_SLOT_BYTES, Reassembler};

#[cfg(test)]
mod tests;
