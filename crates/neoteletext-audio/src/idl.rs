//! Broadcast carriage (REQ-AUD-007): records cut into fragments on EN 300 708
//! independent data lines, data channel 12, as a user-defined
//! programme-independent audio service (clause 6.7.3, Table 21).
//!
//! Packet bytes 8 to 45 hold, in order: a record id (one byte, counting new
//! records modulo 256 as clause 6.5.5's continuity indicator counts packets),
//! the fragment's index, [`FRAGMENT_DATA`] bytes of the record, and a CRC-16
//! over the 36 bytes before it (clause 6.5.8.1).

use neoteletext_core::consts::{
    ADDRESS_BYTE_0, ADDRESS_BYTE_1, IDL_AUDIO_CONTROL_BYTE, IDL_AUDIO_DATA_BYTES, IDL_AUDIO_DATA_START,
    IDL_AUDIO_SERVICE_BYTE, IDL_AUDIO_SERVICE_MASK, IDL_AUDIO_USER_DEFINED, IDL_CHANNEL_INDEPENDENT_AUDIO,
    IDL_DESIGNATION,
};
use neoteletext_core::ecc::hamming84;
use neoteletext_core::packet::Packet;

use crate::MAX_RECORD;
use crate::crc::crc16_idl;
use crate::record::framed_length;

/// Bytes of a record in one fragment: the 38 audio bytes less the record
/// id, the fragment index and the CRC-16.
pub const FRAGMENT_DATA: usize = IDL_AUDIO_DATA_BYTES - 2 - 2;
/// The most fragments of one record: enough for [`MAX_RECORD`].
pub const MAX_FRAGMENTS: usize = MAX_RECORD.div_ceil(FRAGMENT_DATA);
/// Control Byte message bit 1: the first fragment of a record.
pub const CONTROL_FIRST: u8 = 0b0001;
/// Control Byte message bit 2: the last fragment of a record.
pub const CONTROL_LAST: u8 = 0b0010;
/// Control Byte message bits 3 and 4 are 0 in this profile.
const CONTROL_RESERVED: u8 = 0b1100;
/// Bytes of a reassembly slot.
const SLOT_BYTES: usize = MAX_FRAGMENTS * FRAGMENT_DATA;

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

#[derive(Clone, Copy)]
struct Slot {
    service: u8,
    record: u8,
    /// One bit for each fragment received.
    received: u32,
    /// The fragments the record needs, once fragment 0 has said.
    needed: Option<u8>,
    /// Fragments received for this slot, for choosing the oldest.
    stamp: u32,
    bytes: [u8; SLOT_BYTES],
}

/// Reassembles records from fragments: at most `SLOTS` records in flight;
/// a fragment of a further record replaces the oldest incomplete one
/// (REQ-AUD-007).
pub struct Reassembler<const SLOTS: usize> {
    slots: [Option<Slot>; SLOTS],
    clock: u32,
}

