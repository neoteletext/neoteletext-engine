//! An in-memory transport that replays recorded exchanges (REQ-WEB-001
//! verification), and helpers to drive the machines through it.
#![allow(dead_code, clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

use std::collections::{HashMap, VecDeque};
use std::string::{String, ToString};
use std::vec::Vec;

use neoteletext_web::port::{ConnectionId, LinkState, RandomSource, Received, Security, Transport};
use neoteletext_web::url::IpAddress;

/// A reply the scripted server gives to the next request on a host.
#[derive(Clone, Debug)]
pub enum Reply {
    /// These response bytes.
    Bytes(Vec<u8>),
    /// These bytes, then the server closes the connection.
    BytesThenClose(Vec<u8>),
    /// Accept a WebSocket handshake (computing the accept value from the
    /// request's key), then send these frames.
    WebSocket(Vec<u8>),
    /// Accept a WebSocket handshake with a wrong accept value.
    BadWebSocket,
    /// Say nothing.
    Silent,
    /// Close without answering.
    Close,
}

#[derive(Debug)]
struct Connection {
    host: String,
    port: u16,
    state: LinkState,
    inbox: VecDeque<u8>,
    close_when_drained: bool,
    request: Vec<u8>,
    upgraded: bool,
}

/// The replaying transport.
#[derive(Debug, Default)]
pub struct Replay {
    /// Name resolution: host to peer address. Unknown hosts fail.
    pub resolve: HashMap<String, IpAddress>,
    /// Scripted replies per `host:port`, consumed one per request.
    pub replies: HashMap<String, VecDeque<Reply>>,
    /// Every connect: connection, host, port, security.
    pub connects: Vec<(ConnectionId, String, u16, Security)>,
    /// Every complete request or frame sent, per connection.
    pub sent: Vec<(ConnectionId, Vec<u8>)>,
    /// Requests (heads and bodies) as the server saw them.
    pub requests: Vec<(String, Vec<u8>)>,
    /// Bytes handed over per receive call.
    pub chunk: usize,
    /// The port offers TLS.
    pub no_tls: bool,
    connections: HashMap<ConnectionId, Connection>,
}

/// The error type of the replay transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayError;

impl core::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("replay error")
    }
}

impl core::error::Error for ReplayError {}

impl embedded_io::Error for ReplayError {
    fn kind(&self) -> embedded_io::ErrorKind {
        embedded_io::ErrorKind::Other
    }
}

impl embedded_io::ErrorType for Replay {
    type Error = ReplayError;
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The value of a header in a request head, if present.
pub fn header_value(head: &[u8], name: &str) -> Option<String> {
    let text = String::from_utf8_lossy(head);
    text.split("\r\n").skip(1).find_map(|line| {
        let (field, value) = line.split_once(':')?;
        field.eq_ignore_ascii_case(name).then(|| value.trim().to_string())
    })
}

impl Replay {
    /// A transport resolving each `(host, address)` pair.
    pub fn new(hosts: &[(&str, IpAddress)]) -> Self {
        Self { resolve: hosts.iter().map(|(h, a)| (h.to_string(), *a)).collect(), chunk: 7, ..Self::default() }
    }

    /// Adds a scripted reply for `host:port`.
    pub fn reply(&mut self, host_port: &str, reply: Reply) {
        self.replies.entry(host_port.to_string()).or_default().push_back(reply);
    }

    /// Bytes for the client to receive on `connection`, as if the server
    /// sent them.
    pub fn inject(&mut self, connection: ConnectionId, bytes: &[u8]) {
        if let Some(conn) = self.connections.get_mut(&connection) {
            conn.inbox.extend(bytes);
        }
    }

    /// The server drops `connection`.
    pub fn drop_connection(&mut self, connection: ConnectionId) {
        if let Some(conn) = self.connections.get_mut(&connection) {
            conn.close_when_drained = true;
        }
    }

    /// Requests seen by the server for `host:port`.
    pub fn requests_to(&self, host_port: &str) -> Vec<Vec<u8>> {
        self.requests.iter().filter(|(h, _)| h == host_port).map(|(_, r)| r.clone()).collect()
    }

