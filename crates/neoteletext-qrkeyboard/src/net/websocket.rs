//! A client WebSocket (RFC 6455) to the relay, read by a thread of its own
//! that wakes its owner as bytes arrive.

use super::{Incoming, MAX_HEAD, OsRandom, READ_CHUNK, RESPONSE_TIMEOUT, Stream, Waker, later};
use crate::{Error, MAX_FRAME_BYTES};
use neoteletext_transport::{Connection, write_records};
use neoteletext_web::bytes::Writer;
use neoteletext_web::consts::{OPCODE_BINARY, OPCODE_CLOSE, OPCODE_PONG, OPCODE_TEXT};
use neoteletext_web::http::{HeadCollector, parse_head};
use neoteletext_web::port::Received;
use neoteletext_web::proxy::Proxy;
use neoteletext_web::url::Url;
use neoteletext_web::ws::{self, FrameReader, WsMessage};
use std::time::{Duration, Instant};

/// A WebSocket frame's header: at most 14 bytes for a client's frame.
const FRAME_HEADER: usize = 14;
/// A close frame's status code (RFC 6455 section 5.5.1), when it has one.
const CLOSE_CODE_BYTES: usize = 2;

/// Adds what has arrived on `connection` to `unread`, waiting until `until`
/// when nothing has: `false` when the connection has ended. A moment
/// already past looks without waiting.
fn read_into(connection: &mut Connection, unread: &mut Vec<u8>, until: Instant) -> Result<bool, Error> {
    let mut plain = vec![0_u8; READ_CHUNK];
    let received =
        connection.take(&mut plain, Some(until), write_records).map_err(|error| Error::Tls(error.to_string()))?;
    match received {
        Received::Data(count) => {
            unread.extend_from_slice(plain.get(..count).unwrap_or(&[]));
            Ok(true)
        }
        Received::Nothing => Ok(true),
        Received::Closed => Ok(false),
    }
}

/// A client WebSocket (RFC 6455) to the relay. A thread of its own reads
/// its socket, so a wait on it ends the moment bytes arrive.
pub struct WebSocket {
    connection: Connection,
    reader: FrameReader,
    message: Vec<u8>,
    unread: Vec<u8>,
    random: OsRandom,
}

impl WebSocket {
    /// Opens a WebSocket to `url` (`ws` or `wss`), asking for `subprotocol`
    /// where the far end needs one: a channel does (DOC-1794), the relay
    /// does not. When `waker` is given, it is woken each time bytes arrive,
    /// and when the connection ends.
    ///
    /// # Errors
    /// A network failure, or a refused handshake: one that does not name the
    /// subprotocol asked for among them.
    pub fn connect(
        url: &Url,
        subprotocol: Option<&str>,
        proxy: Option<&Proxy>,
        waker: Option<Waker>,
    ) -> Result<Self, Error> {
        let mut stream = Stream::connect(url, proxy)?;
        let mut random = OsRandom;
        let key = ws::new_key(&mut random);
        let mut buffer = vec![0_u8; MAX_HEAD];
        let mut writer = Writer::new(&mut buffer);
        // The engine names itself: neither end here is a channel's page
        // viewed (REQ-WEB-031).
        let user_agent = neoteletext_web::user_agent::UserAgentString::default();
        ws::write_handshake_headers(&mut writer, url, &key, user_agent.as_bytes(), subprotocol)
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
        ws::check_handshake(&parsed, &key, subprotocol)
            .map_err(|_| Error::Protocol("a WebSocket handshake that does not check"))?;
        // Decrypted bytes the handshake's last read left in the TLS
        // connection stay there, and are read before the socket's next.
        let (socket, tls) = match stream {
            Stream::Plain(socket) => (socket, None),
            Stream::Tls(owned) => {
                let rustls::StreamOwned { conn, sock } = *owned;
                (sock, Some(Box::new(conn)))
            }
        };
        let wake = move || {
            if let Some(waker) = &waker {
                waker.wake();
            }
        };
        let connection = Connection::start(socket, tls, Vec::new(), READ_CHUNK, wake)
            .map_err(|error| Error::Network(error.to_string()))?;
        Ok(Self { connection, reader: FrameReader::new(), message: vec![0_u8; MAX_FRAME_BYTES], unread, random })
    }

    fn send(&mut self, opcode: u8, payload: &[u8]) -> Result<(), Error> {
        let mut buffer = vec![0_u8; payload.len().saturating_add(FRAME_HEADER)];
        let mut writer = Writer::new(&mut buffer);
        ws::write_frame(&mut writer, opcode, payload, &mut self.random).map_err(|_| Error::Limit("a frame"))?;
        let length = writer.len();
        self.connection
            .write_all(buffer.get(..length).unwrap_or(&[]))
            .map_err(|error| Error::Network(error.to_string()))
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
            if !read_into(&mut self.connection, &mut self.unread, until)? {
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
        self.connection.shutdown();
    }
}

impl Drop for WebSocket {
    /// Ends the reader thread's wait.
    fn drop(&mut self) {
        self.connection.shutdown();
    }
}
