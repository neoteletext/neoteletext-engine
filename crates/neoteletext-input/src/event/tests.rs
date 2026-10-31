#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]

use super::*;
use crate::MAX_MESSAGE;
use crate::caps::AGENT;
use crate::caps::{CURSOR_CAP, RETIRED_PAGE_BIT};
use crate::keys::code;
use std::vec::Vec;

/// The stamp of INPUT.md section 5.8's examples: page 100, sub-code 0000.
const ON: Stamp = Stamp { page: 0x100, sub: 0 };

fn bytes(item: &Item<'_>) -> Vec<u8> {
    let mut out = [0; MAX_MESSAGE];
    let length = write(item, &mut out).unwrap();
    out[..length].to_vec()
}

fn key(action: Action, key: KeyCode, mods: u8) -> Item<'static> {
    Item::Event(Event::Key(Key { on: ON, lost: false, action, key, mods, text: None, base: None }))
}

// REQ-INT-101; REQ-RCH-029 and REQ-RCH-055 (decision E.133): every row of
// INPUT.md section 5.8, as the encoder writes it, KEY for every key.
#[test]
fn the_size_examples_of_section_5_8() {
    let functional = |name: &str| KeyCode::Functional(code(name).unwrap());
    let press_a = Item::Event(Event::Key(Key {
        on: ON,
        lost: false,
        action: Action::Press,
        key: KeyCode::Character('a'),
        mods: 0,
        text: Some("a"),
        base: None,
    }));
    assert_eq!(bytes(&press_a), [0xFF, 0x01, 0x80, 0x02, 0x00, 0x10, 0x61, 0x01, 0x61]);
    assert_eq!(bytes(&key(Action::Release, KeyCode::Character('a'), 0)), [0xFF, 0x01, 0x80, 0x02, 0x00, 0x02, 0x61]);
    assert_eq!(bytes(&Item::Text("é")), [0xC3, 0xA9]);
    assert_eq!(bytes(&Item::Event(Event::FocusOut)), [0xFF, 0x06]);
    assert_eq!(bytes(&key(Action::Press, functional("Enter"), 0)), [0xFF, 0x01, 0x80, 0x02, 0x00, 0x04, 0x02]);
    assert_eq!(bytes(&key(Action::Press, functional("ArrowUp"), 0)), [0xFF, 0x01, 0x80, 0x02, 0x00, 0x04, 0x09]);
    assert_eq!(bytes(&key(Action::Press, functional("ColorF0Red"), 0)), [0xFF, 0x01, 0x80, 0x02, 0x00, 0x04, 0x5C]);
    assert_eq!(
        bytes(&key(Action::Press, KeyCode::Character('c'), CONTROL)),
        [0xFF, 0x01, 0x80, 0x02, 0x00, 0x08, 0x63, 0x04]
    );
    assert_eq!(
        bytes(&Item::Event(Event::Select(Select { page: 0x100, sub: 0x3F7F }))),
        [0xFF, 0x40, 0x04, 0x80, 0x02, 0xFF, 0x7E]
    );
    let at = Point { column: 10, row: 5, pixels: None, mods: 0 };
    let dot = |x| Point { column: 10, row: 5, pixels: Some((x, 100)), mods: 0 };
    let mouse = |action, button, at, device| Item::Event(Event::Mouse(Mouse { on: ON, action, button, at, device }));
    assert_eq!(
        bytes(&mouse(MouseAction::Press, 0, at, Device::Mouse)),
        [0xFF, 0x02, 0x80, 0x02, 0x00, 0x00, 0x00, 0x0A, 0x05]
    );
    assert_eq!(
        bytes(&mouse(MouseAction::Press, 0, dot(120), Device::Mouse)),
        [0xFF, 0x02, 0x80, 0x02, 0x00, 0x04, 0x00, 0x0A, 0x05, 0x78, 0x64]
    );
    assert_eq!(
        bytes(&mouse(MouseAction::Move, 0, dot(121), Device::Mouse)),
        [0xFF, 0x02, 0x80, 0x02, 0x00, 0x06, 0x00, 0x0A, 0x05, 0x79, 0x64]
    );
    assert_eq!(
        bytes(&mouse(MouseAction::Gone, 0, at, Device::Mouse)),
        [0xFF, 0x02, 0x80, 0x02, 0x00, 0x03, 0x00, 0x0A, 0x05]
    );
    assert_eq!(
        bytes(&Item::Event(Event::Wheel(Wheel { on: ON, dx: 0, dy: 120, at }))),
        [0xFF, 0x03, 0x80, 0x02, 0x00, 0x00, 0x00, 0xF0, 0x01, 0x0A, 0x05]
    );
    assert_eq!(
        bytes(&Item::Event(Event::Hover { on: ON, leave: false, column: 2, row: 20 })),
        [0xFF, 0x43, 0x06, 0x80, 0x02, 0x00, 0x00, 0x02, 0x14]
    );
    // Touch (REQ-RCH-055): a tap, a second finger, two fingers moving.
    assert_eq!(
        bytes(&mouse(MouseAction::Press, 0, at, Device::Touch)),
        [0xFF, 0x02, 0x80, 0x02, 0x00, 0x10, 0x00, 0x0A, 0x05]
    );
    let second =
        |action, pixels| Item::Event(Event::Touch(Touch { on: ON, action, id: 1, column: 11, row: 5, pixels }));
    assert_eq!(bytes(&second(MouseAction::Press, None)), [0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x00, 0x01, 0x0B, 0x05]);
    assert_eq!(
        bytes(&mouse(MouseAction::Move, 1, dot(121), Device::Touch)),
        [0xFF, 0x02, 0x80, 0x02, 0x00, 0x16, 0x01, 0x0A, 0x05, 0x79, 0x64]
    );
    assert_eq!(
        bytes(&second(MouseAction::Move, Some((133, 100)))),
        [0xFF, 0x44, 0x0A, 0x80, 0x02, 0x00, 0x06, 0x01, 0x0B, 0x05, 0x85, 0x01, 0x64]
    );
    let resume = bytes(&Item::Event(Event::Resume(Resume { token: [0; crate::TOKEN_BYTES], acked: 0 })));
    assert_eq!((resume.len(), &resume[..3]), (21, [0xFF, 0x45, 0x12].as_slice()));
}

