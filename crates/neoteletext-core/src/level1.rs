//! Level 1.0 presentation (area L1, layer L3): from a stored page to a grid
//! of 25 by 40 cells, through the row attribute machine of REQ-FSM-006.
//!
//! [`build_grid`] is a pure function of the stored page, the magazine data,
//! the configuration and the display options (REQ-ARCH-006). Every feature
//! that crosses rows is resolved here, so rows are independent from here on
//! (decision E.29).
//!
//! The [`Cell`] and [`Grid`] also hold what Levels 2.5 and 3.5 add
//! (decision E.42, REQ-L25-006): Colour Map colours, size in two parts,
//! underline, invert, flash modes, font styles, which source wrote each
//! character, the side-panel area, the Colour Map, and the screen and row
//! colours. The Level 1 decoding leaves all of them at their defaults.

use crate::acq::{MagazineData, StoredPage};
use crate::charset::{
    self, CharRef, CharSet, Designation, G0, default_second_g0, designation_with_fallback, mosaic_pattern, second_g0,
};
use crate::colour::{ColourIndex, ColourMap, SidePanels};
use crate::config::{Config, ParityPolicy, Row24Policy};
use crate::consts::{
    ALPHA_BLACK, ALPHA_WHITE, ATTRIBUTE_LAST, BLACK_BACKGROUND, C5_NEWSFLASH, C6_SUBTITLE, C7_SUPPRESS_HEADER,
    C10_INHIBIT_DISPLAY, CODE_MASK_7BIT, COLOUR_ATTRIBUTE_MASK, COLOUR_BLACK, COLOUR_GREEN, COLOUR_WHITE, COLUMNS,
    CONCEAL, CONTIGUOUS_MOSAICS, DESIGNATION_GROUP_SHIFT, DOUBLE_HEIGHT, END_BOX, ESC, FLASH, HEADER_DISPLAY_BYTES,
    HEADER_DISPLAY_COLUMN, HOLD_MOSAICS, MOSAIC_BIT5, MOSAIC_BLACK, MOSAIC_WHITE, NATIONAL_INDEX_MASK, NEW_BACKGROUND,
    NORMAL_SIZE, PAGE_FUNCTION_BASIC, RECEIVER_COLUMNS, RELEASE_MOSAICS, ROWS, SEPARATED_MOSAICS,
    SIDE_PANEL_AREA_COLUMNS, SIDE_PANEL_FIRST_COLUMN, SIDE_PANEL_FIRST_ROW, SIDE_PANEL_ROWS, SPACE, START_BOX,
    STATUS_ATTRIBUTE_COLUMN, STATUS_COLUMNS, STEADY,
};

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
    /// The off phase shows the background colour.
    Normal,
    /// As normal, with the on and off phases swapped.
    Inverted,
    /// The off phase shows the foreground index exclusive-ORed with 8: the
    /// same entry of the neighbouring CLUT.
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

/// The Level 3.5 font styles of a cell (Table 29 mode 01110, decision
/// E.50). They apply to G0 and G2 characters only.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct FontStyle {
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
    /// Proportional spacing, recorded but drawn at the fixed pitch.
    pub proportional: bool,
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
/// rate, underline, invert, the font styles, and the source of the
/// character. They are packed into two bytes because a grid holds 1768
/// cells and ports with small memories hold grids; read and set them with
/// the methods.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct CellAttributes(u16);

/// Bit layout of [`CellAttributes`]: width bits 0 and 1, flash mode 2 and
/// 3, flash rate 4 and 5, underline 6, invert 7, bold 8, italic 9,
/// proportional 10, source 11 to 13.
const WIDTH_SHIFT: u32 = 0;
const FLASH_MODE_SHIFT: u32 = 2;
const FLASH_RATE_SHIFT: u32 = 4;
const UNDERLINE_BIT: u16 = 1 << 6;
const INVERT_BIT: u16 = 1 << 7;
const BOLD_BIT: u16 = 1 << 8;
const ITALIC_BIT: u16 = 1 << 9;
const PROPORTIONAL_BIT: u16 = 1 << 10;
const SOURCE_SHIFT: u32 = 11;
const TWO_BITS: u16 = 0b11;
const THREE_BITS: u16 = 0b111;

impl CellAttributes {
    /// Normal width, steady normal slow flash settings, no underline,
    /// invert or font style, written by the Level 1 page.
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
            1 => FlashMode::Inverted,
            2 => FlashMode::AdjacentClut,
            _ => FlashMode::Normal,
        }
    }

    /// With another flash mode.
    #[must_use]
    pub const fn with_flash_mode(self, mode: FlashMode) -> Self {
        let bits = match mode {
            FlashMode::Normal => 0,
            FlashMode::Inverted => 1,
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

    /// The Level 3.5 font styles.
    #[must_use]
    pub const fn font(self) -> FontStyle {
        FontStyle {
            bold: self.0 & BOLD_BIT != 0,
            italic: self.0 & ITALIC_BIT != 0,
            proportional: self.0 & PROPORTIONAL_BIT != 0,
        }
    }

    /// With other font styles.
    #[must_use]
    pub const fn with_font(self, font: FontStyle) -> Self {
        self.with_bit(BOLD_BIT, font.bold)
            .with_bit(ITALIC_BIT, font.italic)
            .with_bit(PROPORTIONAL_BIT, font.proportional)
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
    /// [`CharSet::G3`].
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
    /// underline, invert, font styles and the character's source.
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

/// The decoded page (REQ-L1-001): 25 rows by 40 columns, plus page-wide
/// display state. It is the core's primary output; every renderer works
/// from it (REQ-RND-001).
///
/// The caller sets one capacity (REQ-GEN-003): `PANEL_ROWS`, the rows of
/// the side-panel area, which is 24 for side panels and may be 0 for a
/// port that presents at Levels 1 and 1.5 only. [`Grid`] without a
/// parameter holds every panel row. Everything a row needs travels in the
/// grid, so a row is rendered from the grid alone (decision E.29).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grid<const PANEL_ROWS: usize = SIDE_PANEL_ROWS> {
    /// The cells, row by row.
    pub cells: [[Cell; COLUMNS]; ROWS],
    /// Bit r set when row r contains double height.
    pub double_height_rows: u32,
    /// Only boxed areas are visible: newsflash and subtitle pages
    /// (decision E.22) or a viewer request.
    pub boxed_mode: bool,
    /// Mix mode: backgrounds transparent everywhere (REQ-L1-020).
    pub mix: bool,
    /// The character sets the page uses, for Level 1.5 and form entry.
    pub designation: Designation,
    /// The side-panel area, columns 40 to 71 of rows 1 to 24 (clause
    /// 12.5.3, decision E.49): `panels[row − 1][column − 40]`. The right
    /// panel is columns 40 to 40 + right width − 1, the left panel columns
    /// 72 − left width to 71; [`Grid::panel_cell`] addresses it by screen
    /// position.
    pub panels: [[Cell; SIDE_PANEL_AREA_COLUMNS]; PANEL_ROWS],
    /// The widths of the enabled side panels; none at Levels 1 and 1.5.
    pub side_panels: SidePanels,
    /// The page's Colour Map (REQ-L25-030); Table 30 at Levels 1 and 1.5.
    pub colour_map: ColourMap,
    /// The screen colour, for the borders around the rows (decision E.45).
    pub screen_colour: ColourIndex,
    /// The full row colour of each row (decision E.45).
    pub row_colours: [ColourIndex; ROWS],
}

/// The character sets of an empty grid: Latin with the English national
/// option, until a page's designation replaces them.
const DEFAULT_DESIGNATION: Designation = Designation {
    g0: G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::English) },
    g2: charset::G2Set::Latin,
};

