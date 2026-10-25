#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects, reason = "a test")]

use super::*;
use crate::MAX_PER_SECOND;
use crate::event::{self, Action, MouseAction};
use crate::event::{Key, KeyCode};
use crate::keys;
use crate::modes::{ALL_KEYS, BASE_KEY, COMPOSE, KEY_RELEASE, MOUSE_DRAG, MOUSE_PIXELS};
use crate::modes::{MOUSE_BUTTONS, PAGE_CAP};

fn welcomed(caps: u32) -> Session {
    let mut session = Session::new(caps);
    session.open(0, None);
    session.welcome(VERSION, caps).unwrap();
    session
}

// REQ-INT-106.
#[test]
fn nothing_goes_before_welcome_and_a_late_one_refuses() {
    let mut session = Session::new(PAGE_CAP | MOUSE_BUTTONS);
    let hello = session.open(1_000, None);
    assert_eq!(hello, Event::Hello { version: VERSION, caps: PAGE_CAP | MOUSE_BUTTONS, agent: None });
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
    assert_eq!(session.welcome(VERSION, PAGE_CAP | MOUSE_BUTTONS), Err(WelcomeError::Caps));
    session.open(0, None);
    assert_eq!(session.welcome(1, PAGE_CAP), Err(WelcomeError::Version(1)));
    session.open(0, None);
    assert_eq!(session.welcome(VERSION, PAGE_CAP), Ok(()));
    assert_eq!(session.caps(), PAGE_CAP);
}

// REQ-INT-107.
#[test]
fn modes_apply_only_once_welcomed() {
    let mut session = Session::new(MOUSE_BUTTONS);
    session.open(0, None);
    session.control(&Control::PushMode(MOUSE_BUTTONS)).unwrap();
    session.control(&Control::Welcome { version: VERSION, caps: MOUSE_BUTTONS }).unwrap();
    assert_eq!(session.modes.on(None), 0, "the push before WELCOME was not taken");
    session.control(&Control::PageModes { page: 0x300, modes: MOUSE_BUTTONS }).unwrap();
    assert_eq!(session.modes.on(Some(0x300)), MOUSE_BUTTONS);
}

