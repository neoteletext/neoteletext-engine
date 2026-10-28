//! The rate limit (section 8, REQ-INT-109): units counted over a sliding
//! second.

use super::SessionOf;
use crate::MAX_PER_SECOND;
use crate::event::{Action, Event, Item, MouseAction, units};

/// The window the rate limit counts over.
const SECOND_MS: u32 = 1_000;

/// Whether `event` is held to the rate (section 8 rule 4): every event of
/// input units, and COMPOSE, which counts none in the input count since
/// it is never acknowledged (REQ-RCH-012) but which nothing else bounds.
/// A move, gone and HOVER go only as the pointer moves, at most 60 a
/// second (REQ-RCH-031), so they never cost a key.
pub(super) fn rated(event: &Event<'_>) -> bool {
    matches!(event, Event::Compose { .. }) || units(&Item::Event(*event)) > 0
}

/// Whether `event` lets go of something held: a key's or a button's
/// release, a contact up. It always goes, so nothing is left held at the
/// channel.
fn release(event: &Event<'_>) -> bool {
    match event {
        Event::Key(key) => key.action == Action::Release,
        Event::Mouse(mouse) => mouse.action == MouseAction::Release,
        Event::Touch(touch) => touch.action == MouseAction::Release,
        _ => false,
    }
}

impl<const KEPT: usize> SessionOf<KEPT> {
    /// How many of `item`'s units, as the rate counts them, may go at
    /// `now_ms` (REQ-INT-109), counting those that may: a character of text
    /// each, and 0 or 1 for an event held to the rate. A release always
    /// goes; an event not held to the rate gives 0 and always goes.
    pub fn allow(&mut self, item: &Item<'_>, now_ms: u32) -> u32 {
        let (wanted, release) = match item {
            Item::Text(_) => (units(item), false),
            Item::Event(event) => (u32::from(rated(event)), release(event)),
        };
        let mut allowed: u32 = 0;
        for _ in 0..wanted {
            if !self.limiter.allow(now_ms, release) {
                break;
            }
            allowed = allowed.saturating_add(1);
        }
        allowed
    }

    /// `item` as far as the rate lets it go at `now_ms`: text cut to the
    /// characters allowed, an event held to the rate whole or not at all.
    pub(super) fn within_rate<'a>(&mut self, item: Item<'a>, now_ms: u32) -> Option<Item<'a>> {
        let allowed = self.allow(&item, now_ms);
        match item {
            Item::Text(text) => {
                let end = text
                    .char_indices()
                    .nth(usize::try_from(allowed).unwrap_or(usize::MAX))
                    .map_or(text.len(), |(at, _)| at);
                text.get(..end).filter(|text| !text.is_empty()).map(Item::Text)
            }
            Item::Event(event) if allowed == 0 && rated(&event) => None,
            event @ Item::Event(_) => Some(event),
        }
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