impl Grid {
    /// An empty grid with every panel row.
    pub const EMPTY: Self = Self::empty();
}

impl<const PANEL_ROWS: usize> Grid<PANEL_ROWS> {
    /// An empty grid: blank cells, no side panels, the Table 30 Colour
    /// Map, and black screen and rows. To empty an existing
    /// grid without a temporary copy, use [`Grid::clear`].
    // The grid is the value being built, so its arrays are as large as it
    // is; in a constant such as `Grid::EMPTY` it is built at compile time,
    // and ports with small stacks build grids in place with `clear`.
    #[allow(clippy::large_stack_arrays)]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            cells: [[Cell::BLANK; COLUMNS]; ROWS],
            double_height_rows: 0,
            boxed_mode: false,
            mix: false,
            designation: DEFAULT_DESIGNATION,
            panels: [[Cell::BLANK; SIDE_PANEL_AREA_COLUMNS]; PANEL_ROWS],
            side_panels: SidePanels::NONE,
            colour_map: ColourMap::DEFAULT,
            screen_colour: COLOUR_BLACK,
            row_colours: [COLOUR_BLACK; ROWS],
        }
    }

    /// Makes the grid empty in place, without a temporary copy of it.
    pub fn clear(&mut self) {
        for row in &mut self.cells {
            row.fill(Cell::BLANK);
        }
        for row in &mut self.panels {
            row.fill(Cell::BLANK);
        }
        self.double_height_rows = 0;
        self.boxed_mode = false;
        self.mix = false;
        self.designation = DEFAULT_DESIGNATION;
        self.side_panels = SidePanels::NONE;
        self.colour_map = ColourMap::DEFAULT;
        self.screen_colour = COLOUR_BLACK;
        self.row_colours = [COLOUR_BLACK; ROWS];
    }

    /// The cell at a position.
    #[must_use]
    pub fn cell(&self, row: usize, column: usize) -> Option<&Cell> {
        self.cells.get(row).and_then(|cells| cells.get(column))
    }

    /// The cell at a position, mutably.
    pub fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cell> {
        self.cells.get_mut(row).and_then(|cells| cells.get_mut(column))
    }

    /// The side-panel cell at a screen position: row 1 to 24, column 40 to
    /// 71. `None` elsewhere, including row 0, which has no panel content.
    #[must_use]
    pub fn panel_cell(&self, row: usize, column: usize) -> Option<&Cell> {
        let row = row.checked_sub(SIDE_PANEL_FIRST_ROW)?;
        self.panels.get(row)?.get(column.checked_sub(SIDE_PANEL_FIRST_COLUMN)?)
    }

    /// The side-panel cell at a screen position, mutably.
    pub fn panel_cell_mut(&mut self, row: usize, column: usize) -> Option<&mut Cell> {
        let row = row.checked_sub(SIDE_PANEL_FIRST_ROW)?;
        self.panels.get_mut(row)?.get_mut(column.checked_sub(SIDE_PANEL_FIRST_COLUMN)?)
    }

    /// True when row `row` contains double height.
    #[must_use]
    pub fn row_has_double_height(&self, row: usize) -> bool {
        self.double_height_rows.checked_shr(row as u32).is_some_and(|bits| bits & 1 == 1)
    }
}

/// Page zoom (REQ-L1-020): which half of the page is enlarged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zoom {
    /// No zoom.
    Off,
    /// Rows 0 to 11 enlarged to double height.
    Top,
    /// Rows 12 to 23 enlarged to double height.
    Bottom,
}

/// What the receiver contributes to the grid (decisions E.10, E.11, E.23).
// Each flag is a separate viewer setting (REQ-L1-020, REQ-SEC-005).
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DisplayOptions {
    /// Row 0 columns 0 to 6: the receiver status area, 7-bit codes.
    pub status: [u8; STATUS_COLUMNS],
    /// The requested page has not been found yet: column 7 is green and the
    /// rolling header is shown.
    pub searching: bool,
    /// While searching, row 0 columns 8 to 39 from incoming headers, raw.
    pub rolling_header: Option<[u8; HEADER_DISPLAY_BYTES]>,
    /// A row 24 drawn by the receiver (REQ-NAV-011), 7-bit codes.
    pub row24: Option<[u8; COLUMNS]>,
    /// A security indicator is active: the status area is shown even on
    /// pages with C7 set (REQ-SEC-005).
    pub security_indicator: bool,
    /// Mix mode (REQ-L1-020).
    pub mix: bool,
    /// Boxed mode requested by the viewer (REQ-L1-020).
    pub boxed: bool,
}

impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            status: [SPACE; STATUS_COLUMNS],
            searching: false,
            rolling_header: None,
            row24: None,
            security_indicator: false,
            mix: false,
            boxed: false,
        }
    }
}

/// Where a page's designation group came from (REQ-CHR-003): the
/// highest-priority packet present, in the order of Annex C.4 rule 1, or
/// the receiver's setting when the page sends none.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DesignationSource {
    /// The page's packet X/28/0 Format 1.
    PageX28Format1,
    /// The page's packet X/28/4 (Level 3.5).
    PageX28Format4,
    /// The magazine's packet M/29/0.
    MagazineM29Format0,
    /// The magazine's packet M/29/4 (Level 3.5).
    MagazineM29Format4,
    /// None was sent: the receiver's configured region (decision E.4).
    Receiver,
}

/// Where the three national option bits came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NationalSource {
    /// C12 to C14 of the page header.
    Header,
    /// Bits 8 to 10 of the designation, at Levels 2.5 and 3.5 (Annex E.9).
    Designation,
    /// The receiver's fixed setting (`Config::fixed_national_index`).
    Fixed,
}

/// How a page's character sets were chosen, as the decoder resolved them
/// (REQ-CHR-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DesignationReport {
    /// C12 to C14 of the page header, as the national option index.
    pub header_national: u8,
    /// The four-bit group, and where it came from.
    pub group: u8,
    /// Where the group came from.
    pub source: DesignationSource,
    /// The national option index used, and where it came from.
    pub national: u8,
    /// Where the national option came from.
    pub national_source: NationalSource,
    /// The seven-bit Table 32 value looked up.
    pub value: u8,
    /// The entry used: the value's, or the fallback's.
    pub designation: Designation,
    /// The value's entry is reserved and REQ-CHR-003's fallback was used.
    pub fallback: bool,
    /// The second G0 set and national option of Table 33, if any.
    pub second: Option<G0>,
    /// Display rows 1 to 24 holding an ESC code, which toggles to the
    /// second set.
    pub esc_rows: u8,
}

