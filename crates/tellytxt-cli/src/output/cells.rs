//! The cell dump of `ttx text --cells`, a test interface for the zvbi
//! oracle at Levels 2.5 and 3.5 (REQ-TST-023): every cell of the main page
//! area with its character, Colour Map indices, size and attributes, so
//! `tests/oracles/compare_cells.py` can compare it cell by cell with
//! `tests/oracles/zvbi_cells.c`.
//!
//! Characters are written by what they are, not as Unicode, so the
//! comparison never depends on either decoder's choice of private-use
//! code points:
//! - `S`, a space or nothing in the foreground;
//! - `U+XXXX`, a G0 or G2 character, precomposed with its mark where
//!   Unicode allows, as `ttx text` writes it;
//! - `M` and the two hex digits of the block pattern, then `c` or `s`;
//! - `G3:XX`, a G3 character by its code;
//! - `Dn` (normal) or `Dg` (global), the sub-table in hex, `:`, and the
//!   character number in decimal.
//!
//! A cell is `CHARACTER/FOREGROUND/BACKGROUND/SIZE/FLAGS`. SIZE is the
//! vertical role (`N` normal, `T` top half, `B` bottom half) followed by
//! the horizontal role (`N`, `L` left half, `R` right half). FLAGS is any
//! of `u` underline, `b` bold, `i` italic, `p` proportional, `f` flash,
//! `c` conceal, `x` invert and `o` boxed.

use std::fmt::Write as _;
use tellytxt_core::charset::{CharSet, compose, to_unicode};
use tellytxt_core::level1::{Cell, Glyph, Grid, Size, Width};
use tellytxt_core::types::DrcsTable;

fn character(cell: &Cell, out: &mut String) {
    match cell.glyph {
        Glyph::Space => out.push('S'),
        Glyph::Mosaic { pattern, separated } => {
            let _ = write!(out, "M{pattern:02X}{}", if separated { 's' } else { 'c' });
        }
        Glyph::Char(reference) => match reference.set {
            CharSet::G3 => {
                let _ = write!(out, "G3:{:02X}", reference.code);
            }
            CharSet::Drcs { table, subtable } => {
                let table = if table == DrcsTable::Normal { 'n' } else { 'g' };
                let _ = write!(out, "D{table}{subtable:X}:{}", reference.code);
            }
            CharSet::G0(_) | CharSet::G2(_) => {
                let unicode = to_unicode(reference);
                let base = unicode.base;
                let code = unicode.combining.and_then(|mark| compose(base, mark)).unwrap_or(base);
                let _ = write!(out, "U+{:04X}", u32::from(code));
            }
        },
    }
}

fn size(cell: &Cell, out: &mut String) {
    out.push(match cell.size {
        Size::Normal => 'N',
        Size::DoubleTop => 'T',
        Size::DoubleBottom => 'B',
    });
    out.push(match cell.attributes.width() {
        Width::Normal => 'N',
        Width::LeftHalf => 'L',
        Width::RightHalf => 'R',
    });
}

fn flags(cell: &Cell, out: &mut String) {
    let font = cell.attributes.font();
    for (set, flag) in [
        (cell.attributes.underline(), 'u'),
        (font.bold, 'b'),
        (font.italic, 'i'),
        (font.proportional, 'p'),
        (cell.flash, 'f'),
        (cell.conceal, 'c'),
        (cell.attributes.invert(), 'x'),
        (cell.boxed, 'o'),
    ] {
        if set {
            out.push(flag);
        }
    }
}

/// The cells of the page's main area, one line per row after a line giving
/// the screen colour.
#[must_use]
pub fn cells(grid: &Grid) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "screen {}", grid.screen_colour);
    for (number, row) in grid.cells.iter().enumerate() {
        let _ = write!(out, "{number:02}");
        for cell in row {
            out.push(' ');
            character(cell, &mut out);
            let _ = write!(out, "/{}/{}/", cell.foreground, cell.background);
            size(cell, &mut out);
            out.push('/');
            flags(cell, &mut out);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tellytxt_core::charset::CharRef;

    #[test]
    fn cells_name_each_kind_of_character() {
        // REQ-TST-023: the dump names characters by set and code.
        let mut grid: Box<Grid> = Box::new(Grid::empty());
        grid.cells[1][0].glyph = Glyph::Char(CharRef::new(CharSet::G3, 0x2F));
        grid.cells[1][1].glyph = Glyph::Mosaic { pattern: 0x3F, separated: true };
        grid.cells[1][2].glyph =
            Glyph::Char(CharRef::new(CharSet::Drcs { table: DrcsTable::Normal, subtable: 0xA }, 47));
        grid.cells[1][3].size = Size::DoubleTop;
        grid.cells[1][3].flash = true;
        let dump = cells(&grid);
        let row = dump.lines().nth(2).unwrap_or_default();
        assert!(row.starts_with("01 G3:2F/7/0/NN/ M3Fs/7/0/NN/ DnA:47/7/0/NN/ S/7/0/TN/f "), "{row}");
    }
}
