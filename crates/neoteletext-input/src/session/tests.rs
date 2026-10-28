#![allow(
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test"
)]

use super::*;
use crate::MAX_PER_SECOND;
use crate::caps::{CURSOR_CAP, PAGE_CAP, RESUME_CAP, TOUCH_CAP};
use crate::control::Resumption;
use crate::event::{self, Action, Device, MouseAction, Touch};
use crate::event::{Key, KeyCode};
use crate::keys;

fn welcomed(caps: u32) -> Session {
    let mut session = Session::new(caps);
    session.open(0, None, &mut [0; crate::MAX_MESSAGE]).unwrap();
    session.welcome(VERSION, caps, None).unwrap();
    session
}

/// The first message `open` writes, read back.
fn opening<const N: usize>(session: &mut SessionOf<N>, now: u32, agent: Option<&str>) -> std::vec::Vec<Event<'static>> {
    let mut out = [0; crate::MAX_MESSAGE];
    let length = session.open(now, agent, &mut out).unwrap();
    let mut events = std::vec::Vec::new();
    event::read_message(&out[..length], &mut |item| match item {
        Item::Event(Event::Hello { version, caps, agent: None }) => {
            events.push(Event::Hello { version, caps, agent: None });
        }
        Item::Event(Event::Hello { version, caps, agent: Some(_) }) => {
            events.push(Event::Hello { version, caps, agent: Some("agent") });
        }
        Item::Event(Event::Resume(resume)) => events.push(Event::Resume(resume)),
        other => panic!("{other:?}"),
    })
    .unwrap();
    events
}

// REQ-INT-106.
#[test]
fn nothing_goes_before_welcome_and_a_late_one_refuses() {
    let mut session = Session::new(PAGE_CAP | CURSOR_CAP);
    let hello = opening(&mut session, 1_000, None);
    assert_eq!(hello, [Event::Hello { version: VERSION, caps: PAGE_CAP | CURSOR_CAP, agent: None }]);
    assert!(!session.welcomed());
    assert_eq!(session.welcome_due(), Some(1_000 + WELCOME_TIMEOUT_MS));
    session.tick(1_000 + WELCOME_TIMEOUT_MS - 1);
    assert!(matches!(session.state(), State::Waiting(_)));
    session.tick(1_000 + WELCOME_TIMEOUT_MS);
    assert_eq!(session.state(), State::Refused);
    assert_eq!(session.welcome(VERSION, PAGE_CAP, None), Err(WelcomeError::Unasked));
}

// REQ-INT-106.
#[test]
fn hello_carries_the_agent_only_with_its_capability() {
    let mut session = Session::new(PAGE_CAP | AGENT);
    assert_eq!(opening(&mut session, 0, None), [Event::Hello { version: VERSION, caps: PAGE_CAP, agent: None }]);
    let agent = Some("NeoTeletext/0.1 (macOS)");
    assert_eq!(
        opening(&mut session, 0, agent),
        [Event::Hello { version: VERSION, caps: PAGE_CAP | AGENT, agent: Some("agent") }]
    );
}

// REQ-INT-106.
#[test]
fn welcome_must_be_this_version_and_a_subset() {
    let mut session = Session::new(PAGE_CAP);
    opening(&mut session, 0, None);
    assert_eq!(
        session.welcome(VERSION, PAGE_CAP | CURSOR_CAP, None),
        Err(WelcomeError::Caps),
        "a capability not offered"
    );
    opening(&mut session, 0, None);
    assert_eq!(session.welcome(1, PAGE_CAP, None), Err(WelcomeError::Version(1)));
    opening(&mut session, 0, None);
    assert_eq!(session.welcome(VERSION, PAGE_CAP, None), Ok(Welcomed::New));
    assert_eq!(session.caps(), PAGE_CAP);
}

// REQ-INT-111, decision E.125: an older channel's mode records change
// nothing, before WELCOME or after.
#[test]
fn the_withdrawn_mode_records_change_nothing() {
    let mut session = Session::new(PAGE_CAP);
    opening(&mut session, 0, None);
    assert_eq!(session.control(&Control::Withdrawn(0x02)), Ok(None));
    session.control(&Control::Welcome { version: VERSION, caps: PAGE_CAP, resume: None }).unwrap();
    assert_eq!(session.control(&Control::Withdrawn(0x04)), Ok(None));
    assert!(session.welcomed());
}

// REQ-INT-106.
#[test]
fn a_reconnection_is_a_new_session() {
    let mut session = welcomed(PAGE_CAP);
    sent_item(&mut session, &Item::Text("abc"));
    session.close();
    opening(&mut session, 10, None);
    assert_eq!(session.count(), 1, "the count from zero, HELLO counted");
    assert_eq!(session.acked(), 0);
    assert!(!session.welcomed());
}

