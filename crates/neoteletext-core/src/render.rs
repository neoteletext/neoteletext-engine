//! Rendering (area RND, layer L4).
//!
//! All teletext-specific pixel logic is in [`cell_line`], the cell raster of
//! REQ-RND-016. The row renderer of REQ-RND-017 draws indices into buffers
//! the caller supplies; the library never pushes pixels and keeps no pixel
//! memory (decision E.29).
//!
//! There are two colour modes (decision E.55):
//!
//! - **The palette mode** of REQ-RND-023, for Levels 1 and 1.5: flashing
//!   and concealed pixels get indices of their own, so flash and reveal are
//!   palette changes only.
//! - **The resolved mode** of REQ-RND-031, for Levels 2.5 and 3.5: flash
//!   and conceal are applied at render time from a [`FlashPhase`] and the
//!   reveal setting, and the indices are Colour Map indices 0 to 31, index 8
//!   transparent. The port's palette is the grid's Colour Map.
//!
//! A row is composed of the left side panel, the 40 columns of the main
//! page and the right side panel (decision E.49, REQ-RND-033); without side
//! panels it is 40 columns wide, as at Levels 1 and 1.5.
//!
//! Renderers start from the cell grid and never decode packets
//! (REQ-RND-001). The raster uses integer arithmetic only, so the same grid
//! and settings give the same pixels on every platform (REQ-RND-015).

use crate::config::{CellMode, Config};
use crate::consts::{
    ADJACENT_CLUT_FLIP, CELL_DOTS, CELL_FIELD_LINES, CELL_HEIGHT, CELL_HEIGHT_SMALL, CELL_WIDTH, COLOUR_MAP_ENTRIES,
    COLOUR_TRANSPARENT, COLOURS, COLUMNS, FLASH_CYCLE_US, FLASH_FAST_HALF_UNITS, FLASH_FAST_OFF_UNITS,
    FLASH_SLOW_ON_UNITS, FLASH_UNITS, GLYPH_DOTS, GLYPH_LINES, INDEX_4BIT_MAX, ITALIC_SHIFT_LOG2, MOSAIC_ROW_TENTHS,
    PAGE_WIDTH, PALETTE_CONCEAL_BASE, PALETTE_FLASH_BASE, PALETTE_SIZE, PALETTE_TRANSPARENT, RECEIVER_COLUMNS, ROWS,
    SIDE_PANEL_AREA_COLUMNS, SIDE_PANEL_FIRST_COLUMN, SIDE_PANEL_MAX_COLUMNS,
};
use crate::drcs::{DrcsMode, DrcsPattern};
use crate::glyphs::{CharClass, GlyphSet, class};
use crate::level1::{Cell, CellAttributes, FlashMode, FlashRate, Glyph, Grid, Size, Width, Zoom};
use crate::types::{DrcsCharacter, DrcsTable, Instant};

/// One line of one cell (REQ-RND-016).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellLine {
    /// Twelve half-dots, bit 11 the leftmost; a set bit is foreground.
    pub mask: u16,
    /// Index of the dots.
    pub foreground: u8,
    /// Index of the gaps.
    pub background: u8,
    /// For a multi-colour DRCS character (REQ-RND-038): the index of each
    /// of the twelve dots, leftmost first. `mask` and `foreground` are then
    /// unused, and `background` is the cell's background index.
    pub dots: Option<[u8; CELL_WIDTH]>,
}

impl CellLine {
    /// The index of dot `dot` (0 the leftmost).
    #[must_use]
    pub fn index(&self, dot: usize) -> u8 {
        if let Some(dots) = &self.dots {
            return dots.get(dot).copied().unwrap_or(self.background);
        }
        let on = self.mask.checked_shr(CELL_LAST_HALF_DOT.wrapping_sub(dot) as u32).is_some_and(|bits| bits & 1 == 1);
        if on { self.foreground } else { self.background }
    }
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

/// Which flash rates show their "on" phase at one moment (decision E.48,
/// REQ-RND-040). It is computed once per frame, from the flash unit, so the
/// cell raster only tests a bit (REQ-GEN-016).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct FlashPhase {
    /// Bit r set when [`FlashRate`] number r (slow, then fast phases 1 to
    /// 3) is on.
    on: u8,
}

/// The bit of a flash rate in [`FlashPhase`].
const fn rate_bit(rate: FlashRate) -> u8 {
    match rate {
        FlashRate::Slow => 0b0001,
        FlashRate::FastPhase1 => 0b0010,
        FlashRate::FastPhase2 => 0b0100,
        FlashRate::FastPhase3 => 0b1000,
    }
}

impl FlashPhase {
    /// Every rate on: flashing characters in their "on" phase, for still
    /// images.
    pub const STEADY: Self = Self {
        on: rate_bit(FlashRate::Slow)
            | rate_bit(FlashRate::FastPhase1)
            | rate_bit(FlashRate::FastPhase2)
            | rate_bit(FlashRate::FastPhase3),
    };

    /// The phase of flash unit `unit`, 0 to 11 (taken modulo 12).
    #[must_use]
    pub const fn at_unit(unit: u8) -> Self {
        let rates = [FlashRate::Slow, FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3];
        let mut on = 0;
        let mut index = 0;
        while index < rates.len() {
            // `index` is below the array's constant length.
            #[allow(clippy::indexing_slicing)]
            let rate = rates[index];
            if flash_on(rate, unit) {
                on |= rate_bit(rate);
            }
            index = index.wrapping_add(1);
        }
        Self { on }
    }

    /// The phase at a time on the caller's clock (REQ-GEN-005).
    #[must_use]
    pub fn at(now: Instant) -> Self {
        Self::at_unit(flash_unit(now))
    }

    /// True while a character of this rate shows its "on" phase.
    #[must_use]
    pub const fn is_on(self, rate: FlashRate) -> bool {
        self.on & rate_bit(rate) != 0
    }
}

