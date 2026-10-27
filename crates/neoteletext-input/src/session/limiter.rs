//! The rate limit (section 8, REQ-INT-109): units counted over a sliding
//! second.

use super::Session;
use crate::MAX_PER_SECOND;
use crate::event::{Action, Event, Item, MouseAction, units};

/// The window the rate limit counts over.
const SECOND_MS: u32 = 1_000;

impl Session {
    /// How many of `item`'s units may go at `now_ms` under the rate limit
    /// (REQ-INT-109), counting those that may; a release always goes, and a
    /// move is never counted. Text
    /// that may only partly go is cut to that many characters by the
    /// caller.
    pub fn allow(&mut self, item: &Item<'_>, now_ms: u32) -> u32 {
        // A move or a HOVER counts nothing against the limit (section 8 rule
        // 4): each goes only as the pointer moves, and so can never cost a key.
        if matches!(item, Item::Event(Event::Mouse(mouse)) if mouse.action == MouseAction::Move)
            || matches!(item, Item::Event(Event::Hover { .. }))
        {
            return units(item);
        }
        let release = matches!(item, Item::Event(Event::Key(key)) if key.action == Action::Release);
        let mut allowed: u32 = 0;
        for _ in 0..units(item) {
            if !self.limiter.allow(now_ms, release) {
                break;
            }
            allowed = allowed.saturating_add(1);
        }
        allowed
    }
}

/// Counts input units against [`MAX_PER_SECOND`] over a sliding second; a
/// release is always allowed, so no key is left held at the channel.
#[derive(Clone, Copy, Debug)]
pub struct Limiter {
    /// When each of the last [`MAX_PER_SECOND`] units went, a ring.
    sent_ms: [u32; MAX_PER_SECOND as usize],
    next: usize,
    count: usize,
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Limiter {
    /// A limiter that has sent nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { sent_ms: [0; MAX_PER_SECOND as usize], next: 0, count: 0 }
    }

    /// Whether one unit may go at `now_ms`; one that may is counted, and a
    /// release always goes and is counted.
    pub fn allow(&mut self, now_ms: u32, release: bool) -> bool {
        let full = self.count == self.sent_ms.len();
        let oldest = if full { self.sent_ms.get(self.next).copied() } else { None };
        let recent = oldest.is_some_and(|oldest| now_ms.wrapping_sub(oldest) < SECOND_MS);
        if recent && !release {
            return false;
        }
        if let Some(slot) = self.sent_ms.get_mut(self.next) {
            *slot = now_ms;
        }
        self.next = self.next.saturating_add(1).checked_rem(self.sent_ms.len()).unwrap_or(0);
        self.count = self.count.saturating_add(1).min(self.sent_ms.len());
        true
    }
}