/// How the page's character sets are chosen (decision E.4, REQ-CHR-003):
/// the group from the highest-priority designation, or the
/// configured default.
///
/// The designations are taken in the order of Annex C.4 rule 1: X/28/0
/// format 1, X/28/4 (Level 3.5 only), M/29/0, M/29/4 (Level 3.5 only)
/// (decision E.44); a source whose designation group is damaged is absent.
/// The national option comes from C12 to C14, except at Levels 2.5 and
/// 3.5, where bits 8 to 10 of the chosen designation give it when there is
/// one (Annex E.9, decisions E.4 and E.50; REQ-L25-012).
#[must_use]
pub fn designation_report(page: Option<&StoredPage>, magazine: &MagazineData, config: &Config) -> DesignationReport {
    let level_35 = config.level.is_level35();
    // A page's own X/28/0 or X/28/4 counts only when it is a basic Level 1
    // page; then the magazine's packets apply (decision E.4).
    let page_designations = page
        .and_then(|page| page.page_data)
        .filter(|data| data.format.is_some_and(|format| format.function == PAGE_FUNCTION_BASIC))
        .and_then(|data| data.fields.designations);
    let page_35 = page
        .and_then(|page| page.page_data_level35)
        .filter(|data| level_35 && data.format.is_some_and(|format| format.function == PAGE_FUNCTION_BASIC))
        .and_then(|data| data.fields.designations);
    let magazine_29 = magazine.data.and_then(|fields| fields.designations);
    let magazine_35 = magazine.level35.filter(|_| level_35).and_then(|fields| fields.designations);
    let (designations, source) = match (page_designations, page_35, magazine_29, magazine_35) {
        (Some(found), ..) => (Some(found), DesignationSource::PageX28Format1),
        (None, Some(found), ..) => (Some(found), DesignationSource::PageX28Format4),
        (None, None, Some(found), _) => (Some(found), DesignationSource::MagazineM29Format0),
        (None, None, None, Some(found)) => (Some(found), DesignationSource::MagazineM29Format4),
        (None, None, None, None) => (None, DesignationSource::Receiver),
    };
    let header_national = page.map_or(0, |page| page.control.national_index()) & NATIONAL_INDEX_MASK;
    let (national, national_source) = match (config.fixed_national_index, designations) {
        (Some(fixed), _) => (fixed, NationalSource::Fixed),
        (None, Some(designations)) if config.level.is_enhanced() => (designations.default, NationalSource::Designation),
        _ => (header_national, NationalSource::Header),
    };
    let national = national & NATIONAL_INDEX_MASK;
    let group =
        designations.map_or(config.default_region, |designations| designations.default >> DESIGNATION_GROUP_SHIFT);
    let value = (group << DESIGNATION_GROUP_SHIFT) | national;
    let (designation, fallback) = designation_with_fallback(value);
    // A received Table 33 value is used as it is; without one, the
    // designation's own default applies (REQ-CHR-012).
    let second = match designations {
        Some(designations) => second_g0(designations.second),
        None if fallback => None,
        None => default_second_g0(value),
    };
    let esc_rows = page.map_or(0, |page| {
        let rows = page.cells.iter().take(ROWS).skip(1);
        let with_esc = rows.filter(|row| row.iter().any(|&byte| byte & CODE_MASK_7BIT == ESC)).count();
        u8::try_from(with_esc).unwrap_or(u8::MAX)
    });
    DesignationReport {
        header_national,
        group,
        source,
        national,
        national_source,
        value,
        designation,
        fallback,
        second,
        esc_rows,
    }
}

/// Resolves the page's character sets (decision E.4, REQ-CHR-003): the
/// designation, the second G0 set, and whether a reserved entry needed the
/// fallback, from [`designation_report`].
#[must_use]
pub fn resolve_designation(
    page: Option<&StoredPage>,
    magazine: &MagazineData,
    config: &Config,
) -> (Designation, Option<G0>, bool) {
    let report = designation_report(page, magazine, config);
    (report.designation, report.second, report.fallback)
}

/// The state record of the row attribute machine (REQ-FSM-006).
// The fields are exactly the record REQ-FSM-006 lists.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RowState {
    foreground: u8,
    background: u8,
    mosaic: bool,
    flash: bool,
    conceal: bool,
    boxed: bool,
    double: bool,
    separated: bool,
    hold: bool,
    held: Option<(u8, bool)>,
    second_g0: bool,
}

impl RowState {
    /// The default state at the start of every row (REQ-L1-002).
    pub const DEFAULT: Self = Self {
        foreground: COLOUR_WHITE,
        background: COLOUR_BLACK,
        mosaic: false,
        flash: false,
        conceal: false,
        boxed: false,
        double: false,
        separated: false,
        hold: false,
        held: None,
        second_g0: false,
    };
}

/// The inputs of one row's decoding.
struct RowContext<'a> {
    config: &'a Config,
    default_g0: G0,
    second_g0: Option<G0>,
    /// Double height is honoured only in rows 1 to 22 (decision E.15).
    double_allowed: bool,
}

/// Decodes one row of 7-bit codes (REQ-FSM-006). `failed` marks columns
/// whose byte failed parity. Returns the cells and whether the row contains
/// double height.
fn decode_row(codes: &[u8; COLUMNS], failed: u64, context: &RowContext<'_>) -> ([Cell; COLUMNS], bool) {
    let mut cells = [Cell::BLANK; COLUMNS];
    let mut state = RowState::DEFAULT;
    let mut has_double = false;
    for (column, out) in cells.iter_mut().enumerate() {
        let code = codes.get(column).copied().unwrap_or(SPACE) & CODE_MASK_7BIT;
        let next = codes.get(column.saturating_add(1)).copied().unwrap_or(SPACE) & CODE_MASK_7BIT;
        // Step 1: set-at attributes.
        set_at(&mut state, code);
        // Step 2: what the cell shows.
        let (glyph, held) = cell_glyph(&state, code, context);
        // Step 3: record the cell.
        *out = Cell {
            glyph,
            foreground: state.foreground,
            background: state.background,
            flash: state.flash,
            conceal: state.conceal,
            boxed: state.boxed,
            size: if state.double { Size::DoubleTop } else { Size::Normal },
            held,
            parity_failed: failed.checked_shr(column as u32).is_some_and(|bits| bits & 1 == 1),
            ..Cell::BLANK
        };
        // Step 4: the held mosaic (decision E.16, REQ-L1-007): only codes
        // with bit 6 set qualify, so blast-through capitals are never held.
        if state.mosaic && code > ATTRIBUTE_LAST && code & MOSAIC_BIT5 != 0 {
            state.held = Some((mosaic_pattern(code), state.separated));
        }
        // Step 5: set-after attributes.
        if set_after(&mut state, code, next, context) {
            has_double = true;
        }
    }
    (cells, has_double)
}

/// Step 1 of REQ-FSM-006: the Table 26 set-at attributes, which act in
/// their own cell (REQ-L1-003). Steady ends flash (REQ-L1-009), Conceal
/// starts concealment (REQ-L1-010), and the background codes act at once so
/// the attribute cell itself is painted (REQ-L1-012).
fn set_at(state: &mut RowState, code: u8) {
    match code {
        STEADY => state.flash = false,
        NORMAL_SIZE => {
            // Decision E.16: a set-at size change resets the held mosaic
            // before its own cell is shown.
            if state.double {
                state.held = None;
            }
            state.double = false;
        }
        CONCEAL => state.conceal = true,
        CONTIGUOUS_MOSAICS => state.separated = false,
        SEPARATED_MOSAICS => state.separated = true,
        BLACK_BACKGROUND => state.background = COLOUR_BLACK,
        NEW_BACKGROUND => state.background = state.foreground,
        HOLD_MOSAICS => state.hold = true,
        _ => {}
    }
}

