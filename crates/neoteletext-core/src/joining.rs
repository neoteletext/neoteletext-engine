//! Joining the letters of the Arabic-script sets of group 1001 (REQ-CHR-015,
//! decision E.79, `docs/spec/design/languages.md` section 6).
//!
//! The sets hold plain letters, one code a letter. After the page's grid is
//! composed at its presentation level, each row is passed once, right to
//! left, and every letter of those sets is given its form from its
//! neighbours. Rows are in visual order, as Table 44's are: a letter's
//! previous letter is the cell to its right, its next letter the cell to
//! its left. No allocation, and time proportional to the row.

use crate::charset::{CharSet, G0, JoinForm, JoiningType, joining_type, to_unicode};
use crate::consts::COLUMNS;
use crate::level1::{Glyph, Grid};

/// The letter of a cell when it is one of a group 1001 set's, with its set.
fn letter(glyph: Glyph) -> Option<(CharSet, JoiningType)> {
    let Glyph::Char(character) = glyph else { return None };
    let CharSet::G0(G0 { set, .. }) = character.set else { return None };
    if !set.is_joined() || character.diacritic != 0 {
        return None;
    }
    let kind = joining_type(to_unicode(character).base)?;
    Some((character.set, kind))
}

/// The form of a letter joining the given neighbours (REQ-CHR-015).
const fn form(joins_previous: bool, joins_next: bool) -> JoinForm {
    match (joins_previous, joins_next) {
        (false, false) => JoinForm::Isolated,
        (true, false) => JoinForm::Final,
        (false, true) => JoinForm::Initial,
        (true, true) => JoinForm::Medial,
    }
}

/// Gives every letter of a group 1001 set in one row its joined form.
pub fn join_row(row: &mut [crate::level1::Cell; COLUMNS]) {
    let letters: [Option<(CharSet, JoiningType)>; COLUMNS] =
        core::array::from_fn(|column| row.get(column).and_then(|cell| letter(cell.glyph)));
    for column in 0..COLUMNS {
        let Some((set, kind)) = letters.get(column).copied().flatten() else { continue };
        let neighbour = |at: Option<usize>| {
            at.and_then(|at| letters.get(at).copied().flatten()).filter(|(other, _)| *other == set).map(|(_, k)| k)
        };
        // Right is previous in reading order, left is next.
        let previous = neighbour(column.checked_add(1));
        let next = neighbour(column.checked_sub(1));
        let joins_previous = kind.joins_previous() && previous.is_some_and(JoiningType::joins_next);
        let joins_next = kind.joins_next() && next.is_some_and(JoiningType::joins_previous);
        if let Some(cell) = row.get_mut(column)
            && let Glyph::Char(character) = &mut cell.glyph
        {
            character.form = form(joins_previous, joins_next);
        }
    }
}

/// Joins every row of a grid.
pub fn join<const PANEL_ROWS: usize>(grid: &mut Grid<PANEL_ROWS>) {
    for row in &mut grid.cells {
        join_row(row);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
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
}
