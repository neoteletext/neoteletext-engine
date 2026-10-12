//! A font's conformance for each character set (REQ-FNT-011, decision
//! E.75): the rules decided from the glyph table alone, and the SAA5050
//! cell profile, reported and never required.

use super::LoadedFont;
use crate::charset::{CharRef, CharSet, G0, G0Set, G2Set, NationalOption, to_unicode};
use crate::consts::{CELL_FIELD_LINES, CELL_WIDTH, CODE_MASK_7BIT, SPACE};

/// The first code position of a character set (2/0).
const FIRST_CODE: u8 = SPACE;
/// The last code position of a character set (7/F).
const LAST_CODE: u8 = CODE_MASK_7BIT;
/// The code positions of a character set: 2/0 to 7/F.
pub const SET_POSITIONS: u8 = LAST_CODE - FIRST_CODE + 1;
/// The leftmost dot column of a 12-pixel line, pixels 11 and 10, blank in
/// the SAA5050's cell (Part C.3: one dot space between characters).
const LEFT_DOT_COLUMN: u16 = 0b11 << (CELL_WIDTH - 2);

const fn latin(national: Option<NationalOption>) -> CharSet {
    CharSet::G0(G0 { set: G0Set::Latin, national })
}

const fn g0(set: G0Set) -> CharSet {
    CharSet::G0(G0 { set, national: None })
}

/// The 24 character sets fonts draw, with the names libraries and reports
/// give them, in the order of REQ-FNT-011. A set's place is its bit in the
/// masks of [`conformance_masks`].
pub const FONT_SETS: [(&str, CharSet); 24] = [
    ("G0-Latin", latin(None)),
    ("G0-Latin-English", latin(Some(NationalOption::English))),
    ("G0-Latin-German", latin(Some(NationalOption::German))),
    ("G0-Latin-SwedishFinnishHungarian", latin(Some(NationalOption::SwedishFinnishHungarian))),
    ("G0-Latin-Italian", latin(Some(NationalOption::Italian))),
    ("G0-Latin-French", latin(Some(NationalOption::French))),
    ("G0-Latin-PortugueseSpanish", latin(Some(NationalOption::PortugueseSpanish))),
    ("G0-Latin-CzechSlovak", latin(Some(NationalOption::CzechSlovak))),
    ("G0-Latin-Polish", latin(Some(NationalOption::Polish))),
    ("G0-Latin-Turkish", latin(Some(NationalOption::Turkish))),
    ("G0-Latin-SerbianCroatianSlovenian", latin(Some(NationalOption::SerbianCroatianSlovenian))),
    ("G0-Latin-Rumanian", latin(Some(NationalOption::Rumanian))),
    ("G0-Latin-Estonian", latin(Some(NationalOption::Estonian))),
    ("G0-Latin-LettishLithuanian", latin(Some(NationalOption::LettishLithuanian))),
    ("G0-Cyrillic-1", g0(G0Set::Cyrillic1)),
    ("G0-Cyrillic-2", g0(G0Set::Cyrillic2)),
    ("G0-Cyrillic-3", g0(G0Set::Cyrillic3)),
    ("G0-Greek", g0(G0Set::Greek)),
    ("G0-Arabic", g0(G0Set::Arabic)),
    ("G0-Hebrew", g0(G0Set::Hebrew)),
    ("G2-Latin", CharSet::G2(G2Set::Latin)),
    ("G2-Cyrillic", CharSet::G2(G2Set::Cyrillic)),
    ("G2-Greek", CharSet::G2(G2Set::Greek)),
    ("G2-Arabic", CharSet::G2(G2Set::Arabic)),
];

/// The place in [`FONT_SETS`] of a set's name.
#[must_use]
pub fn font_set_index(name: &str) -> Option<usize> {
    FONT_SETS.iter().position(|(known, _)| *known == name)
}

/// The place in [`FONT_SETS`] of a character set.
#[must_use]
pub fn font_set_of(set: CharSet) -> Option<usize> {
    FONT_SETS.iter().position(|(_, known)| *known == set)
}

