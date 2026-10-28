//! The input count the channel acknowledges (INPUT.md section 7.1), and the
//! messages kept until it does, to send again after a reconnection
//! (section 6.4, REQ-RCH-013): a ring of the messages of input units not
//! yet acknowledged. A message of no input units, a move, gone, HOVER or
//! COMPOSE, is never acknowledged, and so never kept (REQ-INT-126,
//! REQ-RCH-012).

use super::SessionOf;
use crate::event::{Item, units};

/// The most unacknowledged messages remembered (REQ-INT-110): when more
/// are sent, the oldest is forgotten, and the session can no longer be
/// resumed whole.
pub const REMEMBERED: usize = 256;
/// RFC 1982 serial arithmetic on 16 bits: a count this far ahead or more
/// is behind.
const SERIAL_HALF: u16 = 0x8000;

/// Whether `later` is after `earlier` by RFC 1982 serial arithmetic on 16
/// bits.
pub(super) const fn serial_after(later: u16, earlier: u16) -> bool {
    let ahead = later.wrapping_sub(earlier);
    ahead != 0 && ahead < SERIAL_HALF
}

impl<const KEPT: usize> SessionOf<KEPT> {
    /// Records that `item` went as `message`: counts its units and, when it
    /// has any, remembers it until it is acknowledged, its bytes kept to
    /// send again when the session may be resumed.
    pub(super) fn record(&mut self, item: &Item<'_>, message: &[u8]) {
        let units = units(item);
        if units == 0 {
            return;
        }
        // The count is modulo 2¹⁶, so only the low 16 bits of the units add.
        self.count = self.count.wrapping_add(u16::try_from(units & u32::from(u16::MAX)).unwrap_or(u16::MAX));
        if self.sent_length == REMEMBERED {
            // An unacknowledged message forgotten cannot be sent again.
            self.drop_oldest();
            self.kept_whole = false;
        }
        let mut length = 0_u16;
        if KEPT > 0 && self.kept_whole {
            let end = self.kept_length.saturating_add(message.len());
            match (u16::try_from(message.len()), self.kept.get_mut(self.kept_length..end)) {
                (Ok(fits), Some(room)) => {
                    room.copy_from_slice(message);
                    self.kept_length = end;
                    length = fits;
                }
                _ => self.kept_whole = false,
            }
        }
        let slot = self.ring(self.sent_length);
        if let Some(entry) = self.sent.get_mut(slot) {
            *entry = (self.count, length);
            self.sent_length = self.sent_length.saturating_add(1);
        }
    }

    /// Forgets the oldest unacknowledged message and its kept bytes.
    fn drop_oldest(&mut self) {
        let Some(&(after, length)) = self.sent.get(self.sent_first) else { return };
        let length = usize::from(length).min(self.kept_length);
        self.kept.copy_within(length..self.kept_length, 0);
        self.kept_length = self.kept_length.saturating_sub(length);
        self.kept_from = after;
        self.sent_first = self.sent_first.saturating_add(1).checked_rem(REMEMBERED).unwrap_or(0);
        self.sent_length = self.sent_length.saturating_sub(1);
    }

    /// A cursor record's acknowledgement: taken when it is newer than the
    /// last taken and not beyond what was sent (RFC 1982 serial arithmetic:
    /// an older one changes nothing), and the messages it covers are
    /// forgotten. Returns whether it was taken.
    pub fn acknowledged(&mut self, acked: u16) -> bool {
        if !serial_after(acked, self.acked) || serial_after(acked, self.count) {
            return false;
        }
        self.acked = acked;
        while self.sent_length > 0 {
            let Some(&(after, _)) = self.sent.get(self.sent_first) else { break };
            if serial_after(after, acked) {
                break;
            }
            self.drop_oldest();
        }
        true
    }
}
