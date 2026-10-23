//! The packets of object pages (clause 10.5.1, Figure 19; REQ-PKT-017),
//! read from the 40 stored data bytes (packet bytes 6 to 45) of a packet,
//! which the page store keeps as received (decision E.56).

use super::Triplets;
use super::triplets::triplets_of;
use crate::consts::{
    DATA_BYTES, OBJECT_DEFINITION_FIRST_PACKET, OBJECT_POINTER_BITS, OBJECT_POINTER_DATA_BIT, OBJECT_POINTER_UNUSED,
    OBJECT_TRIPLET_LAST, OBJECT_TRIPLETS_IN_ROWS, TRIPLETS_PER_PACKET,
};
use crate::ecc::hamming84;

/// A packet X/1 to X/25 or X/26 of an object page (REQ-PKT-017).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ObjectPacket {
    /// Byte 6, Hamming 8/4: for packets 1 to 4, bit B1 says whether the
    /// packet carries pointers; for X/26 it is the designation code. `None`
    /// when uncorrectable.
    pub byte_6: Option<u8>,
    /// The thirteen triplets, each `None` when uncorrectable.
    pub triplets: Triplets,
}

impl ObjectPacket {
    /// Decodes the 40 stored data bytes of a packet.
    #[must_use]
    pub fn new(data: &[u8; DATA_BYTES]) -> Self {
        let (first, rest) = data.split_first().map_or((0, &[][..]), |(first, rest)| (*first, rest));
        Self { byte_6: hamming84(first), triplets: triplets_of(rest) }
    }

    /// True when byte 6 bit B1 marks pointer data (clause 10.5.1.2). This
    /// is meaningful for packets 1 to 4 only.
    #[must_use]
    pub fn carries_pointers(&self) -> bool {
        self.byte_6.is_some_and(|byte| byte & OBJECT_POINTER_DATA_BIT != 0)
    }

    /// The pointer in triplet `triplet` (1 to 12): bits 1 to 9 when `odd`
    /// is false (the even-numbered object), bits 10 to 18 when it is true.
    /// Triplet 0 is reserved, and an uncorrectable triplet makes both of its
    /// pointers unused (decision E.51): both give
    /// [`OBJECT_POINTER_UNUSED`].
    #[must_use]
    pub fn pointer(&self, triplet: usize, odd: bool) -> u16 {
        if triplet == 0 {
            return OBJECT_POINTER_UNUSED;
        }
        match self.triplets.get(triplet).copied().flatten() {
            Some(value) => {
                let shift = if odd { OBJECT_POINTER_BITS } else { 0 };
                (value.value() >> shift) as u16 & OBJECT_POINTER_UNUSED
            }
            None => OBJECT_POINTER_UNUSED,
        }
    }
}

/// Where an absolute triplet number of an object page lies
/// (clause 10.5.1.2; REQ-PKT-017).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TripletLocation {
    /// Triplet `triplet` of packet `packet`, 3 to 25.
    Row {
        /// The packet number.
        packet: u8,
        /// The triplet, 0 to 12.
        triplet: u8,
    },
    /// Triplet `triplet` of packet X/26 with designation code `designation`.
    Enhancement {
        /// The designation code, 0 to 15.
        designation: u8,
        /// The triplet, 0 to 12.
        triplet: u8,
    },
}

/// Converts an absolute triplet number 0 to 506 into a packet and a
/// triplet: number t below 299 is triplet t mod 13 of packet 3 + t / 13,
/// and any higher number is triplet (t − 299) mod 13 of X/26 with
/// designation code (t − 299) / 13. `None` above 506.
#[must_use]
pub fn triplet_location(number: u16) -> Option<TripletLocation> {
    if number > OBJECT_TRIPLET_LAST {
        return None;
    }
    Some(match number.checked_sub(OBJECT_TRIPLETS_IN_ROWS) {
        None => TripletLocation::Row {
            packet: OBJECT_DEFINITION_FIRST_PACKET.saturating_add((number / TRIPLETS_PER_PACKET_U16) as u8),
            triplet: (number % TRIPLETS_PER_PACKET_U16) as u8,
        },
        Some(beyond) => TripletLocation::Enhancement {
            designation: (beyond / TRIPLETS_PER_PACKET_U16) as u8,
            triplet: (beyond % TRIPLETS_PER_PACKET_U16) as u8,
        },
    })
}

/// Triplets per packet, in the type of a triplet number.
const TRIPLETS_PER_PACKET_U16: u16 = TRIPLETS_PER_PACKET as u16;
