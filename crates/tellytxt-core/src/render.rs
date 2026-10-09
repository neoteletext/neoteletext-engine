//! Rendering (area RND, layer L4).
//!
//! All teletext-specific pixel logic is in [`cell_line`], the cell raster of
//! REQ-RND-016. The row renderer of REQ-RND-017 draws palette indices into
//! buffers the caller supplies; the library never pushes pixels and keeps
//! no pixel memory (decision E.29).

use crate::config::{CellMode, Config};
use crate::consts::{
    CELL_FIELD_LINES, CELL_HEIGHT, CELL_HEIGHT_SMALL, CELL_WIDTH, COLOURS, COLUMNS, GLYPH_DOTS,
    GLYPH_LINES, MOSAIC_ROW_TENTHS, PAGE_WIDTH, PALETTE_CONCEAL_BASE, PALETTE_FLASH_BASE,
    PALETTE_SIZE, PALETTE_TRANSPARENT, ROWS,
};
use crate::glyphs::GlyphSet;
use crate::level1::{Cell, Glyph, Grid, Size};
use crate::types::Instant;

/// One line of one cell (REQ-RND-016).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellLine {
    /// Twelve half-dots, bit 11 the leftmost; a set bit is foreground.
    pub mask: u16,
    /// Palette index of the dots.
    pub foreground: u8,
    /// Palette index of the gaps.
    pub background: u8,
}

/// Which lines of a row to draw (REQ-RND-017).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    /// Both fields: 20 lines per row in the 12-by-20 mode.
    Both,
    /// The first field only: even lines.
    First,
    /// The second field only: odd lines.
    Second,
}

/// Page zoom (REQ-L1-020): which half of the page is enlarged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zoom {
    /// No zoom.
    Off,
    /// Rows 0 to 11 enlarged to double height.
    Top,
    /// Rows 12 to 23 enlarged to double height.
    Bottom,
}

/// Viewer settings the renderer reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderOptions {
    /// Cell size.
    pub mode: CellMode,
    /// Character rounding in the 12-by-20 mode (REQ-RND-012).
    pub rounding: bool,
    /// Reveal is on: decides which palette range concealed flashing cells
    /// use (REQ-RND-023).
    pub reveal: bool,
}

impl RenderOptions {
    /// Options from the configuration.
    #[must_use]
    pub const fn from_config(config: &Config, reveal: bool) -> Self {
        Self { mode: config.cell_mode, rounding: config.rounding, reveal }
    }

    /// Lines per text row in this mode.
    #[must_use]
    pub const fn row_height(self) -> usize {
        match self.mode {
            CellMode::Saa5050 => CELL_HEIGHT,
            CellMode::Saa5246a => CELL_HEIGHT_SMALL,
        }
    }
}

/// The palette index of a cell's foreground pixels (REQ-RND-023).
#[must_use]
pub fn foreground_index(cell: &Cell, grid: &Grid, reveal: bool) -> u8 {
    if grid.boxed_mode && !cell.boxed {
        return PALETTE_TRANSPARENT;
    }
    let pair = cell.foreground.wrapping_mul(COLOURS as u8).wrapping_add(cell.background);
    if cell.conceal && !(reveal && cell.flash) {
        PALETTE_CONCEAL_BASE.wrapping_add(pair)
    } else if cell.flash {
        PALETTE_FLASH_BASE.wrapping_add(pair)
    } else {
        cell.foreground
    }
}

/// The palette index of a cell's background pixels.
#[must_use]
pub fn background_index(cell: &Cell, grid: &Grid) -> u8 {
    if grid.mix || (grid.boxed_mode && !cell.boxed) { PALETTE_TRANSPARENT } else { cell.background }
}

