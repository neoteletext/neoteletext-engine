//! The attributes a cell is drawn with (Tables 27 and 29, Annex A.7.1;
//! decisions E.47 and E.48): flash, the attribute record and its changes.

use crate::charset::{G0Set, G2Set};
use crate::colour::ColourIndex;
use crate::consts::{COLOUR_BLACK, COLOUR_WHITE};
use crate::level1::{FlashMode, FlashRate};
use crate::packet::DisplayAttributes;

/// The rate part of a flash attribute, before incremental and decremental
/// flash are resolved into a phase for each cell (decision E.48).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(
    not(feature = "additional-flash-functions"),
    allow(dead_code, reason = "incremental and decremental flash are set only by the Additional Flash Functions")
)]
pub(super) enum Rate {
    /// One fixed rate and phase.
    Fixed(FlashRate),
    /// Phases 1, 2, 3, 1, ... rightwards from the column it was set at.
    Incremental,
    /// Phases 1, 3, 2, 1, ... rightwards from the column it was set at.
    Decremental,
}

/// The flash attribute type (Annex A.7.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Flash {
    /// No flash.
    Steady,
    /// Flashing.
    On {
        /// How.
        mode: FlashMode,
        /// The rate and phase.
        rate: Rate,
        /// The column it was set at, for incremental and decremental
        /// phases.
        start: u8,
    },
}

/// Whether the non-spacing attribute triplets (Table 27) take effect: the
/// Level 2.5 and 3.5 attributes that change the display without using a
/// character cell, unlike the Level 1 spacing attributes. Off by default
/// (feature `nonspacing-attributes`, the owner's choice); when off, each
/// sets only the column, as reserved data does.
pub(super) const ATTRIBUTES: bool = cfg!(feature = "nonspacing-attributes");

/// Level 1 Flash at Levels 2.5 and 3.5: slow, normal mode (decision E.48).
pub(super) const LEVEL_1_FLASH: Flash =
    Flash::On { mode: FlashMode::Normal, rate: Rate::Fixed(FlashRate::Slow), start: 0 };

/// The attributes a cell is drawn with (REQ-L25-006), each attribute type
/// of decision E.47 a field.
// Each flag is one attribute type of Annex A.7.1 and decision E.47.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Attrs {
    pub(super) foreground: ColourIndex,
    pub(super) background: ColourIndex,
    pub(super) flash: Flash,
    pub(super) conceal: bool,
    pub(super) boxed: bool,
    pub(super) double_height: bool,
    pub(super) double_width: bool,
    /// The non-spacing underline or separated mosaics attribute.
    pub(super) underline: bool,
    pub(super) invert: bool,
}

impl Attrs {
    /// The start of every Passive object: white (CLUT 0 entry 7), black
    /// (CLUT 0 entry 0), normal size, steady, boxing off, not concealed,
    /// contiguous mosaics (clause 13.5, REQ-OBJ-011).
    pub(super) const PASSIVE: Self = Self {
        foreground: COLOUR_WHITE,
        background: COLOUR_BLACK,
        flash: Flash::Steady,
        conceal: false,
        boxed: false,
        double_height: false,
        double_width: false,
        underline: false,
        invert: false,
    };

    /// Applies the attributes a change sets.
    pub(super) fn apply(&mut self, change: &AttrChange) {
        if let Some(foreground) = change.foreground {
            self.foreground = foreground;
        }
        if let Some(background) = change.background {
            self.background = background;
        }
        if let Some(flash) = change.flash {
            self.flash = flash;
        }
        if let Some(display) = change.display {
            self.double_height = display.double_height;
            self.double_width = display.double_width;
            self.boxed = display.boxing;
            self.conceal = display.conceal;
            self.invert = display.invert;
            self.underline = display.underline;
        }
    }
}

/// Non-spacing attributes placed at one cell, each attribute type the
/// latest placed there (Annex C.2 rule 3), and the Modified G0 and G2
/// designation of local enhancement data and Active objects.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) struct AttrChange {
    pub(super) foreground: Option<ColourIndex>,
    pub(super) background: Option<ColourIndex>,
    pub(super) flash: Option<Flash>,
    /// The six display attributes, which one triplet sets together; each
    /// is still its own type (decision E.47), and a 0 bit cancels.
    pub(super) display: Option<DisplayAttributes>,
    pub(super) designation: Option<(G0Set, G2Set)>,
}

impl AttrChange {
    /// This change followed by a later one at the same cell.
    pub(super) fn then(self, later: Self) -> Self {
        Self {
            foreground: later.foreground.or(self.foreground),
            background: later.background.or(self.background),
            flash: later.flash.or(self.flash),
            display: later.display.or(self.display),
            designation: later.designation.or(self.designation),
        }
    }
}

/// The flash an Additional Flash Functions triplet sets: without the
/// Additional Flash Functions compiled in, the triplet changes nothing and
/// the Level 1 flash is unchanged (decision E.105).
#[cfg(not(feature = "additional-flash-functions"))]
pub(super) const fn flash_of(_mode: u8, _rate: u8, _reserved: u8, _column: u8) -> Option<Flash> {
    None
}
#[cfg(feature = "additional-flash-functions")]
pub(super) use super::additional_flash::flash_of;

/// The phase of the cell in column `column` (decision E.48): successive
/// characters rightwards from the triplet's column take phases 1, 2, 3 for
/// incremental flash and 1, 3, 2 for decremental.
pub(super) fn flash_rate(rate: Rate, start: u8, column: usize) -> FlashRate {
    const INCREMENTAL: [FlashRate; 3] = [FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3];
    const DECREMENTAL: [FlashRate; 3] = [FlashRate::FastPhase1, FlashRate::FastPhase3, FlashRate::FastPhase2];
    let step = column.saturating_sub(usize::from(start)).checked_rem(INCREMENTAL.len()).unwrap_or(0);
    match rate {
        Rate::Fixed(rate) => rate,
        Rate::Incremental => INCREMENTAL.get(step).copied().unwrap_or(FlashRate::FastPhase1),
        Rate::Decremental => DECREMENTAL.get(step).copied().unwrap_or(FlashRate::FastPhase1),
    }
}
