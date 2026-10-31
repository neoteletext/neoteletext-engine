//! The cell raster of REQ-RND-016 (decisions E.13 and E.14): glyph lines
//! with rounding, mosaics, loaded fonts, underline, double width and height,
//! and G3 characters.

use super::CellLine;
use super::colours::{background_index, foreground_index, resolved_colours};
use super::options::{ColourMode, RenderOptions};
use crate::config::CellMode;
use crate::consts::{CELL_DOTS, CELL_HEIGHT, CELL_WIDTH, GLYPH_DOTS, GLYPH_LINES, MOSAIC_ROW_TENTHS};
use crate::glyphs::{CharClass, GlyphSet, class};
use crate::level1::{Cell, CellAttributes, Glyph, Grid, Size, Width};

/// Widens a 5-dot row to 10 half-dots in a 12-half-dot cell; the leftmost
/// dot column of the cell stays blank.
pub(super) fn widen(dots: u8) -> u16 {
    (0..GLYPH_DOTS).fold(
        0u16,
        |acc, dot| {
            if (dots >> dot) & 1 == 1 { acc | (0b11 << dot.wrapping_mul(2)) } else { acc }
        },
    )
}

/// A glyph row as half-dots, empty outside the 9-row glyph (decision E.13:
/// no rounding across the cell edge).
fn glyph_row(glyph: &[u8; GLYPH_LINES], row: Option<usize>) -> u16 {
    row.and_then(|row| glyph.get(row)).map_or(0, |&dots| widen(dots))
}

/// The rounding rule of decision E.13 on half-dot rows.
const fn round(a: u16, b: u16) -> u16 {
    a | ((a >> 1) & b & !(b >> 1)) | ((a << 1) & b & !(b << 1))
}

/// Line `line` (0 to 19) of the 20-line form of a glyph. Cell line 0 is the
/// blank line above the glyph; cell lines 1 to 9 hold glyph rows 0 to 8
/// (REQ-RND-010: a 5 by 9 glyph in a 6 by 10 cell).
pub(super) fn glyph_line_20(glyph: &[u8; GLYPH_LINES], line: usize, rounding: bool) -> u16 {
    let cell_line = line / 2;
    let row = cell_line.checked_sub(1);
    let a = glyph_row(glyph, row);
    if !rounding {
        return a;
    }
    // Upper sub-line rounds against the row above, lower against the row
    // below.
    let neighbour =
        if line.is_multiple_of(2) { row.and_then(|r| r.checked_sub(1)) } else { row.map(|r| r.saturating_add(1)) };
    round(a, glyph_row(glyph, neighbour)) & CELL_MASK
}

/// Twelve half-dots.
pub(super) const CELL_MASK: u16 = (1 << CELL_WIDTH) - 1;
/// Shift of the leftmost half-dot in a cell mask.
pub(super) const CELL_LAST_HALF_DOT: usize = CELL_WIDTH - 1;
/// The six half-dots of each half of a cell (REQ-RND-034).
const HALF_MASK: u16 = (1 << CELL_DOTS) - 1;

/// Half-dot columns of the left and right mosaic blocks.
const MOSAIC_LEFT: u16 = 0b1111_1100_0000;
const MOSAIC_RIGHT: u16 = 0b0000_0011_1111;
/// Separated mosaics lose the leftmost half-dot pair of each block
/// (decision E.14: half-dot columns 0, 1, 6 and 7).
pub(super) const SEPARATED_COLUMNS: u16 = 0b1100_0011_0000;

/// The mosaic block row (0 to 2) of line `line` in a cell of `height`
/// lines, with block rows 3/10, 4/10 and 3/10 of the cell (decision E.14),
/// and whether the line is the block's last (blank when separated).
pub(super) fn mosaic_block(line: usize, height: usize) -> (usize, bool) {
    let mut top = 0usize;
    for (block, &tenths) in MOSAIC_ROW_TENTHS.iter().enumerate() {
        let rows = tenths.saturating_mul(height) / 10;
        let bottom = top.saturating_add(rows);
        if line < bottom {
            let last_lines = if height >= CELL_HEIGHT { 2 } else { 1 };
            return (block, line >= bottom.saturating_sub(last_lines));
        }
        top = bottom;
    }
    (2, true)
}

