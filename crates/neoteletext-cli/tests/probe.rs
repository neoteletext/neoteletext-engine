//! `neottx probe` (DEC-1759 lesson 20) against a small channel of this test:
//! what it reports, and that a fault gives a non-zero exit status.
#![cfg(feature = "live")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Command, Output};

use neoteletext_formats::encode::encode_page;
use neoteletext_formats::item::{Item as ReadItem, Reader};
use neoteletext_formats::tti::TtiReader;
use neoteletext_input::VERSION;
use neoteletext_input::caps::SELECT_CAP;
use neoteletext_input::control::{Control, packet_of};
use neoteletext_input::event::{Event, Item, read_message};
use neoteletext_web::channel::SETTINGS_FILE;
use neoteletext_web::consts::{CLOSE_BROKE_PROTOCOL, PUSH_SUBPROTOCOL};
use neoteletext_web::records::denied::{DeniedRecord, STATUS_NOT_FOUND, STATUS_UNAVAILABLE, encode};
use neoteletext_web::ws::accept_for;

/// RFC 6455 section 5.2.
const FIN: u8 = 0x80;
const BINARY: u8 = 0x2;
const CLOSE: u8 = 0x8;
const OPCODE: u8 = 0x0F;
const MASK_BYTES: usize = 4;
const MASKED_LENGTH: u8 = 0x7F;
const LENGTH_16: u8 = 126;

/// How the test's channel behaves.
#[derive(Clone, Copy)]
enum Channel {
    /// Answers page 100 and refuses every other with 404.
    Good,
    /// Answers the upgrade without the subprotocol.
    NoSubprotocol,
    /// Refuses the upgrade.
    Refuses,
    /// Closes with the stop code at HELLO.
    Stops,
    /// Says its program did not answer.
    Unavailable,
    /// Never answers a selection.
    Silent,
}

fn page() -> Vec<u8> {
    let mut packets = Vec::new();
    let mut reader = TtiReader::new();
    let mut each = |item: ReadItem<'_>| {
        if let ReadItem::Page(page) = item {
            encode_page(page, &mut |packet| packets.extend_from_slice(&packet.bytes));
        }
    };
    reader.feed(b"PN,10000\r\nSC,0000\r\nPS,8000\r\nOL,1,Hello\r\n", &mut each);
    reader.finish(&mut each);
    packets
}

fn denied(status: u16, place: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode(DeniedRecord { status, place }, 0, 1, &mut |packet| bytes.extend_from_slice(&packet.bytes)).unwrap();
    bytes
}

fn frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) {
    let mut bytes = vec![FIN | opcode];
    if payload.len() < usize::from(LENGTH_16) {
        bytes.push(u8::try_from(payload.len()).unwrap());
    } else {
        bytes.push(LENGTH_16);
        bytes.extend_from_slice(&u16::try_from(payload.len()).unwrap().to_be_bytes());
    }
    bytes.extend_from_slice(payload);
    let _ = stream.write_all(&bytes);
}

/// One client frame's opcode and payload.
fn read_frame(reader: &mut BufReader<TcpStream>) -> Option<(u8, Vec<u8>)> {
    let mut head = [0_u8; 2];
    reader.read_exact(&mut head).ok()?;
    let length = match head[1] & MASKED_LENGTH {
        LENGTH_16 => {
            let mut bytes = [0_u8; 2];
            reader.read_exact(&mut bytes).ok()?;
            usize::from(u16::from_be_bytes(bytes))
        }
        short => usize::from(short),
    };
    let mut mask = [0_u8; MASK_BYTES];
    reader.read_exact(&mut mask).ok()?;
    let mut payload = vec![0_u8; length];
    reader.read_exact(&mut payload).ok()?;
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[index % MASK_BYTES];
    }
    Some((head[0] & OPCODE, payload))
}