/// The flash unit, 0 to 11, of a time on the caller's clock: the twelfth
/// of the one-second flash cycle it falls in (decision E.48, REQ-RND-040).
#[must_use]
pub fn flash_unit(now: Instant) -> u8 {
    let within = now.0.checked_rem(FLASH_CYCLE_US).unwrap_or(0);
    (within.saturating_mul(u64::from(FLASH_UNITS)) / FLASH_CYCLE_US) as u8
}

/// True when a character of `rate` is in its "on" phase in flash unit
/// `unit` (decision E.48, REQ-RND-040):
///
/// - slow flash (1 hertz) is on in units 0 to 8 and off in 9 to 11;
/// - fast flash (2 hertz) with phase p is off in units 2(p − 1) and 2p − 1
///   of each half-second of six units, and on otherwise.
///
/// Inverted flash swaps the phases; that is the cell raster's business.
#[must_use]
pub const fn flash_on(rate: FlashRate, unit: u8) -> bool {
    let unit = unit % FLASH_UNITS;
    let off_pair = match rate {
        FlashRate::Slow => return unit < FLASH_SLOW_ON_UNITS,
        FlashRate::FastPhase1 => 0,
        FlashRate::FastPhase2 => 1,
        FlashRate::FastPhase3 => 2,
    };
    (unit % FLASH_FAST_HALF_UNITS) / FLASH_FAST_OFF_UNITS != off_pair
}

/// How the row renderer turns colours into indices (decision E.55).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColourMode {
    /// The palette mode of REQ-RND-023, for Levels 1 and 1.5.
    Palette,
    /// The resolved mode of REQ-RND-031, for Levels 2.5 and 3.5, at one
    /// flash phase.
    Resolved(FlashPhase),
}

/// Viewer settings the renderer reads.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderOptions {
    /// Cell size.
    pub mode: CellMode,
    /// Character rounding in the 12-by-20 mode (REQ-RND-012).
    pub rounding: bool,
    /// Reveal is on. In the palette mode it decides which palette range
    /// concealed flashing cells use (REQ-RND-023); in the resolved mode it
    /// shows concealed cells (REQ-RND-036).
    pub reveal: bool,
    /// The colour mode.
    pub colour: ColourMode,
}

impl RenderOptions {
    /// Options from the configuration, in the palette mode of Levels 1 and
    /// 1.5.
    #[must_use]
    pub const fn from_config(config: &Config, reveal: bool) -> Self {
        Self { mode: config.cell_mode, rounding: config.rounding, reveal, colour: ColourMode::Palette }
    }

    /// Options from the configuration, in the resolved mode at a flash
    /// phase (REQ-RND-031).
    #[must_use]
    pub const fn resolved(config: &Config, reveal: bool, flash: FlashPhase) -> Self {
        Self { mode: config.cell_mode, rounding: config.rounding, reveal, colour: ColourMode::Resolved(flash) }
    }

