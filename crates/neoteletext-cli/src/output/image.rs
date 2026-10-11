//! Raster output: palette-index framebuffers, PNG and PPM files
//! (REQ-RND-007), and the iTerm2 inline image protocol (REQ-RND-029).
//!
//! At Levels 1 and 1.5 a raster holds the indices of the palette mode and
//! the 144-entry palette of one flash phase (REQ-RND-023); at Levels 2.5
//! and 3.5 it holds Colour Map indices and the page's 32 colours
//! (REQ-RND-030, REQ-RND-031). Rows are as wide as the page's side panels
//! make them (REQ-RND-033).

use neoteletext_core::config::Config;
use neoteletext_core::consts::{COLOUR_TRANSPARENT, PALETTE_TRANSPARENT, ROWS};
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::Grid;
use neoteletext_core::render::{
    ALL_ROWS, ColourMode, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, palette_mode_allowed, render_screen,
    row_width,
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

/// Which glyphs a raster is drawn with (decision E.62): the project's own,
/// the temporary Bedstead set, or both side by side for comparison.
#[derive(clap::ValueEnum, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Glyphs {
    /// The project's own glyphs.
    #[default]
    Original,
    /// The temporary Bedstead glyphs.
    Bedstead,
    /// The page twice, the original glyphs on the left and Bedstead on the
    /// right.
    Compare,
}

impl Glyphs {
    /// The next choice, for the viewer's glyph key.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Original => Self::Bedstead,
            Self::Bedstead => Self::Compare,
            Self::Compare => Self::Original,
        }
    }

    /// The glyph sets drawn, left to right.
    fn sets(self) -> &'static [GlyphSet] {
        const ORIGINAL: [GlyphSet; 1] = [GlyphSet::original()];
        const BEDSTEAD: [GlyphSet; 1] = [GlyphSet::bedstead()];
        const BOTH: [GlyphSet; 2] = [GlyphSet::original(), GlyphSet::bedstead()];
        match self {
            Self::Original => &ORIGINAL,
            Self::Bedstead => &BEDSTEAD,
            Self::Compare => &BOTH,
        }
    }
}

/// Columns of picture colour between the two pages of a comparison.
const COMPARE_GAP: usize = 12;

/// The settings of one frame (decision E.55, REQ-RND-040): the colour mode
/// of the presentation level with its flash phase of decision E.48, and,
/// for the palette mode, whether flashing characters show under the timing
/// of decision E.12.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Frame {
    /// The renderer's options.
    pub options: RenderOptions,
    /// The phase of decision E.12, used by the palette mode only.
    pub flash_visible: bool,
    /// The glyphs drawn.
    pub glyphs: Glyphs,
}

impl Frame {
    /// A still image: flashing characters in their "on" phase.
    #[must_use]
    pub const fn still(config: &Config, reveal: bool) -> Self {
        Self {
            options: RenderOptions::for_level(config, reveal, FlashPhase::STEADY),
            flash_visible: true,
            glyphs: Glyphs::Original,
        }
    }

    /// The same frame drawn with other glyphs.
    #[must_use]
    pub const fn with_glyphs(self, glyphs: Glyphs) -> Self {
        Self { glyphs, ..self }
    }

    /// The frame at a time on the decoder's clock. In the resolved mode
    /// only the flash phase changes it. Only the viewer animates.
    #[cfg(feature = "view")]
    #[must_use]
    pub fn at(config: &Config, reveal: bool, now: neoteletext_core::types::Instant) -> Self {
        let options = RenderOptions::for_level(config, reveal, FlashPhase::at(now));
        let flash_visible =
            options.colour != ColourMode::Palette || neoteletext_core::render::flash_visible(now, config);
        Self { options, flash_visible, glyphs: Glyphs::Original }
    }
}

/// Renders a grid to a scaled raster. `picture` is the colour of the
/// picture, transparent areas (REQ-RND-006). A grid that the palette mode
/// cannot draw is drawn in the resolved mode.
#[must_use]
pub fn raster(grid: &Grid, frame: Frame, scale: Scale, picture: Rgba) -> Raster {
    let mut options = frame.options;
    if options.colour == ColourMode::Palette && !palette_mode_allowed(grid) {
        options.colour = ColourMode::Resolved(FlashPhase::STEADY);
    }
    let page_width = row_width(grid);
    let height = ROWS * options.row_height();
    let sets = frame.glyphs.sets();
    // A comparison puts the pages side by side, separated by the picture.
    let width = sets.len() * page_width + (sets.len() - 1) * COMPARE_GAP;
    let picture_index = match options.colour {
        ColourMode::Palette => PALETTE_TRANSPARENT,
        ColourMode::Resolved(_) => COLOUR_TRANSPARENT,
    };
    let mut indices = vec![picture_index; width * height];
    for (index, glyphs) in sets.iter().enumerate() {
        let mut page = vec![0u8; page_width * height];
        let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut page, stride: page_width };
        // The buffer is sized for the whole page and the mode suits the
        // grid, so rendering cannot fail.
        let _ = render_screen(grid, ALL_ROWS, glyphs, options, &mut buffer);
        let left = index * (page_width + COMPARE_GAP);
        for (row, source) in page.chunks_exact(page_width).enumerate() {
            let start = row * width + left;
            if let Some(target) = indices.get_mut(start..start + page_width) {
                target.copy_from_slice(source);
            }
        }
    }
    let (scaled_width, scaled_height) = scaled_size(width, height, scale.x, scale.y, scale.aspect);
    let mut scaled = vec![0u8; scaled_width * scaled_height];
    let _ = scale_indices(&indices, width, height, scale.x.max(1), scale.y.max(1), scale.aspect, &mut scaled);
    let palette = match options.colour {
        ColourMode::Palette => {
            palette(&clut0_of(&grid.colour_map), picture, frame.flash_visible, options.reveal).to_vec()
        }
        ColourMode::Resolved(_) => colour_map_palette(&grid.colour_map, picture).to_vec(),
    };
    Raster { width: scaled_width, height: scaled_height, indices: scaled, palette }
}

