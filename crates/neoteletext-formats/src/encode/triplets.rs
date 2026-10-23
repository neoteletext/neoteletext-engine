//! Packed TTI lines (REQ-INP-023): packets from stored packed lines and
//! back, and thirteen triplets as 39 characters and back.

use neoteletext_core::consts::{NIBBLE_MASK, TRIPLETS_PER_PACKET};
use neoteletext_core::ecc::{hamming84, hamming84_encode, hamming2418};
use neoteletext_core::packet::{Packet, Triplet, encode};
use neoteletext_core::types::Magazine;

use crate::consts::{
    TTI_PACKED_CHARACTER_BASE, TTI_PACKED_CHARACTERS, TTI_TRIPLET_CHARACTER_BITS, TTI_TRIPLET_CHARACTER_MASK,
};
use crate::page::{RawBody, RawPacket};

/// Builds a packet from a stored packed line.
#[must_use]
pub fn raw_packet(magazine: Magazine, raw: &RawPacket) -> Packet {
    match &raw.body {
        RawBody::Triplets(triplets) => encode::triplet_packet(magazine, raw.number, raw.designation, triplets),
        RawBody::Nibbles(values) => {
            let body: [u8; TTI_PACKED_CHARACTERS] = values.map(|value| hamming84_encode(value & NIBBLE_MASK));
            encode::raw(magazine, raw.number, raw.designation, &body)
        }
    }
}

/// The packed `OL` line of a packet that carries a designation code and
/// thirteen triplets (X/26, X/27/4 to X/27/15, X/28, M/29): the inverse of
/// [`raw_packet`] for those packets, so that received packets can be
/// written as TTI (REQ-INP-023). `None` when the address or the
/// designation code is uncorrectable; an uncorrectable triplet is written
/// as 0.
#[must_use]
pub fn packed_line_of(packet: &Packet) -> Option<RawPacket> {
    let address = neoteletext_core::packet::decode_address(packet).ok()?;
    let data = packet.data();
    let (&first, body) = data.split_first()?;
    let designation = hamming84(first)?;
    let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
    for (triplet, chunk) in triplets.iter_mut().zip(body.as_chunks::<3>().0) {
        *triplet = Triplet::from_value(hamming2418(*chunk).unwrap_or(0));
    }
    Some(RawPacket { number: address.number, designation, body: RawBody::Triplets(triplets) })
}

/// The character that carries six bits of a packed TTI line (vbit2's form,
/// REQ-INP-023): bit 6 set, so every character is printable.
const fn packed_character(value: u32) -> u8 {
    TTI_PACKED_CHARACTER_BASE | (value & TTI_TRIPLET_CHARACTER_MASK) as u8
}

/// The 39 packed characters of thirteen triplets: three characters each,
/// six bits per character, the least significant first (REQ-INP-023).
#[must_use]
pub fn pack_triplets(triplets: &[Triplet; TRIPLETS_PER_PACKET]) -> [u8; TTI_PACKED_CHARACTERS] {
    let mut out = [TTI_PACKED_CHARACTER_BASE; TTI_PACKED_CHARACTERS];
    for (chunk, triplet) in out.as_chunks_mut::<3>().0.iter_mut().zip(triplets) {
        let value = triplet.value();
        *chunk = [
            packed_character(value),
            packed_character(value >> TTI_TRIPLET_CHARACTER_BITS),
            packed_character(value >> (2 * TTI_TRIPLET_CHARACTER_BITS)),
        ];
    }
    out
}

/// The thirteen triplets of up to 39 packed characters; missing characters
/// count as 0. The inverse of [`pack_triplets`].
#[must_use]
pub fn unpack_triplets(characters: &[u8]) -> [Triplet; TRIPLETS_PER_PACKET] {
    let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
    let (chunks, _) = characters.as_chunks::<3>();
    for (triplet, &[first, second, third]) in triplets.iter_mut().zip(chunks) {
        let value = (u32::from(first) & TTI_TRIPLET_CHARACTER_MASK)
            | ((u32::from(second) & TTI_TRIPLET_CHARACTER_MASK) << TTI_TRIPLET_CHARACTER_BITS)
            | ((u32::from(third) & TTI_TRIPLET_CHARACTER_MASK) << (2 * TTI_TRIPLET_CHARACTER_BITS));
        *triplet = Triplet::from_value(value);
    }
    triplets
}