fn serve_socket(channel: Channel, stream: &mut TcpStream, reader: &mut BufReader<TcpStream>) {
    while let Some((opcode, payload)) = read_frame(reader) {
        if opcode != BINARY {
            continue;
        }
        let mut answers: Vec<(u8, Vec<u8>)> = Vec::new();
        read_message(&payload, &mut |item| match item {
            Item::Event(Event::Hello { version, caps, .. }) => {
                assert_eq!(caps, SELECT_CAP);
                assert_eq!(version, VERSION);
                if matches!(channel, Channel::Stops) {
                    answers.push((CLOSE, CLOSE_BROKE_PROTOCOL.to_be_bytes().to_vec()));
                } else {
                    let control = Control::Welcome { version, caps, resume: None };
                    answers.push((BINARY, packet_of(&control, 0, 0).unwrap().bytes.to_vec()));
                }
            }
            Item::Event(Event::Select(select, _)) => match channel {
                Channel::Unavailable => answers.push((BINARY, denied(STATUS_UNAVAILABLE, ""))),
                Channel::Silent => {}
                _ if select.page == 0x100 => answers.push((BINARY, page())),
                _ => answers.push((BINARY, denied(STATUS_NOT_FOUND, "8FC"))),
            },
            _ => {}
        })
        .unwrap();
        for (opcode, bytes) in answers {
            frame(stream, opcode, &bytes);
        }
    }
}

fn serve(channel: Channel, listener: &TcpListener) {
    for stream in listener.incoming().flatten() {
        std::thread::spawn(move || {
            let mut stream = stream;
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or_default().to_owned();
            let (mut key, mut asked) = (None, None);
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                let Some((name, value)) = line.split_once(':') else { continue };
                if name.eq_ignore_ascii_case("sec-websocket-key") {
                    key = Some(value.trim().to_owned());
                } else if name.eq_ignore_ascii_case("sec-websocket-protocol") {
                    asked = Some(value.trim().to_owned());
                }
            }
            if path.ends_with(SETTINGS_FILE) {
                let body = r#"{"name":"Probe","initial_page":"100","push":{"input":true}}"#;
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.shutdown(Shutdown::Write);
            } else if let (true, Some(key)) = (path.ends_with("/push"), key) {
                assert_eq!(asked.as_deref(), Some(PUSH_SUBPROTOCOL));
                if matches!(channel, Channel::Refuses) {
                    let _ = write!(stream, "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    return;
                }
                let named = if matches!(channel, Channel::NoSubprotocol) {
                    String::new()
                } else {
                    format!("Sec-WebSocket-Protocol: {PUSH_SUBPROTOCOL}\r\n")
                };
                let accept = accept_for(key.as_bytes());
                let _ = write!(
                    stream,
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n{named}Sec-WebSocket-Accept: {}\r\n\r\n",
                    String::from_utf8_lossy(&accept)
                );
                serve_socket(channel, &mut stream, &mut reader);
            } else {
                let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        });
    }
}

/// `neottx probe` of the test's channel.
fn probe(channel: Channel) -> Output {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let port = listener.local_addr().expect("its address").port();
    std::thread::spawn(move || serve(channel, &listener));
    Command::new(env!("CARGO_BIN_EXE_neottx"))
        .args(["probe", &format!("http://127.0.0.1:{port}/ch/"), "--allow-local", "127.0.0.1", "--no-sandbox"])
        .output()
        .expect("run neottx probe")
}

fn text(output: &[u8]) -> String {
    String::from_utf8_lossy(output).into_owned()
}

/// DEC-1759 lesson 20: a good channel is reported step by step, and each
/// selection's ending is named.
#[test]
fn a_good_channel_is_reported_and_exits_zero() {
    let output = probe(Channel::Good);
    let said = text(&output.stdout);
    assert!(output.status.success(), "{said} {}", text(&output.stderr));
    for expected in [
        "initial     page 100",
        &format!("handshake   accepted: {PUSH_SUBPROTOCOL}"),
        &format!("protocol    input protocol {VERSION}, WELCOME received"),
        "selection   100: a page in",
        "selection   8FC: DENIED 404",
        "result      no fault",
    ] {
        assert!(said.contains(expected), "{expected:?} not in: {said}");
    }
}

/// DEC-1759 lesson 20: each fault gives a non-zero exit status, and says why.
#[test]
fn a_fault_exits_non_zero_and_says_why() {
    for (channel, why) in [
        (Channel::NoSubprotocol, "does not speak"),
        (Channel::Refuses, "status 403"),
        (Channel::Stops, "stop code 4000"),
        (Channel::Unavailable, "DENIED 503"),
        (Channel::Silent, "no page and no DENIED"),
    ] {
        let output = probe(channel);
        let said = text(&output.stderr);
        assert!(!output.status.success(), "{why}");
        assert!(said.contains(why), "{why:?} not in: {said}");
    }
}
