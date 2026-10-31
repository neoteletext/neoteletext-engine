//! The viewer's keys: a terminal key as a viewer key, and the key that
//! leaves a channel's keys.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use neoteletext_core::viewer::Key;

/// The key that leaves a channel's keys, as telnet's escape character
/// leaves its host's: Control and the right square bracket.
const LEAVE_TYPING: char = ']';

/// The same key on a terminal without the Kitty keyboard protocol: it sends
/// the control byte 0x1D, which crossterm reads as Control and 5, naming
/// 0x1C to 0x1F Control-4 to Control-7. Control and 5 itself cannot be told
/// from it there, and leaves on every terminal.
const LEAVE_TYPING_AS_READ: char = '5';

/// Whether `key` leaves a channel's keys, or takes them back.
pub(super) fn leaves_typing(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(LEAVE_TYPING | LEAVE_TYPING_AS_READ))
        && key.kind != KeyEventKind::Release
}

pub(super) fn map_key(key: KeyEvent) -> Option<Key> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    Some(match key.code {
        KeyCode::Char(c @ '0'..='9') => Key::Digit((c as u8).saturating_sub(b'0')),
        KeyCode::Char('r') | KeyCode::F(1) => Key::Red,
        KeyCode::Char('g') | KeyCode::F(2) => Key::Green,
        KeyCode::Char('y') | KeyCode::F(3) => Key::Yellow,
        KeyCode::Char('c') | KeyCode::F(4) => Key::Cyan,
        KeyCode::Char('i') => Key::Index,
        KeyCode::Char('h') | KeyCode::Home => Key::Home,
        KeyCode::Up | KeyCode::Char('+') => Key::NextPage,
        KeyCode::Down | KeyCode::Char('-') => Key::PreviousPage,
        KeyCode::Left if alt => Key::Back,
        KeyCode::Right if alt => Key::Forward,
        KeyCode::Right => Key::NextSubPage,
        KeyCode::Left => Key::PreviousSubPage,
        KeyCode::Char('?') => Key::Reveal,
        KeyCode::Char('H') => Key::Hold,
        KeyCode::Char('m') => Key::Mix,
        KeyCode::Char('z') => Key::Zoom,
        KeyCode::Backspace => Key::Back,
        _ => return None,
    })
}
