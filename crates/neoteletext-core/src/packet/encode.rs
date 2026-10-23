//! The encoders: one packet of each kind, built from its fields.

use super::Packet;
use crate::consts::{ADDRESS_BYTE_0, ADDRESS_BYTE_1, DATA_BYTES, DATA_START, MAGAZINE_MASK, NIBBLE_MASK, PACKET_BYTES};
use crate::ecc::hamming84_encode;
use crate::types::Magazine;

pub use super::compositional_links::encode::compositional_links;
pub use super::editorial_links::encode::links;
pub use super::header::encode::header;
pub use super::magazine_table::encode::{mot_object_link, mot_page_entry, mot_totals};
pub use super::page_data::encode::{designation_triplets, magazine_enhancement, page_enhancement, page_function};
pub use super::triplets::encode::{pointer_triplet, triplet_packet};

pub(super) fn set(packet: &mut Packet, index: usize, value: u8) {
    if let Some(byte) = packet.bytes.get_mut(index) {
        *byte = value;
    }
}

/// A packet with the given address and all data bytes zero.
#[must_use]
pub fn address(magazine: Magazine, number: u8) -> Packet {
    let mut packet = Packet { bytes: [0; PACKET_BYTES] };
    let first = (magazine.bits() & MAGAZINE_MASK) | ((number & 1) << 3);
    set(&mut packet, ADDRESS_BYTE_0, hamming84_encode(first));
    set(&mut packet, ADDRESS_BYTE_1, hamming84_encode((number >> 1) & NIBBLE_MASK));
    packet
}

/// A display row. `bytes` are 40 bytes already carrying parity.
#[must_use]
pub fn row(magazine: Magazine, row: u8, bytes: &[u8; DATA_BYTES]) -> Packet {
    let mut packet = address(magazine, row);
    if let Some(target) = packet.bytes.get_mut(DATA_START..) {
        target.copy_from_slice(bytes);
    }
    packet
}

/// A packet whose forty data bytes are Hamming 8/4 coded, such as a
/// packet of the Magazine Organization Table (clause 10.6.1). `values`
/// are the 4-bit values before coding.
#[must_use]
pub fn nibble_packet(magazine: Magazine, number: u8, values: &[u8; DATA_BYTES]) -> Packet {
    let mut packet = address(magazine, number);
    for (index, &value) in values.iter().enumerate() {
        set(&mut packet, DATA_START.saturating_add(index), hamming84_encode(value & NIBBLE_MASK));
    }
    packet
}

/// The 40 raw display bytes of a packet, for building raw packets.
#[must_use]
pub fn raw(magazine: Magazine, number: u8, designation: u8, body: &[u8; DATA_BYTES - 1]) -> Packet {
    let mut packet = address(magazine, number);
    set(&mut packet, DATA_START, hamming84_encode(designation & NIBBLE_MASK));
    if let Some(target) = packet.bytes.get_mut(DATA_START.saturating_add(1)..) {
        target.copy_from_slice(body);
    }
    packet
}