/// Widens a 5-dot row to 10 half-dots in a 12-half-dot cell; the leftmost
/// dot column of the cell stays blank.
fn widen(dots: u8) -> u16 {
    (0..GLYPH_DOTS).fold(0u16, |acc, dot| {
        if (dots >> dot) & 1 == 1 { acc | (0b11 << dot.wrapping_mul(2)) } else { acc }
    })
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
fn glyph_line_20(glyph: &[u8; GLYPH_LINES], line: usize, rounding: bool) -> u16 {
    let cell_line = line / 2;
    let row = cell_line.checked_sub(1);
    let a = glyph_row(glyph, row);
    if !rounding {
        return a;
    }
    // Upper sub-line rounds against the row above, lower against the row
    // below.
    let neighbour = if line.is_multiple_of(2) { row.and_then(|r| r.checked_sub(1)) } else { row.map(|r| r.saturating_add(1)) };
    round(a, glyph_row(glyph, neighbour)) & CELL_MASK
}

/// Twelve half-dots.
const CELL_MASK: u16 = (1 << CELL_WIDTH) - 1;
/// Shift of the leftmost half-dot in a cell mask.
const CELL_LAST_HALF_DOT: usize = CELL_WIDTH - 1;

/// Half-dot columns of the left and right mosaic blocks.
const MOSAIC_LEFT: u16 = 0b1111_1100_0000;
const MOSAIC_RIGHT: u16 = 0b0000_0011_1111;
/// Separated mosaics lose the leftmost half-dot pair of each block
/// (decision E.14: half-dot columns 0, 1, 6 and 7).
const SEPARATED_COLUMNS: u16 = 0b1100_0011_0000;

/// The mosaic block row (0 to 2) of line `line` in a cell of `height`
/// lines, with block rows 3/10, 4/10 and 3/10 of the cell (decision E.14),
/// and whether the line is the block's last (blank when separated).
fn mosaic_block(line: usize, height: usize) -> (usize, bool) {
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
fn mosaic_line(pattern: u8, separated: bool, line: usize, height: usize) -> u16 {
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

/// The cell raster (REQ-RND-016): line `line` of a cell, where `line` is 0
/// to 19 in the 12-by-20 mode and 0 to 9 in the 12-by-10 mode. Double height
/// is applied here (REQ-RND-011).
#[must_use]
pub fn cell_line(cell: &Cell, grid: &Grid, line: usize, glyphs: &GlyphSet, options: RenderOptions) -> CellLine {
    let foreground = foreground_index(cell, grid, options.reveal);
    let background = background_index(cell, grid);
    let height = options.row_height();
    // Which line of the normal-height character this output line shows.
    let source = match cell.size {
        Size::Normal => line,
        Size::DoubleTop => line / 2,
        Size::DoubleBottom => (height / 2).saturating_add(line / 2),
    };
    let mask = match cell.glyph {
        Glyph::Space => 0,
        Glyph::Mosaic { pattern, separated } => mosaic_line(pattern, separated, source, height),
        Glyph::Char(character) => {
            let glyph = glyphs.glyph(character);
            match options.mode {
                CellMode::Saa5050 => glyph_line_20(&glyph, source, options.rounding),
                // The 12-by-10 mode: one line per glyph row, no rounding.
                CellMode::Saa5246a => glyph_row(&glyph, source.checked_sub(1)),
            }
        }
    };
    CellLine { mask: mask & CELL_MASK, foreground, background }
}

/// Errors from the row renderer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderError {
    /// The buffer is too small for the requested row, stride and format.
    BufferTooSmall,
    /// The row or line number is out of range.
    OutOfRange,
}

/// Pixel packing of the caller's buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexFormat {
    /// One palette index per byte.
    Bits8,
    /// Two palette indices per byte, the left pixel in the high nibble.
    /// Only indices 0 to 15 fit, so flash and conceal use the base colours.
    Bits4,
}

/// A caller-supplied buffer of palette indices (REQ-RND-017).
#[derive(Debug)]
pub struct IndexBuffer<'a> {
    /// The pixel packing.
    pub format: IndexFormat,
    /// The memory.
    pub data: &'a mut [u8],
    /// Bytes from one pixel row to the next.
    pub stride: usize,
}

