//! One stored page or sub-page (REQ-ACQ-007, REQ-ACQ-008): its rows,
//! packets and the parity merge of decision E.8.

use crate::config::ParityPolicy;
use crate::consts::{
    COLUMNS, COMPOSITIONAL_LINKS, DATA_BYTES, DESIGNATION_CODES, PAGE_CODING_PARITY, PAGE_FUNCTION_BASIC, SPACE,
    STORED_ROWS, TRIPLETS_PER_PACKET,
};
use crate::ecc::has_odd_parity;
use crate::fsm::{PageKey, PageState};
use crate::packet::{CompositionalLink, Links, PageEnhancement, PageFormat, Triplet};
use crate::types::{ControlBits, Instant, Magazine, PageAddress, PageNumber, SubCode};

/// One stored page or sub-page (REQ-ACQ-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StoredPage {
    /// The lifecycle state (REQ-FSM-004).
    pub state: PageState,
    /// Address and sub-code; meaningless while `state` is `Empty`.
    pub key: PageKey,
    /// Header control bits of the latest reception.
    pub control: ControlBits,
    /// Rows 0 to 25, raw with parity. Row 0 columns 8 to 39 hold header
    /// bytes 14 to 45; columns 0 to 7 are never written (REQ-SEC-005).
    pub cells: [[u8; COLUMNS]; STORED_ROWS],
    /// Per row, bit c set when the byte in column c passed parity.
    pub parity_ok: [u64; STORED_ROWS],
    /// Per row, bit c set when column c has held a byte that passed parity
    /// since the page was last erased (decision E.8).
    pub ever_good: [u64; STORED_ROWS],
    /// Bit r set when row r has been received.
    pub rows_received: u32,
    /// Packets X/26 by designation code; bit d of `enhancements_present`
    /// marks code d as received.
    pub enhancements: [[Option<Triplet>; TRIPLETS_PER_PACKET]; DESIGNATION_CODES],
    /// Which designation codes of X/26 are present.
    pub enhancements_present: u16,
    /// Packet X/27/0.
    pub links: Option<Links>,
    /// Packets X/27/1 to X/27/3, raw; bit d-1 of `links_raw_present`.
    pub links_raw: [[u8; DATA_BYTES]; 3],
    /// Which of X/27/1 to X/27/3 are present.
    pub links_raw_present: u8,
    /// Packets X/27/4 and X/27/5, indexed by designation code minus 4,
    /// with their decoded links (REQ-PKT-011).
    pub compositional_links: [Option<[Option<CompositionalLink>; COMPOSITIONAL_LINKS]>; 2],
    /// Packet X/28/0, read as format 1 (decision E.44).
    pub page_data: Option<PageEnhancement>,
    /// Packet X/28/4: the Level 3.5 page data (REQ-PKT-015).
    pub page_data_level35: Option<PageEnhancement>,
    /// Packet X/28/3: its page function and coding, the only part of it
    /// decoded (REQ-PKT-014, decision E.95), kept so that a page known only
    /// by it is not shown as text (decision E.117).
    pub download_function: Option<PageFormat>,
    /// Time of the first reception.
    pub first_received: Instant,
    /// Time of the most recent packet.
    pub last_received: Instant,
    /// Number of receptions.
    pub receptions: u32,
    /// Number of parity and Hamming errors seen.
    pub errors: u32,
}

/// A blank row.
const SPACES: [u8; COLUMNS] = [SPACE; COLUMNS];

impl StoredPage {
    /// An empty slot, for building caller-provided storage.
    pub const EMPTY: Self = Self {
        state: PageState::Empty,
        key: PageKey {
            address: PageAddress { magazine: Magazine::from_bits(1), page: PageNumber(0) },
            subcode: SubCode::new(0),
        },
        control: ControlBits(0),
        cells: [SPACES; STORED_ROWS],
        parity_ok: [0; STORED_ROWS],
        ever_good: [0; STORED_ROWS],
        rows_received: 0,
        enhancements: [[None; TRIPLETS_PER_PACKET]; DESIGNATION_CODES],
        enhancements_present: 0,
        links: None,
        links_raw: [[0; DATA_BYTES]; 3],
        links_raw_present: 0,
        compositional_links: [None; 2],
        page_data: None,
        page_data_level35: None,
        download_function: None,
        first_received: Instant(0),
        last_received: Instant(0),
        receptions: 0,
        errors: 0,
    };