/// One line of a mosaic in a cell of `height` lines.
pub(super) fn mosaic_line(pattern: u8, separated: bool, line: usize, height: usize) -> u16 {
    let (block, last) = mosaic_block(line, height);
    let left = (pattern >> block.wrapping_mul(2)) & 1 == 1;
    let right = (pattern >> block.wrapping_mul(2).wrapping_add(1)) & 1 == 1;
    let mut mask = 0u16;
    if left {
        mask |= MOSAIC_LEFT;
    }
    if right {
        mask |= MOSAIC_RIGHT;
    }
    if separated {
        if last {
            return 0;
        }
        mask &= !SEPARATED_COLUMNS;
    }
    mask
}

/// A G0 or G2 character's line (REQ-RND-016): the glyph line, with
/// rounding in the 12-by-20 mode.
fn text_line(glyph: &[u8; GLYPH_LINES], source: usize, options: RenderOptions) -> u16 {
    match options.mode {
        CellMode::Saa5050 => glyph_line_20(glyph, source, options.rounding),
        // The 12-by-10 mode: one line per glyph row, no rounding.
        CellMode::Saa5246a => glyph_row(glyph, source.checked_sub(1)),
    }
}

/// A loaded font's character line (REQ-FNT-008): a 12 by 10 glyph drawn
/// as given, each line twice in the 12-by-20 cell, or a 12 by 20 glyph line
/// for line (decision E.77), and rounded as Bedstead is when the font is
/// a doubled dot matrix (issue #209); a character no loaded font has is
/// drawn from the bundled glyphs. Cold and out of line, so that the
/// common case, no loaded font, costs the renderer one test (REQ-TST-050).
#[cold]
#[inline(never)]
fn font_or_text_line(cell: &Cell, glyphs: &GlyphSet<'_>, source: usize, options: RenderOptions) -> u16 {
    let Glyph::Char(character) = cell.glyph else { return 0 };
    let Some(loaded) = glyphs.loaded(character) else {
        return text_line(&glyphs.glyph(character), source, options);
    };
    let line = |index: usize| loaded.rows.get(index).copied().unwrap_or(0) & CELL_MASK;
    match (options.mode, loaded.tall) {
        // A 12 by 10 glyph doubles its lines in the 12-by-20 cell, rounded
        // when it is a doubled dot matrix (issue #209).
        (CellMode::Saa5050, false) => font_line_20(&loaded.rows, source, options.rounding && loaded.dot_matrix),
        // A 12 by 20 glyph shows its upper line of each pair in the
        // 12-by-10 raster.
        (CellMode::Saa5246a, true) => line(source.saturating_mul(2)),
        // Otherwise the glyph's own line: a 12 by 20 glyph line for line in
        // the 12-by-20 cell (decision E.77), a 12 by 10 one in the 12-by-10.
        (CellMode::Saa5050, true) | (CellMode::Saa5246a, false) => line(source),
    }
}

/// Line `line` (0 to 19) of a 12 by 10 loaded glyph, whose lines are the
/// first ten of `rows`, in the 12-by-20 cell: each of its ten lines twice, and with `rounding` the rule of decision E.13 as
/// the bundled glyphs get it, never across the cell's edge.
#[must_use]
pub(crate) fn font_line_20(rows: &[u16], line: usize, rounding: bool) -> u16 {
    let cell_line = line / 2;
    let row = |index: Option<usize>| index.and_then(|index| rows.get(index)).copied().unwrap_or(0) & CELL_MASK;
    let a = row(Some(cell_line));
    if !rounding {
        return a;
    }
    let neighbour = if line.is_multiple_of(2) { cell_line.checked_sub(1) } else { Some(cell_line.saturating_add(1)) };
    round(a, row(neighbour)) & CELL_MASK
}