/// Writes one pixel row of 480 palette indices into `out`.
fn write_line(grid: &Grid, row: usize, line: usize, glyphs: &GlyphSet, options: RenderOptions, format: IndexFormat, out: &mut [u8]) -> Result<(), RenderError> {
    let needed = match format {
        IndexFormat::Bits8 => PAGE_WIDTH,
        IndexFormat::Bits4 => PAGE_WIDTH / 2,
    };
    let out = out.get_mut(..needed).ok_or(RenderError::BufferTooSmall)?;
    let cells = grid.cells.get(row).ok_or(RenderError::OutOfRange)?;
    for (column, cell) in cells.iter().enumerate().take(COLUMNS) {
        let raster = cell_line(cell, grid, line, glyphs, options);
        for dot in 0..CELL_WIDTH {
            let on = (raster.mask >> CELL_LAST_HALF_DOT.wrapping_sub(dot)) & 1 == 1;
            let index = if on { raster.foreground } else { raster.background };
            let x = column.saturating_mul(CELL_WIDTH).saturating_add(dot);
            match format {
                IndexFormat::Bits8 => {
                    if let Some(pixel) = out.get_mut(x) {
                        *pixel = index;
                    }
                }
                IndexFormat::Bits4 => {
                    if let Some(pair) = out.get_mut(x / 2) {
                        let nibble = index & 0x0F;
                        *pair = if x % 2 == 0 { (*pair & 0x0F) | (nibble << 4) } else { (*pair & 0xF0) | nibble };
                    }
                }
            }
        }
    }
    Ok(())
}

/// Renders text row `row` into `out` with a row stride of `stride` bytes
/// (REQ-RND-017). In the 12-by-20 mode this draws 20 lines for
/// [`Field::Both`] and 10 for one field.
pub fn render_row(
    grid: &Grid,
    row: usize,
    field: Field,
    glyphs: &GlyphSet,
    options: RenderOptions,
    buffer: &mut IndexBuffer<'_>,
) -> Result<(), RenderError> {
    if row >= ROWS {
        return Err(RenderError::OutOfRange);
    }
    let height = options.row_height();
    let mut target = 0usize;
    for line in 0..height {
        let wanted = match field {
            Field::Both => true,
            Field::First => line % 2 == 0,
            Field::Second => line % 2 == 1,
        };
        if !wanted {
            continue;
        }
        let start = target.saturating_mul(buffer.stride);
        let slice = buffer.data.get_mut(start..).ok_or(RenderError::BufferTooSmall)?;
        write_line(grid, row, line, glyphs, options, buffer.format, slice)?;
        target = target.saturating_add(1);
    }
    Ok(())
}

/// Renders screen line `y` (0 to 499 in the 12-by-20 mode) into `out`,
/// applying page zoom (REQ-L1-020).
pub fn render_line(grid: &Grid, y: usize, zoom: Zoom, glyphs: &GlyphSet, options: RenderOptions, format: IndexFormat, out: &mut [u8]) -> Result<(), RenderError> {
    let height = options.row_height();
    let (row, line) = match zoom {
        Zoom::Off => (y.checked_div(height).unwrap_or(0), y.checked_rem(height).unwrap_or(0)),
        Zoom::Top | Zoom::Bottom => {
            // Rows 0 to 11 or 12 to 23 doubled; each output line shows half
            // a source line, the way double height does.
            let first = if zoom == Zoom::Top { 0 } else { ZOOM_ROWS };
            let doubled = y / 2;
            (
                first.saturating_add(doubled.checked_div(height).unwrap_or(0)),
                doubled.checked_rem(height).unwrap_or(0),
            )
        }
    };
    write_line(grid, row, line, glyphs, options, format, out)
}

