//! The grid of 25 by 40 cells and the page-wide display state (REQ-L1-001, decision E.42), and the display options.

use super::Cell;
use crate::charset::{self, Designation, G0};
use crate::colour::{ColourIndex, ColourMap};
use crate::consts::{CLOCK_BYTES, COLOUR_BLACK, COLUMNS, HEADER_DISPLAY_BYTES, ROWS, SPACE, STATUS_COLUMNS};

/// The decoded page (REQ-L1-001): 25 rows by 40 columns, plus page-wide
/// display state. It is the core's primary output; every renderer works
/// from it (REQ-RND-001).
///
/// Everything a row needs travels in the grid, so a row is rendered from
/// the grid alone (decision E.29). It is 40 columns at every level
/// (REQ-1644).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grid {
    /// The cells, row by row.
    pub cells: [[Cell; COLUMNS]; ROWS],
    /// Bit r set when row r contains double height.
    pub double_height_rows: u32,
    /// Only boxed areas are visible: newsflash and subtitle pages
    /// (decision E.22) or a viewer request.
    pub boxed_mode: bool,
    /// Mix mode: backgrounds transparent everywhere (REQ-L1-020).
    pub mix: bool,
    /// The character sets the page uses, for Level 1.5 and the port.
    pub designation: Designation,
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
    /// An empty grid.
    pub const EMPTY: Self = Self::empty();

    /// An empty grid: blank cells, the Table 30 Colour Map, and black
    /// screen and rows. To empty an existing grid without a temporary
    /// copy, use [`Grid::clear`].
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
        self.double_height_rows = 0;
        self.boxed_mode = false;
        self.mix = false;
        self.designation = DEFAULT_DESIGNATION;
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
    /// On a page found, row 0 columns 32 to 39 from the latest header of
    /// any magazine, raw: the clock that keeps time (decision E.11).
    pub clock: Option<[u8; CLOCK_BYTES]>,
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
            clock: None,
            row24: None,
            security_indicator: false,
            mix: false,
            boxed: false,
        }
    }
}