    /// Options in the colour mode of the configured presentation level
    /// (decision E.55): the resolved mode at Levels 2.5 and 3.5, the palette
    /// mode otherwise, where `flash` is unused.
    #[must_use]
    pub const fn for_level(config: &Config, reveal: bool, flash: FlashPhase) -> Self {
        if config.level.is_enhanced() {
            Self::resolved(config, reveal, flash)
        } else {
            Self::from_config(config, reveal)
        }
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

/// The highest index of CLUT 0, the only CLUT the palette mode can show.
const CLUT0_LAST: u8 = (COLOURS - 1) as u8;
/// Colour Map indices are five bits (decision E.43).
const COLOUR_INDEX_MASK: u8 = (COLOUR_MAP_ENTRIES - 1) as u8;

/// The cell's foreground and background with invert applied (decision
/// E.47: invert swaps them before flash and conceal act).
const fn displayed_colours(cell: &Cell) -> (u8, u8) {
    if cell.attributes.invert() { (cell.background, cell.foreground) } else { (cell.foreground, cell.background) }
}

/// The palette index of a cell's foreground pixels in the palette mode
/// (REQ-RND-023).
#[must_use]
pub fn foreground_index<const DRCS: usize, const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
    reveal: bool,
) -> u8 {
    if grid.boxed_mode && !cell.boxed {
        return PALETTE_TRANSPARENT;
    }
    let (foreground, background) = displayed_colours(cell);
    let pair = foreground.wrapping_mul(COLOURS as u8).wrapping_add(background);
    if cell.conceal && !(reveal && cell.flash) {
        PALETTE_CONCEAL_BASE.wrapping_add(pair)
    } else if cell.flash {
        PALETTE_FLASH_BASE.wrapping_add(pair)
    } else {
        foreground
    }
}

/// The palette index of a cell's background pixels in the palette mode.
#[must_use]
pub fn background_index<const DRCS: usize, const PANEL_ROWS: usize>(cell: &Cell, grid: &Grid<DRCS, PANEL_ROWS>) -> u8 {
    if grid.mix || (grid.boxed_mode && !cell.boxed) { PALETTE_TRANSPARENT } else { displayed_colours(cell).1 }
}

/// Widens a 5-dot row to 10 half-dots in a 12-half-dot cell; the leftmost
/// dot column of the cell stays blank.
fn widen(dots: u8) -> u16 {
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
fn glyph_line_20(glyph: &[u8; GLYPH_LINES], line: usize, rounding: bool) -> u16 {
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
const CELL_MASK: u16 = (1 << CELL_WIDTH) - 1;
/// Shift of the leftmost half-dot in a cell mask.
const CELL_LAST_HALF_DOT: usize = CELL_WIDTH - 1;
/// The six half-dots of each half of a cell (REQ-RND-034).
const HALF_MASK: u16 = (1 << CELL_DOTS) - 1;

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

/// What one line of a cell shows before its colours are chosen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dots {
    /// Dots in the cell's foreground, gaps in its background.
    Mask(u16),
    /// A multi-colour DRCS line: the Colour Map index of each dot from the
    /// DRCS colour table, before flash and transparency.
    Multi([u8; CELL_WIDTH]),
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

/// The font styles (REQ-RND-039) and underline (REQ-RND-035) of a G0 or
/// G2 character's line, after rounding. Kept out of line: Level 1 cells
/// never have them, and the Level 1 path is every port's hot path.
#[inline(never)]
fn styled_line(mask: u16, cell: &Cell, source: usize, height: usize) -> u16 {
    let mut mask = mask;
    let font = cell.attributes.font();
    if font.bold {
        // Bold: the line ORed with itself one half-dot to the right.
        mask |= mask >> 1;
    }
    let last = height.saturating_sub(1);
    if font.italic {
        // Italic: shifted right by the lines below this one divided by 4.
        let below = last.saturating_sub(source);
        mask = mask.checked_shr((below >> ITALIC_SHIFT_LOG2) as u32).unwrap_or(0);
    }
    if cell.attributes.underline() && source == last {
        // Underline: the last dot line, across the whole cell (decision
        // E.47).
        mask = CELL_MASK;
    }
    mask & CELL_MASK
}

/// A DRCS character's line (REQ-RND-038). A missing pattern shows a space
/// in the background colour (REQ-DRC-009). Each of the ten pattern rows is
/// one line of the 12-by-10 cell and two lines of the 12-by-20 cell; mode
/// 3 is stored doubled, so every mode is drawn alike.
fn drcs_line<const DRCS: usize, const PANEL_ROWS: usize>(
    pattern: Option<&DrcsPattern>,
    grid: &Grid<DRCS, PANEL_ROWS>,
    source: usize,
    mode: CellMode,
) -> Dots {
    let Some(pattern) = pattern else { return Dots::Mask(0) };
    let row = match mode {
        CellMode::Saa5050 => source / 2,
        CellMode::Saa5246a => source,
    };
    let tables = &grid.drcs_colours;
    let global = pattern.character.table == DrcsTable::Global;
    let table: &[u8] = match (pattern.mode, global) {
        (DrcsMode::Mode0, _) => {
            let plane = pattern.planes.first().and_then(|plane| plane.get(row)).copied().unwrap_or(0);
            return Dots::Mask(plane & CELL_MASK);
        }
        (DrcsMode::Mode1, true) => &tables.global_4,
        (DrcsMode::Mode1, false) => &tables.normal_4,
        (DrcsMode::Mode2 | DrcsMode::Mode3, true) => &tables.global_16,
        (DrcsMode::Mode2 | DrcsMode::Mode3, false) => &tables.normal_16,
    };
    let mut dots = [0u8; CELL_WIDTH];
    for (dot, slot) in dots.iter_mut().enumerate() {
        let value = usize::from(pattern.pixel(row, dot));
        *slot = table.get(value).copied().unwrap_or(COLOUR_TRANSPARENT) & COLOUR_INDEX_MASK;
    }
    Dots::Multi(dots)
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

/// Double width for a multi-colour DRCS line: exact replication of the
/// left or right six dots (REQ-RND-034, decision E.54).
fn widen_dots(line: [u8; CELL_WIDTH], width: Width) -> [u8; CELL_WIDTH] {
    let first = match width {
        Width::Normal => return line,
        Width::LeftHalf => 0,
        Width::RightHalf => CELL_DOTS,
    };
    core::array::from_fn(|dot| line.get(first.saturating_add(dot / 2)).copied().unwrap_or(0))
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

/// A G3 character's line (REQ-RND-037): no rounding, font styles or
/// underline. Kept out of line, like [`styled_line`].
#[inline(never)]
fn g3_line(glyphs: &GlyphSet, code: u8, source: usize, mode: CellMode) -> u16 {
    let glyph = glyphs.g3(code);
    let line = match mode {
        CellMode::Saa5050 => glyph.large.get(source),
        CellMode::Saa5246a => glyph.small.get(source),
    };
    line.copied().unwrap_or(0) & CELL_MASK
}

/// True when the cell is in its flash "on" phase: the rate's phase,
/// swapped for inverted flash (decision E.48).
fn flash_shown(cell: &Cell, flash: FlashPhase) -> bool {
    let attributes = cell.attributes;
    flash.is_on(attributes.flash_rate()) != (attributes.flash_mode() == FlashMode::Inverted)
}

/// The display planes of Annex C.1 (Table C.1, REQ-L25-029): the index a
/// pixel of colour `index` shows. A transparent foreground or background
/// shows the full row colour, except inside a window, where the picture
/// shows; outside the boxes of a boxed page everything is picture; mix
/// makes backgrounds picture (REQ-L1-020). Row colours never colour row 0,
/// columns 0 to 7 (decision E.45).
#[allow(clippy::too_many_arguments)] // The pixel's colour, role, cell and position.
fn plane<const DRCS: usize, const PANEL_ROWS: usize>(
    index: u8,
    background: bool,
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    column: usize,
) -> u8 {
    if (grid.boxed_mode && !cell.boxed) || (background && grid.mix) {
        return COLOUR_TRANSPARENT;
    }
    let index = index & COLOUR_INDEX_MASK;
    if index != COLOUR_TRANSPARENT {
        return index;
    }
    let window = cell.boxed && !grid.boxed_mode;
    if window || (row == 0 && column < RECEIVER_COLUMNS) {
        return COLOUR_TRANSPARENT;
    }
    grid.row_colours.get(row).map_or(COLOUR_TRANSPARENT, |&colour| colour & COLOUR_INDEX_MASK)
}

/// The resolved colours of a two-colour cell (REQ-RND-036): invert, then
/// flash, then conceal, then the display planes. Returns the dots' index
/// and the gaps' index.
#[inline(never)]
fn resolved_colours<const DRCS: usize, const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    column: usize,
    flash: FlashPhase,
    reveal: bool,
) -> (u8, u8) {
    let (mut foreground, background) = displayed_colours(cell);
    let mut blank = cell.conceal && !reveal;
    if cell.flash && !flash_shown(cell, flash) {
        if cell.attributes.flash_mode() == FlashMode::AdjacentClut {
            foreground ^= ADJACENT_CLUT_FLIP;
        } else {
            blank = true;
        }
    }
    let gaps = plane(background, true, cell, grid, row, column);
    let dots = if blank { gaps } else { plane(foreground, false, cell, grid, row, column) };
    (dots, gaps)
}

/// The cell raster (REQ-RND-016): line `line` of the cell at text row `row`
/// and column `column` (0 to 39 on the main page, 40 to 71 in the
/// side-panel area), where `line` is 0 to 19 in the 12-by-20 mode and 0 to
/// 9 in the 12-by-10 mode. Size, glyphs, mosaics, G3, DRCS, underline,
/// the font styles and, in the resolved mode, invert, flash, conceal and
/// the display planes are all applied here.
///
/// In the palette mode a multi-colour DRCS character is drawn steady with
/// its table's indices; the row renderer refuses such grids in that mode.
#[must_use]
// The row renderer calls this for every cell of every line; inlined into
// its loop it measured 2 times faster (REQ-TST-050, `tests/performance.rs`).
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn cell_line<const DRCS: usize, const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
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
                let mask = text_line(&glyphs.glyph(character), source, options);
                if plain { mask } else { styled_line(mask, cell, source, height) }
            }
            CharClass::G3(code) => g3_line(glyphs, code, source, options.mode),
            CharClass::Drcs(drcs) => return drcs_cell_line(cell, grid, (row, column), drcs, source, options),
        },
    };
    if !plain {
        mask = widen_mask(mask, attributes.width());
    }
    let (foreground, background) = match options.colour {
        ColourMode::Palette => (foreground_index(cell, grid, options.reveal), background_index(cell, grid)),
        ColourMode::Resolved(flash) => resolved_colours(cell, grid, row, column, flash, options.reveal),
    };
    CellLine { mask, foreground, background, dots: None }
}

