//! Character sets and their designation (EN 300 706 clause 15, area CHR).
//!
//! Inside the decoder a character is a set identifier and a 7-bit code, never
//! a Unicode value (REQ-CHR-001). Conversion to Unicode happens only at the
//! output stage, through [`to_unicode`].

// Tables keep one source row per line, aligned as in the standard.
#[rustfmt::skip]
mod tables;

use crate::consts::{CODE_MASK_7BIT, DIACRITIC_NONE};

/// A G0 primary character set (EN 300 706 clause 15.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum G0Set {
    /// Latin G0 (Table 35), used with a national option sub-set.
    Latin,
    /// Cyrillic G0 option 1, Serbian and Croatian (Table 38).
    Cyrillic1,
    /// Cyrillic G0 option 2, Russian and Bulgarian (Table 39).
    Cyrillic2,
    /// Cyrillic G0 option 3, Ukrainian (Table 40).
    Cyrillic3,
    /// Greek G0 (Table 42).
    Greek,
    /// Arabic G0 (Table 44).
    Arabic,
    /// Hebrew G0 (Table 46).
    Hebrew,
}

/// A G2 supplementary character set (EN 300 706 clause 15.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum G2Set {
    /// Latin G2 (Table 37).
    Latin,
    /// Cyrillic G2 (Table 41).
    Cyrillic,
    /// Greek G2 (Table 43).
    Greek,
    /// Arabic G2 (Table 45).
    Arabic,
}

/// A Latin national option sub-set (EN 300 706 Table 36).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum NationalOption {
    /// Czech and Slovak.
    CzechSlovak,
    /// English.
    English,
    /// Estonian.
    Estonian,
    /// French.
    French,
    /// German.
    German,
    /// Italian.
    Italian,
    /// Lettish and Lithuanian.
    LettishLithuanian,
    /// Polish.
    Polish,
    /// Portuguese and Spanish.
    PortugueseSpanish,
    /// Rumanian.
    Rumanian,
    /// Serbian, Croatian and Slovenian.
    SerbianCroatianSlovenian,
    /// Swedish, Finnish and Hungarian.
    SwedishFinnishHungarian,
    /// Turkish.
    Turkish,
}

impl NationalOption {
    /// Every national option, in the order of Table 36.
    pub const ALL: [Self; 13] = [
        Self::CzechSlovak,
        Self::English,
        Self::Estonian,
        Self::French,
        Self::German,
        Self::Italian,
        Self::LettishLithuanian,
        Self::Polish,
        Self::PortugueseSpanish,
        Self::Rumanian,
        Self::SerbianCroatianSlovenian,
        Self::SwedishFinnishHungarian,
        Self::Turkish,
    ];
}

/// A G0 set together with the national option applied to it, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct G0 {
    /// The primary set.
    pub set: G0Set,
    /// The national option sub-set; only meaningful with [`G0Set::Latin`].
    pub national: Option<NationalOption>,
}

/// The character set of a stored character (REQ-CHR-001).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CharSet {
    /// A G0 set, with or without national option substitution.
    G0(G0),
    /// The G2 supplementary set.
    G2(G2Set),
    /// The G3 smooth mosaics and line drawing set (Table 48).
    G3,
}

/// One character: a set, a 7-bit code and an optional diacritical mark.
///
/// `diacritic` is the column 4 position of the G2 mark (1 to 15), or
/// [`DIACRITIC_NONE`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct CharRef {
    /// The character set.
    pub set: CharSet,
    /// The 7-bit character code, 0x20 to 0x7F.
    pub code: u8,
    /// The diacritical mark number (REQ-CHR-008), 0 for none.
    pub diacritic: u8,
}

impl CharRef {
    /// A plain character with no mark.
    #[must_use]
    pub const fn new(set: CharSet, code: u8) -> Self {
        Self { set, code: code & CODE_MASK_7BIT, diacritic: DIACRITIC_NONE }
    }
}

/// The default sets of a page: Table 32 resolved (REQ-CHR-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Designation {
    /// The default G0 set and national option.
    pub g0: G0,
    /// The default G2 set.
    pub g2: G2Set,
}

/// A character converted to Unicode: a base scalar and an optional
/// combining mark (REQ-CHR-010, decision E.26).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unicode {
    /// The base character.
    pub base: char,
    /// The combining character to follow it, if any.
    pub combining: Option<char>,
}

/// Looks up Table 32 for a 7-bit designation value: group in bits 3 to 6,
/// national option index in bits 0 to 2. `None` when the entry is reserved.
#[must_use]
pub fn designation(value: u8) -> Option<Designation> {
    tables::designation(value & CODE_MASK_7BIT)
}

/// Resolves Table 32 with the fallback of REQ-CHR-003: a reserved entry is
/// first retried in group 0000 with the same national option bits, and then
/// English is used. The flag is true when a fallback was needed.
#[must_use]
pub fn designation_with_fallback(value: u8) -> (Designation, bool) {
    if let Some(found) = designation(value) {
        return (found, false);
    }
    let national_bits = value & tables::NATIONAL_BITS_MASK;
    if let Some(found) = designation(national_bits) {
        return (found, true);
    }
    (tables::ENGLISH_DESIGNATION, true)
}

/// Looks up Table 33 for a second G0 designation value. `None` when the
/// value is reserved or 0x7F ("no second G0 set required").
#[must_use]
pub fn second_g0(value: u8) -> Option<G0> {
    tables::second_g0(value & CODE_MASK_7BIT)
}

/// Converts a character to Unicode (REQ-CHR-010). Codes outside 0x20 to 0x7F
/// give a space. Characters with no Unicode equivalent give the documented
/// fallback U+FFFD.
#[must_use]
pub fn to_unicode(character: CharRef) -> Unicode {
    let base = tables::base_unicode(character.set, character.code);
    let combining =
        if character.diacritic == DIACRITIC_NONE { None } else { tables::diacritic_combining(character.diacritic) };
    Unicode { base, combining }
}

/// Converts a block mosaic pattern to Unicode (REQ-CHR-005). `pattern` is
/// the 6-bit block pattern n, bit 0 the top-left block.
#[must_use]
pub fn mosaic_to_unicode(pattern: u8, separated: bool) -> char {
    tables::mosaic_unicode(pattern, separated)
}

/// The 6-bit block pattern of a G1 mosaic code (REQ-CHR-005):
/// n = (code AND 0x1F) OR ((code AND 0x40) >> 1).
#[must_use]
pub const fn mosaic_pattern(code: u8) -> u8 {
    (code & crate::consts::MOSAIC_LOW_BITS) | ((code & crate::consts::MOSAIC_BIT6) >> 1)
}

/// True when the national option of a G0 set may substitute this code:
/// the 13 positions of Table 36.
#[must_use]
pub fn is_national_position(code: u8) -> bool {
    tables::NATIONAL_POSITIONS.contains(&(code & CODE_MASK_7BIT))
}

/// Precomposes a base letter and a combining mark where Unicode has a
/// single code point for the pair (decision E.26, Normalization Form C).
#[must_use]
pub fn compose(base: char, combining: char) -> Option<char> {
    tables::compose(base, combining)
}