/// The character a code position holds in a set, by the character set
/// tables (REQ-CHR-010); a G2 set's marks in their spacing forms. `None`
/// for a position the standard leaves unassigned, which is drawn as the
/// space.
#[must_use]
pub fn assigned(set: CharSet, code: u8) -> Option<char> {
    let character = to_unicode(CharRef::new(set, code)).base;
    (code == FIRST_CODE || character != ' ').then_some(character)
}

/// The rule of REQ-FNT-011 a font first fails for a set, in code order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Failure {
    /// Rule 2: no glyph for the character at this code position.
    Missing(u8),
    /// Rule 3: the space is not blank, or missing.
    Space,
    /// Rule 4: the character at this code position has no pixel set.
    Blank(u8),
}

/// A font's conformance for one character set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SetConformance {
    /// Code positions drawn as the rules want them, of
    /// [`SET_POSITIONS`]: unassigned positions count when the space is blank.
    pub drawn: u8,
    /// The first rule that fails, or none when the font conforms.
    pub failure: Option<Failure>,
    /// Every glyph of the set's assigned characters the font has leaves
    /// the top line and the leftmost dot column blank.
    pub saa5050_cell: bool,
}

impl SetConformance {
    /// True when the font conforms for the set.
    #[must_use]
    pub const fn conforms(&self) -> bool {
        self.failure.is_none()
    }
}

/// Rows of a glyph, by character.
fn glyph(font: &LoadedFont<'_>, character: char) -> Option<[u16; CELL_FIELD_LINES]> {
    font.lookup(character)
}

/// A font's conformance for a character set (REQ-FNT-011 rules 2 to 4; rule
/// 1 is the reader's, rule 5 holds because only the glyphs are read).
#[must_use]
pub fn conformance(font: &LoadedFont<'_>, set: CharSet) -> SetConformance {
    let space_blank = glyph(font, ' ').is_some_and(|rows| rows.iter().all(|&row| row == 0));
    let mut drawn = 0u8;
    let mut failure = None;
    let mut saa5050_cell = true;
    for code in FIRST_CODE..=LAST_CODE {
        let fault = match assigned(set, code) {
            None | Some(' ') => (!space_blank).then_some(Failure::Space),
            Some(character) => match glyph(font, character) {
                None => Some(Failure::Missing(code)),
                Some(rows) => {
                    saa5050_cell &= rows.first() == Some(&0) && rows.iter().all(|row| row & LEFT_DOT_COLUMN == 0);
                    rows.iter().all(|&row| row == 0).then_some(Failure::Blank(code))
                }
            },
        };
        match fault {
            None => drawn = drawn.saturating_add(1),
            Some(fault) => failure = failure.or(Some(fault)),
        }
    }
    SetConformance { drawn, failure, saa5050_cell }
}

