//! The WebSocket client's waits: a receive that may not wait looks at what
//! has arrived and returns, answering a ping and seeing a close, as
//! `neottx keyboard` asks of the channel's socket between the phone's keys
//! (decision E.94).
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, reason = "a test")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use neoteletext_qrkeyboard::net::{Incoming, WebSocket, parse};
use neoteletext_web::ws::accept_for;

/// RFC 6455 section 5.2: the FIN bit, the opcodes and the mask bit.
const FIN: u8 = 0x80;
const OPCODE: u8 = 0x0F;
const PING: u8 = 0x9;
const CLOSE: u8 = 0x8;
const PONG: u8 = 0xA;
const MASKED: u8 = 0x80;
const LENGTH: u8 = 0x7F;
const MASK_BYTES: usize = 4;
/// RFC 6455 section 7.4.1: a normal closure.
const NORMAL: u16 = 1000;
/// How long the test gives the client before failing.
const LIMIT: Duration = Duration::from_secs(5);

/// Accepts one client, completes the handshake, sends a ping and a close,
/// and reports the opcodes the client sends back.
fn server(listener: &TcpListener, replies: &mpsc::Sender<u8>) {
    let (stream, _) = listener.accept().unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut stream = stream;
    let mut key = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("sec-websocket-key")
        {
            value.trim().clone_into(&mut key);
        }
    }
    let accept = accept_for(key.as_bytes());
    write!(
        stream,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        String::from_utf8_lossy(&accept)
    )
    .unwrap();
    stream.write_all(&[FIN | PING, 0]).unwrap();
    let mut close = vec![FIN | CLOSE, 2];
    close.extend_from_slice(&NORMAL.to_be_bytes());
    stream.write_all(&close).unwrap();
    // The client's frames are masked: two head bytes, the mask, the payload.
    loop {
        let mut head = [0_u8; 2];
        if reader.read_exact(&mut head).is_err() {
            return;
        }
        let length = usize::from(head[1] & LENGTH);
        let mut rest = vec![0_u8; length + if head[1] & MASKED == 0 { 0 } else { MASK_BYTES }];
        if reader.read_exact(&mut rest).is_err() || replies.send(head[0] & OPCODE).is_err() {
            return;
        }
    }
}

#[test]
fn a_receive_that_may_not_wait_answers_a_ping_and_sees_the_close() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (replies, answers) = mpsc::channel();
    std::thread::spawn(move || server(&listener, &replies));
    let mut socket = WebSocket::connect(&parse(&format!("ws://127.0.0.1:{port}/")).unwrap(), None, None).unwrap();

    let started = Instant::now();
    let closed = loop {
        assert!(started.elapsed() < LIMIT, "the close never came");
        // Nothing waits here: a receive with no time to wait returns at
        // once with what has arrived, or with nothing.
        let looked = Instant::now();
        let incoming = socket.receive(Duration::ZERO).expect("a receive that may not wait");
        assert!(looked.elapsed() < LIMIT / 10, "the receive waited");
        if let Some(Incoming::Closed(code)) = incoming {
            break code;
        }
    };
    assert_eq!(closed, Some(NORMAL));
    let answered: Vec<u8> = answers.iter().take(2).collect();
    assert_eq!(answered, vec![PONG, CLOSE], "the ping was answered, then the close");
}
