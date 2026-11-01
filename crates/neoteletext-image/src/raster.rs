//! The palette-index raster of a page (REQ-RND-007, feature `alloc`): the
//! rendered indices and the palette they index, scaled as REQ-RND-004 asks,
//! its PNG, and a whole page's picture from its packets, for a program that
//! has no decoder of its own: a server's web answer (neoteletext-common's
//! REQ-1728 item 4), and `neottx`.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::Infallible;

use neoteletext_core::acq::{Decoder, StoredPage};
use neoteletext_core::compose::compose_level;
use neoteletext_core::config::Config;
use neoteletext_core::consts::{PACKET_BYTES, PAGE_WIDTH, ROWS, STATUS_COLUMNS};
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::{DisplayOptions, Grid, build_grid};
use neoteletext_core::packet::{Packet, PacketMeta};
use neoteletext_core::render::{
    ALL_ROWS, ColourMode, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, palette_mode_allowed, render_screen,
};
use neoteletext_core::types::{Instant, PageAddress};

use crate::Sink;
use crate::palette::{Rgba, clut0_of, colour_map_palette, palette};
use crate::png::{self, PngError};
use crate::scale::{scale_indices, scaled_size};

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
    let width = PAGE_WIDTH;
    let height = ROWS.saturating_mul(options.row_height());
    let mut indices = vec![0u8; width.saturating_mul(height)];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut indices, stride: width };
    // The buffer is sized for the whole page and the mode suits the grid,
    // so rendering cannot fail.
    let _ = render_screen(grid, ALL_ROWS, glyphs, options, &mut buffer);
    let (scaled_width, scaled_height) = scaled_size(width, height, scale.x, scale.y, scale.aspect);
    let mut scaled = vec![0u8; scaled_width.saturating_mul(scaled_height)];
    let _ = scale_indices(&indices, width, height, scale.x.max(1), scale.y.max(1), scale.aspect, &mut scaled);
    let palette = match options.colour {
        ColourMode::Palette => palette(&clut0_of(&grid.colour_map), picture, flash_visible, options.reveal).to_vec(),
        ColourMode::Resolved(_) => colour_map_palette(&grid.colour_map, picture).to_vec(),
    };
    Raster { width: scaled_width, height: scaled_height, indices: scaled, palette }
}

/// A growing sink, which cannot fail.
struct Bytes(Vec<u8>);

impl Sink for Bytes {
    type Error = Infallible;

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Infallible> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

/// A raster as PNG bytes.
///
/// # Errors
/// A raster the writer refuses: an empty one, or a palette over its limit.
pub fn png(raster: &Raster) -> Result<Vec<u8>, PngError<Infallible>> {
    let mut out = Bytes(Vec::new());
    // The two rows the writer compares, each with its filter byte.
    let mut scratch = vec![0u8; raster.width.saturating_add(1).saturating_mul(2)];
    let width = raster.width;
    png::write(
        &mut out,
        raster.width,
        raster.height,
        &raster.palette,
        &mut |y, row: &mut [u8]| {
            if let Some(source) = raster.indices.get(y.saturating_mul(width)..y.saturating_add(1).saturating_mul(width))
            {
                row.copy_from_slice(source);
            }
        },
        &mut scratch,
    )?;
    Ok(out.0)
}

/// A page's grid as a receiver of the default profile composes it, from the
/// packets of one sub-page: the same decoding as a received page, Levels 2.5
/// and 3.5 included, so that nothing here can draw a page otherwise than a
/// receiver does. `status` is the receiver's area of row 0. `None` when the
/// packets hold no page `page`.
#[must_use]
pub fn grid_of_packets(
    packets: &[[u8; PACKET_BYTES]],
    page: PageAddress,
    status: [u8; STATUS_COLUMNS],
) -> Option<Box<Grid>> {
    // One sub-page in, so one slot. It is on the heap because it is too
    // large for the small stacks of an async task and of WebAssembly.
    let mut slots = vec![StoredPage::EMPTY; 1].into_boxed_slice();
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let meta = PacketMeta::default();
    for &bytes in packets {
        decoder.push(&Packet { bytes }, &meta, Instant(0), &mut |_| {});
    }
    let stored = decoder.store().pages().iter().find(|stored| stored.key.address == page)?;
    let mut grid = Box::new(Grid::EMPTY);
    let options = DisplayOptions { status, ..DisplayOptions::default() };
    let magazine = decoder.magazine_data(page.magazine);
    build_grid(&mut grid, Some(stored), &magazine, decoder.config(), &options);
    let _ = compose_level(&mut grid, stored, &magazine, &decoder.enhancement_lookup(), decoder.config(), &options);
    Some(grid)
}

/// A page's picture: `grid` drawn still, as `neottx render` draws it, at the
/// engine's own size with no scaling, flashing text in its "on" phase and
/// concealed text concealed, in Noto Teletext, the one compiled font
/// (REQ-FNT-001). The size is the raster's, which `picture_size` gives.
///
/// # Errors
/// As the function `png`.
pub fn picture(grid: &Grid) -> Result<Vec<u8>, PngError<Infallible>> {
    let options = RenderOptions::for_level(&Config::default(), false, FlashPhase::STEADY);
    let scale = Scale { x: 1, y: 1, aspect: false };
    png(&raster(grid, options, true, &GlyphSet::noto_teletext(), scale, Rgba::opaque(0, 0, 0)))
}

/// The width and height of what [`picture`] draws, in pixels.
#[must_use]
pub fn picture_size() -> (usize, usize) {
    let options = RenderOptions::for_level(&Config::default(), false, FlashPhase::STEADY);
    (PAGE_WIDTH, ROWS.saturating_mul(options.row_height()))
}

#[cfg(test)]
mod tests;
