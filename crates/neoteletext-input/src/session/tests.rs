#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects, reason = "a test")]

use super::*;
use crate::MAX_PER_SECOND;
use crate::caps::{CURSOR_CAP, PAGE_CAP};
use crate::event::{self, Action, MouseAction};
use crate::event::{Key, KeyCode};
use crate::keys;

fn welcomed(caps: u32) -> Session {
    let mut session = Session::new(caps);
    session.open(0, None);
    session.welcome(VERSION, caps).unwrap();
    session
}

// REQ-INT-106.
#[test]
fn nothing_goes_before_welcome_and_a_late_one_refuses() {
    let mut session = Session::new(PAGE_CAP | CURSOR_CAP);
    let hello = session.open(1_000, None);
    assert_eq!(hello, Event::Hello { version: VERSION, caps: PAGE_CAP | CURSOR_CAP, agent: None });
    assert!(!session.welcomed());
    assert_eq!(session.welcome_due(), Some(1_000 + WELCOME_TIMEOUT_MS));
    session.tick(1_000 + WELCOME_TIMEOUT_MS - 1);
    assert!(matches!(session.state(), State::Waiting(_)));
    session.tick(1_000 + WELCOME_TIMEOUT_MS);
    assert_eq!(session.state(), State::Refused);
    assert_eq!(session.welcome(VERSION, PAGE_CAP), Err(WelcomeError::Unasked));
}

// REQ-INT-106.
#[test]
fn hello_carries_the_agent_only_with_its_capability() {
    let mut session = Session::new(PAGE_CAP | AGENT);
    assert_eq!(session.open(0, None), Event::Hello { version: VERSION, caps: PAGE_CAP, agent: None });
    let agent = Some("NeoTeletext/0.1 (macOS)");
    assert_eq!(session.open(0, agent), Event::Hello { version: VERSION, caps: PAGE_CAP | AGENT, agent });
}

// REQ-INT-106.
#[test]
fn welcome_must_be_this_version_and_a_subset() {
    let mut session = Session::new(PAGE_CAP);
    session.open(0, None);
    assert_eq!(session.welcome(VERSION, PAGE_CAP | CURSOR_CAP), Err(WelcomeError::Caps), "a capability not offered");
    session.open(0, None);
    assert_eq!(session.welcome(1, PAGE_CAP), Err(WelcomeError::Version(1)));
    session.open(0, None);
    assert_eq!(session.welcome(VERSION, PAGE_CAP), Ok(()));
    assert_eq!(session.caps(), PAGE_CAP);
}

// REQ-INT-111, decision E.125: an older channel's mode records change
// nothing, before WELCOME or after.
#[test]
fn the_withdrawn_mode_records_change_nothing() {
    let mut session = Session::new(PAGE_CAP);
    session.open(0, None);
    assert_eq!(session.control(&Control::Withdrawn(0x02)), Ok(()));
    session.control(&Control::Welcome { version: VERSION, caps: PAGE_CAP }).unwrap();
    assert_eq!(session.control(&Control::Withdrawn(0x04)), Ok(()));
    assert!(session.welcomed());
}

// REQ-INT-106.
#[test]
fn a_reconnection_is_a_new_session() {
    let mut session = welcomed(PAGE_CAP);
    session.sent(&Item::Text("abc"), 5);
    session.open(10, None);
    assert_eq!(session.count(), 1, "the count from zero, HELLO counted");
    assert_eq!(session.acked(), 0);
    assert!(!session.welcomed());
}

// REQ-INT-110.
#[test]
fn the_input_count_and_its_acknowledgement_give_the_round_trip() {
    let mut session = welcomed(0);
    assert_eq!(session.sent(&Item::Text("ab"), 100), 3, "HELLO, then two characters");
    assert_eq!(session.sent(&Item::Event(Event::Paste("a long paste")), 110), 4, "a paste counts one");
    assert_eq!(session.sent(&Item::Event(Event::FocusIn), 120), 5);
    assert_eq!(session.acknowledged(3, 130), Some(30), "to the text sent at 100");
    assert_eq!(session.acknowledged(2, 140), None, "an older acknowledgement changes nothing");
    assert_eq!(session.acknowledged(5, 150), Some(30), "to the focus sent at 120");
    assert_eq!(session.acked(), 5);
    assert_eq!(session.acknowledged(6, 160), None, "nothing sent beyond 5: no round trip");
}