/// Rows in each half of a zoomed page (REQ-L1-020).
const ZOOM_ROWS: usize = 12;

/// Fills a 144-entry palette for a flash phase and reveal setting
/// (REQ-RND-023). `base` holds the eight colours and, at index 8,
/// transparent. The core never produces real colours itself: `T` is
/// whatever colour form the caller uses (decision E.29).
pub fn fill_palette<T: Copy>(out: &mut [T; PALETTE_SIZE], base: &[T; COLOURS + 1], flash_visible: bool, reveal: bool) {
    let transparent = base.get(COLOURS).copied();
    for (index, slot) in out.iter_mut().enumerate() {
        let colour = palette_colour(index, flash_visible, reveal);
        if let Some(value) = base.get(usize::from(colour)).copied().or(transparent) {
            *slot = value;
        }
    }
}

/// The base colour (0 to 7, or 8 for transparent) of a palette index.
#[must_use]
pub fn palette_colour(index: usize, flash_visible: bool, reveal: bool) -> u8 {
    let flash_base = usize::from(PALETTE_FLASH_BASE);
    let conceal_base = usize::from(PALETTE_CONCEAL_BASE);
    let pair_count = COLOURS.saturating_mul(COLOURS);
    if index < COLOURS {
        return index as u8;
    }
    if (flash_base..flash_base.saturating_add(pair_count)).contains(&index) {
        let pair = index.saturating_sub(flash_base);
        let (fg, bg) = ((pair / COLOURS) as u8, (pair % COLOURS) as u8);
        return if flash_visible { fg } else { bg };
    }
    if (conceal_base..conceal_base.saturating_add(pair_count)).contains(&index) {
        let pair = index.saturating_sub(conceal_base);
        let (fg, bg) = ((pair / COLOURS) as u8, (pair % COLOURS) as u8);
        return if reveal { fg } else { bg };
    }
    PALETTE_TRANSPARENT
}

/// The flash phase at a time (decision E.12): true while shown.
#[must_use]
pub fn flash_visible(now: Instant, config: &Config) -> bool {
    let period = config.flash_shown_us.saturating_add(config.flash_hidden_us);
    if period == 0 {
        return true;
    }
    now.0.checked_rem(period).unwrap_or(0) < config.flash_shown_us
}

/// Change tracking (REQ-RND-019): which rows changed since the port last
/// asked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChangeTracker {
    hashes: [u64; ROWS],
    reveal: bool,
    primed: bool,
}

impl Default for ChangeTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// FNV-1a parameters for row fingerprints.
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

fn mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(FNV_PRIME)
}

fn cell_fingerprint(cell: &Cell) -> u64 {
    let glyph = match cell.glyph {
        Glyph::Space => 0,
        Glyph::Mosaic { pattern, separated } => 0x100 | u64::from(pattern) | (u64::from(separated) << 7),
        Glyph::Char(c) => {
            let set = match c.set {
                crate::charset::CharSet::G0(g0) => (g0.set as u64) << 4 | g0.national.map_or(0xF, |n| n as u64),
                crate::charset::CharSet::G2(g2) => 0x100 | g2 as u64,
                crate::charset::CharSet::G3 => 0x200,
            };
            0x10000 | (set << 16) | (u64::from(c.code) << 4) | u64::from(c.diacritic)
        }
    };
    let flags = u64::from(cell.flash)
        | u64::from(cell.conceal) << 1
        | u64::from(cell.boxed) << 2
        | (cell.size as u64) << 3
        | u64::from(cell.foreground) << 8
        | u64::from(cell.background) << 12;
    glyph.wrapping_mul(FNV_PRIME) ^ flags
}

impl ChangeTracker {
    /// A tracker that reports every row as changed on first use.
    #[must_use]
    pub const fn new() -> Self {
        Self { hashes: [0; ROWS], reveal: false, primed: false }
    }

