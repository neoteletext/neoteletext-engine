//! The status area of row 0 (REQ-SEC-005): the requested page, search
//! state and indicators the receiver draws in columns 0 to 6.

use super::{MAX_LISTED_SUBPAGES, SecurityIndicator, Viewer, ViewerState};
use crate::acq::PageStore;
use crate::consts::{ALPHA_RED, ALPHA_YELLOW, SPACE, STATUS_COLUMNS};
use crate::types::SubCode;

impl Viewer {
    /// Row 0 columns 0 to 6 (decision E.10, REQ-SEC-005).
    #[must_use]
    pub fn status(&self, store: &PageStore<'_>) -> [u8; STATUS_COLUMNS] {
        let mut status = [SPACE; STATUS_COLUMNS];
        let digits = match self.state {
            ViewerState::Entering1 | ViewerState::Entering2 => self.entry.display(),
            _ => self.requested.ascii(),
        };
        let write = |status: &mut [u8; STATUS_COLUMNS], at: usize, bytes: &[u8]| {
            for (slot, &byte) in status.iter_mut().skip(at).zip(bytes) {
                *slot = byte;
            }
        };
        match self.indicator {
            SecurityIndicator::None => {
                write(&mut status, 0, b"P");
                write(&mut status, 1, &digits);
                if !self.subcode.is_any() {
                    // Specific sub-page: "S" and its number of stored
                    // sub-pages (REQ-NAV-005), as hexadecimal digits.
                    let mut codes = [SubCode::ANY; MAX_LISTED_SUBPAGES];
                    let count = store.subcodes(self.requested, &mut codes);
                    let index = codes.get(..count).and_then(|c| c.iter().position(|&s| s == self.subcode));
                    let number = index.map_or(0, |i| i.saturating_add(1));
                    let [tens, units] = two_digits(number);
                    write(&mut status, 4, &[b'/', tens, units]);
                } else if self.stale {
                    write(&mut status, 5, b"?");
                }
            }
            SecurityIndicator::Unencrypted => {
                // A red marker the page cannot forge: the number and "!".
                write(&mut status, 0, &[ALPHA_RED]);
                write(&mut status, 1, &digits);
                write(&mut status, 5, b"!");
            }
            SecurityIndicator::PlainHttpWarning => {
                write(&mut status, 0, &[ALPHA_RED]);
                write(&mut status, 1, b"HTTP?");
            }
            SecurityIndicator::LeavingService => {
                write(&mut status, 0, &[ALPHA_YELLOW]);
                write(&mut status, 1, b"LEAVE?");
            }
            SecurityIndicator::ConfirmSubmission => {
                write(&mut status, 0, &[ALPHA_YELLOW]);
                write(&mut status, 1, b"SEND?");
            }
        }
        status
    }
}

fn two_digits(value: usize) -> [u8; 2] {
    let clamped = value.min(99) as u8;
    [b'0'.saturating_add(clamped / 10), b'0'.saturating_add(clamped % 10)]
}
