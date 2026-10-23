//! Row codings (Table 3, REQ-INP-023): the rows X/1 to X/25 of a page
//! coded as its page coding says, and their codes read back.

use neoteletext_core::consts::{
    COLUMNS, DATA_BYTES, NIBBLE_MASK, PAGE_FUNCTION_BASIC, PAGE_MOT, SPACE, TRIPLETS_PER_PACKET,
};
use neoteletext_core::ecc::{hamming84, hamming84_encode, hamming2418, hamming2418_encode, with_odd_parity};
use neoteletext_core::packet::Triplet;

use crate::consts::{PAGE_CODING_NIBBLES, PAGE_CODING_TRIPLETS, TTI_PACKED_CHARACTER_BASE};
use crate::page::PageFile;

use super::{pack_triplets, unpack_triplets};

/// How the rows X/1 to X/25 of a page are coded (Table 3, REQ-INP-023).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum RowCoding {
    /// Seven data bits and odd parity (coding 000): display pages. Codings this encoder cannot write (8-bit data, and the mixed
    /// codings 100 and 101) are sent this way too.
    Parity,
    /// Coding 010: byte 6 Hamming 8/4, then thirteen Hamming 24/18
    /// triplets. Object pages.
    Triplets,
    /// Coding 011: every byte Hamming 8/4. The Magazine Organization Table.
    Nibbles,
}

impl RowCoding {
    /// The coding of a page coding value (Table 3).
    #[must_use]
    pub const fn from_coding(coding: u8) -> Self {
        match coding {
            PAGE_CODING_TRIPLETS => Self::Triplets,
            PAGE_CODING_NIBBLES => Self::Nibbles,
            _ => Self::Parity,
        }
    }
}

/// The coding of a page's rows: from its page function line (`PF`, or the
/// format of X/28/0, X/28/3 or X/28/4), and for page mFE without one the
/// Hamming 8/4 coding that clause 10.6.1 gives every Magazine Organization
/// Table.
#[must_use]
pub fn row_coding(page: &PageFile) -> RowCoding {
    match page.function {
        Some(function) => RowCoding::from_coding(function.coding),
        None if page.address.page.0 == PAGE_MOT => RowCoding::Nibbles,
        None => RowCoding::Parity,
    }
}

/// True for a page "intended for further processing" (Annex B.2 item 3):
/// one whose page function is not "basic Level 1 page", or page mFE.
#[must_use]
pub fn is_enhancement_page(page: &PageFile) -> bool {
    page.address.page.0 == PAGE_MOT || page.function.is_some_and(|function| function.function != PAGE_FUNCTION_BASIC)
}

/// The forty transmitted bytes of a row given as 7-bit codes, coded as
/// `coding` says.
#[must_use]
pub fn row_bytes(coding: RowCoding, codes: &[u8; COLUMNS]) -> [u8; DATA_BYTES] {
    let mut bytes = [0; DATA_BYTES];
    match coding {
        RowCoding::Parity => {
            for (byte, &code) in bytes.iter_mut().zip(codes) {
                *byte = with_odd_parity(code);
            }
        }
        RowCoding::Nibbles => {
            for (byte, &code) in bytes.iter_mut().zip(codes) {
                *byte = hamming84_encode(code & NIBBLE_MASK);
            }
        }
        RowCoding::Triplets => {
            let (first, rest) = codes.split_first().map_or((0, &[][..]), |(first, rest)| (*first, rest));
            let triplets = unpack_triplets(rest);
            if let Some((byte_6, body)) = bytes.split_first_mut() {
                *byte_6 = hamming84_encode(first & NIBBLE_MASK);
                for (chunk, triplet) in body.as_chunks_mut::<3>().0.iter_mut().zip(&triplets) {
                    *chunk = hamming2418_encode(triplet.value());
                }
            }
        }
    }
    bytes
}

/// The 7-bit codes of a row as a page file holds them, from its forty
/// received bytes: the inverse of [`row_bytes`]. A byte that fails its
/// check is written as a space for parity-coded rows and as 0 otherwise,
/// so that the result can always be written and read again.
#[must_use]
pub fn row_codes(coding: RowCoding, bytes: &[u8; DATA_BYTES]) -> [u8; COLUMNS] {
    let mut codes = [SPACE; COLUMNS];
    match coding {
        RowCoding::Parity => {
            for (code, &byte) in codes.iter_mut().zip(bytes) {
                let (value, passed) = neoteletext_core::ecc::check_parity(byte);
                *code = if passed { value } else { SPACE };
            }
        }
        RowCoding::Nibbles => {
            for (code, &byte) in codes.iter_mut().zip(bytes) {
                *code = TTI_PACKED_CHARACTER_BASE | hamming84(byte).unwrap_or(0);
            }
        }
        RowCoding::Triplets => {
            let (first, rest) = bytes.split_first().map_or((0, &[][..]), |(first, rest)| (*first, rest));
            let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
            for (triplet, chunk) in triplets.iter_mut().zip(rest.as_chunks::<3>().0) {
                *triplet = Triplet::from_value(hamming2418(*chunk).unwrap_or(0));
            }
            codes = packed_row(hamming84(first).unwrap_or(0), &triplets);
        }
    }
    codes
}

/// A row of an object page (coding 010) as 7-bit codes: byte 6's 4-bit
/// value in the low bits of the first character, then the packed triplets.
#[must_use]
pub fn packed_row(byte_6: u8, triplets: &[Triplet; TRIPLETS_PER_PACKET]) -> [u8; COLUMNS] {
    let mut codes = [TTI_PACKED_CHARACTER_BASE; COLUMNS];
    if let Some((first, rest)) = codes.split_first_mut() {
        *first = TTI_PACKED_CHARACTER_BASE | (byte_6 & NIBBLE_MASK);
        for (slot, character) in rest.iter_mut().zip(pack_triplets(triplets)) {
            *slot = character;
        }
    }
    codes
}

/// A row of a Magazine Organization Table (coding 011) as 7-bit codes:
/// each 4-bit value in the low bits of a character 4/0 to 4/F.
#[must_use]
pub fn nibble_row(values: &[u8; DATA_BYTES]) -> [u8; COLUMNS] {
    values.map(|value| TTI_PACKED_CHARACTER_BASE | (value & NIBBLE_MASK))
}
