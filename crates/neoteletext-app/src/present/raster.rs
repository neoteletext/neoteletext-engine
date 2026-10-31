//! The palette-index raster of a page (REQ-RND-007): the rendered
//! indices and the palette they index, scaled as REQ-RND-004 asks.

use neoteletext_core::consts::{PAGE_WIDTH, ROWS};
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::Grid;
use neoteletext_core::render::{
    ALL_ROWS, ColourMode, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, palette_mode_allowed, render_screen,
};
use neoteletext_image::palette::{Rgba, clut0_of, colour_map_palette, palette};
use neoteletext_image::scale::{scale_indices, scaled_size};

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