/// The cell raster of a DRCS character (REQ-RND-038): a mode 0 character
/// is drawn like any other; a multi-colour one gives an index for each dot.
#[cold]
#[inline(never)]
fn drcs_cell_line<const DRCS: usize, const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
    (row, column): (usize, usize),
    drcs: DrcsCharacter,
    source: usize,
    options: RenderOptions,
) -> CellLine {
    let dots = drcs_line(grid.drcs_pattern(drcs), grid, source, options.mode);
    let width = cell.attributes.width();
    match (dots, options.colour) {
        (Dots::Mask(mask), ColourMode::Palette) => CellLine {
            mask: widen_mask(mask, width),
            foreground: foreground_index(cell, grid, options.reveal),
            background: background_index(cell, grid),
            dots: None,
        },
        (Dots::Mask(mask), ColourMode::Resolved(flash)) => {
            let (foreground, background) = resolved_colours(cell, grid, row, column, flash, options.reveal);
            CellLine { mask: widen_mask(mask, width), foreground, background, dots: None }
        }
        (Dots::Multi(line), ColourMode::Palette) => {
            let background = background_index(cell, grid);
            CellLine { mask: 0, foreground: background, background, dots: Some(widen_dots(line, width)) }
        }
        (Dots::Multi(line), ColourMode::Resolved(flash)) => {
            multi_colour_line(cell, grid, row, column, widen_dots(line, width), flash, options.reveal)
        }
    }
}

/// A multi-colour DRCS line in the resolved mode (decisions E.48 and
/// E.54): invert has no effect; conceal, and the off phase of normal or
/// inverted flash, show a space in the background colour; adjacent-CLUT
/// flash exclusive-ORs every dot's index with 8.
fn multi_colour_line<const DRCS: usize, const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    column: usize,
    line: [u8; CELL_WIDTH],
    flash: FlashPhase,
    reveal: bool,
) -> CellLine {
    let background = plane(cell.background, true, cell, grid, row, column);
    let off = cell.flash && !flash_shown(cell, flash);
    let adjacent = off && cell.attributes.flash_mode() == FlashMode::AdjacentClut;
    if (cell.conceal && !reveal) || (off && !adjacent) {
        return CellLine { mask: 0, foreground: background, background, dots: None };
    }
    let flip = if adjacent { ADJACENT_CLUT_FLIP } else { 0 };
    let dots = line.map(|index| plane(index ^ flip, false, cell, grid, row, column));
    CellLine { mask: 0, foreground: background, background, dots: Some(dots) }
}

/// True when the palette mode can draw the cell (decision E.55): colours
/// of CLUT 0 only, no Level 2.5 flash mode, and no multi-colour DRCS.
fn palette_cell_allowed<const DRCS: usize, const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
) -> bool {
    let attributes = cell.attributes;
    let colours = cell.foreground <= CLUT0_LAST && cell.background <= CLUT0_LAST;
    let flash =
        !cell.flash || (attributes.flash_mode() == FlashMode::Normal && attributes.flash_rate() == FlashRate::Slow);
    let multi_colour = match cell.glyph {
        Glyph::Char(character) => match class(character) {
            CharClass::Drcs(drcs) => grid.drcs_pattern(drcs).is_some_and(|pattern| pattern.mode != DrcsMode::Mode0),
            CharClass::Text | CharClass::G3(_) => false,
        },
        Glyph::Space | Glyph::Mosaic { .. } => false,
    };
    colours && flash && !multi_colour
}

/// The widths of the shown side panels, each at most 16 columns.
fn panel_widths<const DRCS: usize, const PANEL_ROWS: usize>(grid: &Grid<DRCS, PANEL_ROWS>) -> (usize, usize) {
    let clamp = |width: u8| usize::from(width.min(SIDE_PANEL_MAX_COLUMNS));
    (clamp(grid.side_panels.left), clamp(grid.side_panels.right))
}

