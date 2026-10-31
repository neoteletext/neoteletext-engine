//! What goes: everything a receiver gives, as sent (INPUT.md section 6.3,
//! decision E.125), but HELLO and RESUME, which go once from `open`, a `base` the same as its key, and what touch
//! adds without the TOUCH capability (REQ-RCH-041, REQ-RCH-042): TOUCH is
//! not sent, and MOUSE goes as the mouse.

use super::SessionOf;
use crate::caps::TOUCH_CAP;
use crate::event::{Device, Event, Item, KeyCode};

impl<const KEPT: usize> SessionOf<KEPT> {
    /// `item` as it is sent, or `None`.
    pub(super) fn filter<'a>(&self, item: &Item<'a>) -> Option<Item<'a>> {
        let event = match *item {
            Item::Text(text) => return Some(Item::Text(text)),
            Item::Event(event) => event,
        };
        let touch = self.caps() & TOUCH_CAP != 0;
        let event = match event {
            Event::Key(mut key) => {
                if key.base.map(KeyCode::Character) == Some(key.key) {
                    key.base = None;
                }
                Event::Key(key)
            }
            Event::Mouse(mut mouse) => {
                if !touch {
                    mouse.device = Device::Mouse;
                }
                Event::Mouse(mouse)
            }
            Event::Touch(_) if !touch => return None,
            Event::Hello { .. } | Event::Resume(_) => return None,
            other => other,
        };
        Some(Item::Event(event))
    }
}
