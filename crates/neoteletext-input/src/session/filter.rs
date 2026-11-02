//! What goes: everything a receiver gives, as sent (DOC-1922,
//! decision E.125), but HELLO and RESUME, which go once from `open`, a
//! `base` the same as its key, and an event whose capability both sides
//! did not set (REQ-205): [`needs`] alone says which those are. Without
//! TOUCH, a device other than the mouse goes as the mouse (REQ-RCH-041,
//! REQ-RCH-042); nothing else is changed to fit.

use super::SessionOf;
use crate::caps::TOUCH_CAP;
use crate::event::{Device, Event, Item, KeyCode, needs};

impl<const KEPT: usize> SessionOf<KEPT> {
    /// `item` as it is sent, or `None`.
    pub(super) fn filter<'a>(&self, item: &Item<'a>) -> Option<Item<'a>> {
        let event = match *item {
            Item::Text(text) => return Some(Item::Text(text)),
            Item::Event(event) => event,
        };
        let caps = self.caps();
        let event = match event {
            Event::Key(mut key) => {
                if key.base.map(KeyCode::Character) == Some(key.key) {
                    key.base = None;
                }
                Event::Key(key)
            }
            Event::Mouse(mut mouse) => {
                if caps & TOUCH_CAP == 0 {
                    mouse.device = Device::Mouse;
                }
                Event::Mouse(mouse)
            }
            Event::Hello { .. } | Event::Resume(_) => return None,
            other => other,
        };
        (needs(&event) & !caps == 0).then_some(Item::Event(event))
    }
}