/// The sets of [`FONT_SETS`] a font conforms for, and of those, the sets for
/// which it also meets the SAA5050 cell profile, each as a bit per set.
#[must_use]
pub fn conformance_masks(font: &LoadedFont<'_>) -> (u32, u32) {
    let mut conforms = 0u32;
    let mut cell = 0u32;
    for (bit, (_, set)) in FONT_SETS.iter().enumerate() {
        let result = conformance(font, *set);
        if result.conforms() {
            conforms |= 1u32.checked_shl(u32::try_from(bit).unwrap_or(u32::MAX)).unwrap_or(0);
            if result.saa5050_cell {
                cell |= 1u32.checked_shl(u32::try_from(bit).unwrap_or(u32::MAX)).unwrap_or(0);
            }
        }
    }
    (conforms, cell)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::types::FontGlyph;
    use std::vec::Vec;

    /// A glyph with one dot, away from the top line and the left column.
    const INKED: [u16; CELL_FIELD_LINES] = [0, 0x0C0, 0x0C0, 0, 0, 0, 0, 0, 0, 0];

    /// A font of every assigned character of a set, each inked, and a
    /// blank space; sorted, as the reader keeps a table.
    fn complete(set: CharSet) -> Vec<FontGlyph> {
        let mut glyphs: Vec<FontGlyph> = (FIRST_CODE..=LAST_CODE)
            .filter_map(|code| assigned(set, code))
            .map(|c| FontGlyph { code_point: u32::from(c), rows: if c == ' ' { [0; CELL_FIELD_LINES] } else { INKED } })
            .collect();
        glyphs.sort_by_key(|glyph| glyph.code_point);
        glyphs.dedup_by_key(|glyph| glyph.code_point);
        glyphs
    }

    fn check(glyphs: &[FontGlyph], set: CharSet) -> SetConformance {
        conformance(&LoadedFont::new(glyphs).unwrap(), set)
    }

    /// Every set, complete, conforms with all 96 positions drawn.
    #[test]
    fn a_complete_font_conforms() {
        for (name, set) in FONT_SETS {
            let result = check(&complete(set), set);
            assert_eq!((result.drawn, result.failure), (SET_POSITIONS, None), "{name}");
            assert!(result.saa5050_cell);
        }
    }

    /// Rules 2, 3 and 4, each failing.
    #[test]
    fn each_rule_fails() {
        let set = FONT_SETS[font_set_index("G0-Latin-German").unwrap()].1;
        let mut glyphs = complete(set);
        // Rule 2: the German option's Ä at 5/B removed.
        let without: Vec<FontGlyph> = glyphs.iter().copied().filter(|g| g.code_point != 0xC4).collect();
        assert_eq!(check(&without, set).failure, Some(Failure::Missing(0x5B)));
        assert_eq!(check(&without, set).drawn, SET_POSITIONS - 1);
        // Rule 4: A drawn blank.
        let a = glyphs.iter_mut().find(|g| g.code_point == u32::from('A')).unwrap();
        a.rows = [0; CELL_FIELD_LINES];
        assert_eq!(check(&glyphs, set).failure, Some(Failure::Blank(0x41)));
        // Rule 3: an inked space.
        let mut glyphs = complete(set);
        glyphs[0].rows = INKED;
        assert_eq!(check(&glyphs, set).failure, Some(Failure::Space));
    }

    /// A G2 set's unassigned positions need only the blank space, and its
    /// marks count in their spacing forms.
    #[test]
    fn g2_unassigned_positions_and_marks() {
        let set = CharSet::G2(G2Set::Latin);
        assert_eq!(assigned(set, 0x40), None, "4/0 is unassigned");
        assert_eq!(assigned(set, 0x41), Some('`'), "the grave accent in its spacing form");
        let glyphs = complete(set);
        assert!(glyphs.iter().all(|g| g.code_point != 0x0300), "no combining mark is needed");
        assert!(check(&glyphs, set).conforms());
    }

    /// The SAA5050 cell profile is reported, not required.
    #[test]
    fn the_saa5050_cell_profile() {
        let set = latin(Some(NationalOption::English));
        let mut glyphs = complete(set);
        let a = glyphs.iter_mut().find(|g| g.code_point == u32::from('A')).unwrap();
        a.rows[0] = 0x0C0;
        let top = check(&glyphs, set);
        assert!(top.conforms() && !top.saa5050_cell, "a mark on the top line");
        let mut glyphs = complete(set);
        let a = glyphs.iter_mut().find(|g| g.code_point == u32::from('A')).unwrap();
        a.rows[3] = 0x800;
        assert!(!check(&glyphs, set).saa5050_cell, "a pixel in the leftmost dot column");
    }

    /// The masks have a bit per set, in the order of the names.
    #[test]
    fn masks_and_names() {
        let english = latin(Some(NationalOption::English));
        let glyphs = complete(english);
        let (conforms, cell) = conformance_masks(&LoadedFont::new(&glyphs).unwrap());
        let bit = 1 << font_set_index("G0-Latin-English").unwrap();
        assert_ne!(conforms & bit, 0);
        assert_eq!(cell & bit, bit);
        assert_eq!(font_set_of(english), font_set_index("G0-Latin-English"));
        assert_eq!(FONT_SETS.len(), 24);
        assert_eq!(font_set_index("G0-Latin-Klingon"), None);
    }
}