// REQ-INT-110.
#[test]
fn the_count_wraps_and_is_compared_as_a_serial_number() {
    let mut session = welcomed(0);
    for at in 0..u32::from(u16::MAX) {
        session.sent(&Item::Event(Event::FocusIn), at);
        session.acknowledged(session.count(), at);
    }
    assert_eq!(session.count(), 0, "65 536 units: wrapped");
    session.sent(&Item::Text("ab"), 70_000);
    assert_eq!(session.acknowledged(2, 70_005), Some(5), "2 is after 0 across the wrap");
    assert_eq!(session.acknowledged(u16::MAX, 70_006), None, "65 535 is before 2");
}

// REQ-INT-137.
#[test]
fn at_most_256_sends_are_remembered() {
    let mut session = welcomed(0);
    for at in 0..300 {
        session.sent(&Item::Text("a"), at);
    }
    assert_eq!(session.acknowledged(45, 400), None, "the sends to 45, the 44 oldest, were forgotten");
    assert_eq!(session.acknowledged(301, 400), Some(101), "the last went at 299");
}

// REQ-INT-109.
#[test]
fn the_rate_is_120_units_a_second_and_releases_always_go() {
    let mut session = Session::new(0);
    assert_eq!(session.allow(&Item::Text("ab"), 5), 2);
    let mut allowed = 0;
    for _ in 0..119 {
        allowed += session.allow(&Item::Text("a"), 5);
    }
    assert_eq!(allowed, 118, "120 units in all");
    let release = Item::Event(Event::Key(Key {
        action: Action::Release,
        key: KeyCode::Character('a'),
        mods: 0,
        text: None,
        base: None,
    }));
    assert_eq!(session.allow(&release, 5), 1, "a release beyond the rate");
    let at = event::Point { column: 1, row: 1, pixels: Some((12, 20)), mods: 0 };
    let moved = Item::Event(Event::Mouse(Mouse { action: MouseAction::Move, button: 0, at }));
    assert_eq!(session.allow(&moved, 5), 1, "a move beyond the rate: moves are never counted (section 8 rule 4)");
    let hover = Item::Event(Event::Hover { leave: false, column: 1, row: 1 });
    assert_eq!(session.allow(&hover, 5), 1, "nor is a HOVER");
    assert_eq!(session.allow(&Item::Text("a"), 5 + 1_000), 1, "a second later");
    let mut limiter = Limiter::new();
    for _ in 0..MAX_PER_SECOND {
        assert!(limiter.allow(u32::MAX - 10, false));
    }
    assert!(!limiter.allow(u32::MAX, false), "the 121st unit");
    assert!(limiter.allow(u32::MAX, true), "a release across the clock's wrap");
    assert!(limiter.allow(1_000 - 11, false), "a second after, past the wrap");
}

fn key(action: Action, key: KeyCode) -> Item<'static> {
    Item::Event(Event::Key(Key { action, key, mods: 0, text: Some("a"), base: Some('q') }))
}

fn sent_item(session: &mut Session, item: &Item<'_>) -> Option<std::vec::Vec<u8>> {
    let mut out = [0; crate::MAX_MESSAGE];
    let length = session.send(item, 0, &mut out).unwrap();
    (length > 0).then(|| out.get(..length).unwrap().to_vec())
}

// REQ-INT-106.
#[test]
fn nothing_is_sent_before_welcome() {
    let mut session = Session::new(0);
    session.open(0, None);
    assert_eq!(session.send(&Item::Text("a"), 0, &mut [0; 8]), Err(SendError::NotWelcomed));
}

// REQ-INT-104, decision E.125: every press, repeat and release goes whole,
// with its text and its base; a base the same as its key is left out.
#[test]
fn every_key_goes_whole() {
    let mut session = welcomed(PAGE_CAP);
    let press = key(Action::Press, KeyCode::Character('a'));
    let release = key(Action::Release, KeyCode::Character('a'));
    assert_eq!(
        sent_item(&mut session, &press),
        Some(std::vec![0xFF, 0x01, 0x30, b'a', 1, b'a', b'q']),
        "text and base"
    );
    assert!(sent_item(&mut session, &release).is_some(), "a release");
    assert_eq!(sent_item(&mut session, &Item::Text("é")), Some("é".as_bytes().to_vec()), "an input method's text");
    let same = Item::Event(Event::Key(Key {
        action: Action::Press,
        key: KeyCode::Character('q'),
        mods: 0,
        text: Some("q"),
        base: Some('q'),
    }));
    assert_eq!(
        sent_item(&mut session, &same),
        Some(std::vec![0xFF, 0x01, 0x10, b'q', 1, b'q']),
        "no base the same as the key"
    );
}