/// The underline (REQ-RND-035) of a G0 or G2 character's line, after
/// rounding. Kept out of line: Level 1 cells never have it, and the Level 1
/// path is every port's hot path.
#[inline(never)]
fn styled_line(mask: u16, cell: &Cell, source: usize, height: usize, underline: bool) -> u16 {
    let mut mask = mask;
    if underline && cell.attributes.underline() && source == height.saturating_sub(1) {
        // Underline: the last dot line, across the whole cell (decision
        // E.47).
        mask = CELL_MASK;
    }
    mask & CELL_MASK
}

/// Doubles six half-dots into twelve (REQ-RND-034).
fn double_half(six: u16) -> u16 {
    (0..CELL_DOTS).fold(0u16, |acc, dot| if (six >> dot) & 1 == 1 { acc | (0b11 << dot.wrapping_mul(2)) } else { acc })
}

/// Double width (REQ-RND-034): the left or right six half-dots of the
/// normal-width line, each drawn twice. Kept out of line, like
/// [`styled_line`].
#[inline(never)]
fn widen_mask(mask: u16, width: Width) -> u16 {
    match width {
        Width::Normal => mask,
        Width::LeftHalf => double_half(mask >> CELL_DOTS),
        Width::RightHalf => double_half(mask & HALF_MASK),
    }
}

/// The line of the normal-height character that output line `line` of a
/// cell shows: the vertical part of the size (REQ-RND-011).
const fn source_line(size: Size, line: usize, height: usize) -> usize {
    match size {
        Size::Normal => line,
        Size::DoubleTop => line / 2,
        Size::DoubleBottom => (height / 2).saturating_add(line / 2),
    }
}

/// A G3 character's line (REQ-RND-037): no rounding or underline. Kept out of line, like [`styled_line`].
#[inline(never)]
fn g3_line(glyphs: &GlyphSet, code: u8, source: usize, mode: CellMode) -> u16 {
    let glyph = glyphs.g3(code);
    let line = match mode {
        CellMode::Saa5050 => glyph.large.get(source),
        CellMode::Saa5246a => glyph.small.get(source),
    };
    line.copied().unwrap_or(0) & CELL_MASK
}

/// The cell raster (REQ-RND-016): line `line` of the cell at text row `row`
/// and column `column`, where `line` is 0 to 19 in the 12-by-20 mode and 0
/// to 9 in the 12-by-10 mode. Size, glyphs, mosaics, G3, underline and, in
/// the resolved mode, invert, flash, conceal and the display planes are
/// all applied here.
#[must_use]
// The row renderer calls this for every cell of every line; inlined into
// its loop it measured 2 times faster (REQ-TST-050, `tests/performance.rs`).
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn cell_line(
    cell: &Cell,
    grid: &Grid,
    row: usize,
    column: usize,
    line: usize,
    glyphs: &GlyphSet,
    options: RenderOptions,
) -> CellLine {
    let height = options.row_height();
    let source = source_line(cell.size, line, height);
    let attributes = cell.attributes;
    // Level 1 cells have no Level 2.5 attributes: one test skips them all.
    let plain = attributes == CellAttributes::DEFAULT;
    let mut mask = match cell.glyph {
        Glyph::Space => 0,
        Glyph::Mosaic { pattern, separated } => mosaic_line(pattern, separated, source, height),
        Glyph::Char(character) => match class(character) {
            CharClass::Text => {
                // REQ-FNT-005: row 0 is the receiver's, always in the
                // bundled glyphs.
                let mask = if glyphs.has_fonts() && row != 0 {
                    font_or_text_line(cell, glyphs, source, options)
                } else {
                    text_line(&glyphs.glyph(character), source, options)
                };
                if plain { mask } else { styled_line(mask, cell, source, height, options.underline) }
            }
            CharClass::G3(code) => g3_line(glyphs, code, source, options.mode),
        },
    };
    if !plain {
        mask = widen_mask(mask, attributes.width());
    }
    let (foreground, background) = match options.colour {
        ColourMode::Palette => (foreground_index(cell, grid, options.reveal), background_index(cell, grid)),
        ColourMode::Resolved(flash) => resolved_colours(cell, grid, row, column, flash, options.reveal),
    };
    CellLine { mask, foreground, background }
}
