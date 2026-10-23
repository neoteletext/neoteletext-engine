//! The cell (decision E.42, REQ-L25-006): what a cell shows, its size,
//! width, flash, which source wrote it, and its packed attributes.

use crate::charset::CharRef;
use crate::colour::ColourIndex;
use crate::consts::{COLOUR_BLACK, COLOUR_WHITE, SPACE};

/// What a cell shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Glyph {
    /// Nothing in the foreground.
    Space,
    /// An alphanumeric character.
    Char(CharRef),
    /// A block mosaic: the 6-bit pattern and its form.
    Mosaic {
        /// Bit 0 top-left to bit 5 bottom-right (REQ-CHR-005).
        pattern: u8,
        /// Separated rather than contiguous.
        separated: bool,
    },
}

impl Glyph {
    /// True when the cell shows nothing in the foreground: no glyph, a
    /// space (2/0 is a space in every G0 set, Tables 35 and 36) without a
    /// diacritical mark, or a mosaic with no cells set.
    #[must_use]
    pub const fn is_blank(self) -> bool {
        match self {
            Self::Space => true,
            Self::Char(character) => character.code == SPACE && character.diacritic == 0,
            Self::Mosaic { pattern, .. } => pattern == 0,
        }
    }
}

/// A cell's role in double height (REQ-L1-008): the vertical part of its
/// size (decision E.42).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Size {
    /// Normal height.
    Normal,
    /// The top half of a double-height character.
    DoubleTop,
    /// The bottom half, shown in the row below.
    DoubleBottom,
}

/// The horizontal part of a cell's size (decision E.42, REQ-RND-034).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Width {
    /// Normal width.
    Normal,
    /// The left half of a double-width character.
    LeftHalf,
    /// The right half, shown in the next column.
    RightHalf,
}

/// How a flashing cell flashes (Table 29 mode 00111, decision E.48). At
/// Levels 1 and 1.5 every flash is [`FlashMode::Normal`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FlashMode {
    /// "Normal flash to background colour" (Table 29, D1 D0 = 01): the off
    /// phase shows the background colour.
    Normal,
    /// "Invert phase of flash to background colour" (10): as normal, with
    /// the on and off phases swapped.
    InvertPhase,
    /// "Flash to the corresponding colour in an adjacent CLUT" (11): the
    /// off phase shows the foreground index exclusive-ORed with 8, the
    /// same entry of CLUT 0 and 1, or 2 and 3.
    AdjacentClut,
}

/// The rate and phase of a flashing cell (decision E.48). Incremental and
/// decremental flash are resolved into a phase for each cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FlashRate {
    /// 1 hertz: on in units 0 to 8 of the twelve, off in 9 to 11. At Levels
    /// 1 and 1.5 the timing of decision E.12 is used instead.
    Slow,
    /// 2 hertz, phase 1: off in units 0 and 1 of each half-second.
    FastPhase1,
    /// 2 hertz, phase 2: off in units 2 and 3.
    FastPhase2,
    /// 2 hertz, phase 3: off in units 4 and 5.
    FastPhase3,
}

/// Which source wrote a cell's character (REQ-L25-006); the display
/// priority of clause 13.2 runs from Passive down to Level 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CharSource {
    /// The Level 1 page.
    Level1,
    /// The local enhancement data, packets X/26; at Level 1.5 every
    /// overwrite.
    Local,
    /// An Active object.
    Active,
    /// An Adaptive object.
    Adaptive,
    /// A Passive object.
    Passive,
}

/// The cell attributes that Levels 2.5 and 3.5 add (decision E.42,
/// REQ-L25-006): the horizontal part of the size, the flash mode and
/// rate, underline, invert, and the source of the character. They are packed into two bytes because a grid holds 1768
/// cells and ports with small memories hold grids; read and set them with
/// the methods.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct CellAttributes(u16);

/// Bit layout of [`CellAttributes`]: width bits 0 and 1, flash mode 2 and
/// 3, flash rate 4 and 5, underline 6, invert 7, source 8 to 10.
const WIDTH_SHIFT: u32 = 0;
const FLASH_MODE_SHIFT: u32 = 2;
const FLASH_RATE_SHIFT: u32 = 4;
const UNDERLINE_BIT: u16 = 1 << 6;
const INVERT_BIT: u16 = 1 << 7;
const SOURCE_SHIFT: u32 = 8;
const TWO_BITS: u16 = 0b11;
const THREE_BITS: u16 = 0b111;

impl CellAttributes {
    /// Normal width, steady normal slow flash settings, no underline or
    /// invert, written by the Level 1 page.
    pub const DEFAULT: Self = Self(0);

    const fn field(self, shift: u32, mask: u16) -> u16 {
        (self.0 >> shift) & mask
    }

    const fn with_field(self, shift: u32, mask: u16, value: u16) -> Self {
        Self((self.0 & !(mask << shift)) | (value & mask) << shift)
    }

    const fn with_bit(self, bit: u16, on: bool) -> Self {
        Self(if on { self.0 | bit } else { self.0 & !bit })
    }