    /// False when packet X/28/0 format 1 or packet X/28/3 says the page is
    /// not a basic Level 1 page coded as 7 bits with odd parity: such pages
    /// are not displayed as text (REQ-ACQ-014, decision E.117).
    #[must_use]
    pub fn is_text_page(&self) -> bool {
        let is_text = |format: Option<PageFormat>| {
            format.is_none_or(|format| format.function == PAGE_FUNCTION_BASIC && format.coding == PAGE_CODING_PARITY)
        };
        is_text(self.page_data.and_then(|data| data.format)) && is_text(self.download_function)
    }

    /// True when row `row` has been received.
    #[must_use]
    pub fn has_row(&self, row: u8) -> bool {
        self.rows_received.checked_shr(u32::from(row)).is_some_and(|bits| bits & 1 == 1)
    }

    /// The stored byte at a position and whether it is displayable under
    /// `policy` (decision E.8): `(byte, passed)`. Never-received positions
    /// give a space.
    #[must_use]
    pub fn byte(&self, row: usize, column: usize, policy: ParityPolicy) -> (u8, bool) {
        let byte = self.cells.get(row).and_then(|cells| cells.get(column)).copied().unwrap_or(SPACE);
        let bit = 1u64.checked_shl(column as u32).unwrap_or(0);
        let passed = self.parity_ok.get(row).is_some_and(|flags| flags & bit != 0);
        let ever = self.ever_good.get(row).is_some_and(|flags| flags & bit != 0);
        match policy {
            ParityPolicy::KeepLastGood => (byte, passed || ever),
            ParityPolicy::AlwaysSpace | ParityPolicy::ErrorMarker => (byte, passed),
        }
    }

    /// Clears rows 1 to 28 and every packet attached to them (REQ-ACQ-004).
    pub(super) fn erase(&mut self) {
        for (index, row) in self.cells.iter_mut().enumerate() {
            if index > 0 {
                *row = SPACES;
            }
        }
        for (index, (ok, ever)) in self.parity_ok.iter_mut().zip(self.ever_good.iter_mut()).enumerate() {
            if index > 0 {
                *ok = 0;
                *ever = 0;
            }
        }
        self.rows_received &= 1;
        self.enhancements = [[None; TRIPLETS_PER_PACKET]; DESIGNATION_CODES];
        self.enhancements_present = 0;
        self.links = None;
        self.links_raw_present = 0;
        self.compositional_links = [None; 2];
        self.page_data = None;
        self.page_data_level35 = None;
        self.download_function = None;
    }

    /// Merges received bytes into `row` starting at `column` (decision E.8,
    /// REQ-ACQ-008): rows not received keep their stored content.
    /// Returns the number of parity failures.
    pub(super) fn merge(&mut self, row: usize, column: usize, bytes: &[u8], policy: ParityPolicy) -> u32 {
        let mut failures = 0u32;
        let (Some(cells), Some(ok), Some(ever)) =
            (self.cells.get_mut(row), self.parity_ok.get_mut(row), self.ever_good.get_mut(row))
        else {
            return 0;
        };
        for (offset, &byte) in bytes.iter().enumerate() {
            let position = column.saturating_add(offset);
            let Some(cell) = cells.get_mut(position) else { break };
            let bit = 1u64.checked_shl(position as u32).unwrap_or(0);
            if has_odd_parity(byte) {
                *cell = byte;
                *ok |= bit;
                *ever |= bit;
            } else {
                failures = failures.saturating_add(1);
                let keep_old = policy == ParityPolicy::KeepLastGood && *ever & bit != 0;
                if !keep_old {
                    *cell = byte;
                    *ok &= !bit;
                }
            }
        }
        if let Some(mask) = 1u32.checked_shl(row as u32) {
            self.rows_received |= mask;
        }
        failures
    }
}
