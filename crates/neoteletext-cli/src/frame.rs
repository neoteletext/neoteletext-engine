//! The settings of one frame of a page: the colour mode and flash phase
//! (decision E.55, REQ-RND-040).

use neoteletext_core::config::Config;
#[cfg(feature = "view")]
use neoteletext_core::render::ColourMode;
use neoteletext_core::render::{FlashPhase, RenderOptions};

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

#[cfg(all(test, feature = "view"))]
mod tests {
    use super::*;
    use crate::image::{Scale, raster};
    use neoteletext_core::config::PresentationLevel;
    use neoteletext_core::level1::Grid;
    use neoteletext_image::palette::Rgba;

    /// REQ-RND-040: at Levels 2.5 and 3.5 a frame changes with the flash
    /// unit; at Level 1.5 with the phase of decision E.12 only.
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
}