/// Step 2 of REQ-FSM-006: the glyph and whether it is a held mosaic.
fn cell_glyph(state: &RowState, code: u8, context: &RowContext<'_>) -> (Glyph, bool) {
    if code <= ATTRIBUTE_LAST {
        // REQ-L1-004: attribute cells show a space, or the held mosaic.
        if state.hold
            && state.mosaic
            && let Some((pattern, separated)) = state.held
        {
            return (Glyph::Mosaic { pattern, separated }, true);
        }
        return (Glyph::Space, false);
    }
    let g0 = if state.second_g0 { context.second_g0.unwrap_or(context.default_g0) } else { context.default_g0 };
    // REQ-L1-006: the G1 pattern keeps the form in force at this cell.
    if state.mosaic && code & MOSAIC_BIT5 != 0 {
        return (Glyph::Mosaic { pattern: mosaic_pattern(code), separated: state.separated }, false);
    }
    // Alphanumerics, and blast-through capitals in mosaic mode, with the
    // national option applied (Table 47 note 2).
    (Glyph::Char(CharRef::new(CharSet::G0(g0), code)), false)
}

/// Step 5 of REQ-FSM-006: the Table 26 set-after attributes, which act
/// from the next cell (REQ-L1-003). Returns true when the row gains double
/// height.
fn set_after(state: &mut RowState, code: u8, next: u8, context: &RowContext<'_>) -> bool {
    // REQ-L1-019, decision E.18: 0/0 and 1/0 act only under the SAA5246A
    // profile; 0/E and 0/F fall through to the no-op arm in every profile.
    let black_codes = context.config.black_colour_codes;
    match code {
        ALPHA_BLACK..=ALPHA_WHITE if code != ALPHA_BLACK || black_codes => {
            if state.mosaic {
                state.held = None;
            }
            state.mosaic = false;
            state.foreground = code & COLOUR_ATTRIBUTE_MASK;
            // REQ-L1-010: a colour attribute ends concealment.
            state.conceal = false;
        }
        MOSAIC_BLACK..=MOSAIC_WHITE if code != MOSAIC_BLACK || black_codes => {
            if !state.mosaic {
                state.held = None;
            }
            state.mosaic = true;
            state.foreground = code & COLOUR_ATTRIBUTE_MASK;
            state.conceal = false;
        }
        // REQ-L1-009: flashing until Steady or the end of the row.
        FLASH => state.flash = true,
        // REQ-L1-011: box membership is set-after at both ends.
        END_BOX => state.boxed = false,
        START_BOX => {
            // Decision E.17: a box starts only at the second of two
            // consecutive Start Box codes.
            if !context.config.double_start_box || next == START_BOX {
                state.boxed = true;
            }
        }
        DOUBLE_HEIGHT if context.double_allowed => {
            if !state.double {
                state.held = None;
            }
            state.double = true;
            return true;
        }
        ESC => state.second_g0 = !state.second_g0,
        RELEASE_MOSAICS => state.hold = false,
        _ => {}
    }
    false
}

/// The 7-bit codes and parity-failure mask of a stored row under the parity
/// policy (decision E.8, REQ-L1-017, REQ-L1-018).
fn stored_row(page: &StoredPage, row: usize, policy: ParityPolicy) -> ([u8; COLUMNS], u64) {
    let mut codes = [SPACE; COLUMNS];
    let mut failed = 0u64;
    if !page.has_row(row as u8) {
        return (codes, 0);
    }
    for (column, code) in codes.iter_mut().enumerate() {
        let (byte, passed) = page.byte(row, column, policy);
        if passed {
            *code = byte & CODE_MASK_7BIT;
        } else {
            failed |= 1u64.checked_shl(column as u32).unwrap_or(0);
            *code = match policy {
                // The Table 35 solid block at 7/F marks the error visibly.
                ParityPolicy::ErrorMarker => CODE_MASK_7BIT,
                ParityPolicy::KeepLastGood | ParityPolicy::AlwaysSpace => SPACE,
            };
        }
    }
    (codes, failed)
}

/// Builds the grid for a page (REQ-ARCH-006). With no page, only row 0's
/// status area and rolling header are shown.
pub fn build_grid<const PANEL_ROWS: usize>(
    out: &mut Grid<PANEL_ROWS>,
    page: Option<&StoredPage>,
    magazine: &MagazineData,
    config: &Config,
    options: &DisplayOptions,
) {
    out.clear();
    let (designation, second, _) = resolve_designation(page, magazine, config);
    out.designation = designation;
    out.mix = options.mix;
    let control = page.map_or(0, |page| page.control.0);
    // REQ-L1-011, REQ-ACQ-015, decision E.22: box membership matters only
    // on newsflash and subtitle pages, or when the viewer asks for it.
    out.boxed_mode = options.boxed || control & (C5_NEWSFLASH | C6_SUBTITLE) != 0;
    let policy = config.parity_policy;
    let suppress_header = control & C7_SUPPRESS_HEADER != 0;
    // REQ-L1-016 inhibit display; REQ-ACQ-014 non-text pages show no rows.
    let inhibit = control & C10_INHIBIT_DISPLAY != 0 || page.is_some_and(|page| !page.is_text_page());

    let mut row = 0usize;
    while row < ROWS {
        let (codes, failed) = row_codes(row, page, config, options, policy);
        let context = RowContext {
            config,
            default_g0: designation.g0,
            second_g0: second,
            // Decision E.15: rows 0, 23 and 24 are always normal height.
            double_allowed: (1..=22).contains(&row),
        };
        let (mut cells, has_double) = decode_row(&codes, failed, &context);
        if row == 0 {
            draw_receiver_columns_in_receiver_set(&mut cells);
        }
        let visible = if row == 0 { true } else { !inhibit };
        if let Some(target) = out.cells.get_mut(row) {
            *target = if visible { cells } else { [Cell::BLANK; COLUMNS] };
        }
        if row == 0 && suppress_header {
            hide_header(out, config.status_on_suppressed_header || options.security_indicator);
        }
        if visible && has_double {
            out.double_height_rows |= 1u32.checked_shl(row as u32).unwrap_or(0);
            // REQ-L1-008: the next row's transmitted content is ignored.
            fill_lower_row(out, row);
            row = row.saturating_add(2);
        } else {
            row = row.saturating_add(1);
        }
    }
}

/// The set the receiver writes its own text in: Latin with the English
/// option, whose letters and digits are ASCII's.
const RECEIVER_SET: CharSet =
    CharSet::G0(G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::English) });

/// Row 0 columns 0 to 7 are the receiver's (REQ-SEC-005, decision E.10):
/// its status text is drawn in its own set, never in the set the page
/// designates, so that a Cyrillic, Arabic-script or Thai page cannot turn
/// "P100" into other letters, join it or draw it from a font without it.
fn draw_receiver_columns_in_receiver_set(cells: &mut [Cell; COLUMNS]) {
    for cell in cells.iter_mut().take(RECEIVER_COLUMNS) {
        if let Glyph::Char(character) = &mut cell.glyph {
            *character = CharRef::new(RECEIVER_SET, character.code);
        }
    }
}