    /// Rows that changed since the last call, as a bitmask (bit r for row
    /// r). A change of reveal marks rows holding flashing concealed cells;
    /// other flash and reveal changes are palette changes only.
    pub fn changed_rows(&mut self, grid: &Grid, reveal: bool) -> u32 {
        let mut changed = 0u32;
        for (row, (cells, stored)) in grid.cells.iter().zip(self.hashes.iter_mut()).enumerate() {
            let mut hash = FNV_OFFSET;
            hash = mix(hash, u64::from(grid.boxed_mode) | u64::from(grid.mix) << 1);
            let mut reveal_sensitive = false;
            for cell in cells {
                hash = mix(hash, cell_fingerprint(cell));
                reveal_sensitive |= cell.flash && cell.conceal;
            }
            let bit = 1u32.checked_shl(row as u32).unwrap_or(0);
            if !self.primed || hash != *stored || (reveal != self.reveal && reveal_sensitive) {
                changed |= bit;
            }
            *stored = hash;
        }
        // REQ-RND-019: the row below a changed double-height row changes too.
        let doubled = changed & grid.double_height_rows;
        changed |= doubled << 1;
        self.reveal = reveal;
        self.primed = true;
        changed & ((1u32 << ROWS) - 1)
    }

    /// Forces a full redraw on the next call.
    pub fn invalidate(&mut self) {
        self.primed = false;
    }
}

/// Renders every row, or only changed rows, into a whole-screen framebuffer
/// (REQ-RND-022). `rows` is a bitmask of rows to draw.
pub fn render_screen(grid: &Grid, rows: u32, glyphs: &GlyphSet, options: RenderOptions, framebuffer: &mut IndexBuffer<'_>) -> Result<(), RenderError> {
    let height = options.row_height();
    for row in 0..ROWS {
        if rows.checked_shr(row as u32).is_some_and(|bits| bits & 1 == 1) {
            let start = row.saturating_mul(height).saturating_mul(framebuffer.stride);
            let data = framebuffer.data.get_mut(start..).ok_or(RenderError::BufferTooSmall)?;
            let mut slice = IndexBuffer { format: framebuffer.format, data, stride: framebuffer.stride };
            render_row(grid, row, Field::Both, glyphs, options, &mut slice)?;
        }
    }
    Ok(())
}

/// Every row.
pub const ALL_ROWS: u32 = (1 << ROWS) - 1;

