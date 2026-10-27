//! The status area of row 0 (REQ-SEC-005): the requested page, search
//! state and indicators the receiver draws in columns 0 to 6.

use super::{SecurityIndicator, Viewer, ViewerState};
use crate::acq::PageStore;
use crate::consts::{ALPHA_RED, ALPHA_YELLOW, SPACE, STATUS_COLUMNS};
use crate::limits::DECIMAL_DIGIT_MAX;

/// The base of the place's digits.
const DECIMAL_BASE: usize = DECIMAL_DIGIT_MAX as usize + 1;
/// The highest place shown as "P100/nn": two digits.
const TWO_DIGIT_PLACE: usize = DECIMAL_BASE * DECIMAL_BASE - 1;
/// The highest place shown as "100/nnn", the "P" given up for a third
/// digit.
const THREE_DIGIT_PLACE: usize = DECIMAL_BASE * DECIMAL_BASE * DECIMAL_BASE - 1;
/// Shown for a place of four digits, which seven columns cannot hold
/// beside the page number (STANDARD-LIMITS.md question 2, open).
const PLACE_TOO_WIDE: &[u8; 3] = b"+++";

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
                    // A specific sub-page: its place among the stored
                    // sub-pages (REQ-NAV-005), "P100/12", counted by the
                    // store. A place of three digits gives up the "P"
                    // ("100/123"); one of four does not fit seven columns,
                    // and shows "100/+++" until the owner chooses
                    // (TECH-DEBT.md).
                    let place = store.subpage_place(self.requested, self.subcode).unwrap_or(0);
                    if place <= TWO_DIGIT_PLACE {
                        write(&mut status, 4, b"/");
                        write(&mut status, 5, &decimal_digits::<2>(place));
                    } else {
                        write(&mut status, 0, &digits);
                        write(&mut status, 3, b"/");
                        let shown =
                            if place <= THREE_DIGIT_PLACE { decimal_digits::<3>(place) } else { *PLACE_TOO_WIDE };
                        write(&mut status, 4, &shown);
                    }
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
            SecurityIndicator::LeavingService => {
                write(&mut status, 0, &[ALPHA_YELLOW]);
                write(&mut status, 1, b"LEAVE?");
            }
        }
        status
    }
}

/// The last `N` decimal digits of `value`, as ASCII.
fn decimal_digits<const N: usize>(value: usize) -> [u8; N] {
    let mut out = [b'0'; N];
    let mut rest = value;
    for slot in out.iter_mut().rev() {
        *slot = b'0'.saturating_add(rest.checked_rem(DECIMAL_BASE).unwrap_or(0) as u8);
        rest = rest.checked_div(DECIMAL_BASE).unwrap_or(0);
    }
    out
}
