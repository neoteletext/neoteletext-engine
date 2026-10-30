//! Colours (Table 30): the Level 1 colours of CLUT 0, the Colour Map of
//! Levels 2.5 and 3.5 (decision E.43), Colour Table Re-mapping, the side
//! panels (clause 12.5, decision E.49) and the screen's columns.

use super::object_pages::SIDE_PANEL_MAX_COLUMNS;
use super::pages::{COLUMNS, ROWS};

/// Colour number of black.
pub const COLOUR_BLACK: u8 = 0;
/// Colour number of green.
pub const COLOUR_GREEN: u8 = 2;
/// Colour number of white.
pub const COLOUR_WHITE: u8 = 7;
/// Number of Level 1 colours.
pub const COLOURS: usize = 8;
/// Bit of the red primary in a colour number (REQ-L1-005).
pub const COLOUR_RED_BIT: u8 = 0x01;
/// Bit of the green primary.
pub const COLOUR_GREEN_BIT: u8 = 0x02;
/// Bit of the blue primary.
pub const COLOUR_BLUE_BIT: u8 = 0x04;
/// Full intensity of a primary in 8-bit output (REQ-RND-005).
pub const PRIMARY_FULL: u8 = 0xFF;

// ---------------------------------------------------------------------------
// The Colour Map of Levels 2.5 and 3.5 (Table 30, decision E.43)
// ---------------------------------------------------------------------------

/// Entries of one colour lookup table (CLUT). The eight Level 1 colours are
/// CLUT 0, so existing colour numbers keep their meaning (decision E.43).
pub const CLUT_ENTRIES: usize = COLOURS;
/// Colour lookup tables in the Colour Map, CLUTs 0 to 3.
pub const CLUTS: usize = 4;
/// Entries of the Colour Map: Colour Map index = 8 × CLUT + entry.
pub const COLOUR_MAP_ENTRIES: usize = CLUTS * CLUT_ENTRIES;
/// Shift of the CLUT number in a Colour Map index.
pub const CLUT_SHIFT: u32 = 3;
/// Mask of the entry number in a Colour Map index.
pub const CLUT_ENTRY_MASK: u8 = 0x07;
/// Colour Map index 8, CLUT 1 entry 0: transparent at every level, and
/// never redefined (Table 30). It is also the renderer's transparent
/// palette index ([`PALETTE_TRANSPARENT`](super::PALETTE_TRANSPARENT)); a test asserts they agree.
pub const COLOUR_TRANSPARENT: u8 = 8;
/// The first CLUT that X/28/4 and M/29/4 redefine: CLUTs 0 and 1 (Table 10,
/// decision E.43).
pub const CLUT_LEVEL_35_FIRST: u8 = 0;
/// The first CLUT that X/28/0 format 1 and M/29/0 redefine: CLUTs 2 and 3
/// (Table 4).
pub const CLUT_LEVEL_25_FIRST: u8 = 2;
/// The default Colour Map of Table 30, verified on the page image (p.89):
/// each entry `0xRGB`, four bits per primary. CLUT 1 entry 5 is 7, 0, 7 and
/// entry 6 is 0, 7, 7 (decision E.43). CLUT 1
/// entry 0 is transparent; its value here is never shown.
pub const COLOUR_MAP_DEFAULTS: [u16; COLOUR_MAP_ENTRIES] = [
    // CLUT 0: black, red, green, yellow, blue, magenta, cyan, white.
    0x000, 0xF00, 0x0F0, 0xFF0, 0x00F, 0xF0F, 0x0FF, 0xFFF, //
    // CLUT 1: transparent, then the half-intensity colours and grey.
    0x000, 0x700, 0x070, 0x770, 0x007, 0x707, 0x077, 0x777, //
    // CLUT 2.
    0xF05, 0xF70, 0x0F7, 0xFFB, 0x0CA, 0x500, 0x652, 0xC77, //
    // CLUT 3.
    0x333, 0xF77, 0x7F7, 0xFF7, 0x77F, 0xF7F, 0x7FF, 0xDDD,
];
/// A 4-bit primary times 17 is its 8-bit value, so that 15 gives 255
/// (REQ-RND-030: 16 equal steps).
pub const PRIMARY_4_TO_8: u8 = 17;
/// Colour Table Re-mapping, Table 4 triplet 13 bits 16 to 18: the
/// foreground and background CLUTs of the Level 1 spacing colours for each
/// value 0 to 7.
pub const REMAPPING_CLUTS: [[u8; 2]; 8] = [[0, 0], [0, 1], [0, 2], [1, 1], [1, 2], [2, 1], [2, 2], [2, 3]];

// ---------------------------------------------------------------------------
// Side panels (clause 12.5, decision E.49)
// ---------------------------------------------------------------------------

/// The first column of the side-panel area, where the right panel starts
/// (clause 12.5.3).
pub const SIDE_PANEL_FIRST_COLUMN: usize = COLUMNS;
/// Columns of the side-panel area, 40 to 71; the left panel ends at 71.
pub const SIDE_PANEL_AREA_COLUMNS: usize = 2 * SIDE_PANEL_MAX_COLUMNS as usize;
/// The first row of the side panels; row 0 has no panel content (clause
/// 12.5.2).
pub const SIDE_PANEL_FIRST_ROW: usize = 1;
/// Rows of the side panels, 1 to 24.
pub const SIDE_PANEL_ROWS: usize = ROWS - SIDE_PANEL_FIRST_ROW;

/// Adjacent-CLUT flash exclusive-ORs a Colour Map index with 8: the same
/// entry of the neighbouring CLUT (decision E.48).
pub const ADJACENT_CLUT_FLIP: u8 = 1 << CLUT_SHIFT;

/// The columns of a screen row in scan order: the 40 of the main page,
/// then the side-panel area of columns 40 to 71 (clause 12.5.3, Annex A.4).
pub const SCREEN_COLUMNS: usize = COLUMNS + SIDE_PANEL_AREA_COLUMNS;
