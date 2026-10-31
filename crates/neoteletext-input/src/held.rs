//! Held input (`neoteletext-common/INPUT.md` section 8 rule 3, REQ-207):
//! the keys, buttons and contacts a receiver has pressed and not released.
//! It is released whenever the page loses the input: on `FOCUS_OUT`, on a
//! change of selection, when a pointer is gone (what it held only, without
//! a click), and when the connection is lost. Releasing never deselects.
//!
//! A channel keeps one for each connection, and gives its program whatever
//! is still held at those signals, marked as lost input. A receiver keeps
//! one to send a lost release of each held key before the event that ends
//! the input ([`lost_key`]).

use crate::event::{Action, Event, Item, Key, KeyCode, MouseAction, Select, Stamp};

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
    /// Bit `n` set while TOUCH contact `n` is down (REQ-RCH-043).
    contacts: u16,
    /// The page the latest press was on. A change of selection releases
    /// everything, so what is held is all of one page.
    on: Option<Stamp>,
    /// The connection's selection, as SELECT last said it.
    selected: Option<Select>,
}

/// Something to treat as released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Released {
    /// A key.
    Key(KeyCode),
    /// A mouse button.
    Button(u8),
    /// A TOUCH contact, by its `id`.
    Contact(u8),
}

// Every TOUCH id has its bit.
const _: () = assert!(crate::MAX_CONTACTS <= u16::BITS as usize);

/// The KEY a receiver sends for a held key the page lost: a release with
/// the L flag, stamped with the page that held it (INPUT.md section 5.5).
#[must_use]
pub const fn lost_key(key: KeyCode, on: Stamp) -> Key<'static> {
    Key { on, action: Action::Release, key, mods: 0, text: None, base: None, lost: true }
}

impl Held {
    /// Nothing held.
    #[must_use]
    pub const fn new() -> Self {
        Self { keys: [None; MAX_HELD_KEYS], buttons: 0, contacts: 0, on: None, selected: None }
    }

