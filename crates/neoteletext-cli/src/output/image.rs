//! Raster output: palette-index framebuffers, PNG and PPM files
//! (REQ-RND-007), and the iTerm2 inline image protocol (REQ-RND-029).
//!
//! At Levels 1 and 1.5 a raster holds the indices of the palette mode and
//! the 144-entry palette of one flash phase (REQ-RND-023); at Levels 2.5
//! and 3.5 it holds Colour Map indices and the page's 32 colours
//! (REQ-RND-030, REQ-RND-031). Rows are as wide as the page's side panels
//! make them (REQ-RND-033).

use neoteletext_app::present;
pub use neoteletext_app::present::{Raster, Scale};
use neoteletext_core::config::Config;
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::Grid;
#[cfg(feature = "view")]
use neoteletext_core::render::ColourMode;
use neoteletext_core::render::{FlashPhase, RenderOptions};
use neoteletext_image::palette::Rgba;

/// Images are drawn in Bedstead, the one bundled font (REQ-FNT-001).
const BEDSTEAD: GlyphSet<'static> = GlyphSet::bedstead();

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
}

impl Frame {
    /// A still image: flashing characters in their "on" phase.
    #[must_use]
    pub const fn still(config: &Config, reveal: bool) -> Self {
        Self { options: RenderOptions::for_level(config, reveal, FlashPhase::STEADY), flash_visible: true }
    }

    /// The frame at a time on the decoder's clock. In the resolved mode
    /// only the flash phase changes it. Only the viewer animates.
    #[cfg(feature = "view")]
    #[must_use]
    pub fn at(config: &Config, reveal: bool, now: neoteletext_core::types::Instant) -> Self {
        let options = RenderOptions::for_level(config, reveal, FlashPhase::at(now));
        let flash_visible =
            options.colour != ColourMode::Palette || neoteletext_core::render::flash_visible(now, config);
        Self { options, flash_visible }
    }
}

/// Renders a grid to a scaled raster in Bedstead. `picture` is the colour
/// of the picture, transparent areas (REQ-RND-006).
#[must_use]
pub fn raster(grid: &Grid, frame: Frame, scale: Scale, picture: Rgba) -> Raster {
    raster_in(grid, frame, scale, picture, &BEDSTEAD)
}

/// Renders a grid to a scaled raster in the given glyphs (`--font`).
#[must_use]
pub fn raster_in(grid: &Grid, frame: Frame, scale: Scale, picture: Rgba, glyphs: &GlyphSet<'_>) -> Raster {
    present::raster(grid, frame.options, frame.flash_visible, glyphs, scale, picture)
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
    #[cfg(feature = "side-panels")]
    use neoteletext_core::colour::SidePanels;
    #[cfg(any(feature = "side-panels", feature = "view"))]
    use neoteletext_core::config::PresentationLevel;

    /// REQ-RND-033, REQ-RND-030: a page with side panels is wider, and at
    /// Level 2.5 its palette is the page's 32 colours; at Level 1.5 the
    /// 144-entry palette of the palette mode.
    #[cfg(feature = "side-panels")]
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