// REQ-INT-116, decision E.125: a dead key and a key that only modifies go
// as every key does.
#[test]
fn a_dead_key_and_a_modifier_alone_go() {
    let functional = |name: &str| key(Action::Press, KeyCode::Functional(keys::code(name).unwrap()));
    let mut session = welcomed(PAGE_CAP);
    for name in ["Dead", "ShiftLeft", "AltGraph", "CapsLock", "Enter"] {
        assert!(sent_item(&mut session, &functional(name)).is_some(), "{name}");
    }
}

// REQ-INT-115, amended 27 October 2026 (decision E.125): every press,
// release and move, and the wheel, go with the cell and the dot.
#[test]
fn the_mouse_and_the_wheel_always_go_with_their_dot() {
    let mut session = welcomed(PAGE_CAP);
    let at = event::Point { column: 10, row: 5, pixels: Some((120, 100)), mods: 0 };
    let click = Item::Event(Event::Mouse(Mouse { action: MouseAction::Press, button: 0, at }));
    let wheel = Item::Event(Event::Wheel(event::Wheel { dx: 0, dy: 120, at }));
    let drag = Item::Event(Event::Mouse(Mouse { action: MouseAction::Move, button: 1, at }));
    assert_eq!(
        sent_item(&mut session, &click),
        Some(std::vec![0xFF, 0x02, 0x04, 0x00, 0x0A, 0x05, 0x78, 0x64]),
        "the cell and the dot"
    );
    assert_eq!(sent_item(&mut session, &wheel).map(|bytes| bytes.len()), Some(10), "the wheel, with its dot");
    assert!(sent_item(&mut session, &drag).is_some(), "a drag");
}

// Decision E.125, INPUT.md section 6.3: a move goes only when the dot under
// the pointer changes, and the first on another page always goes.
#[test]
fn a_move_to_the_same_dot_is_not_sent_again() {
    let mut session = welcomed(PAGE_CAP);
    let at = |column: u32, pixels: (u32, u32)| event::Point { column, row: 5, pixels: Some(pixels), mods: 0 };
    let moved =
        |button: u8, at: event::Point| Item::Event(Event::Mouse(Mouse { action: MouseAction::Move, button, at }));
    assert!(sent_item(&mut session, &moved(0, at(10, (120, 100)))).is_some());
    assert_eq!(sent_item(&mut session, &moved(0, at(10, (120, 100)))), None, "the same dot");
    assert!(sent_item(&mut session, &moved(0, at(10, (121, 100)))).is_some(), "the next dot, in the same cell");
    assert!(sent_item(&mut session, &moved(1, at(10, (121, 100)))).is_some(), "a button held is news");
    assert!(sent_item(&mut session, &Item::Event(Event::Page(0x200))).is_some());
    assert!(sent_item(&mut session, &moved(1, at(10, (121, 100)))).is_some(), "the first move on another page");
}

// REQ-INT-106, decision E.125: PAGE needs its capability; COMPOSE and the
// rest always go.
#[test]
fn page_needs_its_capability_and_compose_always_goes() {
    let compose = Item::Event(Event::Compose { cursor: 1, text: "か" });
    let mut session = welcomed(PAGE_CAP);
    assert!(sent_item(&mut session, &Item::Event(Event::Page(0x100))).is_some());
    assert!(sent_item(&mut session, &compose).is_some());
    let mut session = welcomed(0);
    assert_eq!(sent_item(&mut session, &Item::Event(Event::Page(0x100))), None, "no PAGE capability");
    assert!(sent_item(&mut session, &Item::Event(Event::FocusOut)).is_some());
}

// REQ-INT-109.
#[test]
fn text_over_the_rate_is_cut_and_counted() {
    let mut session = welcomed(0);
    let long: std::string::String = core::iter::repeat_n('é', 130).collect();
    let sent = sent_item(&mut session, &Item::Text(&long)).unwrap();
    assert_eq!(sent.len(), 240, "120 characters of 2 bytes");
    assert_eq!(session.count(), 121, "HELLO and 120 characters");
    assert_eq!(sent_item(&mut session, &Item::Event(Event::FocusIn)), None, "an event over the rate");
}
