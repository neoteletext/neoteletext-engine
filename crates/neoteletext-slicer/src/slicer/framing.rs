//! The framing code search (REQ-SLC-008, decision E.24): offsets within
//! two bits, nearest first, with one-bit errors corrected.

use neoteletext_core::consts::FRAMING_CODE;

use crate::consts::{FRAMING_SEARCH_BITS, RUN_IN_MATCH_BITS};
use crate::fsm::SlicerEvent;

use super::{FRAMING_WINDOW_BITS, Slicer, Work};

/// The order in which framing code offsets are tried: nearest first.
const FRAMING_OFFSETS: [i32; 2 * FRAMING_SEARCH_BITS + 1] = framing_offsets();

// Evaluated only in the constant above, where overflow or an index out of
// range is a compile error, not a run-time panic.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn framing_offsets() -> [i32; 2 * FRAMING_SEARCH_BITS + 1] {
    let mut offsets = [0; 2 * FRAMING_SEARCH_BITS + 1];
    let mut distance = 1;
    let mut index = 1;
    while distance <= FRAMING_SEARCH_BITS {
        // The search reach is a small constant, so the conversion is exact.
        #[allow(clippy::cast_possible_wrap)]
        let signed = distance as i32;
        offsets[index] = -signed;
        offsets[index + 1] = signed;
        index += 2;
        distance += 1;
    }
    offsets
}

impl Slicer {
    pub(super) fn find_framing_code(&self, work: &mut Work) -> SlicerEvent {
        // Bits from FRAMING_SEARCH_BITS before the expected framing code to
        // as many after it; bit 0 of `window` is the earliest.
        let first_index = RUN_IN_MATCH_BITS - FRAMING_SEARCH_BITS;
        let mut window: u32 = 0;
        for bit in 0..FRAMING_WINDOW_BITS {
            if self.bit(work, first_index.saturating_add(bit)) {
                window |= 1 << bit;
            }
        }
        let candidate = |offset: i32| {
            let reach = i32::try_from(FRAMING_SEARCH_BITS).unwrap_or(0);
            let shift = u32::try_from(offset.saturating_add(reach)).unwrap_or(0);
            ((window >> shift) & u32::from(u8::MAX)) as u8
        };
        for offset in FRAMING_OFFSETS {
            if candidate(offset) == FRAMING_CODE {
                work.framing_offset = offset;
                return SlicerEvent::FramingExact;
            }
        }
        if self.config.framing_tolerance {
            for offset in FRAMING_OFFSETS {
                // Exactly one bit differs: the difference is a power of two.
                if (candidate(offset) ^ FRAMING_CODE).is_power_of_two() {
                    work.framing_offset = offset;
                    return SlicerEvent::FramingOneBitError;
                }
            }
        }
        SlicerEvent::FramingNotFound
    }
}