/// The 7-bit codes of display row `row` as the receiver presents them, and
/// the mask of columns whose byte failed parity: the status area and header
/// of row 0, the stored rows under the parity policy, and row 24 as
/// [`row_24_shown`] and the receiver's own row 24 decide. The enhancement
/// processor of Levels 2.5 and 3.5 resolves rows from these same codes
/// (REQ-FSM-012).
#[must_use]
pub fn row_codes(
    row: usize,
    page: Option<&StoredPage>,
    config: &Config,
    options: &DisplayOptions,
    policy: ParityPolicy,
) -> ([u8; COLUMNS], u64) {
    if row == 0 {
        let mut codes = [SPACE; COLUMNS];
        let mut failed = 0u64;
        for (slot, &status) in codes.iter_mut().zip(options.status.iter()) {
            *slot = status & CODE_MASK_7BIT;
        }
        // Decision E.10: column 7 is white when found, green when searching.
        if let Some(slot) = codes.get_mut(STATUS_ATTRIBUTE_COLUMN) {
            *slot = if options.searching { COLOUR_GREEN } else { COLOUR_WHITE };
        }
        match (options.searching, options.rolling_header, page) {
            (true, Some(rolling), _) => {
                for (slot, &byte) in codes.iter_mut().skip(HEADER_DISPLAY_COLUMN).zip(rolling.iter()) {
                    *slot = if crate::ecc::has_odd_parity(byte) { byte & CODE_MASK_7BIT } else { SPACE };
                }
            }
            (_, _, Some(page)) => {
                let (stored, stored_failed) = stored_row(page, 0, policy);
                for column in HEADER_DISPLAY_COLUMN..COLUMNS {
                    if let (Some(slot), Some(&code)) = (codes.get_mut(column), stored.get(column)) {
                        *slot = code;
                    }
                }
                // Columns 0 to 7 are the receiver's; only header failures
                // from column 8 on are reported.
                failed = stored_failed & !0xFF;
            }
            _ => {}
        }
        return (codes, failed);
    }
    if row == ROWS.saturating_sub(1) {
        if let Some(drawn) = options.row24 {
            let mut codes = [SPACE; COLUMNS];
            for (slot, &code) in codes.iter_mut().zip(drawn.iter()) {
                *slot = code & CODE_MASK_7BIT;
            }
            return (codes, 0);
        }
        if !page.is_some_and(|page| row_24_shown(page, config)) {
            // REQ-L1-015: left blank for receiver status messages.
            return ([SPACE; COLUMNS], 0);
        }
    }
    match page {
        Some(page) => stored_row(page, row, policy),
        None => ([SPACE; COLUMNS], 0),
    }
}

/// True when the page's own row 24 is displayed under the configured rule
/// (decision E.23).
#[must_use]
pub fn row_24_shown(page: &StoredPage, config: &Config) -> bool {
    match config.row24 {
        Row24Policy::AnnexH => page.links.is_some_and(|links| links.display_row_24),
        Row24Policy::WhenPresent => page.has_row(24),
        Row24Policy::Never => false,
    }
}

/// Blanks row 0 for a page with C7 set, keeping the status area when asked
/// (REQ-L1-014).
fn hide_header<const PANEL_ROWS: usize>(grid: &mut Grid<PANEL_ROWS>, keep_status: bool) {
    if let Some(cells) = grid.cells.get_mut(0) {
        for (column, cell) in cells.iter_mut().enumerate() {
            if !(keep_status && column <= STATUS_ATTRIBUTE_COLUMN) {
                *cell = Cell::BLANK;
            }
        }
    }
}

