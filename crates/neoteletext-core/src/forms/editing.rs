//! Editing a form from a keyboard or a remote control (REQ-NAV-009): focus,
//! entry, choices and the multi-tap keypad.

use super::spec::{FieldKind, FieldSpec};
use super::validation::Invalid;
use super::{FormModel, MULTITAP_US, VALUE_CHARS};
use crate::charset::{CharRef, CharSet, to_unicode};
use crate::consts::{CODE_MASK_7BIT, SPACE};
use crate::types::Instant;

/// Keys a form understands (REQ-NAV-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormKey {
    /// Tab: next field.
    Tab,
    /// Shift-Tab: previous field.
    BackTab,
    /// A typed character.
    Char(char),
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow: previous choice.
    Up,
    /// Down arrow: next choice.
    Down,
    /// Space bar: toggle a checkbox, or type a space.
    Space,
    /// Enter: press the focused button.
    Enter,
    /// Backspace.
    Backspace,
    /// A digit key on a remote control.
    RemoteDigit(u8),
    /// Red: previous field.
    Red,
    /// Green: next field.
    Green,
    /// Yellow: change the focused field.
    Yellow,
    /// Cyan: press the focused button or submit.
    Cyan,
    /// Index: leave the form.
    Index,
}

/// What a key did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormAction {
    /// Nothing more to do.
    None,
    /// The character cannot be shown by the page's set and was refused.
    Refused,
    /// Submit, naming the button pressed, if any.
    Submit(Option<usize>),
    /// Validation failed at a field; focus moved there.
    Invalid(usize, Invalid),
    /// Leave the form without submitting.
    Leave,
}

/// Remote text entry mode (REQ-NAV-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryMode {
    /// Digit keys give letters, telephone style.
    Letters,
    /// Digit keys give digits.
    Digits,
}

/// ITU E.161 keypad letters (decision 12 of IMPLEMENTATION-LOG.md).
const KEYPAD: [&[u8]; 10] =
    [b" 0", b".,-@_1", b"ABC2", b"DEF3", b"GHI4", b"JKL5", b"MNO6", b"PQRS7", b"TUV8", b"WXYZ9"];

/// The code drawn in a checked CHECKBOX and at the chosen RADIO choice:
/// "*" at 2/A, which is not a national option position, as Appendix C.4
/// recommends for markers.
pub(super) const MARKER: u8 = b'*';

impl FormModel {
    fn move_focus(&mut self, forward: bool) {
        let count = self.spec.fields.len();
        if count == 0 {
            return;
        }
        self.focus = if forward {
            self.focus.saturating_add(1).checked_rem(count).unwrap_or(0)
        } else {
            self.focus.checked_sub(1).unwrap_or(count.saturating_sub(1))
        };
        self.cursor = self.value(self.focus).len();
        self.multitap = None;
    }

    /// The G0 code that shows `c`, if the page's set has one.
    fn code_for(&self, c: char) -> Option<u8> {
        (SPACE..CODE_MASK_7BIT).find(|&code| {
            let unicode = to_unicode(CharRef::new(CharSet::G0(self.g0), code));
            unicode.base == c && unicode.combining.is_none()
        })
    }

    fn insert(&mut self, c: char) -> FormAction {
        let Some(field) = self.field().copied() else { return FormAction::None };
        if !field.is_textual() {
            return FormAction::None;
        }
        let Some(code) = self.code_for(c) else { return FormAction::Refused };
        let limit = if field.maxlen == 0 { VALUE_CHARS } else { usize::from(field.maxlen).min(VALUE_CHARS) };
        let cursor = self.cursor;
        let Some(value) = self.values.get_mut(self.focus) else { return FormAction::None };
        if value.len() >= limit {
            return FormAction::Refused;
        }
        if value.push(code) {
            let slice = value.as_mut_slice();
            let len = slice.len();
            // A rotation of an empty slice panics (REQ-GEN-006).
            if let Some(tail) = slice.get_mut(cursor.min(len.saturating_sub(1))..).filter(|tail| !tail.is_empty()) {
                tail.rotate_right(1);
            }
            self.cursor = cursor.saturating_add(1);
        }
        FormAction::None
    }

