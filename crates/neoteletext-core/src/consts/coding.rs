//! Byte coding (clause 8): parity, Hamming 8/4 nibbles and Hamming 24/18
//! triplets.

/// Mask of the seven data bits of an odd-parity byte (clause 8.1).
pub const CODE_MASK_7BIT: u8 = 0x7F;
/// The parity bit of an odd-parity byte.
pub const PARITY_BIT: u8 = 0x80;
/// Mask of a 4-bit Hamming 8/4 value.
pub const NIBBLE_MASK: u8 = 0x0F;
/// Bits in a Hamming 24/18 triplet.
pub const TRIPLET_BITS: u32 = 24;
/// Data bits in a Hamming 24/18 triplet.
pub const TRIPLET_DATA_BITS: u32 = 18;
/// Mask of the 18 data bits of a triplet.
pub const TRIPLET_DATA_MASK: u32 = (1 << TRIPLET_DATA_BITS) - 1;
/// Bytes in a triplet.
pub const TRIPLET_BYTES: usize = 3;
/// Triplets in packets X/26, X/28 and M/29 (clause 9.4.1).
pub const TRIPLETS_PER_PACKET: usize = 13;
/// Designation codes 0 to 15.
pub const DESIGNATION_CODES: usize = 16;
