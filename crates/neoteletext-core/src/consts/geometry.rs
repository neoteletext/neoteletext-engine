//! Rendering geometry (Part C, decisions E.13 and E.14, REQ-RND-002/003):
//! the cell and the page, loaded fonts (REQ-FNT-002), the aspect ratio, the
//! palette indices (REQ-RND-023) and the rows double height takes effect in.

use super::pages::{COLUMNS, ROWS};

/// Dots across a glyph.
pub const GLYPH_DOTS: usize = 5;
/// Lines of a glyph.
pub const GLYPH_LINES: usize = 9;
/// Dot columns of a cell: the glyph plus one blank column on the left.
pub const CELL_DOTS: usize = 6;
/// Lines of a cell in one field: the glyph plus one blank line.
pub const CELL_FIELD_LINES: usize = 10;
/// Half-dots across a cell in the 12-by-20 mode.
pub const CELL_WIDTH: usize = 12;
/// Lines of a cell in the 12-by-20 mode (both fields).
pub const CELL_HEIGHT: usize = 20;
/// Lines of a cell in the 12-by-10 mode.
pub const CELL_HEIGHT_SMALL: usize = 10;

// ---------------------------------------------------------------------------
// Loaded fonts (REQ-FNT-002, decision E.72): a 12 by 10 cell, the cell of
// the standard's own downloadable characters (clause 14.1)
// ---------------------------------------------------------------------------

/// The most characters a loaded font may have.
pub const FONT_MAX_CHARACTERS: usize = 1024;
/// The most properties a font file may declare.
pub const FONT_MAX_PROPERTIES: usize = 64;
/// The largest font file, in bytes.
pub const FONT_MAX_FILE_BYTES: usize = 256 * 1024;
/// The longest line of a font file, in bytes, not counting its end.
pub const FONT_MAX_LINE_BYTES: usize = 512;
/// Page width in pixels: 40 cells of 12.
pub const PAGE_WIDTH: usize = COLUMNS * CELL_WIDTH;
/// Page height in the 12-by-20 mode: 25 rows of 20 lines.
pub const PAGE_HEIGHT: usize = ROWS * CELL_HEIGHT;
/// Contiguous mosaic block rows: 3/10, 4/10 and 3/10 of the cell (E.14).
pub const MOSAIC_ROW_TENTHS: [usize; 3] = [3, 4, 3];
/// On a 4:3 PAL display a pixel of the 12-by-20 cell is wider than tall
/// (REQ-RND-004): a half-dot, 83 ns of the 52 µs active line, is 1/624 of
/// the screen's width, and a line, one of 576 visible lines with the height
/// three quarters of the width, is 1/768 of it. The width is stretched by
/// 768 / 624, which is 16 / 13.
pub const ASPECT_NUMERATOR: usize = 16;
/// Denominator of the aspect factor.
pub const ASPECT_DENOMINATOR: usize = 13;

// ---------------------------------------------------------------------------
// Palette indices (REQ-RND-023)
// ---------------------------------------------------------------------------

/// Index of transparent ("picture").
pub const PALETTE_TRANSPARENT: u8 = 8;
/// First index of flashing foreground pixels: 16 + 8 × f + b.
pub const PALETTE_FLASH_BASE: u8 = 16;
/// First index of concealed foreground pixels: 80 + 8 × f + b.
pub const PALETTE_CONCEAL_BASE: u8 = 80;
/// Number of palette entries.
pub const PALETTE_SIZE: usize = 144;

/// The largest palette index a 4-bit pixel holds (REQ-RND-017).
pub const INDEX_4BIT_MAX: u8 = 0x0F;

/// The first row in which double height and double size take effect: they
/// never do in rows 0, 23 and 24 (decision E.15, Annex C.3 rule 1).
pub const DOUBLE_HEIGHT_FIRST_ROW: usize = 1;
/// The last row in which double height and double size take effect.
pub const DOUBLE_HEIGHT_LAST_ROW: usize = 22;
