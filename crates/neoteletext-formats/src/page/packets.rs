//! The packet-level records of a page file: packets X/26, the other packed
//! enhancement lines, and the X/27/0 links.

use neoteletext_core::consts::TRIPLETS_PER_PACKET;
use neoteletext_core::packet::Triplet;
use neoteletext_core::types::{PageAddress, SubCode};

use crate::consts::TTI_PACKED_CHARACTERS;

/// Packed enhancement lines other than X/26 (`OL,27` with designation 1 to
/// 15, `OL,28`, `OL,29`) per page.
pub const RAW_PACKETS: usize = 16;

/// Number of X/27/0 links (clause 9.6.1).
pub const FASTEXT_LINKS: usize = 6;

/// One packet X/26 of a page.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EnhancementPacket {
    /// The designation code, 0 to 15.
    pub designation: u8,
    /// The thirteen triplets.
    pub triplets: [Triplet; TRIPLETS_PER_PACKET],
}

/// The body of a packed enhancement line other than X/26.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RawBody {
    /// Thirteen triplets (`OL,28`, `OL,29`, `OL,27` designation 4 to 15).
    Triplets([Triplet; TRIPLETS_PER_PACKET]),
    /// Thirty-nine 4-bit values (`OL,27` designation 1 to 3).
    Nibbles([u8; TTI_PACKED_CHARACTERS]),
}

/// A packed enhancement line kept for encoding and writing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RawPacket {
    /// The packet number: 27, 28 or 29.
    pub number: u8,
    /// The designation code.
    pub designation: u8,
    /// The contents.
    pub body: RawBody,
}

/// One X/27/0 link: a page and sub-code.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct FastextLink {
    /// The page.
    pub address: PageAddress,
    /// The sub-code; 3F7F means any sub-page.
    pub subcode: SubCode,
}
