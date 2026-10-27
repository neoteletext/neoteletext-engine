#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]

use super::terminal_io::{Wakened, next_event};
use super::typing::{host_page, input_of};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::sync::mpsc;
use std::time::{Duration, Instant as StdInstant};

#[test]
fn a_page_from_the_source_ends_the_wait_at_once() {
    // The viewer looked at the source only between 40 ms waits for the
    // terminal, so a page the fetcher had in 0.3 ms waited up to 40 ms
    // more to be drawn (REQ-INT-008).
    const LONG: Duration = Duration::from_secs(5);
    let (woken, wakes) = mpsc::channel();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(5));
        let _ = woken.send(Wakened::Source);
    });
    let started = StdInstant::now();
    assert!(next_event(&wakes, LONG).is_none());
    assert!(started.elapsed() < LONG / 10, "waited {:?}", started.elapsed());
}

fn sent(code: KeyCode, modifiers: KeyModifiers) -> Option<Vec<u8>> {
    let mut buffer = [0_u8; 4];
    let item = input_of(KeyEvent::new(code, modifiers), &mut buffer)?;
    let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
    let length = neoteletext_input::event::write(&item, &mut out).unwrap();
    Some(out[..length].to_vec())
}

/// REQ-INT-104 (amended by decision E.125): every key as KEY, a character
/// key unshifted with its text as typed, a Control combination and a named
/// key with no text; REQ-INT-116: a non-United-States layout's letters in
/// any script, and AltGr as text with no modifiers.
#[test]
fn a_terminal_key_is_sent_as_the_input_stream_has_it() {
    let none = KeyModifiers::NONE;
    assert_eq!(sent(KeyCode::Char('a'), none), Some(vec![0xFF, 0x01, 0x10, b'a', 1, b'a']));
    assert_eq!(
        sent(KeyCode::Char('A'), KeyModifiers::SHIFT),
        Some(vec![0xFF, 0x01, 0x18, b'a', 0x01, 1, b'A']),
        "the key unshifted, Shift in mods, the text as typed"
    );
    assert_eq!(sent(KeyCode::Char('c'), KeyModifiers::CONTROL), Some(vec![0xFF, 0x01, 0x08, b'c', 0x04]));
    assert_eq!(sent(KeyCode::Enter, none), Some(vec![0xFF, 0x01, 0x04, 0x02]));
    assert_eq!(sent(KeyCode::Up, KeyModifiers::SHIFT), Some(vec![0xFF, 0x01, 0x0C, 0x09, 0x01]));
    assert_eq!(sent(KeyCode::BackTab, none), Some(vec![0xFF, 0x01, 0x0C, 0x03, 0x01]), "Shift-Tab");
    // `key` is the code point as a varint, `text` the UTF-8 with its length.
    let typed = |character: char| {
        let mut bytes = vec![0xFF, 0x01, 0x10];
        let mut point = u32::from(character);
        while point >= 0x80 {
            bytes.push(u8::try_from(point & 0x7F).unwrap() | 0x80);
            point >>= 7;
        }
        bytes.push(u8::try_from(point).unwrap());
        bytes.push(u8::try_from(character.len_utf8()).unwrap());
        bytes.extend(character.to_string().as_bytes());
        bytes
    };
    assert_eq!(sent(KeyCode::Char('й'), none), Some(typed('й')), "Russian");
    assert_eq!(sent(KeyCode::Char('ש'), none), Some(typed('ש')), "Hebrew");
    assert_eq!(sent(KeyCode::Char('é'), none), Some(typed('é')), "French, composed by the terminal");
    let alt_gr = KeyModifiers::CONTROL | KeyModifiers::ALT;
    assert_eq!(sent(KeyCode::Char('@'), alt_gr), Some(typed('@')), "German AltGr-Q, as Windows reports it");
    assert_eq!(sent(KeyCode::Char('\u{1b}'), none), None, "a control character is not text");
}

/// INPUT.md section 9.6: with the Kitty keyboard protocol a key's repeat
/// types its text again and its release is a KEY with no text.
#[test]
fn repeats_and_releases_where_the_terminal_reports_them() {
    let event = |kind| KeyEvent::new_with_kind(KeyCode::Char('a'), KeyModifiers::NONE, kind);
    let encode = |key: KeyEvent| {
        let mut buffer = [0_u8; 4];
        let item = input_of(key, &mut buffer).unwrap();
        let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
        let length = neoteletext_input::event::write(&item, &mut out).unwrap();
        out[..length].to_vec()
    };
    assert_eq!(encode(event(KeyEventKind::Repeat)), vec![0xFF, 0x01, 0x11, b'a', 1, b'a']);
    assert_eq!(encode(event(KeyEventKind::Release)), vec![0xFF, 0x01, 0x02, b'a'], "a release, no text");
    let up = KeyEvent::new_with_kind(KeyCode::Up, KeyModifiers::NONE, KeyEventKind::Release);
    assert_eq!(encode(up), vec![0xFF, 0x01, 0x06, 0x09]);
}

/// REQ-INT-113: a cursor record naming another page opens it, only
/// while keys go to the channel.
#[test]
fn a_host_goes_to_a_page_by_naming_it() {
    assert_eq!(host_page(0x301, 0x300, true), Some(0x301));
    assert_eq!(host_page(0x300, 0x300, true), None, "the page already open");
    assert_eq!(host_page(0x301, 0x300, false), None, "keys do not go to the channel");
}
