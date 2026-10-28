//! Numbering the contacts on a touch screen (`neoteletext-common/INPUT.md`
//! section 9.8, REQ-INT-124; REQ-RCH-041, REQ-RCH-043 and REQ-RCH-053): the first contact
//! down goes as MOUSE, contact 0; each further one as TOUCH, with the
//! lowest `id` of 1 to 9 free as it begins, kept until it ends; at most
//! [`MAX_CONTACTS`] at once, a further one sending nothing until one ends.
//! The system's own identifier of a contact (winit's `Touch::id`, iOS's
//! `UITouch`, Android's pointer id) is mapped to its number here, by the
//! trusted part of a receiver, so the drawing process cannot invent one.

use crate::MAX_CONTACTS;

/// The contact that goes as MOUSE.
pub const MOUSE_CONTACT: u8 = 0;

/// The contacts down, by number.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Contacts {
    /// The system's identifier of each contact down, by its number.
    down: [Option<u64>; MAX_CONTACTS],
}

impl Contacts {
    /// No contact down.
    #[must_use]
    pub const fn new() -> Self {
        Self { down: [None; MAX_CONTACTS] }
    }

    /// The contact the system calls `system` began: its number. It is
    /// [`MOUSE_CONTACT`] when no other contact is down and no mouse button
    /// is held (`mouse_held`), so the mouse and a finger never share MOUSE
    /// (REQ-RCH-041); otherwise, when `touch` (both sides set TOUCH), the
    /// lowest free of 1 to 9. `None` when none is free, without TOUCH, or
    /// when `system` is already down: the begin is refused, and sends
    /// nothing.
    pub fn begin(&mut self, system: u64, mouse_held: bool, touch: bool) -> Option<u8> {
        if self.number(system).is_some() {
            return None;
        }
        let first = !mouse_held && self.down.iter().all(Option::is_none);
        let number = if first {
            Some(usize::from(MOUSE_CONTACT))
        } else if touch {
            self.down.iter().enumerate().skip(1).find(|(_, slot)| slot.is_none()).map(|(number, _)| number)
        } else {
            None
        }?;
        *self.down.get_mut(number)? = Some(system);
        u8::try_from(number).ok()
    }

    /// The number of the contact the system calls `system`, while it is
    /// down: `None` refuses a move or an end for a contact not begun.
    #[must_use]
    pub fn number(&self, system: u64) -> Option<u8> {
        self.down.iter().position(|slot| *slot == Some(system)).and_then(|number| u8::try_from(number).ok())
    }

    /// The contact the system calls `system` ended, lifted or cancelled:
    /// its number, now free.
    pub fn end(&mut self, system: u64) -> Option<u8> {
        let number = self.number(system)?;
        *self.down.get_mut(usize::from(number))? = None;
        Some(number)
    }

    /// How many contacts are down.
    #[must_use]
    pub fn down(&self) -> usize {
        self.down.iter().filter(|slot| slot.is_some()).count()
    }

    /// Every contact ended at once: keys stopped going to the channel, or
    /// the system cancelled them all.
    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use super::*;

    /// REQ-INT-124, REQ-RCH-043: ten contacts beginning and ending in turn take the
    /// lowest free number, keep it until they end, and an eleventh sends
    /// nothing until one ends.
    #[test]
    fn contacts_take_the_lowest_free_number() {
        let mut contacts = Contacts::new();
        let numbers: std::vec::Vec<_> = (100..110).map(|system| contacts.begin(system, false, true)).collect();
        assert_eq!(numbers, (0..10).map(Some).collect::<std::vec::Vec<_>>(), "the first is MOUSE's, then 1 to 9");
        assert_eq!(contacts.begin(200, false, true), None, "an eleventh: nothing");
        assert_eq!(contacts.end(104), Some(4));
        assert_eq!(contacts.end(102), Some(2));
        assert_eq!(contacts.begin(201, false, true), Some(2), "the lowest free");
        assert_eq!(contacts.begin(202, false, true), Some(4));
        assert_eq!(contacts.number(107), Some(7), "kept until it ends");
        assert_eq!(contacts.end(100), Some(0));
        assert_eq!(contacts.begin(203, false, true), None, "contact 0 only when none is down; 1 to 9 all taken");
        for system in [101, 103, 105, 106, 107, 108, 109, 201, 202] {
            contacts.end(system).unwrap();
        }
        assert_eq!(contacts.down(), 0);
        assert_eq!(contacts.begin(204, false, true), Some(MOUSE_CONTACT), "once all lift, the next is MOUSE's");
    }

    /// REQ-INT-124, REQ-RCH-041, REQ-RCH-053: a contact while a mouse button is held is
    /// TOUCH's; without TOUCH only the first goes; a contact not begun, or
    /// begun twice, is refused.
    #[test]
    fn the_mouse_and_a_finger_never_share_mouse_and_inventions_are_refused() {
        let mut contacts = Contacts::new();
        assert_eq!(contacts.begin(1, true, true), Some(1), "a button held: TOUCH");
        assert_eq!(contacts.begin(1, false, true), None, "already down");
        let mut plain = Contacts::new();
        assert_eq!(plain.begin(1, false, false), Some(MOUSE_CONTACT));
        assert_eq!(plain.begin(2, false, false), None, "no TOUCH: a second finger sends nothing");
        assert_eq!(plain.number(9), None, "a contact not begun");
        assert_eq!(plain.end(9), None);
    }
}
