//! How a grid is rendered: the colour mode (decision E.55) and the render
//! options.

use super::flash::FlashPhase;
use crate::config::{CellMode, Config};
use crate::consts::{CELL_HEIGHT, CELL_HEIGHT_SMALL};

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
    /// The underline of G0 and G2 characters is drawn (REQ-RND-035). A port
    /// that draws its own link underline over the page turns it off, so
    /// that an underline means one thing there; separated mosaics, which
    /// share the attribute bit, are unaffected (decision E.88).
    pub underline: bool,
}

impl RenderOptions {
    /// Options from the configuration, in the palette mode of Levels 1 and
    /// 1.5.
    #[must_use]
    pub const fn from_config(config: &Config, reveal: bool) -> Self {
        Self { mode: config.cell_mode, rounding: config.rounding, reveal, colour: ColourMode::Palette, underline: true }
    }

    /// Options from the configuration, in the resolved mode at a flash
    /// phase (REQ-RND-031).
    #[must_use]
    pub const fn resolved(config: &Config, reveal: bool, flash: FlashPhase) -> Self {
        Self {
            mode: config.cell_mode,
            rounding: config.rounding,
            reveal,
            colour: ColourMode::Resolved(flash),
            underline: true,
        }
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

    /// The same options with the underline of G0 and G2 characters drawn
    /// or not (decision E.88).
    #[must_use]
    pub const fn with_underline(self, underline: bool) -> Self {
        Self { underline, ..self }
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
