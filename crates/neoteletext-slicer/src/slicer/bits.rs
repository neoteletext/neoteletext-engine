//! Bits and bytes (REQ-SLC-006, REQ-SLC-007, REQ-SLC-009): bit centres at a
//! fixed bit period, read by linear interpolation.

use neoteletext_core::consts::PACKET_BYTES;

use crate::consts::{BITS_PER_BYTE, FRAMING_BITS, RUN_IN_MATCH_BITS};
use crate::fsm::SlicerEvent;
use crate::math::{f32_from_i32, f32_from_usize, floor_index, round_i32};

use super::{Slicer, Work};

impl Slicer {
    pub(super) fn collect(&self, work: &mut Work) -> SlicerEvent {
        let offset = isize::try_from(work.framing_offset).unwrap_or(0);
        let mut index = (RUN_IN_MATCH_BITS + FRAMING_BITS).saturating_add_signed(offset);
        let mut bytes = [0; PACKET_BYTES];
        for byte in &mut bytes {
            let mut value = 0u8;
            for bit in 0..BITS_PER_BYTE {
                if self.bit(work, index) {
                    value |= 1 << bit;
                }
                index = index.saturating_add(1);
            }
            *byte = value;
        }
        work.bytes = bytes;
        SlicerEvent::BitsRead
    }

    /// The bit `index` bit periods after run-in bit 3 (index 0), decided at
    /// its centre against the held threshold.
    pub(super) fn bit(&self, work: &Work, index: usize) -> bool {
        let Some(rate) = self.rate else { return false };
        let position = work.bit3_centre + f32_from_usize(index) * rate.samples_per_bit;
        self.at(position) > work.held_threshold
    }

    /// The signal at a fractional sample position, linearly interpolated
    /// (REQ-SLC-007); 0 (black) outside the line.
    pub(super) fn at(&self, position: f32) -> f32 {
        if position < 0.0 {
            return 0.0;
        }
        let index = floor_index(position);
        let fraction = position - f32_from_usize(index);
        let here = self.samples.get(index).copied().unwrap_or(0.0);
        let next = self.samples.get(index.saturating_add(1)).copied().unwrap_or(here);
        here + fraction * (next - here)
    }

    /// The start of run-in bit 1 in samples, corrected by the framing code
    /// offset; 0 if it falls before the line.
    pub(super) fn run_in_position(&self, work: &Work) -> u32 {
        let Some(rate) = self.rate else { return 0 };
        // Bit 3's centre less two and a half bits is the start of bit 1.
        let lead = 2.5 - f32_from_i32(work.framing_offset);
        let start = work.bit3_centre - lead * rate.samples_per_bit;
        u32::try_from(round_i32(start).max(0)).unwrap_or(0)
    }
}
