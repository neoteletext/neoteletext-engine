//! Typing back through the fetcher process (REQ-INT-008, REQ-INT-112,
//! decisions E.94 and E.97): an
//! input message from the decoder goes to the channel's push WebSocket at
//! once, and the channel's answer comes back at once. The fetcher sleeps
//! until a request or a socket wakes it, never on a fixed timer, so the
//! round trip on this computer's own network is well under a millisecond
//! or two. Only a build with live feeds has the fetcher (decision E.61).
#![cfg(feature = "live")]
#![allow(missing_docs)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use neoteletext_app::pipe::{self, Frame, Notice, Origin};
use neoteletext_input::MAX_MESSAGE;
use neoteletext_input::event::{Item, write};
use neoteletext_web::ws::accept_for;

/// Keys typed, a twentieth of a second apart (the browser's design,
/// `interaction.md` section 2).
const KEYS: u16 = 20;
const KEY_INTERVAL: Duration = Duration::from_millis(50);
/// How long anything may take before the test gives up.
const LIMIT: Duration = Duration::from_secs(10);
/// The round trip's median must stay under this. When the fetcher slept
/// 10 ms whenever it was idle, a key waited for the sleep to end before it
/// went and its answer waited again: medians of 17 to 21 ms in a debug
/// build on 21 October 2026, against 0.2 to 0.5 ms once requests and
/// sockets woke it.
const WAKE_BUDGET: Duration = Duration::from_millis(2);

/// RFC 6455 section 5.2: the FIN bit, the opcodes and the length forms.
const FIN: u8 = 0x80;
const OPCODE: u8 = 0x0F;
const TEXT: u8 = 0x1;
const BINARY: u8 = 0x2;
const CLOSE: u8 = 0x8;
const MASKED: u8 = 0x80;
const LENGTH: u8 = 0x7F;
const LENGTH_16: u8 = 126;
const LENGTH_64: u8 = 127;
const MASK_BYTES: usize = 4;

/// The channel's descriptor: it pushes over a WebSocket and takes keys.
fn descriptor(port: u16) -> String {
    format!(
        r#"{{"name":"Typing","initial_page":"100","page_template":"/{{mpp}}.tti","push":{{"address":"ws://127.0.0.1:{port}/ws","method":"websocket","input":true}}}}"#
    )
}
/// The page the channel shows, and pushes again for each key.
fn page(keys: u16) -> String {
    format!("PN,10000\r\nSC,0000\r\nPS,8000\r\nOL,1,Keys: {keys}\r\n")
}

struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Reads one request head; its path.
fn request_path(reader: &mut BufReader<TcpStream>) -> Option<(String, Option<String>)> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let path = line.split_whitespace().nth(1)?.to_owned();
    let mut key = None;
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("sec-websocket-key")
        {
            key = Some(value.trim().to_owned());
        }
    }
    Some((path, key))
}

fn respond(stream: &mut TcpStream, status: &str, media: &str, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {media}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.shutdown(Shutdown::Write);
}

/// Reads one client frame (always masked); its opcode and payload.
fn read_frame(reader: &mut BufReader<TcpStream>) -> Option<(u8, Vec<u8>)> {
    let mut head = [0u8; 2];
    reader.read_exact(&mut head).ok()?;
    let length = match head[1] & LENGTH {
        LENGTH_16 => {
            let mut bytes = [0u8; 2];
            reader.read_exact(&mut bytes).ok()?;
            usize::from(u16::from_be_bytes(bytes))
        }
        LENGTH_64 => {
            let mut bytes = [0u8; 8];
            reader.read_exact(&mut bytes).ok()?;
            usize::try_from(u64::from_be_bytes(bytes)).ok()?
        }
        short => usize::from(short),
    };
    let mut mask = [0u8; MASK_BYTES];
    if head[1] & MASKED != 0 {
        reader.read_exact(&mut mask).ok()?;
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).ok()?;
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[index % MASK_BYTES];
    }
    Some((head[0] & OPCODE, payload))
}

