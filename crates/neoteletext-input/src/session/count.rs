//! The input count the channel acknowledges and the round trip it gives
//! (REQ-INT-137): a ring of the sends not yet acknowledged.

use super::Session;
use crate::event::{Item, units};

/// Sends remembered for the round trip (REQ-INT-137).
pub const REMEMBERED: usize = 256;
/// RFC 1982 serial arithmetic on 16 bits: a count this far ahead or more
/// is behind.
const SERIAL_HALF: u16 = 0x8000;

/// Whether `later` is after `earlier` by RFC 1982 serial arithmetic on 16
/// bits.
const fn serial_after(later: u16, earlier: u16) -> bool {
    let ahead = later.wrapping_sub(earlier);
    ahead != 0 && ahead < SERIAL_HALF
}

impl Session {
    /// Records that `item` went at `now_ms`, for the input count and the
    /// round trip; returns the count after it.
    pub fn sent(&mut self, item: &Item<'_>, now_ms: u32) -> u16 {
        // The count is modulo 2¹⁶, so only the low 16 bits of the units add.
        let units = u16::try_from(units(item) & u32::from(u16::MAX)).unwrap_or(u16::MAX);
        self.count = self.count.wrapping_add(units);
        if self.sent_length == REMEMBERED {
            self.drop_oldest();
        }
        let slot = self.sent_first.saturating_add(self.sent_length).checked_rem(REMEMBERED).unwrap_or(0);
        if let Some(entry) = self.sent.get_mut(slot) {
            *entry = (self.count, now_ms);
            self.sent_length = self.sent_length.saturating_add(1);
        }
        self.count
    }

    fn drop_oldest(&mut self) {
        self.sent_first = self.sent_first.saturating_add(1).checked_rem(REMEMBERED).unwrap_or(0);
        self.sent_length = self.sent_length.saturating_sub(1);
    }

    /// A cursor record's acknowledgement at `now_ms`: the round trip in
    /// milliseconds to the latest send it covers, when it is newer than the
    /// last taken (RFC 1982 serial arithmetic: an older one changes nothing).
    /// The sends it covers are forgotten.
    pub fn acknowledged(&mut self, acked: u16, now_ms: u32) -> Option<u32> {
        if !serial_after(acked, self.acked) {
            return None;
        }
        self.acked = acked;
        let mut latest = None;
        while self.sent_length > 0 {
            let Some(&(count, at)) = self.sent.get(self.sent_first) else { break };
            if serial_after(count, acked) {
                break;
            }
            latest = Some(at);
            self.drop_oldest();
        }
        latest.map(|at| now_ms.wrapping_sub(at))
    }
}