/// Encodes a raster as PNG bytes.
pub fn png(raster: &Raster) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut scratch = vec![0u8; 2 * (raster.width + 1)];
    let width = raster.width;
    neoteletext_image::png::write(
        &mut out,
        raster.width,
        raster.height,
        &raster.palette,
        &mut |y, row: &mut [u8]| {
            if let Some(source) = raster.indices.get(y * width..(y + 1) * width) {
                row.copy_from_slice(source);
            }
        },
        &mut scratch,
    )
    .map_err(|error| match error {
        neoteletext_image::png::PngError::Sink(error) => error,
        other => std::io::Error::other(format!("{other:?}")),
    })?;
    Ok(out)
}

/// Encodes a raster as PPM bytes, transparent areas in `fill`.
pub fn ppm(raster: &Raster, fill: Rgba) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    neoteletext_image::ppm::write(&mut out, &raster.indices, raster.width, raster.height, &raster.palette, fill)?;
    Ok(out)
}

/// Standard base64 (RFC 4648 section 4), for the iTerm2 protocol.
#[must_use]
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let value = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (value >> (18 - 6 * index)) & 0x3F;
                out.push(char::from(ALPHABET[sextet as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The iTerm2 inline image escape sequence for a PNG (the "File=" form of
/// iTerm2's proprietary OSC 1337). Only this fixed sequence is emitted
/// around data we encoded ourselves (REQ-SEC-010).
#[must_use]
pub fn iterm2_inline(png: &[u8], width: usize, height: usize) -> String {
    format!(
        "\x1b]1337;File=inline=1;size={};width={}px;height={}px;preserveAspectRatio=0:{}\x07",
        png.len(),
        width,
        height,
        base64(png)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoteletext_core::colour::SidePanels;
    use neoteletext_core::config::PresentationLevel;

    /// REQ-RND-033, REQ-RND-030: a page with side panels is wider, and at
    /// Level 2.5 its palette is the page's 32 colours; at Level 1.5 the
    /// 144-entry palette of the palette mode.
    #[test]
    fn rasters_follow_the_level_and_the_panels() {
        let mut grid = Grid::EMPTY;
        grid.side_panels = SidePanels { left: 6, right: 10 };
        let scale = Scale { x: 2, y: 1, aspect: false };
        let enhanced = Config { level: PresentationLevel::Level25, ..Config::default() };
        let image = raster(&grid, Frame::still(&enhanced, false), scale, Rgba::TRANSPARENT);
        assert_eq!((image.width, image.height, image.palette.len()), (56 * 12 * 2, 500, 32));
        let level15 = Config { level: PresentationLevel::Level15, ..Config::default() };
        let image = raster(&Grid::EMPTY, Frame::still(&level15, false), scale, Rgba::TRANSPARENT);
        assert_eq!((image.width, image.palette.len()), (960, 144));
    }

    /// REQ-RND-040: at Levels 2.5 and 3.5 a frame changes with the flash
    /// unit; at Level 1.5 with the phase of decision E.12 only.
    #[cfg(feature = "view")]
    #[test]
    fn frames_follow_the_flash_timing() {
        use neoteletext_core::level1::{Cell, CellAttributes, FlashRate};
        use neoteletext_core::types::Instant;
        let enhanced = Config { level: PresentationLevel::Level35, ..Config::default() };
        let at = |config: &Config, us| Frame::at(config, false, Instant(us));
        assert_ne!(at(&enhanced, 0), at(&enhanced, 200_000), "units 0 and 2 differ");
        assert_eq!(at(&enhanced, 0), at(&enhanced, 100_000), "units 0 and 1 show alike");
        assert_eq!(at(&enhanced, 0), at(&enhanced, 50_000));
        let level15 = Config { level: PresentationLevel::Level15, ..Config::default() };
        assert_eq!(at(&level15, 0), at(&level15, 100_000));
        assert_ne!(at(&level15, 0), at(&level15, 1_000_000));
        // A fast-flashing cell is drawn in the background colour in its
        // off phase.
        let mut grid = Grid::EMPTY;
        grid.cells[1][0] = Cell {
            glyph: neoteletext_core::level1::Glyph::Mosaic { pattern: 0x3F, separated: false },
            foreground: 3,
            background: 4,
            flash: true,
            attributes: CellAttributes::DEFAULT.with_flash_rate(FlashRate::FastPhase1),
            ..Cell::BLANK
        };
        let one = Scale { x: 1, y: 1, aspect: false };
        let on = raster(&grid, at(&enhanced, 200_000), one, Rgba::TRANSPARENT);
        let off = raster(&grid, at(&enhanced, 0), one, Rgba::TRANSPARENT);
        assert_eq!((on.indices[20 * 480], off.indices[20 * 480]), (3, 4));
    }

    #[test]
    fn base64_vectors() {
        // RFC 4648 section 10.
        for (input, expected) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }

    #[test]
    fn inline_sequence_shape() {
        let sequence = iterm2_inline(b"xyz", 480, 500);
        assert!(sequence.starts_with("\x1b]1337;File=inline=1;size=3;width=480px;height=500px"));
        assert!(sequence.ends_with(":eHl6\x07"));
    }
}