/// Fills the row below a double-height row (decision E.15).
fn fill_lower_row<const PANEL_ROWS: usize>(grid: &mut Grid<PANEL_ROWS>, row: usize) {
    let Some(upper) = grid.cells.get(row).copied() else { return };
    let Some(lower) = grid.cells.get_mut(row.saturating_add(1)) else { return };
    for (below, above) in lower.iter_mut().zip(upper.iter()) {
        *below = if above.size == Size::DoubleTop {
            Cell { size: Size::DoubleBottom, ..*above }
        } else {
            // Background colour without foreground.
            Cell { glyph: Glyph::Space, held: false, ..*above }
        };
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::consts::{ALPHA_RED, ALPHA_YELLOW, DOUBLE_SIZE, DOUBLE_WIDTH, MOSAIC_FULL, MOSAIC_RED};

    fn context(config: &Config) -> RowContext<'_> {
        RowContext {
            config,
            default_g0: G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::English) },
            second_g0: None,
            double_allowed: true,
        }
    }

    fn row(bytes: &[u8]) -> [u8; COLUMNS] {
        let mut codes = [SPACE; COLUMNS];
        codes[..bytes.len()].copy_from_slice(bytes);
        codes
    }

    fn decode(bytes: &[u8]) -> [Cell; COLUMNS] {
        let config = Config::default();
        decode_row(&row(bytes), 0, &context(&config)).0
    }

    /// REQ-L1-003, REQ-TST-013.
    #[test]
    fn stored_spaces_are_blank_glyphs() {
        let cells = decode(&[b' ', b'A', MOSAIC_RED, 0x20, 0x21]);
        assert!(matches!(cells[0].glyph, Glyph::Char(_)) && cells[0].glyph.is_blank());
        assert!(!cells[1].glyph.is_blank());
        assert!(cells[2].glyph.is_blank() && cells[3].glyph.is_blank());
        assert!(!cells[4].glyph.is_blank());
    }

    #[test]
    fn colour_attributes_are_set_after() {
        let cells = decode(&[ALPHA_RED, b'A']);
        assert_eq!(cells[0].foreground, COLOUR_WHITE, "attribute cell keeps the old colour");
        assert_eq!(cells[1].foreground, 1);
        assert_eq!(cells[0].glyph, Glyph::Space);
    }

    /// REQ-L1-012; REQ-TST-013's New Background after a colour change.
    #[test]
    fn new_background_is_set_at() {
        let cells = decode(&[ALPHA_RED, NEW_BACKGROUND, b'A', BLACK_BACKGROUND, b'B']);
        assert_eq!(cells[1].background, 1, "New Background acts in its own cell");
        assert_eq!(cells[2].background, 1);
        assert_eq!(cells[3].background, 0);
    }

    /// REQ-L1-007.
    #[test]
    fn hold_mosaics_annex_g33_style() {
        // Mosaic red, a mosaic, hold, a colour change: the attribute cell
        // after the hold shows the held mosaic.
        let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, MOSAIC_RED + 1, 0x21]);
        assert_eq!(cells[1].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false });
        assert!(cells[2].held && cells[3].held);
        assert_eq!(cells[3].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false });
    }

    /// REQ-L1-007; REQ-TST-013's hold with mode changes.
    #[test]
    fn held_mosaic_keeps_its_form_and_resets_on_mode_change() {
        let cells =
            decode(&[MOSAIC_RED, SEPARATED_MOSAICS, 0x7F, HOLD_MOSAICS, CONTIGUOUS_MOSAICS, ALPHA_RED, MOSAIC_RED]);
        // Contiguous (set-at) in column 4: the held mosaic keeps its stored
        // separated form (decision E.16).
        assert_eq!(cells[4].glyph, Glyph::Mosaic { pattern: 0x3F, separated: true });
        // Column 5 switches to alphanumerics (set-after); its own cell still
        // shows the held mosaic, and the reset follows.
        assert!(cells[5].held);
        assert_eq!(cells[6].glyph, Glyph::Space);
    }

    /// REQ-TST-013's blast-through with national options.
    #[test]
    fn blast_through_capitals_in_mosaic_mode() {
        let cells = decode(&[MOSAIC_RED, b'A', 0x23]);
        assert!(matches!(cells[1].glyph, Glyph::Char(c) if c.code == b'A'));
        assert!(matches!(cells[2].glyph, Glyph::Mosaic { .. }));
        // Table 47 note 2: 5/B blasts through as the German national
        // character, not as a mosaic.
        let config = Config::default();
        let german = RowContext {
            default_g0: G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::German) },
            ..context(&config)
        };
        let cells = decode_row(&row(&[MOSAIC_RED, 0x5B]), 0, &german).0;
        let Glyph::Char(character) = cells[1].glyph else { panic!("blast-through gave {:?}", cells[1].glyph) };
        assert_eq!(charset::to_unicode(character).base, '\u{C4}');
    }

    /// REQ-L1-010; REQ-TST-013's conceal ended by a colour attribute.
    #[test]
    fn conceal_ends_at_colour() {
        let cells = decode(&[CONCEAL, b'A', ALPHA_RED, b'B']);
        assert!(cells[0].conceal && cells[1].conceal && cells[2].conceal);
        assert!(!cells[3].conceal);
    }

    /// REQ-L1-011, REQ-TST-013: the Annex G.3.1 boxing example.
    #[test]
    fn start_box_needs_two_codes() {
        // Annex G.3.1: x 0B 0B 61 0A 0A — the second 0B, the "a" and the
        // first 0A are boxed.
        let cells = decode(&[b'x', START_BOX, START_BOX, b'a', END_BOX, END_BOX]);
        let boxed: [bool; 6] = core::array::from_fn(|i| cells[i].boxed);
        assert_eq!(boxed, [false, false, true, true, true, false]);
        let single = decode(&[b'x', START_BOX, b'a']);
        assert!(!single[2].boxed);
    }

    /// REQ-L1-009.
    #[test]
    fn flash_and_steady() {
        let cells = decode(&[FLASH, b'A', STEADY, b'B']);
        assert!(!cells[0].flash && cells[1].flash && !cells[2].flash);
    }

    /// REQ-L1-019, decision E.18.
    #[test]
    fn black_codes_follow_the_profile() {
        // 0/0: alpha black by default and under SAA5246A; no function in the
        // SAA5050 profile.
        let saa5050 = Config::saa5050();
        let cells = decode(&[ALPHA_RED, ALPHA_BLACK, b'A']);
        assert_eq!(cells[2].foreground, COLOUR_BLACK, "0/0 is alpha black by default");
        let cells = decode_row(&row(&[ALPHA_RED, ALPHA_BLACK, b'A']), 0, &context(&saa5050)).0;
        assert_eq!(cells[2].foreground, 1, "0/0 has no function on the SAA5050");
        // The RAI case: alpha black ends mosaic mode, so digits are text.
        let cells = decode(&[MOSAIC_RED, ALPHA_BLACK, b'2']);
        assert!(matches!(cells[2].glyph, Glyph::Char(c) if c.code == b'2'));
        // 1/0: mosaic black by default; no function on the SAA5050.
        let cells = decode(&[ALPHA_RED, MOSAIC_BLACK, 0x7F]);
        assert_eq!(cells[2].foreground, COLOUR_BLACK);
        assert_eq!(cells[2].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false });
        let cells = decode_row(&row(&[MOSAIC_RED, MOSAIC_BLACK, 0x7F]), 0, &context(&saa5050)).0;
        assert_eq!(cells[2].foreground, MOSAIC_RED & COLOUR_ATTRIBUTE_MASK);
        // 0/E and 0/F show a space and change no state in either profile:
        // the row matches one where a repeated colour code fills the cell.
        for config in [Config::default(), Config::saa5246a()] {
            let plain = decode_row(&row(&[ALPHA_RED, ALPHA_RED, b'A']), 0, &context(&config)).0;
            for code in [DOUBLE_WIDTH, DOUBLE_SIZE] {
                let cells = decode_row(&row(&[ALPHA_RED, code, b'A']), 0, &context(&config)).0;
                assert_eq!(cells, plain, "code {code:#04x}");
            }
        }
    }

    /// REQ-L1-003, REQ-TST-013: every Table 26 attribute with a Level 1
    /// function, checked at its own cell and at the next one.
    #[test]
    fn every_spacing_attribute_has_its_table_26_timing() {
        let colour = |code: u8| code & COLOUR_ATTRIBUTE_MASK;
        // Set-after: 0/1 to 0/7 and 1/1 to 1/7.
        for code in ALPHA_RED..=ALPHA_WHITE {
            let before = if code == ALPHA_RED { ALPHA_YELLOW } else { ALPHA_RED };
            let cells = decode(&[before, code, b'A']);
            assert_eq!((cells[1].foreground, cells[2].foreground), (colour(before), colour(code)), "{code:#04x}");
        }
        for code in MOSAIC_RED..=MOSAIC_WHITE {
            let before = if code == MOSAIC_RED { MOSAIC_RED + 2 } else { MOSAIC_RED };
            let cells = decode(&[before, code, 0x7F]);
            assert_eq!((cells[1].foreground, cells[2].foreground), (colour(before), colour(code)), "{code:#04x}");
            assert_eq!(cells[2].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false });
        }
        // 0/8 set-after, 0/9 set-at.
        let cells = decode(&[FLASH, b'A', STEADY, b'B']);
        assert_eq!(cells.map(|cell| cell.flash)[..4], [false, true, false, false]);
        // 0/A set-after, 0/B set-after (a single Start Box under the option).
        let cells = decode(&[START_BOX, START_BOX, END_BOX, b'a']);
        assert_eq!(cells.map(|cell| cell.boxed)[..4], [false, true, true, false]);
        let single = Config { double_start_box: false, ..Config::default() };
        let cells = decode_row(&row(&[START_BOX, b'a']), 0, &context(&single)).0;
        assert_eq!(cells.map(|cell| cell.boxed)[..2], [false, true]);
        // 0/C set-at, 0/D set-after.
        let cells = decode(&[DOUBLE_HEIGHT, b'A', NORMAL_SIZE, b'B']);
        assert_eq!(cells.map(|cell| cell.size)[..4], [Size::Normal, Size::DoubleTop, Size::Normal, Size::Normal]);
        // 1/8 set-at.
        let cells = decode(&[b'A', CONCEAL, b'B']);
        assert_eq!(cells.map(|cell| cell.conceal)[..3], [false, true, true]);
        // 1/9 and 1/A set-at: the next mosaic takes the new form.
        let cells = decode(&[MOSAIC_RED, SEPARATED_MOSAICS, 0x7F, CONTIGUOUS_MOSAICS, 0x7F]);
        assert_eq!(cells[2].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: true });
        assert_eq!(cells[4].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false });
        // 1/B set-after: the cell after ESC uses the second G0 set.
        let config = Config::default();
        let second = G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::German) };
        let with_second = RowContext { second_g0: Some(second), ..context(&config) };
        let cells = decode_row(&row(&[ESC, b'A']), 0, &with_second).0;
        assert_eq!(cells[0].glyph, Glyph::Space);
        assert_eq!(cells[1].glyph, Glyph::Char(CharRef::new(CharSet::G0(second), b'A')));
        // 1/C and 1/D set-at.
        let cells = decode(&[ALPHA_RED, NEW_BACKGROUND, BLACK_BACKGROUND, b'A']);
        assert_eq!(
            cells.map(|cell| cell.background)[..4],
            [COLOUR_BLACK, colour(ALPHA_RED), COLOUR_BLACK, COLOUR_BLACK]
        );
        // 1/E set-at, 1/F set-after: both attribute cells show the held
        // mosaic; the next attribute cell does not.
        let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, RELEASE_MOSAICS, MOSAIC_RED]);
        assert_eq!(cells.map(|cell| cell.held)[..5], [false, false, true, true, false]);
        assert_eq!(cells[4].glyph, Glyph::Space);
    }

    /// REQ-L1-006, REQ-TST-013: the separated mosaics example of Annex
    /// G.3.2, 17 39 1A 39.
    #[test]
    fn annex_g32_separated_mosaics() {
        let cells = decode(&[MOSAIC_WHITE, 0x39, SEPARATED_MOSAICS, 0x39]);
        // 3/9 is bits 0, 3 and 4 of the sixel pattern.
        let pattern = 0b01_1001;
        assert_eq!(cells[1].glyph, Glyph::Mosaic { pattern, separated: false });
        assert_eq!(cells[2].glyph, Glyph::Space);
        assert_eq!(cells[3].glyph, Glyph::Mosaic { pattern, separated: true });
        assert!(cells.iter().all(|cell| !cell.held));
    }

    /// REQ-L1-007, REQ-TST-013: the hold mosaics example of Annex G.3.3,
    /// 17 66 1E 39 1A 1F 66.
    #[test]
    fn annex_g33_hold_mosaics() {
        let cells = decode(&[MOSAIC_WHITE, 0x66, HOLD_MOSAICS, 0x39, SEPARATED_MOSAICS, RELEASE_MOSAICS, 0x66]);
        // 6/6 sets sixels 1, 2 and 5 (bit 6 maps to sixel 5); 3/9 sets
        // sixels 0, 3 and 4.
        let six = |separated| Glyph::Mosaic { pattern: 0b10_0110, separated };
        let three_nine = Glyph::Mosaic { pattern: 0b01_1001, separated: false };
        let expected = [Glyph::Space, six(false), six(false), three_nine, three_nine, three_nine, six(true)];
        assert_eq!(cells.map(|cell| cell.glyph)[..7], expected);
        assert_eq!(cells.map(|cell| cell.held)[..7], [false, false, true, false, true, true, false]);
    }

    /// REQ-L1-007; REQ-TST-013's hold with size changes. A change of size
    /// resets the held mosaic, a repeated size does not, and neither does
    /// turning hold off and on.
    #[test]
    fn hold_resets_on_size_change_only() {
        let held = Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false };
        // Repeated Normal Size keeps it; Double Height (set-after) shows it
        // in its own cell, then resets it.
        let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, NORMAL_SIZE, DOUBLE_HEIGHT, FLASH]);
        assert_eq!(cells.map(|cell| cell.glyph)[3..6], [held, held, Glyph::Space]);
        // Repeated Double Height keeps it; Normal Size (set-at) resets it
        // before its own cell.
        let cells = decode(&[MOSAIC_RED, DOUBLE_HEIGHT, 0x7F, HOLD_MOSAICS, DOUBLE_HEIGHT, NORMAL_SIZE]);
        assert_eq!(cells.map(|cell| cell.glyph)[3..6], [held, held, Glyph::Space]);
        // Release then Hold again: the held mosaic survives.
        let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, RELEASE_MOSAICS, FLASH, HOLD_MOSAICS]);
        assert_eq!(cells.map(|cell| cell.glyph)[3..6], [held, Glyph::Space, held]);
    }

    /// REQ-L25-006: the side-panel area is addressed by screen position,
    /// columns 40 to 71 of rows 1 to 24, and a grid is cleared in place
    /// to the empty grid.
    #[test]
    fn side_panel_area_and_clearing() {
        let mut grid: Grid = Grid::empty();
        let small: Grid<0> = Grid::empty();
        assert!(small.panel_cell(1, 40).is_none(), "a grid without panel rows");
        assert!(grid.panel_cell(0, 40).is_none(), "row 0 has no panel content");
        assert!(
            grid.panel_cell(1, 39).is_none() && grid.panel_cell(1, 72).is_none() && grid.panel_cell(25, 40).is_none()
        );
        grid.panel_cell_mut(24, 71).unwrap().foreground = 20;
        assert_eq!(grid.panels[23][31].foreground, 20);
        let attributes = CellAttributes::DEFAULT.with_invert(true);
        grid.panel_cell_mut(1, 40).unwrap().attributes = attributes;
        assert!(grid.panels[0][0].attributes.invert());
        grid.cells[3][4].attributes = attributes.with_font(FontStyle { bold: true, ..FontStyle::default() });
        grid.row_colours[5] = 17;
        grid.clear();
        assert_eq!(grid, Grid::empty());
    }

    /// REQ-L25-006: every packed attribute reads back as set, without
    /// disturbing the others.
    #[test]
    fn cell_attributes_pack_independently() {
        let fonts = [
            FontStyle::default(),
            FontStyle { bold: true, italic: false, proportional: true },
            FontStyle { bold: false, italic: true, proportional: false },
        ];
        let sources =
            [CharSource::Level1, CharSource::Local, CharSource::Active, CharSource::Adaptive, CharSource::Passive];
        for width in [Width::Normal, Width::LeftHalf, Width::RightHalf] {
            for mode in [FlashMode::Normal, FlashMode::Inverted, FlashMode::AdjacentClut] {
                for rate in [FlashRate::Slow, FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3] {
                    for (font, source) in fonts.iter().zip(sources.iter().cycle()) {
                        for flags in 0..4 {
                            let attributes = CellAttributes::DEFAULT
                                .with_width(width)
                                .with_flash_mode(mode)
                                .with_flash_rate(rate)
                                .with_underline(flags & 1 != 0)
                                .with_invert(flags & 2 != 0)
                                .with_font(*font)
                                .with_source(*source);
                            assert_eq!(attributes.width(), width);
                            assert_eq!((attributes.flash_mode(), attributes.flash_rate()), (mode, rate));
                            assert_eq!((attributes.underline(), attributes.invert()), (flags & 1 != 0, flags & 2 != 0));
                            assert_eq!((attributes.font(), attributes.source()), (*font, *source));
                        }
                    }
                }
            }
        }
        assert_eq!(Cell::BLANK.attributes.source(), CharSource::Level1);
    }

    /// REQ-L25-012, decisions E.4, E.44 and E.50: the designation order of
    /// Annex C.4 rule 1 at each level, and the national option from the
    /// designation at Levels 2.5 and 3.5 only.
    #[test]
    fn designation_follows_annex_c4_and_decision_e50() {
        use crate::config::PresentationLevel as Level;
        use crate::packet::{PageEnhancement, PageFormat};
        use crate::types::{ControlBits, Designations, PresentationFields};
        let fields = |default: u8| PresentationFields {
            designations: Some(Designations { default, second: 0x7F }),
            ..PresentationFields::default()
        };
        let basic = Some(PageFormat { function: PAGE_FUNCTION_BASIC, coding: 0 });
        // Group 1 (Central European): index 0 Polish, index 1 German.
        let polish = 0b0001 << DESIGNATION_GROUP_SHIFT;
        let mut page = StoredPage::EMPTY;
        // C14 set: header national index 1.
        page.control = ControlBits(1 << 14);
        let magazine =
            MagazineData { data: Some(fields(0)), level35: Some(fields(0b0110 << DESIGNATION_GROUP_SHIFT | 0b111)) };
        let option = |page: &StoredPage, level| {
            let config = Config { level, ..Config::default() };
            resolve_designation(Some(page), &magazine, &config).0.g0
        };
        let latin = |national| G0 { set: charset::G0Set::Latin, national: Some(national) };
        // X/28/0 first; the header's option at 1.5, the designation's at 2.5.
        page.page_data = Some(PageEnhancement { format: basic, fields: fields(polish) });
        page.page_data_level35 =
            Some(PageEnhancement { format: basic, fields: fields(0b0100 << DESIGNATION_GROUP_SHIFT) });
        assert_eq!(option(&page, Level::Level15), latin(charset::NationalOption::German));
        assert_eq!(option(&page, Level::Level25), latin(charset::NationalOption::Polish));
        assert_eq!(option(&page, Level::Level35), latin(charset::NationalOption::Polish));
        // Without X/28/0: X/28/4 at 3.5 only (Cyrillic 1), M/29/0 otherwise.
        page.page_data = None;
        assert_eq!(option(&page, Level::Level35).set, charset::G0Set::Cyrillic1);
        assert_eq!(option(&page, Level::Level25), latin(charset::NationalOption::English));
        // Then M/29/0, then M/29/4 (Greek) at 3.5.
        page.page_data_level35 = None;
        assert_eq!(option(&page, Level::Level35), latin(charset::NationalOption::English));
        let only_35 = MagazineData { data: None, ..magazine };
        let config = Config { level: Level::Level35, ..Config::default() };
        assert_eq!(resolve_designation(Some(&page), &only_35, &config).0.g0.set, charset::G0Set::Greek);
        let config = Config { level: Level::Level25, ..Config::default() };
        let header_only = resolve_designation(Some(&page), &only_35, &config).0.g0;
        assert_eq!(header_only, latin(charset::NationalOption::German), "no designation: C12 to C14");
    }

    /// REQ-CHR-003: the designation report names every step and its
    /// source: the receiver's region for a Level 1 page, the page's X/28/0,
    /// the magazine's M/29/0, a reserved entry's fallback, the second G0 set
    /// and the rows with ESC.
    #[test]
    fn the_designation_report() {
        use crate::config::PresentationLevel as Level;
        use crate::packet::{PageEnhancement, PageFormat};
        use crate::types::{ControlBits, Designations, PresentationFields};
        let fields = |default: u8, second: u8| PresentationFields {
            designations: Some(Designations { default, second }),
            ..PresentationFields::default()
        };
        let config = Config { level: Level::Level15, ..Config::default() };
        let mut page = StoredPage::EMPTY;
        page.control = ControlBits(1 << 14);
        page.cells[3][5] = crate::ecc::with_odd_parity(ESC);
        page.cells[5][0] = crate::ecc::with_odd_parity(ESC);
        page.cells[0][9] = crate::ecc::with_odd_parity(ESC);
        // A Level 1 page: only C12 to C14, the group from the receiver.
        let report = designation_report(Some(&page), &MagazineData::default(), &config);
        assert_eq!((report.source, report.group, report.header_national), (DesignationSource::Receiver, 0, 1));
        assert_eq!((report.national, report.national_source), (1, NationalSource::Header));
        assert_eq!(report.designation.g0.national, Some(charset::NationalOption::German));
        assert_eq!((report.fallback, report.second, report.esc_rows), (false, None, 2), "row 0 does not count");
        // The page's X/28/0 Format 1 gives group 0001 and a second G0 set.
        let second = 0b0110 << DESIGNATION_GROUP_SHIFT | 0b111;
        let basic = Some(PageFormat { function: PAGE_FUNCTION_BASIC, coding: 0 });
        page.page_data =
            Some(PageEnhancement { format: basic, fields: fields(0b0001 << DESIGNATION_GROUP_SHIFT, second) });
        let report = designation_report(Some(&page), &MagazineData::default(), &config);
        assert_eq!(
            (report.source, report.group, report.value),
            (DesignationSource::PageX28Format1, 1, 0b0001 << DESIGNATION_GROUP_SHIFT | 1)
        );
        assert_eq!(report.second, second_g0(second));
        assert!(report.second.is_some());
        // Without it, the magazine's M/29/0.
        page.page_data = None;
        let magazine =
            MagazineData { data: Some(fields(0b0100 << DESIGNATION_GROUP_SHIFT, 0x7F)), ..MagazineData::default() };
        let report = designation_report(Some(&page), &magazine, &config);
        assert_eq!((report.source, report.group), (DesignationSource::MagazineM29Format0, 0b0100));
        // A reserved entry falls back to group 0000 with the same option.
        let reserved = MagazineData { data: Some(fields(0b1101 << DESIGNATION_GROUP_SHIFT, 0x7F)), ..magazine };
        let report = designation_report(Some(&page), &reserved, &config);
        assert!(report.fallback);
        assert_eq!(report.designation, charset::designation(1).unwrap());
    }

    /// REQ-CHR-012: neoteletext's own designations name Latin English as
    /// the second set when no Table 33 value is received, and a received
    /// "no second set" is kept.
    #[cfg(feature = "extended-sets")]
    #[test]
    fn an_extended_designation_has_english_second() {
        let ukrainian = 0b0111 << DESIGNATION_GROUP_SHIFT;
        let config = Config { default_region: 0b0111, ..Config::default() };
        let report = designation_report(None, &MagazineData::default(), &config);
        assert_eq!((report.value, report.designation.g0.set), (ukrainian, charset::G0Set::Ukrainian));
        assert_eq!(report.second, second_g0(0));
        let fields = crate::types::PresentationFields {
            designations: Some(crate::types::Designations { default: ukrainian, second: 0x7F }),
            ..crate::types::PresentationFields::default()
        };
        let sent = MagazineData { data: Some(fields), ..MagazineData::default() };
        assert_eq!(designation_report(None, &sent, &Config::default()).second, None);
        // The standard's designations have no default second set.
        let report = designation_report(None, &MagazineData::default(), &Config::default());
        assert_eq!(report.second, None);
    }

    /// REQ-TST-013's double height with its ignored lower row.
    #[test]
    fn double_height_fills_the_row_below() {
        let mut page = StoredPage::EMPTY;
        page.state = crate::fsm::PageState::Complete;
        page.cells[1][..4].copy_from_slice(&[DOUBLE_HEIGHT, b'D', NORMAL_SIZE, b'N']);
        page.cells[2][0] = b'X';
        for row in &mut page.cells {
            for byte in row.iter_mut() {
                *byte = crate::ecc::with_odd_parity(*byte);
            }
        }
        page.parity_ok = [u64::MAX; 26];
        page.ever_good = [u64::MAX; 26];
        page.rows_received = 0b110;
        let mut grid = Grid::EMPTY;
        build_grid(&mut grid, Some(&page), &MagazineData::default(), &Config::default(), &DisplayOptions::default());
        assert!(grid.row_has_double_height(1));
        assert_eq!(grid.cells[1][1].size, Size::DoubleTop);
        assert_eq!(grid.cells[2][1].size, Size::DoubleBottom);
        assert_eq!(grid.cells[2][0].glyph, Glyph::Space, "row 2's own X is ignored");
        assert_eq!(grid.cells[2][3].glyph, Glyph::Space);
    }
}