    fn answer(&mut self, connection: ConnectionId) {
        let conn = self.connections.get_mut(&connection).unwrap();
        if conn.upgraded {
            return;
        }
        let Some(end) = find(&conn.request, b"\r\n\r\n") else { return };
        let head_len = end + 4;
        let body_len =
            header_value(&conn.request[..head_len], "Content-Length").map_or(0, |v| v.parse::<usize>().unwrap());
        if conn.request.len() < head_len + body_len {
            return;
        }
        let request: Vec<u8> = conn.request.drain(..head_len + body_len).collect();
        let key = format!("{}:{}", conn.host, conn.port);
        self.requests.push((key.clone(), request.clone()));
        let reply = self.replies.get_mut(&key).and_then(VecDeque::pop_front).unwrap_or(Reply::Silent);
        let conn = self.connections.get_mut(&connection).unwrap();
        match reply {
            Reply::Bytes(bytes) => conn.inbox.extend(bytes),
            Reply::BytesThenClose(bytes) => {
                conn.inbox.extend(bytes);
                conn.close_when_drained = true;
            }
            Reply::WebSocket(frames) => {
                let key = header_value(&request, "Sec-WebSocket-Key").unwrap();
                let accept = neoteletext_web::ws::accept_for(key.as_bytes());
                let head = format!(
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                    core::str::from_utf8(&accept).unwrap()
                );
                conn.inbox.extend(head.as_bytes());
                conn.inbox.extend(frames);
                conn.upgraded = true;
            }
            Reply::BadWebSocket => {
                conn.inbox.extend(
                    b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: AAAAAAAAAAAAAAAAAAAAAAAAAAA=\r\n\r\n",
                );
                conn.upgraded = true;
            }
            Reply::Silent => {}
            Reply::Close => conn.close_when_drained = true,
        }
    }
}

impl Transport for Replay {
    fn connect(
        &mut self,
        connection: ConnectionId,
        host: &str,
        port: u16,
        security: Security,
    ) -> Result<(), ReplayError> {
        self.connects.push((connection, host.to_string(), port, security));
        if security == Security::Tls && self.no_tls {
            return Err(ReplayError);
        }
        let state = match self.resolve.get(host) {
            Some(address) => LinkState::Open(*address),
            None => LinkState::Failed,
        };
        self.connections.insert(
            connection,
            Connection {
                host: host.to_string(),
                port,
                state,
                inbox: VecDeque::new(),
                close_when_drained: false,
                request: Vec::new(),
                upgraded: false,
            },
        );
        Ok(())
    }

    fn state(&mut self, connection: ConnectionId) -> LinkState {
        self.connections.get(&connection).map_or(LinkState::Closed, |c| c.state)
    }

    fn send(&mut self, connection: ConnectionId, bytes: &[u8]) -> Result<(), ReplayError> {
        self.sent.push((connection, bytes.to_vec()));
        let conn = self.connections.get_mut(&connection).ok_or(ReplayError)?;
        if !matches!(conn.state, LinkState::Open(_)) {
            return Err(ReplayError);
        }
        conn.request.extend_from_slice(bytes);
        self.answer(connection);
        Ok(())
    }

    fn receive(&mut self, connection: ConnectionId, buffer: &mut [u8]) -> Result<Received, ReplayError> {
        let chunk = self.chunk;
        let Some(conn) = self.connections.get_mut(&connection) else { return Ok(Received::Closed) };
        if !conn.inbox.is_empty() {
            let n = conn.inbox.len().min(buffer.len()).min(chunk);
            for slot in buffer.iter_mut().take(n) {
                *slot = conn.inbox.pop_front().unwrap();
            }
            return Ok(Received::Data(n));
        }
        if conn.close_when_drained {
            conn.state = LinkState::Closed;
            return Ok(Received::Closed);
        }
        Ok(Received::Nothing)
    }

    fn close(&mut self, connection: ConnectionId) {
        self.connections.remove(&connection);
    }
}

/// Deterministic "random" bytes for tests.
#[derive(Debug, Default)]
pub struct Counter(pub u8);

impl RandomSource for Counter {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}

/// Unmasks a client frame (RFC 6455 section 5.3): the opcode and payload.
pub fn client_frame(frame: &[u8]) -> (u8, Vec<u8>) {
    assert!(frame[1] & 0x80 != 0, "client frames are masked");
    let (len, offset) = match frame[1] & 0x7F {
        126 => (usize::from(u16::from_be_bytes([frame[2], frame[3]])), 4),
        127 => panic!("not used"),
        n => (usize::from(n), 2),
    };
    let mask = &frame[offset..offset + 4];
    let payload = frame[offset + 4..offset + 4 + len].iter().enumerate().map(|(i, b)| b ^ mask[i % 4]).collect();
    (frame[0] & 0x0F, payload)
}

/// An unmasked server frame.
pub fn server_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = std::vec![0x80 | opcode];
    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else if let Ok(length) = u16::try_from(payload.len()) {
        frame.push(126);
        frame.extend_from_slice(&length.to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    frame
}

/// An HTTP response with a body and a Content-Length.
pub fn response(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
    let mut text = format!("HTTP/1.1 {status}\r\n");
    for header in headers {
        text.push_str(header);
        text.push_str("\r\n");
    }
    text.push_str("Content-Length: ");
    text.push_str(&body.len().to_string());
    text.push_str("\r\n\r\n");
    let mut bytes = text.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}
