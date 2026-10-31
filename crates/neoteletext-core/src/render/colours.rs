//! The colours of a pixel: palette indices in the palette mode
//! (REQ-RND-023), Colour Map indices in the resolved mode (REQ-RND-031,
//! REQ-RND-036) with the display planes of Annex C.1, and the palette.

use super::flash::{FlashPhase, flash_shown};
use crate::consts::{
    ADJACENT_CLUT_FLIP, COLOUR_MAP_ENTRIES, COLOUR_TRANSPARENT, COLOURS, PALETTE_CONCEAL_BASE, PALETTE_FLASH_BASE,
    PALETTE_SIZE, PALETTE_TRANSPARENT, RECEIVER_COLUMNS, ROWS,
};
use crate::level1::{Cell, FlashMode, FlashRate, Grid};

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
pub fn foreground_index(cell: &Cell, grid: &Grid, reveal: bool) -> u8 {
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
pub fn background_index(cell: &Cell, grid: &Grid) -> u8 {
    if grid.mix || (grid.boxed_mode && !cell.boxed) { PALETTE_TRANSPARENT } else { displayed_colours(cell).1 }
}

/// The display planes of Annex C.1 (Table C.1, REQ-L25-029): the index a
/// pixel of colour `index` shows. A transparent foreground or background
/// shows the full row colour, except inside a window, where the picture
/// shows; outside the boxes of a boxed page everything is picture; mix
/// makes backgrounds picture (REQ-L1-020). Row colours never colour row 0,
/// columns 0 to 7 (decision E.45).
#[allow(clippy::too_many_arguments)] // The pixel's colour, role, cell and position.
fn plane(index: u8, background: bool, cell: &Cell, grid: &Grid, row: usize, column: usize) -> u8 {
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
pub(super) fn resolved_colours(
    cell: &Cell,
    grid: &Grid,
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

/// True when the palette mode can draw the cell (decision E.55): colours
/// of CLUT 0 only, and no Level 2.5 flash mode.
fn palette_cell_allowed(cell: &Cell) -> bool {
    let attributes = cell.attributes;
    let colours = cell.foreground <= CLUT0_LAST && cell.background <= CLUT0_LAST;
    let flash =
        !cell.flash || (attributes.flash_mode() == FlashMode::Normal && attributes.flash_rate() == FlashRate::Level1);
    colours && flash
}

/// The index of the screen colour, for the borders around the rows
/// (decision E.45): the picture on a boxed page and in mix mode.
#[must_use]
pub fn border_index(grid: &Grid) -> u8 {
    if grid.boxed_mode || grid.mix { COLOUR_TRANSPARENT } else { grid.screen_colour & COLOUR_INDEX_MASK }
}

/// True when the palette mode can draw the whole grid (decision E.55,
/// REQ-RND-031): every cell uses CLUT 0 colours only and no flash mode of
/// Level 2.5.
#[must_use]
pub fn palette_mode_allowed(grid: &Grid) -> bool {
    (0..ROWS).all(|row| palette_row_allowed(grid, row))
}

pub(super) fn palette_row_allowed(grid: &Grid, row: usize) -> bool {
    grid.cells.get(row).is_none_or(|cells| cells.iter().all(palette_cell_allowed))
}

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
