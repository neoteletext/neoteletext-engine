//! Presenting a grid for any front end: plain text (REQ-RND-008) and a
//! palette-index raster (REQ-RND-007), for `neottx` and any other front
//! end.
//!
//! Text: rows are composed with their side panels, and G3 characters
//! convert as REQ-CHR-011 says.
//! Every character written passes the allow-list of REQ-SEC-010.

use neoteletext_core::charset::{compose, mosaic_to_unicode, to_unicode};
use neoteletext_core::level1::{Cell, Glyph, Grid, Size, Width};
use neoteletext_core::policy::is_safe_output_char;
use neoteletext_core::render::{source_cell, source_columns};

mod raster;
mod receiver_page;
#[cfg(test)]
mod tests;

pub use raster::{Raster, Scale, raster};
pub use receiver_page::receiver_grid;

/// How mosaics are written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MosaicStyle {
    /// Separated mosaics as the Unicode 16 separated sextants (REQ-CHR-005).
    Unicode16,
    /// Separated mosaics drawn with the contiguous forms, for older fonts.
    Legacy,
}

/// Whether a cell's foreground is visible at all. The lower half of double
/// height and the right half of double width are written as their
/// background (REQ-RND-009, REQ-RND-041).
fn visible(cell: &Cell, grid: &Grid, reveal: bool) -> bool {
    (!grid.boxed_mode || cell.boxed)
        && (!cell.conceal || reveal)
        && cell.size != Size::DoubleBottom
        && cell.attributes.width() != Width::RightHalf
}

/// The text of one cell: a base character and an optional combining mark,
/// precomposed where Unicode allows (decision E.26).
pub fn cell_text(cell: &Cell, grid: &Grid, reveal: bool, mosaics: MosaicStyle, out: &mut String) {
    if !visible(cell, grid, reveal) {
        out.push(' ');
        return;
    }
    match cell.glyph {
        Glyph::Space => out.push(' '),
        Glyph::Mosaic { pattern, separated } => {
            out.push(mosaic_to_unicode(pattern, separated && mosaics == MosaicStyle::Unicode16));
        }
        Glyph::Char(character) => {
            let unicode = to_unicode(character);
            let base = if is_safe_output_char(unicode.base) { unicode.base } else { ' ' };
            match unicode.combining {
                Some(mark) => {
                    if let Some(precomposed) = compose(base, mark) {
                        out.push(precomposed);
                    } else {
                        out.push(base);
                        if is_safe_output_char(mark) {
                            out.push(mark);
                        }
                    }
                }
                None => out.push(base),
            }
        }
    }
}

/// The cells of row `row` in display order: the left panel, the main page
/// and the right panel (REQ-RND-033). `None` is the screen colour, where
/// the panel area has no content.
pub fn row_cells(grid: &Grid, row: usize) -> impl Iterator<Item = Option<&Cell>> {
    source_columns(grid).map(move |column| source_cell(grid, row, column))
}

/// The page as 25 lines of UTF-8 (REQ-RND-008, REQ-RND-041).
#[must_use]
pub fn plain(grid: &Grid, reveal: bool, mosaics: MosaicStyle) -> String {
    let mut out = String::new();
    for row in 0..grid.cells.len() {
        let mut line = String::new();
        for cell in row_cells(grid, row) {
            match cell {
                Some(cell) => cell_text(cell, grid, reveal, mosaics, &mut line),
                None => line.push(' '),
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}
