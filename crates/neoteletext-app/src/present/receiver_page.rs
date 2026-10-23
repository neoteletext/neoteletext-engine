//! The grid of a page the receiver draws itself, built through the
//! acquisition types so that it goes through the same Level 1 decoding as
//! a received page.

use neoteletext_core::acq::{MagazineData, StoredPage};
use neoteletext_core::config::Config;
use neoteletext_core::consts::{COLUMNS, ROWS, STATUS_COLUMNS, STORED_ROWS};
use neoteletext_core::ecc::with_odd_parity;
use neoteletext_core::fsm::PageState;
use neoteletext_core::level1::{DisplayOptions, Grid, build_grid};

/// Every column passed parity: one bit per column of a stored row.
const ALL_COLUMNS_GOOD: u64 = (1 << COLUMNS) - 1;
/// Every row received: one bit per row of the display.
const ALL_ROWS_RECEIVED: u32 = (1 << ROWS) - 1;

/// The grid of a page the receiver draws itself: `rows` are 7-bit codes,
/// row 0's columns 8 to 39 its header, and `status` the receiver's status
/// area. The rows go through the same Level 1 decoding as a received page,
/// so they look like teletext, but they never come from a decoder process.
#[must_use]
pub fn receiver_grid(rows: &[[u8; COLUMNS]; ROWS], status: [u8; STATUS_COLUMNS]) -> Grid {
    let mut page = StoredPage::EMPTY;
    page.state = PageState::Complete;
    for (stored, row) in page.cells.iter_mut().zip(rows) {
        for (cell, &code) in stored.iter_mut().zip(row) {
            *cell = with_odd_parity(code);
        }
    }
    page.parity_ok = [ALL_COLUMNS_GOOD; STORED_ROWS];
    page.ever_good = [ALL_COLUMNS_GOOD; STORED_ROWS];
    page.rows_received = ALL_ROWS_RECEIVED;
    let mut grid = Grid::EMPTY;
    let options = DisplayOptions { status, ..DisplayOptions::default() };
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &Config::default(), &options);
    grid
}
