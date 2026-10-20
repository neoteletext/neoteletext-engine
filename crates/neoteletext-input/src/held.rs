//! Held input, at the channel's end (`neoteletext-common/INPUT.md` section
//! 8, REQ-INT-108): the keys and buttons a receiver has pressed and not
//! released. On `FOCUS_OUT`, or when the connection is lost, the channel
//! treats every one as released; the receiver sends no releases for them.

use crate::event::{Action, Event, Item, KeyCode, MouseAction};

/// The most keys tracked as held at once: more than hands hold. A press
/// beyond it is not tracked, so its release changes nothing.
pub const MAX_HELD_KEYS: usize = 16;
/// Mouse buttons tracked: left, middle, right, back and forward, and three
/// more (section 5.4's `button`).
const BUTTONS: u8 = 8;

/// What is held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    keys: [Option<KeyCode>; MAX_HELD_KEYS],
    /// Bit `n` set while button `n` is held.
    buttons: u8,
}

/// Something to treat as released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Released {
    /// A key.
    Key(KeyCode),
    /// A mouse button.
    Button(u8),
}

impl Held {
    /// Nothing held.
    #[must_use]
    pub const fn new() -> Self {
        Self { keys: [None; MAX_HELD_KEYS], buttons: 0 }
    }

    /// Takes one item of the stream; on `FOCUS_OUT`, gives each held key and
    /// button to `released` and forgets them.
    pub fn take(&mut self, item: &Item<'_>, released: &mut dyn FnMut(Released)) {
        let Item::Event(event) = item else { return };
        match event {
            Event::Key(key) => match key.action {
                Action::Press | Action::Repeat => {
                    if !self.keys.contains(&Some(key.key))
                        && let Some(slot) = self.keys.iter_mut().find(|slot| slot.is_none())
                    {
                        *slot = Some(key.key);
                    }
                }
                Action::Release => {
                    for slot in &mut self.keys {
                        if *slot == Some(key.key) {
                            *slot = None;
                        }
                    }
                }
            },
            Event::Mouse(mouse) if mouse.button < BUTTONS => {
                let bit = 1_u8 << mouse.button;
                match mouse.action {
                    MouseAction::Press => self.buttons |= bit,
                    MouseAction::Release => self.buttons &= !bit,
                    MouseAction::Move => {}
                }
            }
            Event::FocusOut => self.release_all(released),
            _ => {}
        }
    }

    /// The connection was lost, or focus went: each held key and button
    /// to `released`, and nothing held after.
    pub fn release_all(&mut self, released: &mut dyn FnMut(Released)) {
        for slot in &mut self.keys {
            if let Some(key) = slot.take() {
                released(Released::Key(key));
            }
        }
        for button in 0..BUTTONS {
            if self.buttons & (1 << button) != 0 {
                released(Released::Button(button));
            }
        }
        self.buttons = 0;
    }

    /// Whether anything is held.
    #[must_use]
    pub fn any(&self) -> bool {
        self.buttons != 0 || self.keys.iter().any(Option::is_some)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use super::*;
    use crate::event::{Key, Mouse, Point, read_message};
    use std::vec::Vec;

    fn released_by(stream: &[u8]) -> Vec<Released> {
        let mut held = Held::new();
        let mut released = Vec::new();
        read_message(stream, &mut |item| held.take(&item, &mut |each| released.push(each))).unwrap();
        held.release_all(&mut |each| released.push(each));
        released
    }

    #[test]
    fn focus_out_releases_what_is_held_and_nothing_else() {
        // Shift and `a` pressed, `a` released; the left button pressed;
        // then the focus goes (REQ-INT-108).
        let stream = [
            0xFF, 0x01, 0x04, 80, // Shift (ShiftLeft) pressed
            0xFF, 0x01, 0x00, b'a', // a pressed
            0xFF, 0x01, 0x02, b'a', // a released
            0xFF, 0x02, 0x00, 0x00, 0x0A, 0x05, // left button pressed
            0xFF, 0x06, // FOCUS_OUT
        ];
        let mut held = Held::new();
        let mut released = Vec::new();
        read_message(&stream, &mut |item| held.take(&item, &mut |each| released.push(each))).unwrap();
        assert_eq!(released, [Released::Key(KeyCode::Functional(80)), Released::Button(0)]);
        assert!(!held.any(), "nothing held after");
    }

    #[test]
    fn a_lost_connection_releases_too() {
        let stream = [0xFF, 0x01, 0x04, 9, 0xFF, 0x01, 0x05, 9];
        assert_eq!(released_by(&stream), [Released::Key(KeyCode::Functional(9))], "Up, pressed and repeating");
        assert_eq!(released_by(b"ab"), [], "text holds nothing");
    }

    #[test]
    fn presses_beyond_the_bound_are_not_tracked() {
        let mut held = Held::new();
        for character in ('a'..='z').take(MAX_HELD_KEYS + 2) {
            let key =
                Key { action: Action::Press, key: KeyCode::Character(character), mods: 0, text: None, base: None };
            held.take(&Item::Event(Event::Key(key)), &mut |_| {});
        }
        let at = Point { column: 0, row: 0, pixels: None, mods: 0 };
        held.take(&Item::Event(Event::Mouse(Mouse { action: MouseAction::Press, button: 200, at })), &mut |_| {});
        let mut count = 0;
        held.release_all(&mut |_| count += 1);
        assert_eq!(count, MAX_HELD_KEYS, "the bound, and no button out of range");
    }
}