    fn backspace(&mut self) {
        let Some(at) = self.cursor.checked_sub(1) else { return };
        if let Some(value) = self.values.get_mut(self.focus) {
            let _ = value.remove(at);
            self.cursor = at;
        }
    }

    fn step_choice(&mut self, forward: bool) {
        let Some(field) = self.field().copied() else { return };
        let count = field.option_count;
        if count == 0 {
            return;
        }
        if let Some(choice) = self.choice.get_mut(self.focus) {
            *choice = if forward {
                choice.saturating_add(1).checked_rem(count).unwrap_or(0)
            } else {
                choice.checked_sub(1).unwrap_or(count.saturating_sub(1))
            };
        }
    }

    fn toggle(&mut self) {
        let bit = 1u64.checked_shl(self.focus as u32).unwrap_or(0);
        self.checked ^= bit;
    }

    fn press(&mut self) -> FormAction {
        match self.field().map(|field| field.kind) {
            Some(FieldKind::Submit) => self.submit(Some(self.focus)),
            Some(FieldKind::Reset) => {
                self.reset();
                FormAction::None
            }
            _ => self.submit(None),
        }
    }

    fn submit(&mut self, button: Option<usize>) -> FormAction {
        match self.validate() {
            Ok(()) => FormAction::Submit(button),
            Err((index, reason)) => {
                self.focus = index;
                self.cursor = self.value(index).len();
                FormAction::Invalid(index, reason)
            }
        }
    }

    /// Handles a key (REQ-NAV-009).
    pub fn key(&mut self, key: FormKey, now: Instant) -> FormAction {
        if !matches!(key, FormKey::RemoteDigit(_)) {
            self.multitap = None;
        }
        let kind = self.field().map(|field| field.kind);
        match key {
            FormKey::Tab | FormKey::Green => {
                self.move_focus(true);
                FormAction::None
            }
            FormKey::BackTab | FormKey::Red => {
                self.move_focus(false);
                FormAction::None
            }
            FormKey::Index => FormAction::Leave,
            FormKey::Cyan | FormKey::Enter => self.press(),
            FormKey::Char(c) => self.insert(c),
            FormKey::Space => match kind {
                Some(FieldKind::Checkbox) => {
                    self.toggle();
                    FormAction::None
                }
                _ => self.insert(' '),
            },
            FormKey::Backspace => {
                self.backspace();
                FormAction::None
            }
            FormKey::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                FormAction::None
            }
            FormKey::Right => {
                self.cursor = self.cursor.saturating_add(1).min(self.value(self.focus).len());
                FormAction::None
            }
            FormKey::Up | FormKey::Down => {
                self.step_choice(key == FormKey::Down);
                FormAction::None
            }
            FormKey::Yellow => {
                match kind {
                    Some(FieldKind::Select | FieldKind::Radio) => self.step_choice(true),
                    Some(FieldKind::Checkbox) => self.toggle(),
                    _ => {
                        self.mode = match self.mode {
                            EntryMode::Letters => EntryMode::Digits,
                            EntryMode::Digits => EntryMode::Letters,
                        };
                    }
                }
                FormAction::None
            }
            FormKey::RemoteDigit(digit) => self.remote_digit(digit, now),
        }
    }

    fn remote_digit(&mut self, digit: u8, now: Instant) -> FormAction {
        let Some(letters) = KEYPAD.get(usize::from(digit)) else { return FormAction::None };
        if self.mode == EntryMode::Digits || !self.field().is_some_and(FieldSpec::is_textual) {
            self.multitap = None;
            return self.insert(char::from(b'0'.saturating_add(digit)));
        }
        match self.multitap {
            Some((last, index, at)) if last == digit && now.since(at) < MULTITAP_US => {
                // Cycle: replace the character just typed with the next.
                let next = index.saturating_add(1).checked_rem(letters.len()).unwrap_or(0);
                self.backspace();
                let c = letters.get(next).copied().map_or(' ', char::from);
                self.multitap = Some((digit, next, now));
                self.insert(c)
            }
            _ => {
                let c = letters.first().copied().map_or(' ', char::from);
                self.multitap = Some((digit, 0, now));
                self.insert(c)
            }
        }
    }
}