/// Lines of the cell in one field (for documentation of the geometry).
pub const FIELD_LINES: usize = CELL_FIELD_LINES;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::charset::{CharRef, CharSet, G0, G0Set, NationalOption};
    use std::vec;
    use std::vec::Vec;

    fn options() -> RenderOptions {
        RenderOptions { mode: CellMode::Saa5050, rounding: true, reveal: false }
    }

    #[test]
    fn rounding_matches_decision_e13() {
        // A diagonal: row 0 has the dot at bit 0, row 1 at bit 1.
        let glyph = [0b00001, 0b00010, 0, 0, 0, 0, 0, 0, 0];
        // Lower sub-line of glyph row 0 rounds against row 1.
        let lower = glyph_line_20(&glyph, 3, true);
        assert_eq!(lower, widen(0b00001) | (widen(0b00001) << 1) & widen(0b00010) & !(widen(0b00010) << 1));
        // Upper sub-line of row 0 has no neighbour above: no rounding.
        assert_eq!(glyph_line_20(&glyph, 2, true), widen(0b00001));
        assert_eq!(glyph_line_20(&glyph, 3, false), widen(0b00001));
        // Cell line 0 is blank.
        assert_eq!(glyph_line_20(&glyph, 0, true), 0);
    }

    #[test]
    fn separated_mosaic_geometry_matches_e14() {
        let full = 0x3F;
        let blank_lines: Vec<usize> = (0..20).filter(|&l| mosaic_line(full, true, l, 20) == 0).collect();
        assert_eq!(blank_lines, vec![4, 5, 12, 13, 18, 19]);
        assert_eq!(mosaic_line(full, true, 0, 20), !SEPARATED_COLUMNS & CELL_MASK);
        let small: Vec<usize> = (0..10).filter(|&l| mosaic_line(full, true, l, 10) == 0).collect();
        assert_eq!(small, vec![2, 6, 9]);
        // Contiguous blocks: 6, 8 and 6 lines.
        assert_eq!(mosaic_block(5, 20).0, 0);
        assert_eq!(mosaic_block(6, 20).0, 1);
        assert_eq!(mosaic_block(13, 20).0, 1);
        assert_eq!(mosaic_block(14, 20).0, 2);
    }

    #[test]
    fn palette_flash_and_reveal() {
        let mut palette = [0u8; PALETTE_SIZE];
        let base = [0, 1, 2, 3, 4, 5, 6, 7, 8];
        fill_palette(&mut palette, &base, true, false);
        let flash = usize::from(PALETTE_FLASH_BASE) + 8 * 3 + 4;
        let conceal = usize::from(PALETTE_CONCEAL_BASE) + 8 * 3 + 4;
        assert_eq!((palette[flash], palette[conceal]), (3, 4));
        fill_palette(&mut palette, &base, false, true);
        assert_eq!((palette[flash], palette[conceal]), (4, 3));
        assert_eq!(palette[usize::from(PALETTE_TRANSPARENT)], 8);
    }

    #[test]
    fn rows_render_identically_in_any_order() {
        let mut grid = Grid::EMPTY;
        let g0 = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) });
        grid.cells[2][3].glyph = Glyph::Char(CharRef::new(g0, b'A'));
        grid.cells[2][4].glyph = Glyph::Mosaic { pattern: 0x15, separated: false };
        let glyphs = GlyphSet::bedstead();
        let mut whole = vec![0u8; PAGE_WIDTH * 500];
        let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut whole, stride: PAGE_WIDTH };
        render_screen(&grid, ALL_ROWS, &glyphs, options(), &mut buffer).unwrap();
        for row in (0..ROWS).rev() {
            let mut single = vec![0u8; PAGE_WIDTH * 20];
            let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut single, stride: PAGE_WIDTH };
            render_row(&grid, row, Field::Both, &glyphs, options(), &mut buffer).unwrap();
            assert_eq!(&whole[row * 20 * PAGE_WIDTH..(row + 1) * 20 * PAGE_WIDTH], &single[..]);
        }
        for y in 0..500 {
            let mut line = vec![0u8; PAGE_WIDTH];
            render_line(&grid, y, Zoom::Off, &glyphs, options(), IndexFormat::Bits8, &mut line).unwrap();
            assert_eq!(&whole[y * PAGE_WIDTH..(y + 1) * PAGE_WIDTH], &line[..]);
        }
    }

    #[test]
    fn change_tracking_reports_only_changed_rows() {
        let mut tracker = ChangeTracker::new();
        let mut grid = Grid::EMPTY;
        assert_eq!(tracker.changed_rows(&grid, false), ALL_ROWS);
        assert_eq!(tracker.changed_rows(&grid, false), 0);
        grid.cells[5][0].foreground = 3;
        assert_eq!(tracker.changed_rows(&grid, false), 1 << 5);
        grid.cells[7][0].flash = true;
        grid.cells[7][0].conceal = true;
        tracker.changed_rows(&grid, false);
        assert_eq!(tracker.changed_rows(&grid, true), 1 << 7, "reveal redraws flashing concealed rows only");
    }

    #[test]
    fn flash_timing_is_three_to_one() {
        let config = Config::default();
        assert!(flash_visible(Instant(0), &config));
        assert!(flash_visible(Instant(959_999), &config));
        assert!(!flash_visible(Instant(960_000), &config));
        assert!(flash_visible(Instant(1_280_000), &config));
    }
}
