#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]

use super::*;
use crate::MAX_MESSAGE;
use crate::caps::AGENT;
use crate::caps::{ECHO_CAP, PAGE_CAP};
use crate::keys::code;
use std::vec::Vec;

fn bytes(item: &Item<'_>) -> Vec<u8> {
    let mut out = [0; MAX_MESSAGE];
    let length = write(item, &mut out).unwrap();
    out[..length].to_vec()
}

fn key(action: Action, key: KeyCode, mods: u8) -> Item<'static> {
    Item::Event(Event::Key(Key { action, key, mods, text: None, base: None }))
}

// REQ-INT-101.
#[test]
fn the_size_examples_of_section_5_8() {
    let functional = |name: &str| KeyCode::Functional(code(name).unwrap());
    assert_eq!(bytes(&Item::Text("a")), [0x61]);
    assert_eq!(bytes(&Item::Text("é")), [0xC3, 0xA9]);
    assert_eq!(bytes(&Item::Event(Event::FocusOut)), [0xFF, 0x06]);
    assert_eq!(bytes(&key(Action::Press, functional("Enter"), 0)), [0xFF, 0x01, 0x04, 0x02]);
    assert_eq!(bytes(&key(Action::Press, functional("ArrowUp"), 0)), [0xFF, 0x01, 0x04, 0x09]);
    assert_eq!(bytes(&key(Action::Press, functional("ColorF0Red"), 0)), [0xFF, 0x01, 0x04, 0x5C]);
    assert_eq!(bytes(&key(Action::Press, KeyCode::Character('c'), CONTROL)), [0xFF, 0x01, 0x08, 0x63, 0x04]);
    assert_eq!(bytes(&Item::Event(Event::Page(0x100))), [0xFF, 0x0A, 0x80, 0x02]);
    let at = Point { column: 10, row: 5, pixels: None, mods: 0 };
    assert_eq!(
        bytes(&Item::Event(Event::Mouse(Mouse { action: MouseAction::Press, button: 0, at }))),
        [0xFF, 0x02, 0x00, 0x00, 0x0A, 0x05]
    );
    assert_eq!(
        bytes(&Item::Event(Event::Wheel(Wheel { dx: 0, dy: 120, at }))),
        [0xFF, 0x03, 0x00, 0x00, 0xF0, 0x01, 0x0A, 0x05]
    );
}

// REQ-INT-103.
#[test]
fn every_event_round_trips() {
    let at = Point { column: 71, row: 24, pixels: Some((960, 1000)), mods: SHIFT | CAPS_LOCK };
    let events = [
        Event::Key(Key {
            action: Action::Release,
            key: KeyCode::Character('я'),
            mods: ALT,
            text: Some("Я"),
            base: Some('z'),
        }),
        Event::Key(Key {
            action: Action::Repeat,
            key: KeyCode::Functional(code("Dead").unwrap()),
            mods: 0,
            text: None,
            base: Some('\''),
        }),
        Event::Mouse(Mouse { action: MouseAction::Move, button: 0b101, at }),
        Event::Wheel(Wheel { dx: -240, dy: 1, at }),
        Event::Paste("line one\nline two\t\u{7}"),
        Event::FocusIn,
        Event::Resize { columns: 40, rows: 25, width: 480, height: 500 },
        Event::Compose { cursor: 2, text: "にほん" },
        Event::Compose { cursor: 0, text: "" },
        Event::Hello { version: 2, caps: PAGE_CAP | ECHO_CAP, agent: None },
        Event::Hello { version: 2, caps: AGENT, agent: Some("NeoTeletext/0.1 (macOS)") },
        Event::Page(0),
        Event::Page(0x8FF),
        Event::Extension { kind: 0x41, payload: &[1, 2, 3] },
    ];
    for event in events {
        let item = Item::Event(event);
        let encoded = bytes(&item);
        assert_eq!(read(&encoded), Ok(Some((item, encoded.len()))), "{event:?}");
    }
}