// REQ-INT-110; REQ-INT-137 as amended by REQ-RCH-016 (decision E.133):
// the round trip is no longer measured from acknowledgements.
#[test]
fn the_input_count_and_its_acknowledgement() {
    let mut session = welcomed(0);
    sent_item(&mut session, &Item::Text("ab"));
    assert_eq!(session.count(), 3, "HELLO, then two characters");
    sent_item(&mut session, &Item::Event(Event::Paste("a long paste")));
    assert_eq!(session.count(), 4, "a paste counts one");
    sent_item(&mut session, &Item::Event(Event::FocusIn));
    assert_eq!(session.count(), 5);
    assert!(session.acknowledged(3));
    assert!(!session.acknowledged(2), "an older acknowledgement changes nothing");
    assert!(session.acknowledged(5));
    assert_eq!(session.acked(), 5);
    assert!(!session.acknowledged(6), "beyond what was sent: refused");
    assert_eq!(session.acked(), 5);
}

// REQ-INT-110.
#[test]
fn the_count_wraps_and_is_compared_as_a_serial_number() {
    let mut session = welcomed(0);
    for at in 0..u32::from(u16::MAX) {
        session.record(&Item::Event(Event::FocusIn), &[]);
        assert!(session.acknowledged(session.count()), "{at}");
    }
    assert_eq!(session.count(), 0, "65 536 units: wrapped");
    session.record(&Item::Text("ab"), &[]);
    assert!(session.acknowledged(2), "2 is after 0 across the wrap");
    assert!(!session.acknowledged(u16::MAX), "65 535 is before 2");
}

// REQ-INT-126, REQ-RCH-012, REQ-RCH-054 (decision E.133): a message of no input units is
// never acknowledged, so it adds nothing to the count or to what is waited
// for, and a press and a release of touch count as a click's.
#[test]
fn what_is_never_acknowledged_counts_nothing() {
    let mut session = welcomed(TOUCH_CAP);
    let at = event::Point { column: 2, row: 3, pixels: None, mods: 0 };
    let mouse = |action| Item::Event(Event::Mouse(Mouse { action, button: 0, at, device: Device::Touch }));
    let touch = |action| Item::Event(Event::Touch(Touch { action, id: 1, column: 2, row: 3, pixels: None }));
    for item in [
        mouse(MouseAction::Move),
        mouse(MouseAction::Gone),
        touch(MouseAction::Move),
        touch(MouseAction::Gone),
        Item::Event(Event::Hover { leave: false, column: 2, row: 3 }),
        Item::Event(Event::Compose { cursor: 0, text: "か" }),
    ] {
        assert!(sent_item(&mut session, &item).is_some(), "{item:?} goes");
        assert_eq!(session.count(), 1, "{item:?} counts nothing");
    }
    assert!(!session.acknowledged(2), "nothing to acknowledge");
    sent_item(&mut session, &mouse(MouseAction::Press)).unwrap();
    sent_item(&mut session, &touch(MouseAction::Press)).unwrap();
    sent_item(&mut session, &touch(MouseAction::Release)).unwrap();
    assert_eq!(session.count(), 4, "downs and ups count one each");
}

// REQ-INT-109, amended by decision E.133: a button's release and a contact's
// up always go too, and COMPOSE is held to the rate.
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
    let mouse = |action, button| Item::Event(Event::Mouse(Mouse { action, button, at, device: Device::Mouse }));
    assert_eq!(session.allow(&mouse(MouseAction::Release, 0), 5), 1, "a button's release beyond the rate");
    let up = Item::Event(Event::Touch(Touch { action: MouseAction::Release, id: 1, column: 1, row: 1, pixels: None }));
    assert_eq!(session.allow(&up, 5), 1, "a contact's up beyond the rate");
    assert_eq!(session.allow(&mouse(MouseAction::Press, 0), 5), 0, "a press is held to it");
    let compose = Item::Event(Event::Compose { cursor: 0, text: "" });
    assert_eq!(session.allow(&compose, 5), 0, "so is COMPOSE, which nothing else bounds");
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

fn move_to(column: u32, pixels: (u32, u32), button: u8) -> Item<'static> {
    let at = event::Point { column, row: 5, pixels: Some(pixels), mods: 0 };
    Item::Event(Event::Mouse(Mouse { action: MouseAction::Move, button, at, device: Device::Mouse }))
}