// REQ-INT-106.
#[test]
fn a_reconnection_is_a_new_session() {
    let mut session = welcomed(MOUSE_BUTTONS);
    session.control(&Control::PushMode(MOUSE_BUTTONS)).unwrap();
    session.sent(&Item::Text("abc"), 5);
    assert_eq!(session.modes.on(None), MOUSE_BUTTONS);
    session.open(10, None);
    assert_eq!(session.modes.on(None), 0, "the mode stack empty");
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

fn sent_item(session: &mut Session, item: &Item<'_>, open: Option<u16>) -> Option<std::vec::Vec<u8>> {
    let mut out = [0; crate::MAX_MESSAGE];
    let length = session.send(item, open, 0, &mut out).unwrap();
    (length > 0).then(|| out.get(..length).unwrap().to_vec())
}

// REQ-INT-106.
#[test]
fn nothing_is_sent_before_welcome() {
    let mut session = Session::new(0);
    session.open(0, None);
    assert_eq!(session.send(&Item::Text("a"), None, 0, &mut [0; 8]), Err(SendError::NotWelcomed));
}

// REQ-INT-104 and REQ-INT-107.
#[test]
fn keys_send_what_the_modes_ask_for() {
    let mut session = welcomed(ALL_KEYS | KEY_RELEASE | BASE_KEY);
    let press = key(Action::Press, KeyCode::Character('a'));
    let release = key(Action::Release, KeyCode::Character('a'));
    assert_eq!(sent_item(&mut session, &press, None), Some(std::vec![0xFF, 0x01, 0x00, b'a']), "no text, no base");
    assert_eq!(sent_item(&mut session, &release, None), None, "no release without KEY_RELEASE");
    assert_eq!(sent_item(&mut session, &Item::Text("a"), None), Some(std::vec![b'a']));
    session.control(&Control::PushMode(ALL_KEYS | KEY_RELEASE | BASE_KEY)).unwrap();
    assert_eq!(sent_item(&mut session, &Item::Text("a"), None), None, "no raw text in ALL_KEYS");
    assert_eq!(
        sent_item(&mut session, &press, None),
        Some(std::vec![0xFF, 0x01, 0x30, b'a', 1, b'a', b'q']),
        "text and base"
    );
    assert!(sent_item(&mut session, &release, None).is_some());
}

// REQ-INT-116.
#[test]
fn a_dead_key_and_a_modifier_alone_go_only_when_asked_for() {
    let functional = |name: &str| key(Action::Press, KeyCode::Functional(keys::code(name).unwrap()));
    let mut session = welcomed(ALL_KEYS | KEY_RELEASE);
    for quiet in ["Dead", "ShiftLeft", "AltGraph", "CapsLock"] {
        assert_eq!(sent_item(&mut session, &functional(quiet), None), None, "{quiet}");
    }
    assert!(sent_item(&mut session, &functional("Enter"), None).is_some(), "a non-text key always");
    session.control(&Control::PushMode(KEY_RELEASE)).unwrap();
    assert!(sent_item(&mut session, &functional("ShiftLeft"), None).is_some(), "in KEY_RELEASE");
    assert_eq!(sent_item(&mut session, &functional("Dead"), None), None, "the dead key is ALL_KEYS's");
    session.control(&Control::PushMode(ALL_KEYS)).unwrap();
    assert!(sent_item(&mut session, &functional("Dead"), None).is_some());
}

// REQ-INT-115, amended 25 October 2026 (decision E.119).
#[test]
fn the_mouse_goes_only_in_a_mouse_mode_or_on_its_page() {
    let mut session = welcomed(MOUSE_BUTTONS | MOUSE_DRAG | MOUSE_PIXELS);
    let at = event::Point { column: 10, row: 5, pixels: Some((120, 100)), mods: 0 };
    let click = Item::Event(Event::Mouse(event::Mouse { action: MouseAction::Press, button: 0, at }));
    let wheel = Item::Event(Event::Wheel(event::Wheel { dx: 0, dy: 120, at }));
    let drag = Item::Event(Event::Mouse(event::Mouse { action: MouseAction::Move, button: 1, at }));
    let hover = Item::Event(Event::Mouse(event::Mouse { action: MouseAction::Move, button: 0, at }));
    assert_eq!(sent_item(&mut session, &click, Some(0x300)), None, "REQ-INT-115: not asked for");
    assert_eq!(
        sent_item(&mut session, &wheel, Some(0x300)).map(|bytes| bytes.len()),
        Some(8),
        "the wheel goes with no mode, its cell alone, without MOUSE_PIXELS"
    );
    session.control(&Control::PageModes { page: 0x300, modes: MOUSE_BUTTONS }).unwrap();
    assert_eq!(sent_item(&mut session, &click, Some(0x301)), None, "another page did not ask");
    assert_eq!(
        sent_item(&mut session, &click, Some(0x300)),
        Some(std::vec![0xFF, 0x02, 0x00, 0x00, 0x0A, 0x05]),
        "the cell alone"
    );
    assert!(sent_item(&mut session, &wheel, Some(0x300)).is_some());
    assert_eq!(sent_item(&mut session, &drag, Some(0x300)), None, "no moves in MOUSE_BUTTONS");
    session.control(&Control::PushMode(MOUSE_DRAG | MOUSE_PIXELS)).unwrap();
    assert!(sent_item(&mut session, &drag, None).is_some(), "a drag");
    assert_eq!(sent_item(&mut session, &hover, None), None, "a move with no button held is MOUSE_ALL's");
    assert_eq!(sent_item(&mut session, &click, None).map(|bytes| bytes.len()), Some(8), "with pixels");
}

// REQ-INT-107.
#[test]
fn page_goes_only_with_its_capability_and_compose_only_in_its_mode() {
    let mut session = welcomed(PAGE_CAP);
    assert!(sent_item(&mut session, &Item::Event(Event::Page(0x100)), None).is_some());
    let compose = Item::Event(Event::Compose { cursor: 1, text: "か" });
    assert_eq!(sent_item(&mut session, &compose, None), None);
    let mut session = welcomed(COMPOSE);
    assert_eq!(sent_item(&mut session, &Item::Event(Event::Page(0x100)), None), None);
    session.control(&Control::PushMode(COMPOSE)).unwrap();
    assert!(sent_item(&mut session, &compose, None).is_some());
    assert!(sent_item(&mut session, &Item::Event(Event::FocusOut), None).is_some(), "always on");
}

// REQ-INT-109.
#[test]
fn text_over_the_rate_is_cut_and_counted() {
    let mut session = welcomed(0);
    let long: std::string::String = core::iter::repeat_n('é', 130).collect();
    let sent = sent_item(&mut session, &Item::Text(&long), None).unwrap();
    assert_eq!(sent.len(), 240, "120 characters of 2 bytes");
    assert_eq!(session.count(), 121, "HELLO and 120 characters");
    assert_eq!(sent_item(&mut session, &Item::Event(Event::FocusIn), None), None, "an event over the rate");
}
