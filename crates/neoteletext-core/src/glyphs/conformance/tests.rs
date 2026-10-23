//! Tests of the conformance of a loaded font to a character set (REQ-FNT-003).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::consts::CELL_FIELD_LINES;
use crate::types::FontGlyph;
use std::vec::Vec;

/// A glyph with one dot, away from the top line and the left column.
const INKED: [u16; CELL_FIELD_LINES] = [0, 0x0C0, 0x0C0, 0, 0, 0, 0, 0, 0, 0];

/// A font of every assigned character of a set, each inked, and a
/// blank space; sorted, as the reader keeps a table.
fn complete(set: CharSet) -> Vec<FontGlyph> {
    let mut glyphs: Vec<FontGlyph> = (FIRST_CODE..=LAST_CODE)
        .filter_map(|code| assigned(set, code))
        .map(|c| FontGlyph::small(u32::from(c), if c == ' ' { [0; CELL_FIELD_LINES] } else { INKED }))
        .collect();
    // A joined set's letters in every form (REQ-CHR-015).
    if matches!(set, CharSet::G0(G0 { set, .. }) if set.is_joined()) {
        let letters: Vec<char> = (FIRST_CODE..=LAST_CODE).filter_map(|code| assigned(set, code)).collect();
        glyphs.extend(
            crate::charset::joined_forms()
                .iter()
                .filter(|(letter, _, _)| letters.contains(letter))
                .map(|&(_, _, point)| FontGlyph::small(u32::from(point), INKED)),
        );
    }
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

/// REQ-CHR-015: a font for a joined set needs every form of every
/// joining letter; sin's initial form missing fails at sin's position.
/// The joined sets are neoteletext's own.
#[cfg(feature = "extended-sets")]
#[test]
fn a_missing_joined_form_fails() {
    let set = FONT_SETS[font_set_index("G0-Arabic-Persian").unwrap()].1;
    let without: Vec<FontGlyph> = complete(set).into_iter().filter(|g| g.code_point != 0xFEB3).collect();
    let sin = (FIRST_CODE..=LAST_CODE).find(|&code| assigned(set, code) == Some('\u{0633}')).unwrap();
    assert_eq!(check(&without, set).failure, Some(Failure::MissingForm(sin)));
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
    a.rows = [0; CELL_HEIGHT];
    assert_eq!(check(&glyphs, set).failure, Some(Failure::Blank(0x41)));
    // Rule 3: an inked space.
    let mut glyphs = complete(set);
    glyphs[0] = FontGlyph::small(glyphs[0].code_point, INKED);
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

/// REQ-FNT-011 as amended by decision E.77: a complete 12 by 20 font
/// conforms, and the SAA5050 cell profile is never reported for it.
#[test]
fn a_tall_font_conforms_without_the_saa5050_profile() {
    let set = latin(Some(NationalOption::English));
    let tall: Vec<FontGlyph> = complete(set)
        .iter()
        .map(|glyph| {
            let mut rows = [0; CELL_HEIGHT];
            if glyph.code_point != u32::from(' ') {
                rows[10] = 0x0C0;
            }
            FontGlyph::tall(glyph.code_point, rows)
        })
        .collect();
    let result = check(&tall, set);
    assert!(result.conforms(), "ink only below the tenth line still counts");
    assert!(!result.saa5050_cell);
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
    for (place, (name, set)) in FONT_SETS.iter().enumerate() {
        assert_eq!(font_set_of(*set), Some(place), "{name}");
    }
    assert_eq!(font_set_index("G0-Latin-Klingon"), None);
}
