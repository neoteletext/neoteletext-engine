//! A client WebSocket (RFC 6455) to the relay, read by a thread of its own
//! that wakes its owner as bytes arrive.

use super::{Incoming, MAX_HEAD, OsRandom, READ_CHUNK, RESPONSE_TIMEOUT, Stream, Waker, later};
use crate::{Error, MAX_FRAME_BYTES};
use neoteletext_web::bytes::Writer;
use neoteletext_web::consts::{OPCODE_BINARY, OPCODE_CLOSE, OPCODE_PONG, OPCODE_TEXT};
use neoteletext_web::http::{HeadCollector, parse_head};
use neoteletext_web::proxy::Proxy;
use neoteletext_web::url::Url;
use neoteletext_web::ws::{self, FrameReader, WsMessage};
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A WebSocket frame's header: at most 14 bytes for a client's frame.
const FRAME_HEADER: usize = 14;
/// A close frame's status code (RFC 6455 section 5.5.1), when it has one.
const CLOSE_CODE_BYTES: usize = 2;

/// Bytes the reader thread may hold before the owner takes them: with
/// [`READ_CHUNK`], what one connection keeps in memory at most.
const READS_AHEAD: usize = 4;

/// A WebSocket's reader thread: passes on what the socket gives, waking
/// the owner's wait each time, until the socket ends or the owner has
/// gone.
fn read_socket(mut socket: TcpStream, chunks: &mpsc::SyncSender<Vec<u8>>, waker: Option<&Waker>) {
    let mut buffer = vec![0_u8; READ_CHUNK];
    loop {
        let count = match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let Some(bytes) = buffer.get(..count) else { break };
        if chunks.send(bytes.to_vec()).is_err() {
            return;
        }
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    // The owner sees the end as the channel's: this thread's sender is the
    // only one.
    if let Some(waker) = waker {
        waker.wake();
    }
}

/// The bytes a WebSocket reads, from its reader thread, decrypted here
/// when the connection is TLS.
struct Reading {
    socket: TcpStream,
    tls: Option<Box<rustls::ClientConnection>>,
    chunks: mpsc::Receiver<Vec<u8>>,
}

impl Reading {
    /// Writes all of `bytes`, encrypting them first over TLS.
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let mut socket = &self.socket;
        let written = match self.tls.as_deref_mut() {
            None => socket.write_all(bytes).and_then(|()| socket.flush()),
            Some(tls) => tls.writer().write_all(bytes).and_then(|()| write_records(tls, socket)),
        };
        written.map_err(|error| Error::Network(error.to_string()))
    }

    /// Adds what has arrived to `unread`, waiting until `until` when
    /// nothing has: `false` when the connection has ended. A moment already
    /// past looks without waiting.
    fn read_into(&mut self, unread: &mut Vec<u8>, until: Instant) -> Result<bool, Error> {
        let mut plain = vec![0_u8; READ_CHUNK];
        loop {
            if let Some(tls) = self.tls.as_deref_mut() {
                match tls.reader().read(&mut plain) {
                    // The relay's close_notify.
                    Ok(0) => return Ok(false),
                    Ok(count) => {
                        unread.extend_from_slice(plain.get(..count).unwrap_or(&[]));
                        return Ok(true);
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {}
                    Err(error) => return Err(Error::Tls(error.to_string())),
                }
            }
            let left = until.saturating_duration_since(Instant::now());
            let chunk = if left.is_zero() {
                match self.chunks.try_recv() {
                    Ok(chunk) => chunk,
                    Err(mpsc::TryRecvError::Empty) => return Ok(true),
                    Err(mpsc::TryRecvError::Disconnected) => return Ok(false),
                }
            } else {
                match self.chunks.recv_timeout(left) {
                    Ok(chunk) => chunk,
                    Err(mpsc::RecvTimeoutError::Timeout) => return Ok(true),
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(false),
                }
            };
            let Some(tls) = self.tls.as_deref_mut() else {
                unread.extend_from_slice(&chunk);
                return Ok(true);
            };
            let mut records = chunk.as_slice();
            while !records.is_empty() {
                let taken = tls.read_tls(&mut records).map_err(|error| Error::Tls(error.to_string()))?;
                tls.process_new_packets().map_err(|error| Error::Tls(error.to_string()))?;
                if taken == 0 {
                    break;
                }
            }
            // An alert or a key update may need an answer.
            write_records(tls, &self.socket).map_err(|error| Error::Tls(error.to_string()))?;
        }
    }
}

/// Writes whatever records rustls has ready, blocking, within the write
/// limit.
fn write_records(tls: &mut rustls::ClientConnection, mut socket: &TcpStream) -> std::io::Result<()> {
    while tls.wants_write() {
        tls.write_tls(&mut socket)?;
    }
    socket.flush()
}

/// A client WebSocket (RFC 6455) to the relay. A thread of its own reads
/// its socket, so a wait on it ends the moment bytes arrive.
pub struct WebSocket {
    reading: Reading,
    reader: FrameReader,
    message: Vec<u8>,
    unread: Vec<u8>,
    random: OsRandom,
}

impl WebSocket {
    /// Opens a WebSocket to `url` (`ws` or `wss`). When `waker` is given,
    /// it is woken each time bytes arrive, and when the connection ends.
    ///
    /// # Errors
    /// A network failure or a refused handshake.
    pub fn connect(url: &Url, proxy: Option<&Proxy>, waker: Option<Waker>) -> Result<Self, Error> {
        let mut stream = Stream::connect(url, proxy)?;
        let mut random = OsRandom;
        let key = ws::new_key(&mut random);
        let mut buffer = vec![0_u8; MAX_HEAD];
        let mut writer = Writer::new(&mut buffer);
        // The relay is not a channel: the engine names itself (REQ-WEB-031).
        let user_agent = neoteletext_web::user_agent::UserAgentString::default();
        ws::write_handshake_headers(&mut writer, url, &key, user_agent.as_bytes())
            .map_err(|_| Error::Limit("the handshake"))?;
        writer.put(b"\r\n").map_err(|_| Error::Limit("the handshake"))?;
        let length = writer.len();
        stream.write_all(buffer.get(..length).unwrap_or(&[]))?;
        stream.set_read_timeout(RESPONSE_TIMEOUT)?;
        let mut collector = HeadCollector::default();
        let mut head = vec![0_u8; MAX_HEAD];
        let mut chunk = vec![0_u8; READ_CHUNK];
        let unread = loop {
            let count =
                stream.read(&mut chunk)?.ok_or_else(|| Error::Network("the relay did not answer in time".into()))?;
            if count == 0 {
                return Err(Error::Protocol("no WebSocket handshake"));
            }
            let input = chunk.get(..count).unwrap_or(&[]);
            let (used, complete) =
                collector.feed(&mut head, input).map_err(|_| Error::Protocol("a handshake head too large"))?;
            if complete {
                break input.get(used..).unwrap_or(&[]).to_vec();
            }
        };
        let parsed = parse_head(head.get(..collector.len()).unwrap_or(&[]), &mut |_| {})
            .map_err(|_| Error::Protocol("a malformed handshake"))?;
        if parsed.status != neoteletext_web::consts::STATUS_SWITCHING_PROTOCOLS {
            return Err(Error::Status(parsed.status));
        }
        ws::check_handshake(&parsed, &key).map_err(|_| Error::Protocol("a WebSocket handshake that does not check"))?;
        // Decrypted bytes the handshake's last read left in the TLS
        // connection stay there, and are read before the socket's next.
        let (socket, tls) = match stream {
            Stream::Plain(socket) => (socket, None),
            Stream::Tls(owned) => {
                let rustls::StreamOwned { conn, sock } = *owned;
                (sock, Some(Box::new(conn)))
            }
        };
        // From here the reader thread waits on the socket for as long as it
        // is open; closing it ends the wait.
        socket.set_read_timeout(None).map_err(|error| Error::Network(error.to_string()))?;
        let thread_socket = socket.try_clone().map_err(|error| Error::Network(error.to_string()))?;
        let (chunks, incoming) = mpsc::sync_channel(READS_AHEAD);
        std::thread::spawn(move || read_socket(thread_socket, &chunks, waker.as_ref()));
        Ok(Self {
            reading: Reading { socket, tls, chunks: incoming },
            reader: FrameReader::new(),
            message: vec![0_u8; MAX_FRAME_BYTES],
            unread,
            random,
        })
    }

    fn send(&mut self, opcode: u8, payload: &[u8]) -> Result<(), Error> {
        let mut buffer = vec![0_u8; payload.len().saturating_add(FRAME_HEADER)];
        let mut writer = Writer::new(&mut buffer);
        ws::write_frame(&mut writer, opcode, payload, &mut self.random).map_err(|_| Error::Limit("a frame"))?;
        let length = writer.len();
        self.reading.write_all(buffer.get(..length).unwrap_or(&[]))
    }

    /// Sends a text message.
    ///
    /// # Errors
    /// When the connection breaks.
    pub fn send_text(&mut self, text: &str) -> Result<(), Error> {
        self.send(OPCODE_TEXT, text.as_bytes())
    }

    /// Sends a binary message of at most [`MAX_FRAME_BYTES`].
    ///
    /// # Errors
    /// When the message is too long or the connection breaks.
    pub fn send_binary(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(Error::Limit("a frame longer than the relay takes"));
        }
        self.send(OPCODE_BINARY, bytes)
    }

    /// Waits up to `timeout` for a message: `None` when none came. A zero
    /// timeout looks at what has arrived without waiting. Pings are
    /// answered here.
    ///
    /// # Errors
    /// When the connection breaks or the relay breaks the protocol.
    pub fn receive(&mut self, timeout: Duration) -> Result<Option<Incoming>, Error> {
        let until = later(timeout);
        loop {
            if !self.unread.is_empty() {
                let input = core::mem::take(&mut self.unread);
                let (used, done) =
                    self.reader.feed(&mut self.message, &input).map_err(|_| Error::Protocol("a WebSocket frame"))?;
                self.unread = input.get(used..).unwrap_or(&[]).to_vec();
                match done {
                    Some(WsMessage::Text(length)) => {
                        let text = self.message.get(..length).unwrap_or(&[]);
                        return Ok(Some(Incoming::Text(String::from_utf8_lossy(text).into_owned())));
                    }
                    Some(WsMessage::Binary(length)) => {
                        return Ok(Some(Incoming::Binary(self.message.get(..length).unwrap_or(&[]).to_vec())));
                    }
                    Some(WsMessage::Ping) => {
                        let payload = self.reader.control_payload().to_vec();
                        self.send(OPCODE_PONG, &payload)?;
                        continue;
                    }
                    Some(WsMessage::Close(code)) => {
                        let _ = self
                            .send(OPCODE_CLOSE, &code.unwrap_or(neoteletext_web::consts::CLOSE_NORMAL).to_be_bytes());
                        return Ok(Some(Incoming::Closed(code)));
                    }
                    Some(WsMessage::Pong | WsMessage::Skipped) | None => {
                        if used > 0 {
                            continue;
                        }
                    }
                }
            }
            let before = self.unread.len();
            if !self.reading.read_into(&mut self.unread, until)? {
                return Ok(Some(Incoming::Closed(None)));
            }
            if self.unread.len() == before {
                return Ok(None);
            }
        }
    }

    /// Closes with a normal close frame.
    pub fn close(&mut self) {
        let mut code = [0_u8; CLOSE_CODE_BYTES];
        code.copy_from_slice(&neoteletext_web::consts::CLOSE_NORMAL.to_be_bytes());
        let _ = self.send(OPCODE_CLOSE, &code);
        let _ = self.reading.socket.shutdown(Shutdown::Both);
    }
}

impl Drop for WebSocket {
    /// Ends the reader thread's wait.
    fn drop(&mut self) {
        let _ = self.reading.socket.shutdown(Shutdown::Both);
    }
}
