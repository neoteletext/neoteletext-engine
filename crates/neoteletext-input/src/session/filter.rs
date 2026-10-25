//! What the modes in force let go (REQ-INT-115): the mouse and the wheel
//! only in a mouse mode, and only the fields the modes ask for.

use super::Session;
use crate::event::{self, Action, Event, Item, KeyCode, MouseAction};
use crate::keys;
use crate::modes::{
    ALL_KEYS, BASE_KEY, COMPOSE, KEY_RELEASE, MOUSE_ALL, MOUSE_DRAG, MOUSE_MODES, MOUSE_PIXELS, PAGE_CAP,
};

impl Session {
    /// `item` as the modes in force on `open` have it sent, or `None`: the
    /// mouse and the wheel only in a mouse mode (REQ-INT-115).
    pub(super) fn filter<'a>(&self, item: &Item<'a>, open: Option<u16>) -> Option<Item<'a>> {
        let modes = self.modes.on(open);
        let has = |mode: u32| modes & mode != 0;
        let event = match *item {
            Item::Text(_) if has(ALL_KEYS) => return None,
            Item::Text(text) => return Some(Item::Text(text)),
            Item::Event(event) => event,
        };
        let event = match event {
            Event::Key(key) if key.action != Action::Press && !has(KEY_RELEASE) => return None,
            // A dead key goes only in ALL_KEYS (section 5.7 rule 4), and a
            // key that only modifies or locks only when keys are asked for
            // whole (section 6.3).
            Event::Key(event::Key { key: KeyCode::Functional(code), .. }) if keys::is_dead(code) && !has(ALL_KEYS) => {
                return None;
            }
            Event::Key(event::Key { key: KeyCode::Functional(code), .. })
                if keys::is_modifier(code) && !has(ALL_KEYS | KEY_RELEASE) =>
            {
                return None;
            }
            Event::Key(mut key) => {
                if !has(ALL_KEYS) {
                    key.text = None;
                }
                if !has(BASE_KEY) || key.base.map(KeyCode::Character) == Some(key.key) {
                    key.base = None;
                }
                Event::Key(key)
            }
            Event::Mouse(mut mouse) => {
                let wanted = match mouse.action {
                    MouseAction::Press | MouseAction::Release => has(MOUSE_MODES),
                    MouseAction::Move => has(MOUSE_ALL) || (has(MOUSE_DRAG) && mouse.button != 0),
                };
                if !wanted {
                    return None;
                }
                if !has(MOUSE_PIXELS) {
                    mouse.at.pixels = None;
                }
                Event::Mouse(mouse)
            }
            // The wheel goes in any mode (INPUT.md section 6.5, decision
            // E.119): the receiver sends it only over the page.
            Event::Wheel(mut wheel) => {
                if !has(MOUSE_PIXELS) {
                    wheel.at.pixels = None;
                }
                Event::Wheel(wheel)
            }
            Event::Compose { .. } if !has(COMPOSE) => return None,
            Event::Page(_) if self.caps() & PAGE_CAP == 0 => return None,
            // HELLO goes once, from `open`.
            Event::Hello { .. } => return None,
            other => other,
        };
        Some(Item::Event(event))
    }
}
