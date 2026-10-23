//! Page assembly (REQ-ARCH-006): the codes of each row as the receiver
//! presents them, row 0's receiver columns, row 24, the suppressed header
//! and the row below double height (decisions E.10, E.15, E.23).

use super::row::{RowContext, decode_row};
use super::{Cell, DisplayOptions, Glyph, Grid, Size, resolve_designation};
use crate::acq::{MagazineData, StoredPage};
use crate::charset::{self, CharRef, CharSet, G0};
use crate::config::{Config, ParityPolicy, Row24Policy};
use crate::consts::{
    C5_NEWSFLASH, C6_SUBTITLE, C7_SUPPRESS_HEADER, C10_INHIBIT_DISPLAY, CODE_MASK_7BIT, COLOUR_GREEN, COLOUR_WHITE,
    COLUMNS, HEADER_DISPLAY_COLUMN, RECEIVER_COLUMNS, ROWS, SPACE, STATUS_ATTRIBUTE_COLUMN,
};

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
        // Decision E.15: rows 0, 23 and 24 are always normal height.
        let context = RowContext::new(config, designation.g0, second, (1..=22).contains(&row));
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