/// The end of the side-panel area: column 72.
const PANEL_AREA_END: usize = SIDE_PANEL_FIRST_COLUMN + SIDE_PANEL_AREA_COLUMNS;

/// The source columns of a row in display order (REQ-RND-033): the left
/// panel (columns 72 − left width to 71), the main page (0 to 39) and the
/// right panel (40 to 40 + right width − 1).
pub fn source_columns<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
) -> impl Iterator<Item = usize> + use<DRCS, PANEL_ROWS> {
    let (left, right) = panel_widths(grid);
    (PANEL_AREA_END.saturating_sub(left)..PANEL_AREA_END)
        .chain(0..COLUMNS)
        .chain(SIDE_PANEL_FIRST_COLUMN..SIDE_PANEL_FIRST_COLUMN.saturating_add(right))
}

/// The columns of every row of a grid: 40 plus the widths of the shown side
/// panels (decision E.49, REQ-RND-033).
#[must_use]
pub fn row_columns<const DRCS: usize, const PANEL_ROWS: usize>(grid: &Grid<DRCS, PANEL_ROWS>) -> usize {
    let (left, right) = panel_widths(grid);
    COLUMNS.saturating_add(left).saturating_add(right)
}

/// The width of every row of a grid in pixels (REQ-RND-033): 480 without
/// side panels, at most 672 with panels of 16 columns in all.
#[must_use]
pub fn row_width<const DRCS: usize, const PANEL_ROWS: usize>(grid: &Grid<DRCS, PANEL_ROWS>) -> usize {
    row_columns(grid).saturating_mul(CELL_WIDTH)
}

/// The index of the screen colour, for the borders around the rows and the
/// side-panel area of row 0 (decision E.45): the picture on a boxed page
/// and in mix mode.
#[must_use]
pub fn border_index<const DRCS: usize, const PANEL_ROWS: usize>(grid: &Grid<DRCS, PANEL_ROWS>) -> u8 {
    if grid.boxed_mode || grid.mix { COLOUR_TRANSPARENT } else { grid.screen_colour & COLOUR_INDEX_MASK }
}

/// True when the palette mode can draw the whole grid (decision E.55,
/// REQ-RND-031): every cell uses CLUT 0 colours only, no flash mode of
/// Level 2.5 and no multi-colour DRCS character, and the screen colour is
/// of CLUT 0 when side panels show it.
#[must_use]
pub fn palette_mode_allowed<const DRCS: usize, const PANEL_ROWS: usize>(grid: &Grid<DRCS, PANEL_ROWS>) -> bool {
    (0..ROWS).all(|row| palette_row_allowed(grid, row))
}

fn palette_row_allowed<const DRCS: usize, const PANEL_ROWS: usize>(grid: &Grid<DRCS, PANEL_ROWS>, row: usize) -> bool {
    let panels = row_columns(grid) > COLUMNS;
    source_columns(grid).all(|column| match source_cell(grid, row, column) {
        Some(cell) => palette_cell_allowed(cell, grid),
        None => !panels || border_index(grid) <= CLUT0_LAST || border_index(grid) == PALETTE_TRANSPARENT,
    })
}

/// The cell at a row and source column, main page or side panel; `None`
/// where the side-panel area has no content (row 0, or a grid without
/// panel rows).
#[must_use]
pub fn source_cell<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    column: usize,
) -> Option<&Cell> {
    if column < COLUMNS { grid.cell(row, column) } else { grid.panel_cell(row, column) }
}

/// Errors from the row renderer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderError {
    /// The buffer is too small for the requested row, stride and format.
    BufferTooSmall,
    /// The row or line number is out of range.
    OutOfRange,
    /// The palette mode was asked to draw a grid that needs the resolved
    /// mode: a colour outside CLUT 0, a flash mode of Level 2.5 or a
    /// multi-colour DRCS character (decision E.55, REQ-RND-031).
    NeedsResolvedMode,
    /// The 4-bit format was asked to hold a Colour Map index above 15.
    IndexTooWide,
}

/// Pixel packing of the caller's buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexFormat {
    /// One index per byte.
    Bits8,
    /// Two indices per byte, the left pixel in the high nibble. In the
    /// palette mode only indices 0 to 15 fit, so flash and conceal use the
    /// base colours; in the resolved mode an index above 15 is an error.
    Bits4,
}

/// A caller-supplied buffer of indices (REQ-RND-017).
#[derive(Debug)]
pub struct IndexBuffer<'a> {
    /// The pixel packing.
    pub format: IndexFormat,
    /// The memory.
    pub data: &'a mut [u8],
    /// Bytes from one pixel row to the next.
    pub stride: usize,
}

/// Writes one cell line as twelve 8-bit indices.
// Called for every cell of every line, as cell_line is.
#[allow(clippy::inline_always)]
#[inline(always)]
fn fill_cell(target: &mut [u8; CELL_WIDTH], raster: &CellLine) {
    match &raster.dots {
        Some(dots) => {
            for (pixel, &index) in target.iter_mut().zip(dots) {
                *pixel = index;
            }
        }
        None => {
            for (dot, pixel) in target.iter_mut().enumerate() {
                let on = (raster.mask >> CELL_LAST_HALF_DOT.wrapping_sub(dot)) & 1 == 1;
                *pixel = if on { raster.foreground } else { raster.background };
            }
        }
    }
}

