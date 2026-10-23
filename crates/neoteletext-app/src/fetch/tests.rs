//! The live feed's loop against a server played in memory: the feed's
//! own deadlines decide when the loop wakes (decision E.97).
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "a test")]

use super::wakes::{Wake, Wakes};
use super::zxnet::live_loop;
use neoteletext_core::types::Instant;
use neoteletext_web::port::{ConnectionId, LinkState, Received, Security, Transport};
use neoteletext_web::url::IpAddress;
use std::collections::VecDeque;
use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant as StdInstant};

/// How long the tests give the loop.
const LIMIT: Duration = Duration::from_secs(12);
/// A public address for the server, so no local-network rule applies.
const PEER: IpAddress = IpAddress::V4([93, 184, 216, 34]);
/// RFC 6455 section 5.2: a final text frame.
const TEXT_FRAME: u8 = 0x81;

/// What the server does once the WebSocket is open.
#[derive(Clone, Copy, PartialEq)]
enum Script {
    /// Says nothing more.
    Silent,
    /// Refuses the service name, which stops the feed.
    WrongName,
}

#[derive(Default)]
struct Server {
    open: bool,
    to_client: VecDeque<u8>,
    opened_at: Option<StdInstant>,
    closed_at: Option<StdInstant>,
}

struct Fake {
    server: Arc<Mutex<Server>>,
    script: Script,
    wake: Sender<Wake>,
}

impl embedded_io::ErrorType for Fake {
    type Error = embedded_io::ErrorKind;
}

impl Transport for Fake {
    fn connect(&mut self, _: ConnectionId, _: &str, _: u16, _: Security) -> Result<(), Self::Error> {
        self.server.lock().unwrap().open = true;
        Ok(())
    }

    fn state(&mut self, _: ConnectionId) -> LinkState {
        if self.server.lock().unwrap().open { LinkState::Open(PEER) } else { LinkState::Closed }
    }

    fn send(&mut self, _: ConnectionId, bytes: &[u8]) -> Result<(), Self::Error> {
        let mut server = self.server.lock().unwrap();
        if server.opened_at.is_some() {
            return Ok(());
        }
        // The upgrade request: answer it, then follow the script.
        let request = String::from_utf8_lossy(bytes);
        let key = request
            .lines()
            .find_map(|line| line.split_once(':').filter(|(name, _)| name.eq_ignore_ascii_case("sec-websocket-key")))
            .map(|(_, value)| value.trim().to_owned())
            .unwrap();
        let accept = neoteletext_web::ws::accept_for(key.as_bytes());
        let head = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            String::from_utf8_lossy(&accept)
        );
        server.to_client.extend(head.bytes());
        if self.script == Script::WrongName {
            let message = br#"["wrongname"]"#;
            server.to_client.extend([TEXT_FRAME, u8::try_from(message.len()).unwrap()]);
            server.to_client.extend(message);
        }
        server.opened_at = Some(StdInstant::now());
        let _ = self.wake.send(Wake::Network);
        Ok(())
    }

    fn receive(&mut self, _: ConnectionId, buffer: &mut [u8]) -> Result<Received, Self::Error> {
        let mut server = self.server.lock().unwrap();
        if !server.open {
            return Ok(Received::Closed);
        }
        let count = server.to_client.read(buffer).unwrap_or(0);
        Ok(if count == 0 { Received::Nothing } else { Received::Data(count) })
    }

    fn close(&mut self, _: ConnectionId) {
        let mut server = self.server.lock().unwrap();
        server.open = false;
        // The slot is closed once before anything opens; only a close
        // of the open WebSocket counts.
        if server.opened_at.is_some() {
            server.closed_at.get_or_insert_with(StdInstant::now);
        }
    }
}

/// Runs the loop against the script on a thread; returns the server,
/// the loop's count of sleeps, and the sender that ends it.
fn start(script: Script) -> (Arc<Mutex<Server>>, Arc<std::sync::atomic::AtomicUsize>, Sender<Wake>) {
    let (wake, receiver) = mpsc::channel();
    let mut wakes = Wakes::new(receiver);
    let waits = wakes.waits.clone();
    let server = Arc::new(Mutex::new(Server::default()));
    let fake = Fake { server: server.clone(), script, wake: wake.clone() };
    let channel = neoteletext_web::zxnet::Channel::from_address("wss://feed.example/live", "test").unwrap();
    std::thread::spawn(move || live_loop(&channel, &mut wakes, fake, &[], false, &mut |_| Ok(())));
    (server, waits, wake)
}

/// Waits for the server to see the connection close.
fn closed(server: &Mutex<Server>) -> (StdInstant, StdInstant) {
    let started = StdInstant::now();
    loop {
        let state = server.lock().unwrap();
        if let (Some(opened), Some(closed)) = (state.opened_at, state.closed_at) {
            return (opened, closed);
        }
        drop(state);
        assert!(started.elapsed() < LIMIT, "the connection was never closed");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A socket's wake taken while the loop looked for requests is not
/// lost: the loop does not then sleep with the socket's bytes unread.
/// It did, until the next timer, when a channel's answer arrived
/// between the loop's look at its sockets and its look for requests: a
/// key's answer waited for the heartbeat, 25 s.
#[test]
fn a_network_wake_taken_with_the_requests_is_not_lost() {
    let (wake, receiver) = mpsc::channel();
    let mut wakes = Wakes::new(receiver);
    wake.send(Wake::Network).unwrap();
    assert!(wakes.next_request().is_none(), "no request, only bytes");
    let started = StdInstant::now();
    wakes.wait(Instant(0), Some(Instant(5_000_000)));
    assert!(started.elapsed() < Duration::from_secs(1), "slept {:?} with bytes waiting", started.elapsed());
}

#[test]
fn a_silent_feed_is_dropped_when_its_inactivity_deadline_passes() {
    // The feed's inactivity deadline, 5 s after the handshake, was lost
    // behind the handshake's 30 s one: the silence went unnoticed, and
    // the first keepalive went, about 25 s late.
    let (server, _, end) = start(Script::Silent);
    let (opened, closed) = closed(&server);
    let silence = closed.duration_since(opened);
    let inactivity = Duration::from_micros(neoteletext_web::consts::ZXNET_INACTIVITY_US);
    assert!(silence >= inactivity && silence < inactivity * 3 / 2, "dropped after {silence:?}");
    let _ = end.send(Wake::Ended);
}

#[test]
fn a_stopped_feed_sleeps_until_something_happens() {
    // A feed that has stopped asks for no tick; the loop slept 200 ms
    // and looked again, five times a second, for ever, once its stale
    // handshake deadline had passed: run 32 s after the stop on 21
    // October 2026, the old loop woke 5 times in the second counted.
    let (server, waits, end) = start(Script::WrongName);
    closed(&server);
    std::thread::sleep(Duration::from_millis(100));
    let before = waits.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_secs(1));
    let woken = waits.load(Ordering::Relaxed) - before;
    assert!(woken <= 1, "the stopped loop woke {woken} times in a second");
    let _ = end.send(Wake::Ended);
}
