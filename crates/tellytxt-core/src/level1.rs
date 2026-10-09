//! Level 1.0 presentation (area L1, layer L3): from a stored page to a grid
//! of 25 by 40 cells, through the row attribute machine of REQ-FSM-006.
//!
//! [`build_grid`] is a pure function of the stored page, the magazine data,
//! the configuration and the display options (REQ-ARCH-006). Every feature
//! that crosses rows is resolved here, so rows are independent from here on
//! (decision E.29).

use crate::acq::{MagazineData, StoredPage};
use crate::charset::{
    self, CharRef, CharSet, Designation, G0, designation_with_fallback, mosaic_pattern, second_g0,
};
use crate::config::{Config, ParityPolicy, Row24Policy};
use crate::consts::{
    ALPHA_BLACK, ALPHA_WHITE, ATTRIBUTE_LAST, BLACK_BACKGROUND, C5_NEWSFLASH, C6_SUBTITLE,
    C7_SUPPRESS_HEADER, C10_INHIBIT_DISPLAY, CODE_MASK_7BIT, COLOUR_ATTRIBUTE_MASK, COLOUR_BLACK,
    COLOUR_GREEN, COLOUR_WHITE, COLUMNS, CONCEAL, CONTIGUOUS_MOSAICS, DOUBLE_HEIGHT, END_BOX, ESC,
    FLASH, HEADER_DISPLAY_BYTES, HEADER_DISPLAY_COLUMN, HOLD_MOSAICS, MOSAIC_BIT5, MOSAIC_BLACK, MOSAIC_WHITE, NATIONAL_INDEX_MASK, NEW_BACKGROUND, NORMAL_SIZE,
    RELEASE_MOSAICS, ROWS, SEPARATED_MOSAICS, SPACE, START_BOX, STATUS_ATTRIBUTE_COLUMN,
    STATUS_COLUMNS, STEADY,
};
use crate::packet::EnhancementData;

/// What a cell shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// A cell's role in double height (REQ-L1-008).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    /// Normal height.
    Normal,
    /// The top half of a double-height character.
    DoubleTop,
    /// The bottom half, shown in the row below.
    DoubleBottom,
}

/// One cell of the grid (REQ-L1-001).
// The flags are the independent cell attributes REQ-L1-001 lists one by one.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    /// What the cell shows.
    pub glyph: Glyph,
    /// Foreground colour 0 to 7.
    pub foreground: u8,
    /// Background colour 0 to 7.
    pub background: u8,
    /// The cell flashes.
    pub flash: bool,
    /// The cell is concealed.
    pub conceal: bool,
    /// The cell lies inside a box.
    pub boxed: bool,
    /// The cell's role in double height.
    pub size: Size,
    /// The cell shows a held mosaic.
    pub held: bool,
    /// The glyph came from a Level 1.5 overwrite.
    pub overwritten: bool,
    /// The source byte failed parity.
    pub parity_failed: bool,
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
        overwritten: false,
        parity_failed: false,
    };
}

/// The decoded page (REQ-L1-001): 25 rows by 40 columns, plus page-wide
/// display state.
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
    /// The character sets the page uses, for Level 1.5 and form entry.
    pub designation: Designation,
}

impl Grid {
    /// An empty grid.
    pub const EMPTY: Self = Self {
        cells: [[Cell::BLANK; COLUMNS]; ROWS],
        double_height_rows: 0,
        boxed_mode: false,
        mix: false,
        designation: Designation {
            g0: G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::English) },
            g2: charset::G2Set::Latin,
        },
    };

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

/// Resolves the page's character sets (decision E.4, REQ-CHR-003): the
/// national option from C12 to C14, the group from X/28/0, M/29/0 or the
/// configured default. Returns the designation, the second G0 set, and
/// whether a reserved entry needed the fallback.
#[must_use]
pub fn resolve_designation(
    page: Option<&StoredPage>,
    magazine: &MagazineData,
    config: &Config,
) -> (Designation, Option<G0>, bool) {
    let national = config
        .fixed_national_index
        .unwrap_or_else(|| page.map_or(0, |page| page.control.national_index()))
        & NATIONAL_INDEX_MASK;
    let enhancement: Option<EnhancementData> =
        page.and_then(|page| page.page_data).filter(|data| data.page_function == 0).or(magazine.data);
    let group = enhancement.map_or(config.default_region, |data| data.default_designation >> 3);
    let (designation, fallback) = designation_with_fallback((group << 3) | national);
    let second = enhancement.and_then(|data| second_g0(data.second_designation));
    (designation, second, fallback)
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
            overwritten: false,
            parity_failed: failed.checked_shr(column as u32).is_some_and(|bits| bits & 1 == 1),
        };
        // Step 4: the held mosaic (decision E.16).
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

/// Step 1 of REQ-FSM-006.
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
    if state.mosaic && code & MOSAIC_BIT5 != 0 {
        return (Glyph::Mosaic { pattern: mosaic_pattern(code), separated: state.separated }, false);
    }
    // Alphanumerics, and blast-through capitals in mosaic mode, with the
    // national option applied (Table 47 note 2).
    (Glyph::Char(CharRef::new(CharSet::G0(g0), code)), false)
}