/// Writes one cell line at cell slot `slot` of a pixel row.
fn write_cell(
    out: &mut [u8],
    slot: usize,
    raster: &CellLine,
    format: IndexFormat,
    check: bool,
) -> Result<(), RenderError> {
    if check {
        let widest = raster.dots.map_or(raster.foreground.max(raster.background), |dots| {
            dots.iter().copied().fold(raster.background, u8::max)
        });
        if widest > INDEX_4BIT_MAX {
            return Err(RenderError::IndexTooWide);
        }
    }
    match format {
        IndexFormat::Bits8 => {
            let start = slot.saturating_mul(CELL_WIDTH);
            let target = out.get_mut(start..start.saturating_add(CELL_WIDTH)).ok_or(RenderError::BufferTooSmall)?;
            let target = <&mut [u8; CELL_WIDTH]>::try_from(target).map_err(|_| RenderError::BufferTooSmall)?;
            fill_cell(target, raster);
        }
        IndexFormat::Bits4 => {
            let bytes = CELL_WIDTH / 2;
            let start = slot.saturating_mul(bytes);
            let target = out.get_mut(start..start.saturating_add(bytes)).ok_or(RenderError::BufferTooSmall)?;
            for (pair, byte) in target.iter_mut().enumerate() {
                let left = raster.index(pair.wrapping_mul(2)) & INDEX_4BIT_MAX;
                let right = raster.index(pair.wrapping_mul(2).wrapping_add(1)) & INDEX_4BIT_MAX;
                *byte = (left << 4) | right;
            }
        }
    }
    Ok(())
}

/// Writes one pixel row of a text row into `out`: the left panel, the main
/// page and the right panel (REQ-RND-033).
fn write_line<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    line: usize,
    glyphs: &GlyphSet,
    options: RenderOptions,
    format: IndexFormat,
    out: &mut [u8],
) -> Result<(), RenderError> {
    let width = row_width(grid);
    let needed = match format {
        IndexFormat::Bits8 => width,
        IndexFormat::Bits4 => width / 2,
    };
    let out = out.get_mut(..needed).ok_or(RenderError::BufferTooSmall)?;
    if row >= ROWS {
        return Err(RenderError::OutOfRange);
    }
    let cells = grid.cells.get(row).ok_or(RenderError::OutOfRange)?;
    let check = format == IndexFormat::Bits4 && matches!(options.colour, ColourMode::Resolved(_));
    let (left, right) = panel_widths(grid);
    if left > 0 {
        let columns = PANEL_AREA_END.saturating_sub(left)..PANEL_AREA_END;
        write_panel(grid, row, line, glyphs, options, (format, check), columns, &mut *out)?;
    }
    if format == IndexFormat::Bits8 && !check {
        // The main page, the hot path of every port: whole cells of the
        // buffer, with no per-pixel bounds checks.
        let start = left.saturating_mul(CELL_WIDTH);
        let main = out.get_mut(start..start.saturating_add(PAGE_WIDTH)).ok_or(RenderError::BufferTooSmall)?;
        for (column, (cell, target)) in cells.iter().zip(main.as_chunks_mut::<CELL_WIDTH>().0).enumerate() {
            fill_cell(target, &cell_line(cell, grid, row, column, line, glyphs, options));
        }
    } else {
        for (column, cell) in cells.iter().enumerate() {
            let raster = cell_line(cell, grid, row, column, line, glyphs, options);
            write_cell(out, left.saturating_add(column), &raster, format, check)?;
        }
    }
    if right > 0 {
        let columns = SIDE_PANEL_FIRST_COLUMN..SIDE_PANEL_FIRST_COLUMN.saturating_add(right);
        let offset = left.saturating_add(COLUMNS);
        let start = match format {
            IndexFormat::Bits8 => offset.saturating_mul(CELL_WIDTH),
            IndexFormat::Bits4 => offset.saturating_mul(CELL_WIDTH / 2),
        };
        let rest = out.get_mut(start..).ok_or(RenderError::BufferTooSmall)?;
        write_panel(grid, row, line, glyphs, options, (format, check), columns, rest)?;
    }
    Ok(())
}

/// Writes the cells of one side panel, `columns` of the side-panel area,
/// from the start of `out`. Row 0, and a grid without panel rows, show the
/// screen colour there (decision E.45).
#[allow(clippy::too_many_arguments)] // The arguments of write_line, for a part of the row.
fn write_panel<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    line: usize,
    glyphs: &GlyphSet,
    options: RenderOptions,
    (format, check): (IndexFormat, bool),
    columns: core::ops::Range<usize>,
    out: &mut [u8],
) -> Result<(), RenderError> {
    let border = border_index(grid);
    let screen = CellLine { mask: 0, foreground: border, background: border, dots: None };
    for (slot, column) in columns.enumerate() {
        let raster = match grid.panel_cell(row, column) {
            Some(cell) => cell_line(cell, grid, row, column, line, glyphs, options),
            None => screen,
        };
        write_cell(out, slot, &raster, format, check)?;
    }
    Ok(())
}

/// Refuses the palette mode for a row that needs the resolved mode.
fn check_mode<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    options: RenderOptions,
) -> Result<(), RenderError> {
    if options.colour == ColourMode::Palette && !palette_row_allowed(grid, row) {
        return Err(RenderError::NeedsResolvedMode);
    }
    Ok(())
}

/// Renders text row `row` into `out` with a row stride of `stride` bytes
/// (REQ-RND-017). In the 12-by-20 mode this draws 20 lines for
/// [`Field::Both`] and 10 for one field. Each line is [`row_width`] pixels.
/// On an error the buffer's contents are unspecified.
pub fn render_row<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    field: Field,
    glyphs: &GlyphSet,
    options: RenderOptions,
    buffer: &mut IndexBuffer<'_>,
) -> Result<(), RenderError> {
    if row >= ROWS {
        return Err(RenderError::OutOfRange);
    }
    check_mode(grid, row, options)?;
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
pub fn render_line<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
    y: usize,
    zoom: Zoom,
    glyphs: &GlyphSet,
    options: RenderOptions,
    format: IndexFormat,
    out: &mut [u8],
) -> Result<(), RenderError> {
    let height = options.row_height();
    let (row, line) = match zoom {
        Zoom::Off => (y.checked_div(height).unwrap_or(0), y.checked_rem(height).unwrap_or(0)),
        Zoom::Top | Zoom::Bottom => {
            // Rows 0 to 11 or 12 to 23 doubled; each output line shows half
            // a source line, the way double height does.
            let first = if zoom == Zoom::Top { 0 } else { ZOOM_ROWS };
            let doubled = y / 2;
            (first.saturating_add(doubled.checked_div(height).unwrap_or(0)), doubled.checked_rem(height).unwrap_or(0))
        }
    };
    if row >= ROWS {
        return Err(RenderError::OutOfRange);
    }
    check_mode(grid, row, options)?;
    write_line(grid, row, line, glyphs, options, format, out)
}

