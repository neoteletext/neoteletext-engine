//! The viewer's keys: a terminal key as a viewer key, and the key that
//! leaves a channel's keys.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use neoteletext_core::viewer::Key;

/// The key that leaves a channel's keys, as telnet's escape character
/// leaves its host's: Control and the right square bracket.
pub(super) const LEAVE_TYPING: char = ']';

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
