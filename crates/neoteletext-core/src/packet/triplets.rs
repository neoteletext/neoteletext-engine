//! Enhancement triplets and packet X/26 (clauses 9.4.1 and 12.3.1,
//! REQ-PKT-004): the triplet, the thirteen triplets of a packet, and their
//! Hamming 24/18 values, decoded and encoded.

use super::{Packet, TripletValues};
use crate::consts::{
    DATA_START, TRIPLET_ADDRESS_BITS, TRIPLET_ADDRESS_MASK, TRIPLET_BYTES, TRIPLET_DATA_FIELD_MASK, TRIPLET_MODE_BITS,
    TRIPLET_MODE_MASK, TRIPLETS_PER_PACKET,
};
use crate::ecc::hamming2418;
use crate::types::Magazine;

/// An enhancement triplet (clause 12.3.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Triplet {
    /// The 6-bit address: 0 to 39 a column, 40 to 63 a row.
    pub address: u8,
    /// The 5-bit mode.
    pub mode: u8,
    /// The 7-bit data.
    pub data: u8,
}

impl Triplet {
    /// Splits an 18-bit triplet value into its fields (clause 12.3.1).
    #[must_use]
    pub const fn from_value(value: u32) -> Self {
        Self {
            address: (value & TRIPLET_ADDRESS_MASK) as u8,
            mode: ((value >> TRIPLET_ADDRESS_BITS) & TRIPLET_MODE_MASK) as u8,
            data: ((value >> (TRIPLET_ADDRESS_BITS + TRIPLET_MODE_BITS)) & TRIPLET_DATA_FIELD_MASK) as u8,
        }
    }

    /// The 18-bit value of the fields.
    #[must_use]
    pub const fn value(self) -> u32 {
        (self.address as u32 & TRIPLET_ADDRESS_MASK)
            | ((self.mode as u32 & TRIPLET_MODE_MASK) << TRIPLET_ADDRESS_BITS)
            | ((self.data as u32 & TRIPLET_DATA_FIELD_MASK) << (TRIPLET_ADDRESS_BITS + TRIPLET_MODE_BITS))
    }

    /// True for a row address triplet (address 40 to 63).
    #[must_use]
    pub const fn is_row_address(self) -> bool {
        self.address >= crate::consts::ROW_ADDRESS_BASE
    }
}

/// Thirteen triplets, each `None` when its Hamming 24/18 check failed.
pub type Triplets = [Option<Triplet>; TRIPLETS_PER_PACKET];

/// An enhancement packet X/26 (REQ-PKT-004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enhancement {
    /// The magazine.
    pub magazine: Magazine,
    /// The designation code, 0 to 15.
    pub designation: u8,
    /// The triplets.
    pub triplets: Triplets,
}

/// The thirteen triplet values of bytes that follow byte 6.
fn values_of(body: &[u8]) -> TripletValues {
    let mut result = [None; TRIPLETS_PER_PACKET];
    let (chunks, _) = body.as_chunks::<TRIPLET_BYTES>();
    for (slot, chunk) in result.iter_mut().zip(chunks) {
        *slot = hamming2418(*chunk);
    }
    result
}

/// The thirteen triplets of bytes that follow byte 6.
pub(super) fn triplets_of(body: &[u8]) -> Triplets {
    values_of(body).map(|value| value.map(Triplet::from_value))
}

/// Packet bytes 7 to 45.
fn triplet_body(packet: &Packet) -> &[u8] {
    packet.bytes.get(DATA_START.saturating_add(1)..).unwrap_or(&[])
}

/// The thirteen triplets of packet bytes 7 to 45.
pub(super) fn triplets(packet: &Packet) -> Triplets {
    triplets_of(triplet_body(packet))
}

/// The thirteen triplet values of packet bytes 7 to 45.
pub(super) fn triplet_values(packet: &Packet) -> TripletValues {
    values_of(triplet_body(packet))
}

/// The encoders of triplet packets: the inverse of [`triplets`].
pub(super) mod encode {
    use super::super::encode::{address, set};
    use super::{Packet, Triplet};
    use crate::consts::{
        DATA_START, NIBBLE_MASK, OBJECT_POINTER_BITS, OBJECT_POINTER_UNUSED, TRIPLET_BYTES, TRIPLETS_PER_PACKET,
    };
    use crate::ecc::{hamming84_encode, hamming2418_encode};
    use crate::types::Magazine;

    /// A packet carrying a designation code and thirteen triplets; `None`
    /// triplets are written as the Termination Marker's neighbour, an empty
    /// value 0.
    #[must_use]
    pub fn triplet_packet(
        magazine: Magazine,
        number: u8,
        designation: u8,
        triplets: &[Triplet; TRIPLETS_PER_PACKET],
    ) -> Packet {
        let mut packet = address(magazine, number);
        set(&mut packet, DATA_START, hamming84_encode(designation & NIBBLE_MASK));
        for (index, triplet) in triplets.iter().enumerate() {
            let start = DATA_START.saturating_add(1).saturating_add(index.saturating_mul(TRIPLET_BYTES));
            let [a, b, c] = hamming2418_encode(triplet.value());
            set(&mut packet, start, a);
            set(&mut packet, start.saturating_add(1), b);
            set(&mut packet, start.saturating_add(2), c);
        }
        packet
    }

    /// A packet carrying a designation code and thirteen 18-bit triplet
    /// values.
    pub(in crate::packet) fn value_packet(
        magazine: Magazine,
        number: u8,
        designation: u8,
        values: &[u32; TRIPLETS_PER_PACKET],
    ) -> Packet {
        triplet_packet(magazine, number, designation, &values.map(Triplet::from_value))
    }

    /// A pointer triplet of an object page: `even` in bits 1 to 9 and `odd`
    /// in bits 10 to 18 (clause 10.5.1.2).
    #[must_use]
    pub const fn pointer_triplet(even: u16, odd: u16) -> Triplet {
        let mask = OBJECT_POINTER_UNUSED as u32;
        Triplet::from_value((even as u32 & mask) | (odd as u32 & mask) << OBJECT_POINTER_BITS)
    }
}