/// Rows in each half of a zoomed page (REQ-L1-020).
const ZOOM_ROWS: usize = 12;

/// Fills a 144-entry palette for a flash phase and reveal setting
/// (REQ-RND-023). `base` holds the eight colours and, at index 8,
/// transparent. The core never produces real colours itself: `T` is
/// whatever colour form the caller uses (decision E.29). In the resolved
/// mode the palette is the grid's Colour Map instead.
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

/// The flash phase at a time at Levels 1 and 1.5 (decision E.12): true
/// while shown. Levels 2.5 and 3.5 use [`FlashPhase::at`].
#[must_use]
pub fn flash_visible(now: Instant, config: &Config) -> bool {
    let period = config.flash_shown_us.saturating_add(config.flash_hidden_us);
    if period == 0 {
        return true;
    }
    now.0.checked_rem(period).unwrap_or(0) < config.flash_shown_us
}

/// Change tracking (REQ-RND-019, REQ-RND-032): which rows changed since the
/// port last asked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChangeTracker {
    hashes: [u64; ROWS],
    reveal: bool,
    flash: FlashPhase,
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

/// An FNV-1a hasher for row fingerprints.
struct Fnv(u64);

impl core::hash::Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = mix(self.0, u64::from(byte));
        }
    }
}

/// True when a flashing cell looks different in phases `old` and `new`
/// (REQ-RND-032): its rate changes phase, it can show dots, and the dots'
/// colour differs between the two phases.
fn flash_changes<const DRCS: usize, const PANEL_ROWS: usize>(
    cell: &Cell,
    grid: &Grid<DRCS, PANEL_ROWS>,
    row: usize,
    column: usize,
    (old, new): (FlashPhase, FlashPhase),
    reveal: bool,
) -> bool {
    let rate = cell.attributes.flash_rate();
    if !cell.flash || old.is_on(rate) == new.is_on(rate) {
        return false;
    }
    let hidden = (cell.conceal && !reveal) || (grid.boxed_mode && !cell.boxed);
    let shows_dots = match cell.glyph {
        Glyph::Space => false,
        Glyph::Mosaic { pattern, .. } => pattern != 0,
        Glyph::Char(character) => match class(character) {
            CharClass::Drcs(drcs) => match grid.drcs_pattern(drcs) {
                None => false,
                // A multi-colour character flashes to a space, or to the
                // adjacent CLUT, dot by dot (decision E.48).
                Some(pattern) if pattern.mode != DrcsMode::Mode0 => return !hidden,
                Some(_) => true,
            },
            CharClass::Text => !cell.glyph.is_blank() || cell.attributes.underline(),
            CharClass::G3(_) => true,
        },
    };
    shows_dots
        && resolved_colours(cell, grid, row, column, old, reveal)
            != resolved_colours(cell, grid, row, column, new, reveal)
}

fn class_of(cell: &Cell) -> Option<CharClass> {
    match cell.glyph {
        Glyph::Char(character) => Some(class(character)),
        Glyph::Space | Glyph::Mosaic { .. } => None,
    }
}

impl ChangeTracker {
    /// A tracker that reports every row as changed on first use.
    #[must_use]
    pub const fn new() -> Self {
        Self { hashes: [0; ROWS], reveal: false, flash: FlashPhase::STEADY, primed: false }
    }

    /// Rows that changed since the last call, as a bitmask (bit r for row
    /// r), for drawing with `options`. A row changes when its cells, its
    /// side-panel cells, its row colour or the page-wide presentation data
    /// change, and the row below a changed double-height row changes too.
    ///
    /// - In the palette mode (REQ-RND-023), a change of reveal marks the
    ///   rows holding flashing concealed cells; other flash and reveal
    ///   changes are palette changes only.
    /// - In the resolved mode (REQ-RND-032), a change of reveal marks the
    ///   rows holding concealed cells, and a change of flash phase marks
    ///   exactly the rows holding a cell that looks different in the two
    ///   phases.
    pub fn changed_rows<const DRCS: usize, const PANEL_ROWS: usize>(
        &mut self,
        grid: &Grid<DRCS, PANEL_ROWS>,
        options: RenderOptions,
    ) -> u32 {
        let reveal = options.reveal;
        let reveal_changed = reveal != self.reveal;
        let mut page = Fnv(FNV_OFFSET);
        core::hash::Hash::hash(
            &(grid.boxed_mode, grid.mix, grid.side_panels, grid.colour_map, grid.screen_colour),
            &mut page,
        );
        let page = core::hash::Hasher::finish(&page);
        let mut changed = 0u32;
        for (row, stored) in self.hashes.iter_mut().enumerate() {
            let panel_row = row.checked_sub(1).and_then(|index| grid.panels.get(index));
            let cells = grid.cells.get(row).into_iter().flat_map(|cells| cells.iter().enumerate()).chain(
                panel_row.into_iter().flat_map(|cells| {
                    cells.iter().enumerate().map(|(index, cell)| (index.saturating_add(SIDE_PANEL_FIRST_COLUMN), cell))
                }),
            );
            let mut hasher = Fnv(page);
            if let Some(cells) = grid.cells.get(row) {
                core::hash::Hash::hash(cells, &mut hasher);
            }
            core::hash::Hash::hash(&grid.row_colours.get(row), &mut hasher);
            if let Some(panel) = panel_row {
                core::hash::Hash::hash(panel, &mut hasher);
            }
            // A DRCS page that arrives or changes redraws only the rows
            // whose cells use it (REQ-RND-032).
            for (_, cell) in cells.clone() {
                if let Some(CharClass::Drcs(drcs)) = class_of(cell) {
                    core::hash::Hash::hash(&(grid.drcs_pattern(drcs), grid.drcs_colours), &mut hasher);
                }
            }
            let hash = core::hash::Hasher::finish(&hasher);
            let sensitive = match options.colour {
                ColourMode::Palette => reveal_changed && cells.clone().any(|(_, cell)| cell.flash && cell.conceal),
                ColourMode::Resolved(flash) => {
                    (reveal_changed && cells.clone().any(|(_, cell)| cell.conceal))
                        || (flash != self.flash
                            && cells.clone().any(|(column, cell)| {
                                flash_changes(cell, grid, row, column, (self.flash, flash), reveal)
                            }))
                }
            };
            let bit = 1u32.checked_shl(row as u32).unwrap_or(0);
            if !self.primed || hash != *stored || sensitive {
                changed |= bit;
            }
            *stored = hash;
        }
        // REQ-RND-019: the row below a changed double-height row changes too.
        let doubled = changed & grid.double_height_rows;
        changed |= doubled << 1;
        self.reveal = reveal;
        self.flash = match options.colour {
            ColourMode::Resolved(flash) => flash,
            ColourMode::Palette => FlashPhase::STEADY,
        };
        self.primed = true;
        changed & ALL_ROWS
    }

