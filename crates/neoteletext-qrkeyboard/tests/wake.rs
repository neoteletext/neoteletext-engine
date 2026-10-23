//! The screen's wait (REQ-INT-009, decision E.106): it ends the moment the relay sends
//! something or a waker wakes it, and otherwise sleeps until its limit,
//! against a relay played here: its instance answer and its WebSocket.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, reason = "a test")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use neoteletext_qrkeyboard::config::{Layout, ScreenConfig};
use neoteletext_qrkeyboard::screen::{Event, Screen};
use neoteletext_web::ws::accept_for;

/// RFC 6455 section 5.2: an unmasked final text frame of a short payload.
const FIN_TEXT: u8 = 0x81;
/// A wait that should end at once must end well inside this; the old
/// screen's wait could not be ended at all, and polled every 50 ms.
const AT_ONCE: Duration = Duration::from_millis(40);
/// A wait nothing ends lasts its limit.
const QUIET: Duration = Duration::from_millis(300);
/// How long the wakers wait before they act, so the screen is asleep.
const ASLEEP: Duration = Duration::from_millis(150);
/// The join code the played relay hands out: 16 base-32 symbols.
const CODE: &str = "ABCDEFGHIJKLMN27";

/// Reads one request head; its lines, lower-cased names.
fn read_head(reader: &mut BufReader<TcpStream>) -> Vec<String> {
    let mut lines = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            return lines;
        }
        lines.push(line.trim_end().to_owned());
    }
}

/// The relay: answers `POST /i` with an instance on its own WebSocket,
/// accepts that WebSocket, and then sends each text it is given.
fn relay(listener: &TcpListener, texts: &mpsc::Receiver<String>) {
    let port = listener.local_addr().unwrap().port();
    let (stream, _) = listener.accept().unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let head = read_head(&mut reader);
    let length: usize = head
        .iter()
        .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|value| value.trim().to_owned()))
        .unwrap()
        .parse()
        .unwrap();
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body).unwrap();
    let answer = format!(r#"{{"code":"{CODE}","ownerToken":"owner","relay":"ws://127.0.0.1:{port}/s"}}"#);
    let mut stream = stream;
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len()).unwrap();
    drop(stream);

    let (stream, _) = listener.accept().unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let key = read_head(&mut reader)
        .iter()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("sec-websocket-key").then(|| value.trim().to_owned())
        })
        .unwrap();
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        String::from_utf8_lossy(&accept_for(key.as_bytes()))
    )
    .unwrap();
    // The screen's frames (its auth) are read and dropped.
    thread::spawn(move || {
        let mut sink = [0_u8; 1024];
        while reader.read(&mut sink).is_ok_and(|count| count > 0) {}
    });
    for text in texts {
        let mut frame = vec![FIN_TEXT, u8::try_from(text.len()).unwrap()];
        frame.extend_from_slice(text.as_bytes());
        stream.write_all(&frame).unwrap();
    }
}

/// A screen on a played relay, and the sender of what the relay says.
fn screen() -> (Screen, mpsc::Sender<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (texts, said) = mpsc::channel();
    thread::spawn(move || relay(&listener, &said));
    let config = ScreenConfig { name: "wake test".into(), layout: Layout::Terminal, pin: false, rejoin_seconds: None };
    (Screen::create(&format!("http://127.0.0.1:{port}"), config, None).unwrap(), texts)
}

#[test]
fn a_waker_ends_the_wait_at_once() {
    let (mut screen, _texts) = screen();
    let waker = screen.waker();
    let woke = Instant::now();
    thread::spawn(move || {
        thread::sleep(ASLEEP);
        waker.wake();
    });
    let events = screen.wait(Some(Duration::from_secs(10))).unwrap();
    let took = woke.elapsed();
    assert!(events.is_empty(), "{events:?}");
    let late = took.checked_sub(ASLEEP).unwrap();
    println!("woken {late:?} after the wake");
    assert!(late < AT_ONCE, "the wait went on {late:?} after the wake");
}

#[test]
fn the_relays_message_ends_the_wait_at_once() {
    let (mut screen, texts) = screen();
    let sent = Instant::now();
    thread::spawn(move || {
        thread::sleep(ASLEEP);
        texts.send(r#"{"type":"ready"}"#.to_owned()).unwrap();
        // Kept open: the relay does not close.
        thread::sleep(Duration::from_secs(10));
    });
    let events = screen.wait(Some(Duration::from_secs(10))).unwrap();
    let late = sent.elapsed().checked_sub(ASLEEP).unwrap();
    assert_eq!(events, [Event::Scan]);
    println!("the relay's message reported {late:?} after it went");
    assert!(late < AT_ONCE, "the message waited {late:?}");
}

#[test]
fn a_wait_nothing_ends_lasts_its_limit_and_reports_nothing() {
    let (mut screen, _texts) = screen();
    let started = Instant::now();
    let events = screen.wait(Some(QUIET)).unwrap();
    let took = started.elapsed();
    assert!(events.is_empty(), "{events:?}");
    assert!(took >= QUIET && took < QUIET + AT_ONCE, "{took:?}");
}
