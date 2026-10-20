//! Presenting a grid for any front end: plain text (REQ-RND-008) and a
//! palette-index raster (REQ-RND-007), for `neottx` and any other front
//! end.
//!
//! Text: rows are composed with their side panels, and G3 characters
//! convert as REQ-CHR-011 says.
//! Every character written passes the allow-list of REQ-SEC-010.

use neoteletext_core::charset::{compose, mosaic_to_unicode, to_unicode};
use neoteletext_core::consts::ROWS;
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::{Cell, Glyph, Grid, Size, Width};
use neoteletext_core::policy::is_safe_output_char;
use neoteletext_core::render::{
    ALL_ROWS, ColourMode, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, palette_mode_allowed, render_screen,
    row_width, source_cell, source_columns,
};
use neoteletext_image::palette::{Rgba, clut0_of, colour_map_palette, palette};
use neoteletext_image::scale::{scale_indices, scaled_size};

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

/// A rendered page: indices plus the palette they index.
pub struct Raster {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Indices, row by row.
    pub indices: Vec<u8>,
    /// The palette: 144 entries in the palette mode, the 32 colours of the
    /// page's Colour Map in the resolved mode.
    pub palette: Vec<Rgba>,
}

/// Scaling of the output image (REQ-RND-004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Scale {
    /// Horizontal factor.
    pub x: usize,
    /// Vertical factor.
    pub y: usize,
    /// Stretch height by 1.2 for 4:3 proportions.
    pub aspect: bool,
}

/// Renders a grid to a scaled raster with `glyphs`. `picture` is the colour of the picture, transparent areas
/// (REQ-RND-006). A grid that the palette mode cannot draw is drawn in the
/// resolved mode. `flash_visible` is the phase of decision E.12, used by
/// the palette mode only.
#[must_use]
pub fn raster(
    grid: &Grid,
    options: RenderOptions,
    flash_visible: bool,
    glyphs: &GlyphSet,
    scale: Scale,
    picture: Rgba,
) -> Raster {
    let mut options = options;
    if options.colour == ColourMode::Palette && !palette_mode_allowed(grid) {
        options.colour = ColourMode::Resolved(FlashPhase::STEADY);
    }
    let width = row_width(grid);
    let height = ROWS * options.row_height();
    let mut indices = vec![0u8; width * height];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut indices, stride: width };
    // The buffer is sized for the whole page and the mode suits the grid,
    // so rendering cannot fail.
    let _ = render_screen(grid, ALL_ROWS, glyphs, options, &mut buffer);
    let (scaled_width, scaled_height) = scaled_size(width, height, scale.x, scale.y, scale.aspect);
    let mut scaled = vec![0u8; scaled_width * scaled_height];
    let _ = scale_indices(&indices, width, height, scale.x.max(1), scale.y.max(1), scale.aspect, &mut scaled);
    let palette = match options.colour {
        ColourMode::Palette => palette(&clut0_of(&grid.colour_map), picture, flash_visible, options.reveal).to_vec(),
        ColourMode::Resolved(_) => colour_map_palette(&grid.colour_map, picture).to_vec(),
    };
    Raster { width: scaled_width, height: scaled_height, indices: scaled, palette }
}

/// Every column passed parity: one bit per column of a stored row.
const ALL_COLUMNS_GOOD: u64 = (1 << neoteletext_core::consts::COLUMNS) - 1;
/// Every row received: one bit per row of the display.
const ALL_ROWS_RECEIVED: u32 = (1 << ROWS) - 1;

/// The grid of a page the receiver draws itself: `rows` are 7-bit codes,
/// row 0's columns 8 to 39 its header, and `status` the receiver's status
/// area. The rows go through the same Level 1 decoding as a received page,
/// so they look like teletext, but they never come from a decoder process.
#[must_use]
pub fn receiver_grid(
    rows: &[[u8; neoteletext_core::consts::COLUMNS]; ROWS],
    status: [u8; neoteletext_core::consts::STATUS_COLUMNS],
) -> Grid {
    use neoteletext_core::acq::{MagazineData, StoredPage};
    use neoteletext_core::config::Config;
    use neoteletext_core::ecc::with_odd_parity;
    use neoteletext_core::fsm::PageState;
    use neoteletext_core::level1::{DisplayOptions, build_grid};
    let mut page = StoredPage::EMPTY;
    page.state = PageState::Complete;
    for (stored, row) in page.cells.iter_mut().zip(rows) {
        for (cell, &code) in stored.iter_mut().zip(row) {
            *cell = with_odd_parity(code);
        }
    }
    page.parity_ok = [ALL_COLUMNS_GOOD; neoteletext_core::consts::STORED_ROWS];
    page.ever_good = [ALL_COLUMNS_GOOD; neoteletext_core::consts::STORED_ROWS];
    page.rows_received = ALL_ROWS_RECEIVED;
    let mut grid = Grid::EMPTY;
    let options = DisplayOptions { status, ..DisplayOptions::default() };
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &Config::default(), &options);
    grid
}

#[cfg(test)]
mod receiver_tests {
    use super::*;
    use neoteletext_core::consts::{ALPHA_YELLOW, COLUMNS, SPACE, STATUS_COLUMNS};

    #[test]
    fn the_receivers_rows_read_back_as_written() {
        let mut rows = [[SPACE; COLUMNS]; ROWS];
        rows[0][8..19].copy_from_slice(b" example.or");
        rows[3][..6].copy_from_slice(&[ALPHA_YELLOW, b'H', b'E', b'L', b'L', b'O']);
        let mut status = [SPACE; STATUS_COLUMNS];
        status[..4].copy_from_slice(b"P100");
        let text = plain(&receiver_grid(&rows, status), false, MosaicStyle::Unicode16);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("P100") && lines[0].contains("example.or"), "{text}");
        assert_eq!(lines[3].trim(), "HELLO");
    }
}
