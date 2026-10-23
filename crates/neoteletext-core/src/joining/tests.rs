//! Tests of the Arabic-script joining pass (REQ-CHR-015).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::charset::{CharRef, G0Set, joined_form};
use crate::level1::Cell;
use std::vec::Vec;

const PERSIAN: CharSet = CharSet::G0(G0 { set: G0Set::Persian, national: None });

/// A row holding `text`, already in visual order, from column 0.
fn row(text: &str) -> [Cell; COLUMNS] {
    let mut row = [Cell::BLANK; COLUMNS];
    for (cell, letter) in row.iter_mut().zip(text.chars()) {
        let code = (0x20..0x7F).find(|&code| to_unicode(CharRef::new(PERSIAN, code)).base == letter).unwrap();
        cell.glyph = Glyph::Char(CharRef::new(PERSIAN, code));
    }
    row
}

fn forms(row: &[Cell; COLUMNS], length: usize) -> Vec<JoinForm> {
    row.iter()
        .take(length)
        .map(|cell| match cell.glyph {
            Glyph::Char(character) => character.form,
            _ => JoinForm::Isolated,
        })
        .collect()
}

/// سلام in visual order is م ا ل س: seen final... read right to left,
/// sin initial, lam medial, alef final, meem isolated (alef does not
/// join what follows).
#[test]
fn a_word_joins_right_to_left() {
    let mut cells = row("مالس");
    join_row(&mut cells);
    assert_eq!(forms(&cells, 4), [JoinForm::Isolated, JoinForm::Final, JoinForm::Medial, JoinForm::Initial]);
    let Glyph::Char(sin) = cells[3].glyph else { panic!() };
    assert_eq!(joined_form('\u{0633}', sin.form), '\u{FEB3}', "sin initial");
}

/// Joining stops at a space and at a non-joining letter; lam-alef is
/// right-joining; a Pashto letter without a presentation form draws
/// from the private-use area.
#[test]
fn breaks_ligature_and_private_forms() {
    // Visual "ب ب" (two words): both isolated.
    let mut cells = row("ب ب");
    join_row(&mut cells);
    assert_eq!(forms(&cells, 3), [JoinForm::Isolated; 3]);
    // Visual: lam-alef, beh: beh initial, lam-alef final.
    let mut cells = row("\u{FEFB}ب");
    join_row(&mut cells);
    assert_eq!(forms(&cells, 2), [JoinForm::Final, JoinForm::Initial]);
    assert_eq!(joined_form('\u{FEFB}', JoinForm::Final), '\u{FEFC}');
    // Hamza joins nothing: visual "ب ء ب" without spaces.
    let mut cells = row("بءب");
    join_row(&mut cells);
    assert_eq!(forms(&cells, 3), [JoinForm::Isolated; 3]);
    // Pashto tteh (U+067C) has no presentation forms.
    assert!(('\u{F0000}'..='\u{F00FF}').contains(&joined_form('\u{067C}', JoinForm::Medial)));
    assert_eq!(joined_form('\u{067C}', JoinForm::Isolated), '\u{067C}');
}