    /// The horizontal part of the size.
    #[must_use]
    pub const fn width(self) -> Width {
        match self.field(WIDTH_SHIFT, TWO_BITS) {
            1 => Width::LeftHalf,
            2 => Width::RightHalf,
            _ => Width::Normal,
        }
    }

    /// With another horizontal part.
    #[must_use]
    pub const fn with_width(self, width: Width) -> Self {
        let bits = match width {
            Width::Normal => 0,
            Width::LeftHalf => 1,
            Width::RightHalf => 2,
        };
        self.with_field(WIDTH_SHIFT, TWO_BITS, bits)
    }

    /// How the cell flashes, when it does.
    #[must_use]
    pub const fn flash_mode(self) -> FlashMode {
        match self.field(FLASH_MODE_SHIFT, TWO_BITS) {
            1 => FlashMode::InvertPhase,
            2 => FlashMode::AdjacentClut,
            _ => FlashMode::Normal,
        }
    }

    /// With another flash mode.
    #[must_use]
    pub const fn with_flash_mode(self, mode: FlashMode) -> Self {
        let bits = match mode {
            FlashMode::Normal => 0,
            FlashMode::InvertPhase => 1,
            FlashMode::AdjacentClut => 2,
        };
        self.with_field(FLASH_MODE_SHIFT, TWO_BITS, bits)
    }

    /// The flash rate and phase, when the cell flashes.
    #[must_use]
    pub const fn flash_rate(self) -> FlashRate {
        match self.field(FLASH_RATE_SHIFT, TWO_BITS) {
            1 => FlashRate::FastPhase1,
            2 => FlashRate::FastPhase2,
            3 => FlashRate::FastPhase3,
            _ => FlashRate::Slow,
        }
    }

    /// With another flash rate and phase.
    #[must_use]
    pub const fn with_flash_rate(self, rate: FlashRate) -> Self {
        let bits = match rate {
            FlashRate::Slow => 0,
            FlashRate::FastPhase1 => 1,
            FlashRate::FastPhase2 => 2,
            FlashRate::FastPhase3 => 3,
        };
        self.with_field(FLASH_RATE_SHIFT, TWO_BITS, bits)
    }

    /// Underline (Table 29 mode 01100 D5); separated mosaics are in the
    /// glyph.
    #[must_use]
    pub const fn underline(self) -> bool {
        self.0 & UNDERLINE_BIT != 0
    }

    /// With underline on or off.
    #[must_use]
    pub const fn with_underline(self, on: bool) -> Self {
        self.with_bit(UNDERLINE_BIT, on)
    }

    /// Invert: foreground and background are swapped when drawn.
    #[must_use]
    pub const fn invert(self) -> bool {
        self.0 & INVERT_BIT != 0
    }

    /// With invert on or off.
    #[must_use]
    pub const fn with_invert(self, on: bool) -> Self {
        self.with_bit(INVERT_BIT, on)
    }

    /// Which source wrote the character.
    #[must_use]
    pub const fn source(self) -> CharSource {
        match self.field(SOURCE_SHIFT, THREE_BITS) {
            1 => CharSource::Local,
            2 => CharSource::Active,
            3 => CharSource::Adaptive,
            4 => CharSource::Passive,
            _ => CharSource::Level1,
        }
    }

    /// With another source.
    #[must_use]
    pub const fn with_source(self, source: CharSource) -> Self {
        let bits = match source {
            CharSource::Level1 => 0,
            CharSource::Local => 1,
            CharSource::Active => 2,
            CharSource::Adaptive => 3,
            CharSource::Passive => 4,
        };
        self.with_field(SOURCE_SHIFT, THREE_BITS, bits)
    }
}

/// One cell of the grid (REQ-L1-001, REQ-L25-006).
// The flags are the independent cell attributes REQ-L1-001 and REQ-L25-006
// list one by one.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Cell {
    /// What the cell shows. G3 characters are characters of the set
    /// [`CharSet::G3`](crate::charset::CharSet::G3).
    pub glyph: Glyph,
    /// Foreground colour: a Colour Map index 0 to 31; the Level 1 colours
    /// 0 to 7 are CLUT 0 (decision E.43).
    pub foreground: ColourIndex,
    /// Background colour: a Colour Map index 0 to 31.
    pub background: ColourIndex,
    /// The cell flashes; [`CellAttributes`] says how.
    pub flash: bool,
    /// The cell is concealed.
    pub conceal: bool,
    /// The cell lies inside a box, or a window (REQ-L25-029).
    pub boxed: bool,
    /// The cell's role in double height: the vertical part of its size.
    pub size: Size,
    /// The cell shows a held mosaic.
    pub held: bool,
    /// The source byte failed parity.
    pub parity_failed: bool,
    /// The attributes of Levels 2.5 and 3.5: width, flash mode and rate,
    /// underline, invert and the character's source.
    pub attributes: CellAttributes,
}

impl Cell {
    /// A blank cell: white on black, space.
    pub const BLANK: Self = Self {
        glyph: Glyph::Space,
        foreground: COLOUR_WHITE,
        background: COLOUR_BLACK,
        flash: false,
        conceal: false,
        boxed: false,
        size: Size::Normal,
        held: false,
        parity_failed: false,
        attributes: CellAttributes::DEFAULT,
    };
}
