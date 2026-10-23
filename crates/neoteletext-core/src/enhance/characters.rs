//! Enhancement characters (REQ-L25-010, REQ-L25-011): what a character
//! placed by a triplet shows.

use crate::charset::{self, CharRef, CharSet, G0, G0Set, G2Set, mosaic_pattern};
use crate::consts::{G1_G0_COLUMNS_FIRST, G1_G0_COLUMNS_LAST, G3_UNDEFINED, LATIN_AT_SIGN, LATIN_G2_UNDEFINED};
use crate::level1::Glyph;

/// A character written by enhancement data (REQ-L25-010), before its
/// character set is resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum EnhChar {
    /// A G0 character with a diacritical mark (0 for none); `at` for the
    /// "@" of mode 10000 with data 2/A.
    G0 { code: u8, diacritic: u8, at: bool },
    /// A G1 character: a block mosaic, or in columns 4 and 5 the current G0
    /// set's character (Table 47 note 3).
    G1(u8),
    /// A G2 character.
    G2(u8),
    /// A G3 character.
    G3(u8),
}

/// What an enhancement character shows (REQ-L25-010, REQ-L25-011): national
/// options never apply, G1 mosaics are separated by the non-spacing
/// attribute only, and undefined positions show a space (decision E.50).
/// The flag is true for an undefined position.
pub(super) fn enhancement_glyph(character: EnhChar, sets: (G0Set, G2Set), separated: bool) -> (Glyph, bool) {
    let g0 = CharSet::G0(G0 { set: sets.0, national: None });
    let glyph = match character {
        EnhChar::G0 { at: true, .. } => Glyph::Char(CharRef::new(g0, LATIN_AT_SIGN)),
        EnhChar::G0 { code, diacritic, .. } => {
            let mut character = CharRef::new(g0, code);
            character.diacritic = charset::page_mark(sets.1, diacritic);
            Glyph::Char(character)
        }
        EnhChar::G1(code) if (G1_G0_COLUMNS_FIRST..=G1_G0_COLUMNS_LAST).contains(&code) => {
            Glyph::Char(CharRef::new(g0, code))
        }
        EnhChar::G1(code) => Glyph::Mosaic { pattern: mosaic_pattern(code), separated },
        EnhChar::G2(code) if sets.1 == G2Set::Latin && LATIN_G2_UNDEFINED.contains(&code) => {
            return (Glyph::Space, true);
        }
        EnhChar::G2(code) => Glyph::Char(CharRef::new(CharSet::G2(sets.1), code)),
        EnhChar::G3(code) if G3_UNDEFINED.contains(&code) => return (Glyph::Space, true),
        EnhChar::G3(code) => Glyph::Char(CharRef::new(CharSet::G3, code)),
    };
    (glyph, false)
}

/// True for a G0 or G2 character, the only ones that take underline
/// (decisions E.47 and E.50).
pub(super) const fn is_text(glyph: Glyph) -> bool {
    matches!(glyph, Glyph::Char(CharRef { set: CharSet::G0(_) | CharSet::G2(_), .. }))
}