    /// Takes one item of the stream; on `FOCUS_OUT` or a change of
    /// selection, gives everything held to `released` with the page that
    /// held it, and forgets it; on gone, what the pointer held.
    pub fn take(&mut self, item: &Item<'_>, released: &mut dyn FnMut(Released, Stamp)) {
        let Item::Event(event) = item else { return };
        match event {
            Event::Key(key) => match key.action {
                Action::Press | Action::Repeat => {
                    self.on = Some(key.on);
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
            Event::Mouse(mouse) => match mouse.action {
                MouseAction::Press if mouse.button < BUTTONS => {
                    self.on = Some(mouse.on);
                    self.buttons |= 1_u8 << mouse.button;
                }
                MouseAction::Release if mouse.button < BUTTONS => self.buttons &= !(1_u8 << mouse.button),
                // On gone `button` is the mask of those held: each is let go
                // without a click.
                MouseAction::Gone => {
                    for button in 0..BUTTONS {
                        let bit = 1_u8 << button;
                        if self.buttons & mouse.button & bit != 0 {
                            self.buttons &= !bit;
                            released(Released::Button(button), mouse.on);
                        }
                    }
                }
                _ => {}
            },
            Event::Touch(touch) => {
                let bit = 1_u16.checked_shl(u32::from(touch.id)).unwrap_or(0);
                match touch.action {
                    MouseAction::Press => {
                        self.on = Some(touch.on);
                        self.contacts |= bit;
                    }
                    MouseAction::Release => self.contacts &= !bit,
                    MouseAction::Gone if self.contacts & bit != 0 => {
                        self.contacts &= !bit;
                        released(Released::Contact(touch.id), touch.on);
                    }
                    _ => {}
                }
            }
            Event::FocusOut => self.release_all(released),
            Event::Select(select) if self.selected != Some(*select) => {
                self.selected = Some(*select);
                self.release_all(released);
            }
            _ => {}
        }
    }

    /// The connection was lost, the focus went, or the selection changed:
    /// each held key, button and contact to `released`, with the page that
    /// held it, and nothing held after.
    pub fn release_all(&mut self, released: &mut dyn FnMut(Released, Stamp)) {
        let on = self.on.take();
        let mut release = |what| {
            if let Some(on) = on {
                released(what, on);
            }
        };
        for slot in &mut self.keys {
            if let Some(key) = slot.take() {
                release(Released::Key(key));
            }
        }
        for button in 0..BUTTONS {
            if self.buttons & (1 << button) != 0 {
                release(Released::Button(button));
            }
        }
        self.buttons = 0;
        for id in 0..u16::BITS {
            if self.contacts & (1 << id) != 0 {
                release(Released::Contact(id as u8));
            }
        }
        self.contacts = 0;
    }

    /// Whether anything is held.
    #[must_use]
    pub fn any(&self) -> bool {
        self.buttons != 0 || self.contacts != 0 || self.keys.iter().any(Option::is_some)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
    use super::*;
    use crate::event::{Device, Mouse, Point, Touch, read_message};
    use std::vec::Vec;

    /// The stamp's bytes on page 100, sub-code 0000.
    const ON: Stamp = Stamp { page: 0x100, sub: 0 };

    fn released_by(stream: &[u8]) -> Vec<Released> {
        let mut held = Held::new();
        let mut released = Vec::new();
        read_message(stream, &mut |item| held.take(&item, &mut |each, _| released.push(each))).unwrap();
        held.release_all(&mut |each, _| released.push(each));
        released
    }

    #[test]
    fn focus_out_releases_what_is_held_and_nothing_else() {
        // Shift and `a` pressed, `a` released; the left button pressed;
        // then the focus goes (REQ-207).
        let stream = [
            0xFF, 0x01, 0x80, 0x02, 0x00, 0x04, 80, // Shift (ShiftLeft) pressed
            0xFF, 0x01, 0x80, 0x02, 0x00, 0x00, b'a', // a pressed
            0xFF, 0x01, 0x80, 0x02, 0x00, 0x02, b'a', // a released
            0xFF, 0x02, 0x80, 0x02, 0x00, 0x00, 0x00, 0x0A, 0x05, // left button pressed
            0xFF, 0x06, // FOCUS_OUT
        ];
        let mut held = Held::new();
        let mut released = Vec::new();
        read_message(&stream, &mut |item| held.take(&item, &mut |each, on| released.push((each, on)))).unwrap();
        assert_eq!(released, [(Released::Key(KeyCode::Functional(80)), ON), (Released::Button(0), ON)]);
        assert!(!held.any(), "nothing held after");
    }

    #[test]
    fn a_lost_connection_releases_too() {
        let stream = [0xFF, 0x01, 0x80, 0x02, 0x00, 0x04, 9, 0xFF, 0x01, 0x80, 0x02, 0x00, 0x05, 9];
        assert_eq!(released_by(&stream), [Released::Key(KeyCode::Functional(9))], "Up, pressed and repeating");
        assert_eq!(released_by(b"ab"), [], "text holds nothing");
    }

    /// REQ-207, REQ-RCH-046: a pointer gone, MOUSE's or a TOUCH's, lets go
    /// of what it held without a click, and a lost connection lets go of
    /// every contact down.
    #[test]
    fn gone_releases_what_the_pointer_held() {
        let stream = [
            0xFF, 0x02, 0x80, 0x02, 0x00, 0x00, 0x00, 0x0A, 0x05, // left button pressed
            0xFF, 0x02, 0x80, 0x02, 0x00, 0x00, 0x02, 0x0A, 0x05, // right button pressed
            0xFF, 0x02, 0x80, 0x02, 0x00, 0x03, 0x01, 0x0A, 0x05, // gone, the left held
            0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x00, 0x03, 0x01, 0x01, // TOUCH 3 down
            0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x00, 0x05, 0x01, 0x01, // TOUCH 5 down
            0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x03, 0x03, 0x01, 0x01, // TOUCH 3 gone
        ];
        let mut held = Held::new();
        let mut released = Vec::new();
        read_message(&stream, &mut |item| held.take(&item, &mut |each, _| released.push(each))).unwrap();
        assert_eq!(released, [Released::Button(0), Released::Contact(3)]);
        held.release_all(&mut |each, _| released.push(each));
        assert_eq!(released[2..], [Released::Button(2), Released::Contact(5)], "what is left, on a lost connection");
    }

    /// REQ-207 item 3: a change of selection releases keys, buttons and
    /// contacts, stamped with the page that held them; the same selection
    /// said again, at a session's start, releases nothing.
    #[test]
    fn a_change_of_selection_releases_everything() {
        let other = Stamp { page: 0x200, sub: 0 };
        let select = |page, sub| Item::Event(Event::Select(Select::new(page, sub).unwrap()));
        let at = Point { column: 1, row: 1, pixels: None, mods: 0 };
        let press = Mouse { on: ON, action: MouseAction::Press, button: 0, at, device: Device::Mouse };
        let contact = Touch { on: ON, action: MouseAction::Press, id: 2, column: 1, row: 1, pixels: None };
        let key = Key {
            on: ON,
            action: Action::Press,
            key: KeyCode::Character('q'),
            mods: 0,
            text: None,
            base: None,
            lost: false,
        };
        let mut held = Held::new();
        let mut released = Vec::new();
        held.take(&select(0x100, 0), &mut |each, on| released.push((each, on)));
        for event in [Event::Key(key), Event::Mouse(press), Event::Touch(contact)] {
            held.take(&Item::Event(event), &mut |each, on| released.push((each, on)));
        }
        held.take(&select(0x100, 0), &mut |each, on| released.push((each, on)));
        assert_eq!(released, [], "the same selection");
        held.take(&select(other.page, other.sub), &mut |each, on| released.push((each, on)));
        assert_eq!(
            released,
            [(Released::Key(KeyCode::Character('q')), ON), (Released::Button(0), ON), (Released::Contact(2), ON)],
            "stamped with the page that held them"
        );
        assert!(!held.any(), "releasing leaves nothing held");
    }

    #[test]
    fn a_lost_key_is_a_release_marked_lost() {
        let key = lost_key(KeyCode::Functional(9), ON);
        assert!(key.lost && key.action == Action::Release);
        let mut out = [0; 32];
        let length = crate::event::write(&Item::Event(Event::Key(key)), &mut out).unwrap();
        assert_eq!(out[..length], [0xFF, 0x01, 0x80, 0x02, 0x00, 0x46, 9], "the L flag, section 5.5");
    }

    #[test]
    fn presses_beyond_the_bound_are_not_tracked() {
        let mut held = Held::new();
        for character in ('a'..='z').take(MAX_HELD_KEYS + 2) {
            let key = Key {
                on: ON,
                action: Action::Press,
                key: KeyCode::Character(character),
                mods: 0,
                text: None,
                base: None,
                lost: false,
            };
            held.take(&Item::Event(Event::Key(key)), &mut |_, _| {});
        }
        let at = Point { column: 0, row: 0, pixels: None, mods: 0 };
        let mouse = Mouse { on: ON, action: MouseAction::Press, button: 200, at, device: Device::Mouse };
        held.take(&Item::Event(Event::Mouse(mouse)), &mut |_, _| {});
        let mut count = 0;
        held.release_all(&mut |_, _| count += 1);
        assert_eq!(count, MAX_HELD_KEYS, "the bound, and no button out of range");
    }
}