// REQ-INT-102.
#[test]
fn what_section_5_refuses_is_refused() {
    let refuse = |bytes: &[u8]| read(bytes).is_err();
    assert!(refuse(&[0x07]), "a control character as text");
    assert!(refuse(&[0xC0, 0x80]), "a byte UTF-8 never uses");
    assert!(refuse(&[0xF5]), "another");
    assert!(refuse(&[0xFF, 0x00]), "reserved type 0");
    assert!(refuse(&[0xFF, 0x0B]), "an undefined core type");
    assert!(refuse(&[0xFF, 0x80]), "reserved type 0x80");
    assert!(refuse(&[0xFF, 0x01, 0x40, 0x61]), "a reserved key flag");
    assert!(refuse(&[0xFF, 0x01, 0x03, 0x61]), "key action 3");
    assert!(refuse(&[0xFF, 0x01, 0x00, 0x0D]), "a control character as a key");
    assert!(refuse(&[0xFF, 0x01, 0x04, 0x38]), "a functional code not in use");
    assert!(refuse(&[0xFF, 0x01, 0x08, 0x61, 0x00]), "mods present and empty");
    assert!(refuse(&[0xFF, 0x02, 0x00, 0x00, 0x48, 0x00]), "column 72");
    assert!(refuse(&[0xFF, 0x03, 0x01, 0x00, 0x00, 0x00, 0x00]), "a wheel with an action");
    assert!(refuse(&[0xFF, 0x07, 0x00, 0x19, 0x00, 0x00]), "no columns");
    assert!(refuse(&[0xFF, 0x08, 0x05, 0x02, 0x61, 0x62]), "a cursor past the preedit");
    assert!(refuse(&[0xFF, 0x0A, 0x80, 0x01]), "page 0x080");
    let mut long = std::vec![0xFF, PASTE];
    long.extend_from_slice(&[0xA1, 0x1F]); // 4001
    long.extend(std::iter::repeat_n(b'a', 4001));
    assert!(refuse(&long), "a paste over its bound");
    assert_eq!(read(&[0xFF, 0x01]), Err(InputError::Wire(WireError::Incomplete)));
}

// REQ-INT-101.
#[test]
fn a_message_holds_whole_items_and_text_runs_to_the_next_event() {
    let mut message = Vec::new();
    message.extend_from_slice(b"*100#");
    message.extend_from_slice(&bytes(&key(Action::Press, KeyCode::Functional(code("Enter").unwrap()), 0)));
    message.extend_from_slice("ok é".as_bytes());
    let mut items = Vec::new();
    read_message(&message, &mut |item| items.push(std::format!("{item:?}"))).unwrap();
    assert_eq!(items.len(), 3);
    assert!(items[0].contains("*100#") && items[2].contains("ok é"));
    assert_eq!(read_message(&[0xFF, 0x01, 0x04], &mut |_| {}), Err(InputError::Message), "cut by the end");
    assert_eq!(read_message(&[b'a'; MAX_MESSAGE + 1], &mut |_| {}), Err(InputError::Message), "over the bound");
}

// REQ-INT-101.
#[test]
fn a_stream_split_anywhere_decodes_alike() {
    let mut stream = Vec::new();
    stream.extend_from_slice("héllo".as_bytes());
    stream.extend_from_slice(&bytes(&Item::Event(Event::Paste("pasted"))));
    stream.extend_from_slice(&bytes(&key(Action::Press, KeyCode::Character('x'), CONTROL)));
    for split in 0..stream.len() {
        let mut decoder = StreamDecoder::<MAX_STREAM_ITEM>::default();
        let mut text = std::string::String::new();
        let mut events = 0;
        let mut take = |item: Item<'_>| match item {
            Item::Text(run) => text.push_str(run),
            Item::Event(_) => events += 1,
        };
        decoder.push(&stream[..split], &mut take).unwrap();
        decoder.push(&stream[split..], &mut take).unwrap();
        assert_eq!((text.as_str(), events), ("héllo", 2), "split at {split}");
    }
}

// REQ-INT-110.
#[test]
fn the_input_count_counts_characters_and_events() {
    assert_eq!(units(&Item::Text("héllo")), 5);
    assert_eq!(units(&Item::Event(Event::Paste("a long paste"))), 1);
    assert_eq!(units(&Item::Event(Event::FocusIn)), 1);
}

/// Decision E.129: HOVER is the extension `0x43`, `flags`, `col` and `row`;
/// it reads back, and a bad flag, a cell off the page or bytes after are
/// refused. A receiver that does not know `0x43` skips it as any extension.
#[test]
fn hover_round_trips_and_is_checked() {
    for event in [Event::Hover { leave: false, column: 2, row: 20 }, Event::Hover { leave: true, column: 39, row: 23 }]
    {
        let mut out = [0_u8; 32];
        let length = write(&Item::Event(event), &mut out).unwrap();
        let mut back = Vec::new();
        read_message(&out[..length], &mut |item| back.push(item)).unwrap();
        assert_eq!(back, [Item::Event(event)]);
    }
    let mut written = [0_u8; 32];
    let length = write(&Item::Event(Event::Hover { leave: true, column: 2, row: 20 }), &mut written).unwrap();
    assert_eq!(&written[..length], &[0xFF, 0x43, 3, 1, 2, 20], "the type, the length, then the payload");
    for bad in [[0xFF, 0x43, 3, 2, 2, 20].as_slice(), &[0xFF, 0x43, 3, 0, 72, 20], &[0xFF, 0x43, 4, 0, 2, 20, 0]] {
        assert!(read_message(bad, &mut |_| {}).is_err(), "{bad:?}");
    }
}