/// Writes one server frame (never masked).
fn write_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = vec![FIN | opcode];
    match u16::try_from(payload.len()) {
        Ok(short) if short < u16::from(LENGTH_16) => frame.push(u8::try_from(short).unwrap_or(0)),
        Ok(medium) => {
            frame.push(LENGTH_16);
            frame.extend_from_slice(&medium.to_be_bytes());
        }
        Err(_) => {
            frame.push(LENGTH_64);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)
}

/// The channel: its descriptor, its page, and a push WebSocket that
/// answers every input message with the page again.
fn serve(listener: &TcpListener) {
    let port = listener.local_addr().map_or(0, |address| address.port());
    for stream in listener.incoming().flatten() {
        std::thread::spawn(move || {
            let _ = stream.set_nodelay(true);
            let Ok(clone) = stream.try_clone() else { return };
            let mut reader = BufReader::new(clone);
            let mut stream = stream;
            let Some((path, key)) = request_path(&mut reader) else { return };
            match (path.as_str(), key) {
                ("/service.json", _) => respond(&mut stream, "200 OK", "application/json", &descriptor(port)),
                ("/100.tti", _) => respond(&mut stream, "200 OK", "text/x-teletext-tti", &page(0)),
                ("/ws", Some(key)) => {
                    let accept = accept_for(key.as_bytes());
                    let _ = write!(
                        stream,
                        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                        String::from_utf8_lossy(&accept)
                    );
                    let mut keys = 0;
                    while let Some((opcode, _)) = read_frame(&mut reader) {
                        match opcode {
                            BINARY => {
                                keys += 1;
                                if write_frame(&mut stream, TEXT, page(keys).as_bytes()).is_err() {
                                    return;
                                }
                            }
                            CLOSE => return,
                            // The subscription, and anything else, is not a key.
                            _ => {}
                        }
                    }
                }
                _ => respond(&mut stream, "404 Not Found", "text/plain", ""),
            }
        });
    }
}

/// A key typed, as the input stream has it: its text (INPUT.md section
/// 5.3).
fn key_message(character: char) -> Vec<u8> {
    let mut text = [0u8; 4];
    let mut out = [0u8; MAX_MESSAGE];
    let length = write(&Item::Text(character.encode_utf8(&mut text)), &mut out).expect("an input message");
    out[..length].to_vec()
}

fn median(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

#[test]
fn keys_reach_the_channel_and_its_answer_comes_back_at_once() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let port = listener.local_addr().expect("its address").port();
    std::thread::spawn(move || serve(&listener));

    let mut fetcher = Killed(
        Command::new(env!("CARGO_BIN_EXE_neottx"))
            .args(["fetch", &format!("http://127.0.0.1:{port}/service.json"), "--allow-local", "127.0.0.1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start the fetcher"),
    );
    let mut input = fetcher.0.stdin.take().expect("its input");
    let mut output = fetcher.0.stdout.take().expect("its output");
    let (frames, received) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(Some(frame)) = pipe::read(&mut output) {
            if frames.send((Instant::now(), frame)).is_err() {
                return;
            }
        }
    });
    let wait_for = |wanted: &dyn Fn(&Frame) -> bool| -> Instant {
        loop {
            let (at, frame) = received.recv_timeout(LIMIT).expect("the fetcher answered");
            if wanted(&frame) {
                return at;
            }
        }
    };
    wait_for(&|frame| matches!(frame, Frame::Notice(Notice::Input(true))));

    let mut round_trips = Vec::new();
    for index in 0..KEYS {
        let sent = Instant::now();
        pipe::write(&mut input, &Frame::Input(key_message('a'))).expect("send a key");
        input.flush().expect("send a key");
        let pushed = wait_for(
            &|frame| matches!(frame, Frame::Tti(data) if data.origin == Origin::Push && data.bytes.ends_with(format!("Keys: {}\r\n", index + 1).as_bytes())),
        );
        round_trips.push(pushed.duration_since(sent));
        std::thread::sleep(KEY_INTERVAL.saturating_sub(sent.elapsed()));
    }
    let worst = round_trips.iter().max().copied().unwrap_or_default();
    let middle = median(round_trips.clone());
    eprintln!("typing over {KEYS} keys: key to pushed page median {middle:?} worst {worst:?}");
    assert!(middle <= WAKE_BUDGET, "the fetcher waited before sending or before reading: {round_trips:?}");
}
