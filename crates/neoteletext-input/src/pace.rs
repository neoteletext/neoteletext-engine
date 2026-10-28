//! The pointer's moves paced at 60 a second (`neoteletext-common/INPUT.md`
//! section 9.8, REQ-INT-122; REQ-RCH-031 to REQ-RCH-033 and REQ-RCH-045): one slot and
//! one pace for each contact, the mouse's and each finger's, shared by
//! every receiver so that one implementation enforces the rule.
//!
//! - **A move goes at once if it can.** One that comes 1000/60 ms or more
//!   after the contact's last move sent goes at once, and begins a run.
//! - **Otherwise the latest waits.** One that comes sooner waits in the
//!   contact's slot, replacing any position waiting there, and goes when
//!   the period ends: in a run the *k*th period ends at the run's start
//!   plus *k* × 1,000,000 / 60 µs, counted exactly, so the average is 60 a
//!   second with no drift.
//! - **Nothing unchanged.** A move the same as the last sent is not sent
//!   again, and one waiting is dropped when the pointer comes back.
//! - **Order.** Before any other event is written, [`Pacer::flush`] gives
//!   every waiting move, so a release never comes before the move that
//!   ended the drag.
//! - **No timer while still.** [`Pacer::due`] is `None` while no move
//!   waits.
//!
//! Times are microseconds of any monotonic clock.

use crate::MAX_CONTACTS;
use crate::MOVES_PER_SECOND;
use crate::event::{Event, move_contact};

/// Microseconds in a second.
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
/// A period's whole microseconds: 16,666 of the 16,666⅔.
pub const PERIOD_WHOLE_US: u64 = MICROSECONDS_PER_SECOND / MOVES_PER_SECOND;
/// What a period has beyond its whole microseconds, in 1/60ths of one:
/// 40, the ⅔.
const PERIOD_REMAINDER: u64 = MICROSECONDS_PER_SECOND % MOVES_PER_SECOND;

/// What became of a move offered to the pacer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Offer {
    /// Send it now.
    Now(Event<'static>),
    /// It waits, the latest of its contact, until `due_us`: the caller's
    /// timer, unless one is due sooner ([`Pacer::due`]).
    Held {
        /// When it goes.
        due_us: u64,
    },
    /// It is the move last sent for its contact: nothing goes.
    Unchanged,
    /// Not a move of a contact the pacer has: it takes MOUSE moves and
    /// TOUCH moves of ids below [`MAX_CONTACTS`].
    NotAMove,
}

/// One contact's slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Slot {
    /// The move last sent.
    last: Option<Event<'static>>,
    /// The latest move not yet sent.
    waiting: Option<Event<'static>>,
    /// When the period after the last move sent ends; `None` before any.
    next_due_us: Option<u64>,
    /// The run's fraction of a microsecond beyond `next_due_us`, in 1/60ths.
    remainder: u64,
}

impl Slot {
    const EMPTY: Self = Self { last: None, waiting: None, next_due_us: None, remainder: 0 };

    /// The move goes at `at_us`, the end of the period it waited for.
    fn advance(&mut self, at_us: u64) {
        let mut due = at_us.saturating_add(PERIOD_WHOLE_US);
        let remainder = self.remainder.saturating_add(PERIOD_REMAINDER);
        self.remainder = if remainder >= MOVES_PER_SECOND {
            due = due.saturating_add(1);
            remainder.saturating_sub(MOVES_PER_SECOND)
        } else {
            remainder
        };
        self.next_due_us = Some(due);
    }

    /// A move goes at once at `now_us`, beginning a run.
    fn start(&mut self, now_us: u64) {
        self.remainder = 0;
        self.advance(now_us);
    }
}

/// The slots of every contact.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pacer {
    slots: [Slot; MAX_CONTACTS],
}

impl Default for Pacer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pacer {
    /// No move sent or waiting.
    #[must_use]
    pub const fn new() -> Self {
        Self { slots: [Slot::EMPTY; MAX_CONTACTS] }
    }

    /// A move, MOUSE's or a TOUCH's, at `now_us`: whether it goes now, waits
    /// or is unchanged.
    pub fn offer(&mut self, event: Event<'static>, now_us: u64) -> Offer {
        let Some(slot) = move_contact(&event).and_then(|contact| self.slots.get_mut(contact)) else {
            return Offer::NotAMove;
        };
        if slot.last == Some(event) {
            slot.waiting = None;
            return Offer::Unchanged;
        }
        match slot.next_due_us {
            Some(due_us) if now_us < due_us => {
                slot.waiting = Some(event);
                Offer::Held { due_us }
            }
            _ => {
                slot.waiting = None;
                slot.last = Some(event);
                slot.start(now_us);
                Offer::Now(event)
            }
        }
    }

    /// When the next waiting move falls due, the caller's one timer; `None`
    /// while none waits, when no timer is needed.
    #[must_use]
    pub fn due(&self) -> Option<u64> {
        self.slots.iter().filter(|slot| slot.waiting.is_some()).filter_map(|slot| slot.next_due_us).min()
    }

    /// At `now_us`, gives `each` every waiting move whose period has ended,
    /// in contact order, as sent: moves of several contacts due at one
    /// moment come together, to be written together (REQ-RCH-045).
    pub fn take_due(&mut self, now_us: u64, each: &mut dyn FnMut(Event<'static>)) {
        for slot in &mut self.slots {
            if let (Some(event), Some(due_us)) = (slot.waiting, slot.next_due_us)
                && now_us >= due_us
            {
                slot.waiting = None;
                slot.last = Some(event);
                // From the deadline, not the moment the timer ran: no drift.
                slot.advance(due_us);
                each(event);
            }
        }
    }

    /// Before any other event is written at `now_us` (REQ-RCH-033), gives
    /// `each` every waiting move, in contact order, as sent; each begins its
    /// contact's next run.
    pub fn flush(&mut self, now_us: u64, each: &mut dyn FnMut(Event<'static>)) {
        for slot in &mut self.slots {
            if let Some(event) = slot.waiting.take() {
                slot.last = Some(event);
                slot.start(now_us);
                each(event);
            }
        }
    }

    /// The contact ended, or the pointer left the page: its slot is
    /// emptied, so its next move goes at once, whatever it is. Flush
    /// first: a waiting move is dropped.
    pub fn forget(&mut self, contact: usize) {
        if let Some(slot) = self.slots.get_mut(contact) {
            *slot = Slot::EMPTY;
        }
    }

    /// Every slot emptied: keys stopped going to the channel.
    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests;