// REQ-INT-103.
#[test]
fn every_event_round_trips() {
    let at = Point { column: 39, row: 24, pixels: Some((960, 1000)), mods: SHIFT | CAPS_LOCK };
    let events = [
        Event::Key(Key {
            on: ON,
            lost: false,
            action: Action::Release,
            key: KeyCode::Character('я'),
            mods: ALT,
            text: Some("Я"),
            base: Some('z'),
        }),
        Event::Key(Key {
            on: ON,
            lost: false,
            action: Action::Repeat,
            key: KeyCode::Functional(code("Dead").unwrap()),
            mods: 0,
            text: None,
            base: Some('\''),
        }),
        Event::Mouse(Mouse { on: ON, action: MouseAction::Move, button: 0b101, at, device: Device::Mouse }),
        Event::Mouse(Mouse { on: ON, action: MouseAction::Gone, button: 1, at, device: Device::Touch }),
        Event::Mouse(Mouse { on: ON, action: MouseAction::Press, button: 2, at, device: Device::Pen }),
        Event::Touch(Touch { on: ON, action: MouseAction::Move, id: 9, column: 39, row: 24, pixels: Some((479, 499)) }),
        Event::Touch(Touch { on: ON, action: MouseAction::Gone, id: 1, column: 0, row: 0, pixels: None }),
        Event::Resume(Resume { token: [0xA5; crate::TOKEN_BYTES], acked: 0xFFFE }),
        Event::Wheel(Wheel { on: ON, dx: -240, dy: 1, at }),
        Event::Paste { on: ON, text: "line one\nline two\t\u{7}" },
        Event::FocusIn,
        Event::Resize { columns: 40, rows: 25, width: 480, height: 500 },
        Event::Compose { on: ON, cursor: 2, text: "にほん" },
        Event::Compose { on: ON, cursor: 0, text: "" },
        Event::Hello { version: crate::VERSION, caps: CURSOR_CAP, agent: None },
        Event::Hello { version: crate::VERSION, caps: AGENT, agent: Some("NeoTeletext/0.1 (macOS)") },
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
    assert!(refuse(&[0xFF, 0x0A]), "an undefined core type");
    assert!(refuse(&[0xFF, 0x80]), "reserved type 0x80");
    assert!(refuse(&[0xFF, 0x01, 0x80, 0x02, 0x00, 0x40, 0x61]), "a reserved key flag");
    assert!(refuse(&[0xFF, 0x01, 0x80, 0x02, 0x00, 0x03, 0x61]), "key action 3");
    assert!(refuse(&[0xFF, 0x01, 0x80, 0x02, 0x00, 0x00, 0x0D]), "a control character as a key");
    assert!(refuse(&[0xFF, 0x01, 0x80, 0x02, 0x00, 0x04, 0x38]), "a functional code not in use");
    assert!(refuse(&[0xFF, 0x01, 0x80, 0x02, 0x00, 0x08, 0x61, 0x00]), "mods present and empty");
    assert!(refuse(&[0xFF, 0x02, 0x80, 0x02, 0x00, 0x00, 0x00, 0x28, 0x00]), "column 40: a page is always 40 by 25");
    assert!(refuse(&[0xFF, 0x03, 0x80, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00]), "a wheel with an action");
    assert!(refuse(&[0xFF, 0x07, 0x00, 0x19, 0x00, 0x00]), "no columns");
    assert!(refuse(&[0xFF, 0x08, 0x80, 0x02, 0x00, 0x05, 0x02, 0x61, 0x62]), "a cursor past the preedit");
    assert!(refuse(&[0xFF, 0x0A, 0x80, 0x02]), "PAGE, withdrawn: 0x0A is unused");
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
    assert_eq!(
        read_message(&[0xFF, 0x01, 0x80, 0x02, 0x00, 0x04], &mut |_| {}),
        Err(InputError::Message),
        "cut by the end"
    );
    assert_eq!(read_message(&[b'a'; MAX_MESSAGE + 1], &mut |_| {}), Err(InputError::Message), "over the bound");
}

// REQ-INT-101.
#[test]
fn a_stream_split_anywhere_decodes_alike() {
    let mut stream = Vec::new();
    stream.extend_from_slice("héllo".as_bytes());
    stream.extend_from_slice(&bytes(&Item::Event(Event::Paste { on: ON, text: "pasted" })));
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

// REQ-INT-110, REQ-INT-126, amended by REQ-RCH-012 and REQ-RCH-054 (decision E.133):
// what is never acknowledged counts nothing.
#[test]
fn the_input_count_counts_characters_and_events() {
    assert_eq!(units(&Item::Text("héllo")), 5);
    assert_eq!(units(&Item::Event(Event::Paste { on: ON, text: "a long paste" })), 1);
    assert_eq!(units(&Item::Event(Event::FocusIn)), 1);
    let at = Point { column: 1, row: 1, pixels: None, mods: 0 };
    let mouse = |action| Item::Event(Event::Mouse(Mouse { on: ON, action, button: 0, at, device: Device::Touch }));
    let touch = |action| Item::Event(Event::Touch(Touch { on: ON, action, id: 1, column: 1, row: 1, pixels: None }));
    for (action, counted) in
        [(MouseAction::Press, 1), (MouseAction::Release, 1), (MouseAction::Move, 0), (MouseAction::Gone, 0)]
    {
        assert_eq!(units(&mouse(action)), counted, "MOUSE {action:?}");
        assert_eq!(units(&touch(action)), counted, "TOUCH {action:?}");
    }
    assert_eq!(units(&Item::Event(Event::Hover { on: ON, leave: false, column: 1, row: 1 })), 0, "HOVER");
    assert_eq!(units(&Item::Event(Event::Compose { on: ON, cursor: 0, text: "か" })), 0, "COMPOSE");
    assert_eq!(units(&Item::Event(Event::Resume(Resume { token: [0; crate::TOKEN_BYTES], acked: 1 }))), 0);
}

/// REQ-INT-124, REQ-RCH-042, decision E.133: MOUSE's `flags` bits 4 and 5 give the
/// device, 0 mouse, 1 touch, 2 pen; 3 is refused, and so are bits 6 and 7.
/// A mouse's bytes are those of before.
#[test]
fn mouse_carries_its_device() {
    let at = Point { column: 10, row: 5, pixels: None, mods: 0 };
    for (device, flags) in [(Device::Mouse, 0x00), (Device::Touch, 0x10), (Device::Pen, 0x20)] {
        let item = Item::Event(Event::Mouse(Mouse { on: ON, action: MouseAction::Press, button: 0, at, device }));
        assert_eq!(bytes(&item), [0xFF, 0x02, 0x80, 0x02, 0x00, flags, 0x00, 0x0A, 0x05], "{device:?}");
    }
    assert_eq!(
        read(&[0xFF, 0x02, 0x80, 0x02, 0x00, 0x30, 0x00, 0x0A, 0x05]),
        Err(InputError::BadValue("mouse device"))
    );
    assert_eq!(read(&[0xFF, 0x02, 0x80, 0x02, 0x00, 0x40, 0x00, 0x0A, 0x05]), Err(InputError::Flags));
    assert_eq!(read(&[0xFF, 0x02, 0x80, 0x02, 0x00, 0x80, 0x00, 0x0A, 0x05]), Err(InputError::Flags));
}

/// REQ-INT-123, REQ-RCH-034, REQ-RCH-046, decision E.133: MOUSE action 3 is gone, the
/// buttons held as on a move; it was a protocol error before.
#[test]
fn mouse_action_3_is_gone() {
    let at = Point { column: 3, row: 4, pixels: None, mods: 0 };
    let gone =
        Item::Event(Event::Mouse(Mouse { on: ON, action: MouseAction::Gone, button: 1, at, device: Device::Mouse }));
    assert_eq!(bytes(&gone), [0xFF, 0x02, 0x80, 0x02, 0x00, 0x03, 0x01, 0x03, 0x04]);
    assert_eq!(read(&[0xFF, 0x02, 0x80, 0x02, 0x00, 0x03, 0x01, 0x03, 0x04]), Ok(Some((gone, 9))));
}

/// REQ-INT-124, REQ-RCH-043, decision E.133: TOUCH is the extension `0x44`, `flags`
/// (the action, the dot), `id` 1 to 9, the cell and the dot; an id out of
/// range, a reserved flag, a cell off the page, a dot missing and bytes
/// after are refused. A receiver that does not know `0x44` skips it.
#[test]
fn touch_round_trips_and_is_checked() {
    let down = Item::Event(Event::Touch(Touch {
        on: ON,
        action: MouseAction::Press,
        id: 1,
        column: 10,
        row: 5,
        pixels: None,
    }));
    assert_eq!(bytes(&down), [0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x00, 0x01, 0x0A, 0x05]);
    let moved = Item::Event(Event::Touch(Touch {
        on: ON,
        action: MouseAction::Move,
        id: 2,
        column: 10,
        row: 5,
        pixels: Some((121, 100)),
    }));
    assert_eq!(bytes(&moved), [0xFF, 0x44, 0x09, 0x80, 0x02, 0x00, 0x06, 0x02, 0x0A, 0x05, 0x79, 0x64]);
    for bad in [
        [0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x00, 0x00, 0x0A, 0x05].as_slice(),
        &[0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x00, 0x0A, 0x0A, 0x05],
        &[0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x08, 0x01, 0x0A, 0x05],
        &[0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x00, 0x01, 0x28, 0x05],
        &[0xFF, 0x44, 0x07, 0x80, 0x02, 0x00, 0x04, 0x01, 0x0A, 0x05],
        &[0xFF, 0x44, 0x08, 0x80, 0x02, 0x00, 0x00, 0x01, 0x0A, 0x05, 0x00],
    ] {
        assert!(read_message(bad, &mut |_| {}).is_err(), "{bad:?}");
    }
    // A field cut short inside a payload is refused at once, never waited
    // for: a stream decoder would otherwise hold it until its buffer filled.
    assert_eq!(
        read(&[0xFF, 0x44, 0x06, 0x80, 0x02, 0x00, 0x04, 0x01, 0x0A]),
        Err(InputError::BadValue("extension cut short"))
    );
    assert_eq!(
        read(&[0xFF, 0x43, 0x04, 0x80, 0x02, 0x00, 0x00]),
        Err(InputError::BadValue("extension cut short")),
        "HOVER too"
    );
    // The longest moves fit a holder of one.
    let longest = Stamp::new(0x8FF, 0x3F7E).unwrap();
    let far = Point { column: 39, row: 24, pixels: Some((u32::MAX, u32::MAX)), mods: u8::MAX };
    let longest_mouse = Item::Event(Event::Mouse(Mouse {
        on: longest,
        action: MouseAction::Move,
        button: 0xFF,
        at: far,
        device: Device::Pen,
    }));
    let longest_touch = Item::Event(Event::Touch(Touch {
        on: longest,
        action: MouseAction::Move,
        id: crate::MAX_TOUCH_ID,
        column: 39,
        row: 24,
        pixels: Some((u32::MAX, u32::MAX)),
    }));
    assert_eq!(bytes(&longest_mouse).len().max(bytes(&longest_touch).len()), MAX_MOVE_BYTES);
}

/// REQ-INT-125, REQ-RCH-013, decision E.133: RESUME is the extension `0x45`, the token
/// and the last acknowledgement big-endian, exactly 18 bytes.
#[test]
fn resume_round_trips_and_is_checked() {
    let token = [0x11; crate::TOKEN_BYTES];
    let resume = Item::Event(Event::Resume(Resume { token, acked: 0x0102 }));
    let written = bytes(&resume);
    assert_eq!(&written[..3], &[0xFF, 0x45, 18]);
    assert_eq!(&written[3..19], &token);
    assert_eq!(&written[19..], &[0x01, 0x02]);
    let mut short = written.clone();
    short[2] = 17;
    short.pop();
    assert_eq!(read(&short), Err(InputError::BadValue("resume")));
}

/// REQ-INT-106, REQ-INT-124, INPUT.md section 6.2: what each event needs both sides to have agreed.
#[test]
fn each_new_event_needs_its_capability() {
    use crate::caps::{RESUME_CAP, TOUCH_CAP};
    let at = Point { column: 1, row: 1, pixels: None, mods: 0 };
    let mouse = |device| Event::Mouse(Mouse { on: ON, action: MouseAction::Press, button: 0, at, device });
    assert_eq!(needs(&mouse(Device::Mouse)), 0);
    assert_eq!(needs(&mouse(Device::Touch)), TOUCH_CAP);
    assert_eq!(needs(&mouse(Device::Pen)), TOUCH_CAP);
    assert_eq!(
        needs(&Event::Touch(Touch { on: ON, action: MouseAction::Press, id: 1, column: 1, row: 1, pixels: None })),
        TOUCH_CAP
    );
    assert_eq!(needs(&Event::Resume(Resume { token: [0; crate::TOKEN_BYTES], acked: 0 })), RESUME_CAP);
    assert_eq!(needs(&Event::FocusIn), 0);
}

/// REQ-INT-121, decision E.129: HOVER is the extension `0x43`, `flags`, `col` and `row`;
/// it reads back, and a bad flag, a cell off the page or bytes after are
/// refused. A receiver that does not know `0x43` skips it as any extension.
#[test]
fn hover_round_trips_and_is_checked() {
    for event in [
        Event::Hover { on: ON, leave: false, column: 2, row: 20 },
        Event::Hover { on: ON, leave: true, column: 39, row: 23 },
    ] {
        let mut out = [0_u8; 32];
        let length = write(&Item::Event(event), &mut out).unwrap();
        let mut back = Vec::new();
        read_message(&out[..length], &mut |item| back.push(item)).unwrap();
        assert_eq!(back, [Item::Event(event)]);
    }
    let mut written = [0_u8; 32];
    let length = write(&Item::Event(Event::Hover { on: ON, leave: true, column: 2, row: 20 }), &mut written).unwrap();
    assert_eq!(&written[..length], &[0xFF, 0x43, 6, 0x80, 0x02, 0, 1, 2, 20], "the type, the length, then the payload");
    for bad in [
        [0xFF, 0x43, 6, 0x80, 0x02, 0, 2, 2, 20].as_slice(),
        &[0xFF, 0x43, 6, 0x80, 0x02, 0, 0, 40, 20],
        &[0xFF, 0x43, 7, 0x80, 0x02, 0, 0, 2, 20, 0],
    ] {
        assert!(read_message(bad, &mut |_| {}).is_err(), "{bad:?}");
    }
}

// REQ-1555: every stamped event carries the stamp, at each bound, and the
// bounds are SELECT's but for 3F7F, since a page on screen is a sub-page.
#[test]
fn the_stamp_goes_first_and_keeps_its_bounds() {
    let on = |page, sub| Stamp::new(page, sub).unwrap();
    let at = Point { column: 1, row: 1, pixels: None, mods: 0 };
    for stamp in [on(0x100, 0), on(0x8FF, 0x3F7E), on(0x8FF, 0), on(0x100, 0x3F7E)] {
        for event in [
            Event::Key(Key {
                on: stamp,
                lost: false,
                action: Action::Press,
                key: KeyCode::Character('a'),
                mods: 0,
                text: None,
                base: None,
            }),
            Event::Mouse(Mouse { on: stamp, action: MouseAction::Press, button: 0, at, device: Device::Mouse }),
            Event::Wheel(Wheel { on: stamp, dx: 0, dy: 120, at }),
            Event::Paste { on: stamp, text: "x" },
            Event::Compose { on: stamp, cursor: 0, text: "x" },
            Event::Hover { on: stamp, leave: false, column: 1, row: 1 },
            Event::Touch(Touch { on: stamp, action: MouseAction::Press, id: 1, column: 1, row: 1, pixels: None }),
        ] {
            let item = Item::Event(event);
            let written = bytes(&item);
            assert_eq!(read(&written), Ok(Some((item, written.len()))));
        }
    }
    assert_eq!(Stamp::new(0xFF, 0), None, "below the first page");
    assert_eq!(Stamp::new(0x900, 0), None, "past the last page");
    assert_eq!(Stamp::new(0, 0x3F7F), None, "no page");
    assert_eq!(Stamp::new(0x100, 0x3F7F), None, "3F7F names no sub-page");
    assert_eq!(Stamp::new(0x100, 0x4000), None, "a bit outside the thirteen");
    assert_eq!(read(&[0xFF, 0x01, 0xFF, 0x01, 0x00, 0x04, 0x02]), Err(InputError::BadValue("stamp")), "page 0x0FF");
    assert_eq!(read(&[0xFF, 0x01, 0x80, 0x12, 0x00, 0x04, 0x02]), Err(InputError::BadValue("stamp")), "page 0x900");
    assert_eq!(read(&[0xFF, 0x01, 0x80, 0x02, 0xFF, 0x7E, 0x04, 0x02]), Err(InputError::BadValue("stamp")), "3F7F");
    assert_eq!(read(&[0xFF, 0x01, 0x00, 0x00, 0x04, 0x02]), Err(InputError::BadValue("stamp")), "page 0");
}

// REQ-1555, REQ-1556: nothing else carries a stamp, FOCUS_OUT least of all.
#[test]
fn focus_and_session_events_carry_no_stamp() {
    assert_eq!(bytes(&Item::Event(Event::FocusIn)), [0xFF, 0x05]);
    assert_eq!(bytes(&Item::Event(Event::FocusOut)), [0xFF, 0x06]);
    assert_eq!(read(&[0xFF, 0x0A, 0x80, 0x02]), Err(InputError::Type(0x0A)), "PAGE is an unknown core type");
    let hello = [0xFF, 0x09, 0x03, 0x80, 0x80, 0x04];
    assert_eq!(read(&hello), Err(InputError::BadValue("caps")), "PAGE's capability, bit 65,536");
    assert_eq!(RETIRED_PAGE_BIT, 65_536);
}

// REQ-207 item 5: the L flag says a key did not come up, on a release alone.
#[test]
fn only_a_release_may_be_lost() {
    let lost =
        |action| Key { on: ON, lost: true, action, key: KeyCode::Character('a'), mods: 0, text: None, base: None };
    let release = Item::Event(Event::Key(lost(Action::Release)));
    assert_eq!(bytes(&release), [0xFF, 0x01, 0x80, 0x02, 0x00, 0x42, 0x61]);
    assert_eq!(read(&bytes(&release)), Ok(Some((release, 7))));
    let mut out = [0; 32];
    assert!(write(&Item::Event(Event::Key(lost(Action::Press))), &mut out).is_err());
    assert!(read(&[0xFF, 0x01, 0x80, 0x02, 0x00, 0x41, 0x61]).is_err(), "a repeat, lost");
}