/// Step 5 of REQ-FSM-006. Returns true when the row gains double height.
fn set_after(state: &mut RowState, code: u8, next: u8, context: &RowContext<'_>) -> bool {
    let black_codes = context.config.black_colour_codes;
    match code {
        ALPHA_BLACK..=ALPHA_WHITE if code != ALPHA_BLACK || black_codes => {
            if state.mosaic {
                state.held = None;
            }
            state.mosaic = false;
            state.foreground = code & COLOUR_ATTRIBUTE_MASK;
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
        FLASH => state.flash = true,
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
pub fn build_grid(
    out: &mut Grid,
    page: Option<&StoredPage>,
    magazine: &MagazineData,
    config: &Config,
    options: &DisplayOptions,
) {
    *out = Grid::EMPTY;
    let (designation, second, _) = resolve_designation(page, magazine, config);
    out.designation = designation;
    out.mix = options.mix;
    let control = page.map_or(0, |page| page.control.0);
    out.boxed_mode = options.boxed || control & (C5_NEWSFLASH | C6_SUBTITLE) != 0;
    let policy = config.parity_policy;
    let suppress_header = control & C7_SUPPRESS_HEADER != 0;
    let inhibit = control & C10_INHIBIT_DISPLAY != 0;

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
        let (cells, has_double) = decode_row(&codes, failed, &context);
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

/// The codes of display row `row` as the receiver presents them.
fn row_codes(
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
        let show = page.is_some_and(|page| match config.row24 {
            Row24Policy::AnnexH => page.links.is_some_and(|links| links.display_row_24),
            Row24Policy::WhenPresent => page.has_row(24),
        });
        if !show {
            // REQ-L1-015: left blank for receiver status messages.
            return ([SPACE; COLUMNS], 0);
        }
    }
    match page {
        Some(page) => stored_row(page, row, policy),
        None => ([SPACE; COLUMNS], 0),
    }
}

/// Blanks row 0 for a page with C7 set, keeping the status area when asked
/// (REQ-L1-014).
fn hide_header(grid: &mut Grid, keep_status: bool) {
    if let Some(cells) = grid.cells.get_mut(0) {
        for (column, cell) in cells.iter_mut().enumerate() {
            if !(keep_status && column <= STATUS_ATTRIBUTE_COLUMN) {
                *cell = Cell::BLANK;
            }
        }
    }
}

/// Fills the row below a double-height row (decision E.15).
fn fill_lower_row(grid: &mut Grid, row: usize) {
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
    use crate::consts::{ALPHA_RED, MOSAIC_RED};

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

    #[test]
    fn colour_attributes_are_set_after() {
        let cells = decode(&[ALPHA_RED, b'A']);
        assert_eq!(cells[0].foreground, COLOUR_WHITE, "attribute cell keeps the old colour");
        assert_eq!(cells[1].foreground, 1);
        assert_eq!(cells[0].glyph, Glyph::Space);
    }

    #[test]
    fn new_background_is_set_at() {
        let cells = decode(&[ALPHA_RED, NEW_BACKGROUND, b'A', BLACK_BACKGROUND, b'B']);
        assert_eq!(cells[1].background, 1, "New Background acts in its own cell");
        assert_eq!(cells[2].background, 1);
        assert_eq!(cells[3].background, 0);
    }

    #[test]
    fn hold_mosaics_annex_g33_style() {
        // Mosaic red, a mosaic, hold, a colour change: the attribute cell
        // after the hold shows the held mosaic.
        let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, MOSAIC_RED + 1, 0x21]);
        assert_eq!(cells[1].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false });
        assert!(cells[2].held && cells[3].held);
        assert_eq!(cells[3].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false });
    }

    #[test]
    fn held_mosaic_keeps_its_form_and_resets_on_mode_change() {
        let cells = decode(&[MOSAIC_RED, SEPARATED_MOSAICS, 0x7F, HOLD_MOSAICS, CONTIGUOUS_MOSAICS, ALPHA_RED, MOSAIC_RED]);
        // Contiguous (set-at) in column 4: the held mosaic keeps its stored
        // separated form (decision E.16).
        assert_eq!(cells[4].glyph, Glyph::Mosaic { pattern: 0x3F, separated: true });
        // Column 5 switches to alphanumerics (set-after); its own cell still
        // shows the held mosaic, and the reset follows.
        assert!(cells[5].held);
        assert_eq!(cells[6].glyph, Glyph::Space);
    }

    #[test]
    fn blast_through_capitals_in_mosaic_mode() {
        let cells = decode(&[MOSAIC_RED, b'A', 0x23]);
        assert!(matches!(cells[1].glyph, Glyph::Char(c) if c.code == b'A'));
        assert!(matches!(cells[2].glyph, Glyph::Mosaic { .. }));
    }

    #[test]
    fn conceal_ends_at_colour() {
        let cells = decode(&[CONCEAL, b'A', ALPHA_RED, b'B']);
        assert!(cells[0].conceal && cells[1].conceal && cells[2].conceal);
        assert!(!cells[3].conceal);
    }

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

    #[test]
    fn flash_and_steady() {
        let cells = decode(&[FLASH, b'A', STEADY, b'B']);
        assert!(!cells[0].flash && cells[1].flash && !cells[2].flash);
    }

    #[test]
    fn black_codes_follow_the_profile() {
        let cells = decode(&[ALPHA_RED, ALPHA_BLACK, b'A']);
        assert_eq!(cells[2].foreground, 1, "0/0 has no function by default");
        let config = Config::saa5246a();
        let cells = decode_row(&row(&[ALPHA_RED, ALPHA_BLACK, b'A']), 0, &context(&config)).0;
        assert_eq!(cells[2].foreground, 0);
    }

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
