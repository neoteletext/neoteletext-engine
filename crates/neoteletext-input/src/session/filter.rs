//! What goes: everything a receiver gives, as sent (INPUT.md section 6.3,
//! decision E.125), but PAGE without its capability, HELLO, which goes once
//! from `open`, and a `base` the same as its key.

use super::Session;
use crate::caps::PAGE_CAP;
use crate::event::{Event, Item, KeyCode};

impl Session {
    /// `item` as it is sent, or `None`.
    pub(super) fn filter<'a>(&self, item: &Item<'a>) -> Option<Item<'a>> {
        let event = match *item {
            Item::Text(text) => return Some(Item::Text(text)),
            Item::Event(event) => event,
        };
        let event = match event {
            Event::Key(mut key) => {
                if key.base.map(KeyCode::Character) == Some(key.key) {
                    key.base = None;
                }
                Event::Key(key)
            }
            Event::Page(_) if self.caps() & PAGE_CAP == 0 => return None,
            Event::Hello { .. } => return None,
            other => other,
        };
        Some(Item::Event(event))
    }
}