    /// Forces a full redraw on the next call.
    pub fn invalidate(&mut self) {
        self.primed = false;
    }
}

/// Renders every row, or only changed rows, into a whole-screen framebuffer
/// (REQ-RND-022), the reference caller of REQ-RND-021. `rows` is a bitmask
/// of rows to draw. Each row is [`row_width`] pixels wide from the start of
/// the framebuffer's lines; filling any border beyond it with
/// [`border_index`] is for the caller.
pub fn render_screen<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &Grid<DRCS, PANEL_ROWS>,
    rows: u32,
    glyphs: &GlyphSet,
    options: RenderOptions,
    framebuffer: &mut IndexBuffer<'_>,
) -> Result<(), RenderError> {
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

// The tests draw with the shipped glyph set.
#[cfg(all(test, feature = "original"))]
mod level25_tests;

#[cfg(all(test, feature = "original"))]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::charset::{CharRef, CharSet, G0, G0Set, NationalOption};
    use crate::consts::{COLOUR_BLUE_BIT, COLOUR_RED_BIT, PAGE_WIDTH};
    use std::vec;
    use std::vec::Vec;

    fn options() -> RenderOptions {
        RenderOptions { mode: CellMode::Saa5050, rounding: true, reveal: false, colour: ColourMode::Palette }
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

    /// REQ-RND-013: decision E.14's geometry in both cell sizes.
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

    /// REQ-RND-021's test harness: rows drawn alone, in reverse order and
    /// line by line match the whole-screen loop.
    #[test]
    fn rows_render_identically_in_any_order() {
        let mut grid = Grid::EMPTY;
        let g0 = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) });
        grid.cells[2][3].glyph = Glyph::Char(CharRef::new(g0, b'A'));
        grid.cells[2][4].glyph = Glyph::Mosaic { pattern: 0x15, separated: false };
        let glyphs = GlyphSet::original();
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
        assert_eq!(tracker.changed_rows(&grid, options()), ALL_ROWS);
        assert_eq!(tracker.changed_rows(&grid, options()), 0);
        grid.cells[5][0].foreground = 3;
        assert_eq!(tracker.changed_rows(&grid, options()), 1 << 5);
        grid.cells[7][0].flash = true;
        grid.cells[7][0].conceal = true;
        tracker.changed_rows(&grid, options());
        let revealed = RenderOptions { reveal: true, ..options() };
        assert_eq!(tracker.changed_rows(&grid, revealed), 1 << 7, "reveal redraws flashing concealed rows only");
    }

    /// REQ-L1-009, REQ-L1-010, REQ-L1-011: what the flash, conceal and box
    /// marks of a cell do to its pixels.
    #[test]
    fn flash_conceal_and_box_marks_drive_the_palette() {
        // Colours 1 and 4 are the single-gun colours.
        let (red, blue) = (COLOUR_RED_BIT, COLOUR_BLUE_BIT);
        let base = [0, 1, 2, 3, 4, 5, 6, 7, 8];
        let mut grid = Grid::EMPTY;
        let plain = Cell { foreground: red, background: blue, ..Cell::BLANK };
        // Flash: foreground pixels take the background colour in the hidden
        // phase.
        let index = usize::from(foreground_index(&Cell { flash: true, ..plain }, &grid, false));
        for (visible, colour) in [(true, red), (false, blue)] {
            let mut palette = [0u8; PALETTE_SIZE];
            fill_palette(&mut palette, &base, visible, false);
            assert_eq!(palette[index], colour, "flash visible {visible}");
        }
        // Conceal: shown as a space on its background unless revealed.
        let index = usize::from(foreground_index(&Cell { conceal: true, ..plain }, &grid, false));
        for (reveal, colour) in [(false, blue), (true, red)] {
            let mut palette = [0u8; PALETTE_SIZE];
            fill_palette(&mut palette, &base, true, reveal);
            assert_eq!(palette[index], colour, "reveal {reveal}");
        }
        // Box membership changes nothing outside boxed mode (decision E.22).
        let boxed = Cell { boxed: true, ..plain };
        assert_eq!((foreground_index(&plain, &grid, false), background_index(&plain, &grid)), (red, blue));
        assert_eq!((foreground_index(&boxed, &grid, false), background_index(&boxed, &grid)), (red, blue));
        grid.boxed_mode = true;
        let picture = (PALETTE_TRANSPARENT, PALETTE_TRANSPARENT);
        assert_eq!((foreground_index(&plain, &grid, false), background_index(&plain, &grid)), picture);
        assert_eq!((foreground_index(&boxed, &grid, false), background_index(&boxed, &grid)), (red, blue));
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
