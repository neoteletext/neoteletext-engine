#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]

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
    let fragments: Vec<Fragment> = packets(1, &whole).iter().map(|packet| fragment(packet).unwrap().unwrap()).collect();
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
