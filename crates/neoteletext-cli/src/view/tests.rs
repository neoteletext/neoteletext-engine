#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]

use super::terminal_io::{Wakened, next_event};
use super::typing::{Typing, host_page, input_of};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use neoteletext_input::event::Stamp;
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

/// The page on screen in these tests: 100, sub-code 0000.
const ON: Stamp = Stamp { page: 0x100, sub: 0 };

fn sent(code: KeyCode, modifiers: KeyModifiers) -> Option<Vec<u8>> {
    let mut buffer = [0_u8; 4];
    let item = input_of(KeyEvent::new(code, modifiers), ON, &mut buffer)?;
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
    assert_eq!(sent(KeyCode::Char('a'), none), Some(vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x10, b'a', 1, b'a']));
    assert_eq!(
        sent(KeyCode::Char('A'), KeyModifiers::SHIFT),
        Some(vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x18, b'a', 0x01, 1, b'A']),
        "the key unshifted, Shift in mods, the text as typed"
    );
    assert_eq!(
        sent(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x08, b'c', 0x04])
    );
    assert_eq!(sent(KeyCode::Enter, none), Some(vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x04, 0x02]));
    assert_eq!(sent(KeyCode::Up, KeyModifiers::SHIFT), Some(vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x0C, 0x09, 0x01]));
    assert_eq!(sent(KeyCode::BackTab, none), Some(vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x0C, 0x03, 0x01]), "Shift-Tab");
    // `key` is the code point as a varint, `text` the UTF-8 with its length.
    let typed = |character: char| {
        let mut bytes = vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x10];
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
        let item = input_of(key, ON, &mut buffer).unwrap();
        let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
        let length = neoteletext_input::event::write(&item, &mut out).unwrap();
        out[..length].to_vec()
    };
    assert_eq!(encode(event(KeyEventKind::Repeat)), vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x11, b'a', 1, b'a']);
    assert_eq!(
        encode(event(KeyEventKind::Release)),
        vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x02, b'a'],
        "a release, no text"
    );
    let up = KeyEvent::new_with_kind(KeyCode::Up, KeyModifiers::NONE, KeyEventKind::Release);
    assert_eq!(encode(up), vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x06, 0x09]);
}

/// REQ-INT-113: a cursor record naming another page opens it, only
/// while keys go to the channel.
#[test]
fn a_host_goes_to_a_page_by_naming_it() {
    assert_eq!(host_page(0x301, 0x300, true), Some(0x301));
    assert_eq!(host_page(0x300, 0x300, true), None, "the page already open");
    assert_eq!(host_page(0x301, 0x300, false), None, "keys do not go to the channel");
}

/// REQ-INT-110 as amended by REQ-RCH-012 (decision E.133): the channel's
/// acknowledgement is taken where it comes, in the answer's own frame
/// among the page's packets, and alone.
#[test]
fn an_acknowledgement_is_taken_in_the_answers_frame_and_alone() {
    use neoteletext_core::packet::{PacketMeta, encode};
    use neoteletext_core::types::Magazine;
    use neoteletext_input::cursor::{Cursor, from_packet};
    use neoteletext_input::event::{Event, Item};
    let mut session = neoteletext_app::session::Session::new(neoteletext_core::config::Config::default(), 8);
    session.keep(neoteletext_input::is_input_packet);
    let cursor = |acked| Cursor { page: 0x100, acked, place: None, flags: 0 }.packet(3, 1).unwrap();
    let magazine = Magazine::new(1).unwrap();
    // The answer: the page's header and a row, and the acknowledgement in
    // the same message.
    for packet in [encode::address(magazine, 0), encode::row(magazine, 1, &[0x20; 40]), cursor(2)] {
        session.push(&packet, &PacketMeta::default());
    }
    // A channel that published nothing: the acknowledgement alone.
    session.push(&cursor(3), &PacketMeta::default());
    let mut input = neoteletext_input::session::Session::new(0);
    let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
    input.open(0, None, &mut out).unwrap();
    input.welcome(neoteletext_input::VERSION, 0, None).unwrap();
    input.send(&Item::Event(Event::FocusIn), 0, &mut out).unwrap();
    input.send(&Item::Event(Event::FocusOut), 0, &mut out).unwrap();
    let mut taken = Vec::new();
    for packet in session.take_kept() {
        let acked = from_packet(&packet).unwrap().unwrap().acked;
        taken.push((acked, input.acknowledged(acked)));
    }
    assert_eq!(taken, [(2, true), (3, true)], "both kept and taken, and nothing else of the answer");
    assert_eq!(input.acked(), 3);
}