impl<const SLOTS: usize> Default for Reassembler<SLOTS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SLOTS: usize> Reassembler<SLOTS> {
    /// An empty reassembler.
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: [None; SLOTS], clock: 0 }
    }

    /// Forgets every record in flight.
    pub fn clear(&mut self) {
        self.slots = [None; SLOTS];
    }

    /// Adds a fragment; returns the service and the whole record when this
    /// fragment completes one. The record is valid until the next call.
    pub fn push(&mut self, fragment: &Fragment) -> Option<(u8, &[u8])> {
        self.clock = self.clock.wrapping_add(1);
        let index = self.slot_for(fragment.service, fragment.record)?;
        let slot = self.slots.get_mut(index)?.as_mut()?;
        slot.stamp = self.clock;
        let start = usize::from(fragment.index).saturating_mul(FRAGMENT_DATA);
        slot.bytes.get_mut(start..start.saturating_add(FRAGMENT_DATA))?.copy_from_slice(&fragment.data);
        slot.received |= 1_u32.checked_shl(u32::from(fragment.index)).unwrap_or(0);
        if fragment.index == 0 {
            let length = framed_length(&fragment.data).ok()?;
            slot.needed = u8::try_from(length.div_ceil(FRAGMENT_DATA)).ok();
        }
        let needed = slot.needed?;
        if fragment.last && fragment.index.checked_add(1) != Some(needed) {
            // A last flag where the length says otherwise: start again.
            *self.slots.get_mut(index)? = None;
            return None;
        }
        let all = 1_u32.checked_shl(u32::from(needed)).map_or(u32::MAX, |bit| bit.wrapping_sub(1));
        if slot.received & all != all {
            return None;
        }
        let length = framed_length(&slot.bytes).ok()?;
        let service = slot.service;
        // The slot is free for the next record; its bytes stay readable
        // until then.
        slot.received = 0;
        slot.needed = None;
        let bytes = self.slots.get(index)?.as_ref()?.bytes.get(..length)?;
        Some((service, bytes))
    }

    /// The slot of a record, starting one if it is new, replacing the
    /// oldest incomplete record when every slot is busy.
    fn slot_for(&mut self, service: u8, record: u8) -> Option<usize> {
        if let Some(found) = self.slots.iter().position(|slot| {
            slot.as_ref().is_some_and(|slot| slot.service == service && slot.record == record && slot.received != 0)
        }) {
            return Some(found);
        }
        // A new record of a service replaces that service's unfinished one
        // with a different id, which was abandoned by the transmitter.
        let index = self
            .slots
            .iter()
            .position(|slot| slot.as_ref().is_none_or(|slot| slot.received == 0 || slot.service == service))
            .or_else(|| {
                self.slots
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, slot)| slot.as_ref().map_or(0, |slot| slot.stamp))
                    .map(|(index, _)| index)
            })?;
        *self.slots.get_mut(index)? =
            Some(Slot { service, record, received: 0, needed: None, stamp: self.clock, bytes: [0; SLOT_BYTES] });
        Some(index)
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-007.
    use super::*;
    use neoteletext_core::consts::PACKET_BYTES;
    use neoteletext_core::ecc::hamming84_encode;
    use std::vec::Vec;

    /// An audio packet carrying `data` as fragment `index` of `record`.
    fn packet(record: u8, index: u8, first: bool, last: bool, data: &[u8]) -> Packet {
        let mut bytes = [0_u8; PACKET_BYTES];
        bytes[ADDRESS_BYTE_0] = hamming84_encode(IDL_CHANNEL_INDEPENDENT_AUDIO);
        bytes[ADDRESS_BYTE_1] = hamming84_encode(IDL_DESIGNATION);
        bytes[IDL_AUDIO_SERVICE_BYTE] = hamming84_encode(IDL_AUDIO_USER_DEFINED | 3);
        bytes[IDL_AUDIO_CONTROL_BYTE] =
            hamming84_encode((u8::from(first) * CONTROL_FIRST) | (u8::from(last) * CONTROL_LAST));
        let audio = &mut bytes[IDL_AUDIO_DATA_START..];
        audio[0] = record;
        audio[1] = index;
        audio[2..2 + data.len()].copy_from_slice(data);
        let crc = crc16_idl(&audio[..IDL_AUDIO_DATA_BYTES - 2]);
        audio[IDL_AUDIO_DATA_BYTES - 2..].copy_from_slice(&crc.to_be_bytes());
        Packet { bytes }
    }

    /// A record of `body_bytes` body bytes, framed but with a dummy CRC:
    /// the reassembler checks only the length.
    fn record(body_bytes: usize) -> Vec<u8> {
        let mut bytes = std::vec![crate::PROFILE_VERSION, 2];
        bytes.extend_from_slice(&u16::try_from(body_bytes).unwrap().to_be_bytes());
        bytes.extend((0..body_bytes + 4).map(|n| (n % 251) as u8));
        bytes
    }

    fn packets(id: u8, bytes: &[u8]) -> Vec<Packet> {
        let chunks: Vec<&[u8]> = bytes.chunks(FRAGMENT_DATA).collect();
        let count = chunks.len();
        chunks.iter().enumerate().map(|(n, chunk)| packet(id, n as u8, n == 0, n + 1 == count, chunk)).collect()
    }

    #[test]
    fn data_channel_12_is_packet_4_31() {
        let packet = packet(0, 0, true, true, &[]);
        let address = neoteletext_core::packet::decode_address(&packet).unwrap();
        assert_eq!((address.magazine.number(), address.number), (4, 31));
    }

    #[test]
    fn a_record_is_reassembled_from_fragments_in_any_order() {
        let whole = record(100);
        let mut fragments: Vec<Fragment> =
            packets(7, &whole).iter().map(|packet| fragment(packet).unwrap().unwrap()).collect();
        assert_eq!(fragments.len(), 4);
        fragments.swap(0, 3);
        let mut reassembler = Reassembler::<2>::new();
        for fragment in &fragments[..3] {
            assert!(reassembler.push(fragment).is_none());
        }
        let (service, bytes) = reassembler.push(&fragments[3]).unwrap();
        assert_eq!((service, bytes), (3, whole.as_slice()));
    }

    #[test]
    fn repeats_and_lost_fragments_are_handled() {
        let whole = record(200);
        let fragments: Vec<Fragment> =
            packets(1, &whole).iter().map(|packet| fragment(packet).unwrap().unwrap()).collect();
        let mut reassembler = Reassembler::<2>::new();
        // The second fragment is lost on the first pass; the carousel's
        // repeat fills it.
        for (n, fragment) in fragments.iter().enumerate() {
            if n != 1 {
                assert!(reassembler.push(fragment).is_none());
            }
        }
        assert_eq!(reassembler.push(&fragments[1]).map(|(_, bytes)| bytes.to_vec()), Some(whole.clone()));
    }

    #[test]
    fn damaged_packets_are_dropped_and_other_channels_ignored() {
        let mut damaged = packets(0, &record(10))[0];
        damaged.bytes[IDL_AUDIO_DATA_START + 5] ^= 0x01;
        assert_eq!(fragment(&damaged), Some(Err(IdlError::Crc)));
        let mut other = damaged;
        other.bytes[ADDRESS_BYTE_0] = hamming84_encode(4);
        assert_eq!(fragment(&other), None, "data channel 4 is Audetel's, not this profile's");
        let mut not_user = packets(0, &record(10))[0];
        not_user.bytes[IDL_AUDIO_SERVICE_BYTE] = hamming84_encode(0);
        assert_eq!(fragment(&not_user), Some(Err(IdlError::NotUserDefined)));
        let mut bad_flag = packets(0, &record(10))[0];
        bad_flag.bytes[IDL_AUDIO_CONTROL_BYTE] = hamming84_encode(0);
        assert_eq!(fragment(&bad_flag), Some(Err(IdlError::Control)), "fragment 0 must say it is first");
    }

    #[test]
    fn a_further_record_replaces_the_oldest_incomplete_one() {
        let mut reassembler = Reassembler::<2>::new();
        // Records 1 and 2 of service 3 left incomplete, then record 3.
        let one = packets(1, &record(100));
        let two = packets(2, &record(100));
        reassembler.push(&fragment(&one[0]).unwrap().unwrap());
        reassembler.push(&fragment(&two[0]).unwrap().unwrap());
        let three = record(20);
        let done = packets(3, &three)
            .iter()
            .find_map(|packet| reassembler.push(&fragment(packet).unwrap().unwrap()).map(|(_, b)| b.to_vec()));
        assert_eq!(done, Some(three));
    }
}
