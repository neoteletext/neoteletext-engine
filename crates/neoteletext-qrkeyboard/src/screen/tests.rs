//! The screen's handling of the phone's frames, with no relay: the frames
//! a relay would deliver are handed to it, and what it would send is lost.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "a test")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::thread;

use super::*;
use crate::config::Layout;
use crate::crypto::Role;
use crate::json::Value;
use crate::pairing::Outcome;

/// The join code the screens here were given.
const CODE: &str = "ABCDEFGHIJKLMN27";
/// The no-PIN key the QR would carry.
const KEY: [u8; SECRET_BYTES] = [7; SECRET_BYTES];

impl Screen {
    /// A screen with no relay, in no-PIN mode.
    fn offline() -> Self {
        let config = ScreenConfig { name: "test".into(), layout: Layout::Terminal, pin: false, rejoin_seconds: None };
        let hash = config.hash();
        let (wake_sender, wakes) = mpsc::channel();
        Self {
            relay: net::parse("http://127.0.0.1:1").unwrap(),
            config,
            hash,
            join: Join { code: CODE.into(), url: String::new(), pin: None },
            owner: "owner".into(),
            socket_address: net::parse("ws://127.0.0.1:1/s").unwrap(),
            secret: Secret::Key(KEY),
            socket: None,
            pairing: None,
            channel: None,
            pin_attempts: 0,
            locked: false,
            finished: false,
            keepalive: Instant::now(),
            reconnect_at: None,
            reconnect_delay: RECONNECT_FIRST,
            acknowledged: VecDeque::new(),
            held: Vec::new(),
            proxy: None,
            wakes,
            wake_sender,
        }
    }
}

/// Runs a pairing between the screen and a phone holding `key`: the
/// phone's sealer, and the screen's events.
fn pair(screen: &mut Screen, key: [u8; SECRET_BYTES]) -> (Sealer, Vec<Event>) {
    let device = Pairing::new(Role::Device, CODE, &screen.secret, &screen.hash).unwrap();
    let to_phone = device.start();
    screen.pairing = Some(device);
    let mut phone = Pairing::new(Role::Phone, CODE, &Secret::Key(key), &screen.hash).unwrap();
    let to_screen = phone.start();
    let mut sealer = None;
    for frame in to_phone {
        if let (_, Some(Outcome::Paired(channel))) = phone.receive(&frame) {
            sealer = Some(channel.0);
        }
    }
    let mut events = Vec::new();
    for frame in to_screen {
        screen.binary(&frame, &mut events);
    }
    (sealer.expect("the phone paired"), events)
}

/// The phone's message `kind` for `key`, sealed.
fn sealed(sealer: &mut Sealer, kind: &str, key: &str) -> Vec<u8> {
    let id = message::new_id().unwrap();
    let message =
        Value::object(&[("id", Value::string(&id)), ("type", Value::string(kind)), ("key", Value::string(key))]);
    sealer.seal(&message::pack(&message).unwrap()).unwrap()
}

/// What the screen reports for one frame.
fn receive(screen: &mut Screen, frame: &[u8]) -> Vec<Event> {
    let mut events = Vec::new();
    screen.binary(frame, &mut events);
    events
}

#[test]
fn join_codes_are_the_relays() {
    assert!(is_code("ABCDEFGHIJKLMN27"));
    assert!(!is_code("ABCDEFGHIJKLMN28"), "8 is not base-32");
    assert!(!is_code("abcdefghijklmn27"));
    assert!(!is_code("ABC"));
}

/// Finding F-38, decision E.116 item 1: a release is passed on only for a
/// key the screen holds, so a phone cannot send releases of keys it never
/// pressed, which no limit on presses bounds.
#[test]
fn a_release_of_a_key_not_held_is_dropped() {
    let mut screen = Screen::offline();
    let (mut phone, _) = pair(&mut screen, KEY);
    let down = sealed(&mut phone, "down", "ArrowUp");
    assert!(receive(&mut screen, &down).contains(&Event::Down("ArrowUp".into())));
    let up = sealed(&mut phone, "up", "ArrowUp");
    assert_eq!(receive(&mut screen, &up), [Event::Up("ArrowUp".into())]);
    let again = sealed(&mut phone, "up", "ArrowUp");
    assert_eq!(receive(&mut screen, &again), [], "a second release of a released key");
    let never = sealed(&mut phone, "up", "ArrowLeft");
    assert_eq!(receive(&mut screen, &never), [], "a release of a key never pressed");
}

/// A relay played here: it answers one `POST /i` with the instance `answer`
/// names, on its own address, and closes. Its origin.
fn instance_relay(owner_token_json: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
        }
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body).unwrap();
        let answer =
            format!(r#"{{"code":"{CODE}","ownerToken":"{owner_token_json}","relay":"ws://127.0.0.1:{port}/s"}}"#);
        let mut stream = stream;
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len())
            .unwrap();
    });
    format!("http://127.0.0.1:{port}")
}

/// Finding F-39, decision E.116 item 2: the relay's owner token is written
/// into `Authorization: Bearer`, so it is held to the message id alphabet,
/// and a token with a line break, a space or a byte outside ASCII is
/// refused before anything is sent with it.
#[test]
fn an_owner_token_outside_the_id_alphabet_is_refused() {
    // As JSON writes them: a CR LF that would start a header of its own,
    // a space, a non-ASCII letter and a NUL.
    for token in [r"owner\r\nX-Injected: 1", "own er", r"ownér", r"own\u0000er"] {
        let config = ScreenConfig { name: "test".into(), layout: Layout::Terminal, pin: false, rejoin_seconds: None };
        let result = Screen::create(&instance_relay(token), config, None);
        assert_eq!(result.err(), Some(Error::Protocol("an owner token")), "{token}");
    }
}