/// What a channel is sent.
#[derive(Default)]
struct Recorded(Vec<Vec<u8>>);

impl neoteletext_app::source::Source for Recorded {
    fn poll(
        &mut self,
        _session: &mut neoteletext_app::session::Session,
        _now: neoteletext_core::types::Instant,
    ) -> bool {
        false
    }
    fn send_input(&mut self, message: &[u8]) {
        self.0.push(message.to_vec());
    }
}

/// A session the channel welcomed, what it was sent so far forgotten.
fn welcomed(source: &mut Recorded) -> Typing {
    use neoteletext_input::caps::SELECT_CAP;
    use neoteletext_input::control::{Control, packet_of};
    let mut typing = Typing::new();
    typing.connection(source, true, 0);
    let welcome = Control::Welcome { version: neoteletext_input::VERSION, caps: SELECT_CAP, resume: None };
    let packet = packet_of(&welcome, 1, 1).unwrap();
    assert!(!typing.records(source, std::iter::once(packet), 0));
    source.0.clear();
    typing
}

/// `character` as a press stamped with what is drawn.
fn press(typing: &mut Typing, source: &mut Recorded, character: char, kind: KeyEventKind) {
    let on = typing.stamp().unwrap();
    let mut buffer = [0_u8; 4];
    let mut key = KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE);
    key.kind = kind;
    typing.send(source, &input_of(key, on, &mut buffer).unwrap(), 0);
}

/// REQ-1556, REQ-1090: opening a page sends SELECT, never 0x0A, with 3F7F
/// for the page alone and the sub-code for a sub-page, once a change.
#[test]
fn opening_a_page_sends_select_and_never_page() {
    let mut source = Recorded::default();
    let mut typing = welcomed(&mut source);
    typing.tell_selection(&mut source, 0x100, 0x3F7F, 0);
    typing.tell_selection(&mut source, 0x100, 0x3F7F, 0);
    typing.tell_selection(&mut source, 0x100, 0x0002, 0);
    assert_eq!(
        source.0,
        [vec![0xFF, 0x40, 0x04, 0x80, 0x02, 0xFF, 0x7E], vec![0xFF, 0x40, 0x03, 0x80, 0x02, 0x02]],
        "the page, then its sub-page, and no message of type 0x0A"
    );
}

/// REQ-1555, REQ-1554: a key is stamped with the page drawn, whatever was
/// selected since, and no key goes while nothing is drawn.
#[test]
fn a_key_is_stamped_with_the_page_drawn() {
    let mut source = Recorded::default();
    let mut typing = welcomed(&mut source);
    typing.tell_selection(&mut source, 0x100, 0x3F7F, 0);
    assert_eq!(typing.stamp(), None, "nothing drawn yet");
    typing.set_drawn(Some(ON));
    typing.tell_selection(&mut source, 0x200, 0x3F7F, 0);
    source.0.clear();
    press(&mut typing, &mut source, 'a', KeyEventKind::Press);
    assert_eq!(source.0[0][2..5], [0x80, 0x02, 0x00], "page 100 is still on screen");
}

/// REQ-1554, REQ-207: after `Control-]` no key goes, `FOCUS_OUT` has gone, and
/// the selection stays; a held key goes first, lost, where the terminal
/// reports releases.
#[test]
fn after_leaving_keys_stop_and_focus_goes() {
    let mut source = Recorded::default();
    let mut typing = welcomed(&mut source);
    typing.set_drawn(Some(ON));
    typing.tell_selection(&mut source, 0x100, 0x3F7F, 0);
    press(&mut typing, &mut source, 'a', KeyEventKind::Press);
    press(&mut typing, &mut source, 'a', KeyEventKind::Release);
    press(&mut typing, &mut source, 'b', KeyEventKind::Press);
    source.0.clear();
    assert!(typing.leave_or_return(&mut source, 0));
    assert_eq!(
        source.0,
        [vec![0xFF, 0x01, 0x80, 0x02, 0x00, 0x42, b'b'], vec![0xFF, 0x06]],
        "the held key released as lost, then FOCUS_OUT, and no SELECT"
    );
    assert_eq!(typing.stamp(), None, "no key goes");
}