// REQ-INT-106.
#[test]
fn nothing_is_sent_before_welcome() {
    let mut session = Session::new(0);
    opening(&mut session, 0, None);
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
    let click = Item::Event(Event::Mouse(Mouse { action: MouseAction::Press, button: 0, at, device: Device::Mouse }));
    let wheel = Item::Event(Event::Wheel(event::Wheel { dx: 0, dy: 120, at }));
    let drag = Item::Event(Event::Mouse(Mouse { action: MouseAction::Move, button: 1, at, device: Device::Mouse }));
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
    assert!(sent_item(&mut session, &move_to(10, (120, 100), 0)).is_some());
    assert_eq!(sent_item(&mut session, &move_to(10, (120, 100), 0)), None, "the same dot");
    assert!(sent_item(&mut session, &move_to(10, (121, 100), 0)).is_some(), "the next dot, in the same cell");
    assert!(sent_item(&mut session, &move_to(10, (121, 100), 1)).is_some(), "a button held is news");
    assert!(sent_item(&mut session, &Item::Event(Event::Page(0x200))).is_some());
    assert!(sent_item(&mut session, &move_to(10, (121, 100), 1)).is_some(), "the first move on another page");
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

// REQ-INT-124, REQ-RCH-041, REQ-RCH-042 (decision E.133): without TOUCH a channel sees
// nothing a mouse would not send: no TOUCH, and MOUSE's device as the
// mouse's; with it, both go.
#[test]
fn touch_goes_only_when_both_sides_set_it() {
    let at = event::Point { column: 10, row: 5, pixels: None, mods: 0 };
    let tap = Item::Event(Event::Mouse(Mouse { action: MouseAction::Press, button: 0, at, device: Device::Touch }));
    let second =
        Item::Event(Event::Touch(Touch { action: MouseAction::Press, id: 1, column: 10, row: 5, pixels: None }));
    let mut plain = welcomed(0);
    assert_eq!(sent_item(&mut plain, &tap), Some(std::vec![0xFF, 0x02, 0x00, 0x00, 0x0A, 0x05]), "as the mouse");
    assert_eq!(sent_item(&mut plain, &second), None, "no TOUCH");
    let mut touch = welcomed(TOUCH_CAP);
    assert_eq!(sent_item(&mut touch, &tap), Some(std::vec![0xFF, 0x02, 0x10, 0x00, 0x0A, 0x05]), "device 1");
    assert_eq!(sent_item(&mut touch, &second), Some(std::vec![0xFF, 0x44, 0x04, 0x00, 0x01, 0x0A, 0x05]));
}

const TOKEN: [u8; TOKEN_BYTES] = [0x5A; TOKEN_BYTES];

/// A resumable session welcomed as a new one with RESUME and `TOKEN`.
fn resumable() -> ResumableSession {
    let mut session = ResumableSession::new(PAGE_CAP);
    let first = opening(&mut session, 0, None);
    assert_eq!(first, [Event::Hello { version: VERSION, caps: PAGE_CAP | RESUME_CAP, agent: None }], "RESUME offered");
    let resume = Some(Resumption { token: TOKEN, taken: None });
    assert_eq!(session.welcome(VERSION, PAGE_CAP | RESUME_CAP, resume), Ok(Welcomed::New));
    session
}

fn text(session: &mut ResumableSession, text: &str) -> std::vec::Vec<u8> {
    let mut out = [0; crate::MAX_MESSAGE];
    let length = session.send(&Item::Text(text), 0, &mut out).unwrap();
    out[..length].to_vec()
}

fn resent(session: &mut ResumableSession) -> std::vec::Vec<std::vec::Vec<u8>> {
    let mut messages = std::vec::Vec::new();
    session.resend(&mut |message| messages.push(message.to_vec()));
    messages
}

// REQ-INT-125, REQ-RCH-013 (decision E.133): after a reconnection the session is asked
// for with its token and the last acknowledgement; a channel that held it
// says what it had taken, and the receiver sends again, in order, only
// what came after, before anything else; the count goes on.
#[test]
fn a_resumed_session_sends_again_only_what_the_channel_had_not_taken() {
    let mut session = resumable();
    text(&mut session, "a"); // 2
    text(&mut session, "bc"); // 4
    text(&mut session, "d"); // 5
    let at = event::Point { column: 1, row: 1, pixels: None, mods: 0 };
    let moved = Item::Event(Event::Mouse(Mouse { action: MouseAction::Move, button: 0, at, device: Device::Mouse }));
    assert!(sent_any(&mut session, &moved), "a move goes, and is never kept");
    assert!(session.acknowledged(2));
    session.close();
    assert!(session.resumable());
    let first = opening(&mut session, 100, None);
    assert_eq!(
        first,
        [
            Event::Hello { version: VERSION, caps: PAGE_CAP | RESUME_CAP, agent: None },
            Event::Resume(Resume { token: TOKEN, acked: 2 }),
        ]
    );
    let next = [0x77; TOKEN_BYTES];
    let resume = Some(Resumption { token: next, taken: Some(4) });
    assert_eq!(session.welcome(VERSION, PAGE_CAP | RESUME_CAP, resume), Ok(Welcomed::Resumed));
    assert_eq!(session.send(&Item::Text("e"), 0, &mut [0; 8]), Err(SendError::ResendFirst), "the order kept");
    assert_eq!(resent(&mut session), [b"d".to_vec()], "only what came after 4");
    assert_eq!(session.count(), 5, "the count goes on");
    assert_eq!(text(&mut session, "e"), b"e");
    assert_eq!(session.count(), 6);
    // Taken from what was acknowledged: everything kept goes again.
    session.close();
    opening(&mut session, 200, None);
    let resume = Some(Resumption { token: next, taken: Some(2) });
    assert_eq!(session.welcome(VERSION, PAGE_CAP | RESUME_CAP, resume), Ok(Welcomed::Resumed));
    assert_eq!(resent(&mut session), [b"bc".to_vec(), b"d".to_vec(), b"e".to_vec()]);
}

fn sent_any(session: &mut ResumableSession, item: &Item<'_>) -> bool {
    session.send(item, 0, &mut [0; crate::MAX_MESSAGE]).unwrap() > 0
}

// REQ-INT-125, REQ-RCH-013, REQ-INT-106: a channel that did not hold the session, or
// that does not set RESUME, begins a new one: the count from HELLO and
// nothing sent again; a count taken that no message ends at is refused,
// and the session forgotten.
#[test]
fn a_session_not_held_is_a_new_one_and_a_false_count_is_refused() {
    let mut session = resumable();
    text(&mut session, "ab");
    session.close();
    opening(&mut session, 10, None);
    let resume = Some(Resumption { token: [1; TOKEN_BYTES], taken: None });
    assert_eq!(session.welcome(VERSION, PAGE_CAP | RESUME_CAP, resume), Ok(Welcomed::New));
    assert_eq!(session.count(), 1, "from HELLO");
    assert_eq!(resent(&mut session).len(), 0, "nothing sent again");
    text(&mut session, "ab");
    session.close();
    opening(&mut session, 20, None);
    assert_eq!(session.welcome(VERSION, PAGE_CAP, None), Ok(Welcomed::New), "a channel without RESUME");
    assert_eq!(session.count(), 1);
    session.close();
    assert!(!session.resumable(), "no token: forgotten on close");
    let mut session = resumable();
    text(&mut session, "ab"); // 3
    session.close();
    opening(&mut session, 30, None);
    let resume = Some(Resumption { token: TOKEN, taken: Some(2) });
    assert_eq!(session.welcome(VERSION, PAGE_CAP | RESUME_CAP, resume), Err(WelcomeError::Resume), "inside a message");
    assert!(!session.resumable());
    let mut session = resumable();
    let resume = Some(Resumption { token: TOKEN, taken: Some(1) });
    session.close();
    opening(&mut session, 40, None);
    session.forget();
    opening(&mut session, 41, None);
    assert_eq!(session.welcome(VERSION, PAGE_CAP | RESUME_CAP, resume), Err(WelcomeError::Resume), "not asked");
}

// REQ-INT-125, REQ-RCH-013: what is kept is bounded; a message that does not fit, or
// more unacknowledged messages than are remembered, end the session's
// chance to resume, and the next connection is a new session.
#[test]
fn what_is_kept_is_bounded() {
    let mut session = resumable();
    let paste: std::string::String = core::iter::repeat_n('x', crate::MAX_PASTE).collect();
    for _ in 0..RESEND_BYTES / crate::MAX_PASTE - 1 {
        assert!(sent_any(&mut session, &Item::Event(Event::Paste(&paste))));
        assert!(session.resumable());
    }
    assert!(sent_any(&mut session, &Item::Event(Event::Paste(&paste))));
    assert!(sent_any(&mut session, &Item::Event(Event::Paste(&paste))), "one too many to keep");
    assert!(!session.resumable());
    session.close();
    assert_eq!(opening(&mut session, 0, None).len(), 1, "HELLO alone: a new session");
    let mut session = resumable();
    for _ in 0..REMEMBERED {
        session.record(&Item::Event(Event::FocusIn), &[0xFF, 0x05]);
    }
    assert!(session.resumable());
    session.record(&Item::Event(Event::FocusIn), &[0xFF, 0x05]);
    assert!(!session.resumable(), "past the ring");
}

// REQ-INT-125, REQ-RCH-013: a plain session never offers RESUME.
#[test]
fn a_session_that_keeps_nothing_never_offers_resume() {
    let mut session = Session::new(PAGE_CAP | RESUME_CAP);
    assert_eq!(opening(&mut session, 0, None), [Event::Hello { version: VERSION, caps: PAGE_CAP, agent: None }]);
    let resume = Some(Resumption { token: TOKEN, taken: None });
    assert_eq!(session.welcome(VERSION, PAGE_CAP | RESUME_CAP, resume), Err(WelcomeError::Caps));
}
