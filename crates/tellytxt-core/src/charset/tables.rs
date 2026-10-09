//! Character set tables: placeholder until transcribed from EN 300 706.

use super::{CharSet, Designation, G0, G0Set, G2Set, NationalOption};

/// Mask of the national option bits of a Table 32 value.
pub(super) const NATIONAL_BITS_MASK: u8 = 0x07;

/// English Latin G0 with Latin G2: the last-resort fallback.
pub(super) const ENGLISH_DESIGNATION: Designation = Designation {
    g0: G0 { set: G0Set::Latin, national: Some(NationalOption::English) },
    g2: G2Set::Latin,
};

/// The 13 national option positions of Table 36.
pub(super) const NATIONAL_POSITIONS: [u8; 13] =
    [0x23, 0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x7B, 0x7C, 0x7D, 0x7E];

pub(super) fn designation(_value: u8) -> Option<Designation> {
    None
}

pub(super) fn second_g0(_value: u8) -> Option<G0> {
    None
}

pub(super) fn base_unicode(_set: CharSet, code: u8) -> char {
    char::from(code)
}

pub(super) fn diacritic_combining(_mark: u8) -> Option<char> {
    None
}

pub(super) fn mosaic_unicode(_pattern: u8, _separated: bool) -> char {
    ' '
}

pub(super) fn compose(_base: char, _combining: char) -> Option<char> {
    None
}
