//! Connections to the relay: HTTP/1.1 requests and a WebSocket, plain or
//! over TLS (rustls, Mozilla's roots). HTTP heads and WebSocket frames are
//! read and written by the engine's own bounded code (`neoteletext_web`'s
//! `http` and `ws`). Every read has a time limit; nothing waits forever.

use neoteletext_web::proxy::Proxy;
use std::fmt::Write as _;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use neoteletext_web::bytes::Writer;
use neoteletext_web::consts::{OPCODE_BINARY, OPCODE_CLOSE, OPCODE_PONG, OPCODE_TEXT};
use neoteletext_web::http::{BodyDecoder, HeadCollector, parse_head};
use neoteletext_web::port::RandomSource;
use neoteletext_web::url::Url;
use neoteletext_web::ws::{self, FrameReader, WsMessage};

use crate::{Error, MAX_FRAME_BYTES};

/// How long a connection and its TLS handshake may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long an HTTP answer may take.
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);
/// The longest response head read.
const MAX_HEAD: usize = 16 * 1024;
/// The longest response body read: the relay's answers are a few hundred
/// bytes, a configuration at most its 8 KB limit.
pub const MAX_BODY: usize = 64 * 1024;
/// Bytes read from the socket at a time.
const READ_CHUNK: usize = 4096;
/// A WebSocket frame's header: at most 14 bytes for a client's frame.
const FRAME_HEADER: usize = 14;
/// A close frame's status code (RFC 6455 section 5.5.1), when it has one.
const CLOSE_CODE_BYTES: usize = 2;

/// Random bytes for WebSocket masks and keys.
pub struct OsRandom;

impl RandomSource for OsRandom {
    fn fill(&mut self, bytes: &mut [u8]) {
        // A mask needs no secrecy; on the impossible failure it stays zero.
        let _ = crate::random(bytes);
    }
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots: rustls::RootCertStore = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();
            Arc::new(rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth())
        })
        .clone()
}

/// A connection, plain or over TLS.
pub enum Stream {
    /// Plain TCP: only for a relay on this computer, for testing.
    Plain(TcpStream),
    /// TLS.
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    /// Connects to `url`'s host, with TLS for `https` and `wss`.
    ///
    /// # Errors
    /// When the name does not resolve, the connection is refused or times
    /// out, or TLS fails.
    pub fn connect(url: &Url, proxy: Option<&Proxy>) -> Result<Self, Error> {
        let host = url.host_text();
        let socket = if let Some(proxy) = proxy {
            // Through the proxy the port's setting names (REQ-WEB-032),
            // whose address the user chose.
            let target = (proxy.host.as_str(), proxy.port)
                .to_socket_addrs()
                .map_err(|error| Error::Network(format!("{}: {error}", proxy.host)))?
                .next()
                .ok_or_else(|| Error::Network(format!("{}: no address", proxy.host)))?;
            let mut socket = TcpStream::connect_timeout(&target, CONNECT_TIMEOUT)
                .map_err(|error| Error::Network(format!("{}: {error}", proxy.host)))?;
            socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
            socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
            proxy.open_tunnel(&mut socket, host, url.port()).map_err(Error::Network)?;
            socket
        } else {
            let target = (host, url.port())
                .to_socket_addrs()
                .map_err(|error| Error::Network(format!("{host}: {error}")))?
                .next()
                .ok_or_else(|| Error::Network(format!("{host}: no address")))?;
            TcpStream::connect_timeout(&target, CONNECT_TIMEOUT)
                .map_err(|error| Error::Network(format!("{host}: {error}")))?
        };
        socket.set_nodelay(true).map_err(|error| Error::Network(error.to_string()))?;
        socket.set_read_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
        socket.set_write_timeout(Some(CONNECT_TIMEOUT)).map_err(|error| Error::Network(error.to_string()))?;
        if !url.is_secure() {
            return Ok(Self::Plain(socket));
        }
        let name =
            rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(|error| Error::Tls(error.to_string()))?;
        let connection =
            rustls::ClientConnection::new(tls_config(), name).map_err(|error| Error::Tls(error.to_string()))?;
        let mut stream = rustls::StreamOwned::new(connection, socket);
        while stream.conn.is_handshaking() {
            stream.conn.complete_io(&mut stream.sock).map_err(|error| Error::Tls(error.to_string()))?;
        }
        Ok(Self::Tls(Box::new(stream)))
    }

    fn socket(&self) -> &TcpStream {
        match self {
            Self::Plain(socket) => socket,
            Self::Tls(stream) => &stream.sock,
        }
    }

    /// How long a read waits for data.
    ///
    /// # Errors
    /// When the socket refuses the setting.
    pub fn set_read_timeout(&self, timeout: Duration) -> Result<(), Error> {
        self.socket().set_read_timeout(Some(timeout)).map_err(|error| Error::Network(error.to_string()))
    }

    /// Writes all of `bytes`.
    ///
    /// # Errors
    /// When the connection breaks.
    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let written = match self {
            Self::Plain(socket) => socket.write_all(bytes).and_then(|()| socket.flush()),
            Self::Tls(stream) => stream.write_all(bytes).and_then(|()| stream.flush()),
        };
        written.map_err(|error| Error::Network(error.to_string()))
    }

    /// Reads what has arrived: `Ok(0)` when the peer closed, `Ok(None)`
    /// when nothing came within the read timeout.
    ///
    /// # Errors
    /// When the connection breaks.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<Option<usize>, Error> {
        let read = match self {
            Self::Plain(socket) => socket.read(buffer),
            Self::Tls(stream) => stream.read(buffer),
        };
        match read {
            Ok(count) => Ok(Some(count)),
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(None),
            // A TLS peer that closes without close_notify has still closed.
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => Ok(Some(0)),
            Err(error) => Err(Error::Network(error.to_string())),
        }
    }

    /// Reads what arrives within `wait`, as [`Stream::read`] does. A zero
    /// wait looks without waiting: the operating system refuses a zero
    /// read timeout, so the socket is made non-blocking for that one read.
    ///
    /// # Errors
    /// When the connection breaks or the socket refuses the setting.
    pub fn read_within(&mut self, buffer: &mut [u8], wait: Duration) -> Result<Option<usize>, Error> {
        if !wait.is_zero() {
            self.set_read_timeout(wait)?;
            return self.read(buffer);
        }
        let blocking = |stream: &Self, on: bool| {
            stream.socket().set_nonblocking(!on).map_err(|error| Error::Network(error.to_string()))
        };
        blocking(self, false)?;
        let read = self.read(buffer);
        blocking(self, true)?;
        read
    }

    /// Closes the connection.
    pub fn close(&self) {
        let _ = self.socket().shutdown(Shutdown::Both);
    }
}

/// Parses an `http`, `https`, `ws` or `wss` address.
///
/// # Errors
/// Any other address.
pub fn parse(address: &str) -> Result<Url, Error> {
    Url::parse(address).map_err(|_| Error::Address)
}

/// One HTTP/1.1 request with `Connection: close`, answered with its status
/// and body (at most [`MAX_BODY`] bytes).
///
/// # Errors
/// A network failure, or a response that breaks HTTP or a bound.
pub fn request(
    url: &Url,
    method: &str,
    authorization: Option<&str>,
    json: Option<&str>,
    proxy: Option<&Proxy>,
) -> Result<(u16, Vec<u8>), Error> {
    let mut stream = Stream::connect(url, proxy)?;
    let mut head = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n",
        url.target(),
        host_header(url)
    );
    if let Some(token) = authorization {
        let _ = write!(head, "Authorization: Bearer {token}\r\n");
    }
    let body = json.unwrap_or("");
    if json.is_some() || method == "POST" {
        let _ = write!(head, "Content-Type: application/json\r\nContent-Length: {}\r\n", body.len());
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.set_read_timeout(RESPONSE_TIMEOUT)?;
    let mut collector = HeadCollector::default();
    let mut head = vec![0_u8; MAX_HEAD];
    let mut chunk = vec![0_u8; READ_CHUNK];
    let mut body = Vec::new();
    let mut decoder: Option<BodyDecoder> = None;
    let mut status = 0;
    loop {
        let count =
            stream.read(&mut chunk)?.ok_or_else(|| Error::Network("the relay did not answer in time".into()))?;
        let mut input = chunk.get(..count).unwrap_or(&[]);
        if count == 0 {
            if let Some(decoder) = &mut decoder {
                decoder.finish_at_close().map_err(|_| Error::Protocol("a truncated HTTP body"))?;
                break;
            }
            return Err(Error::Protocol("no HTTP response"));
        }
        if decoder.is_none() {
            let (used, complete) =
                collector.feed(&mut head, input).map_err(|_| Error::Protocol("an HTTP head too large"))?;
            input = input.get(used..).unwrap_or(&[]);
            if !complete {
                continue;
            }
            let parsed = parse_head(head.get(..collector.len()).unwrap_or(&[]), &mut |_| {})
                .map_err(|_| Error::Protocol("a malformed HTTP head"))?;
            status = parsed.status;
            decoder = Some(
                BodyDecoder::new(parsed.framing, MAX_BODY).map_err(|_| Error::Protocol("an HTTP body too large"))?,
            );
        }
        let Some(decoder) = &mut decoder else { continue };
        while !input.is_empty() && !decoder.is_done() {
            let step = decoder.feed(input).map_err(|_| Error::Protocol("a malformed HTTP body"))?;
            body.extend_from_slice(step.data);
            input = input.get(step.consumed..).unwrap_or(&[]);
            if step.consumed == 0 {
                break;
            }
        }
        if decoder.is_done() {
            break;
        }
    }
    stream.close();
    Ok((status, body))
}

/// The `Host` header: the host, and the port when it is not the scheme's.
fn host_header(url: &Url) -> String {
    let default = if url.is_secure() { 443 } else { 80 };
    if url.port() == default { url.host_text().to_owned() } else { format!("{}:{}", url.host_text(), url.port()) }
}

/// What a WebSocket delivered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Incoming {
    /// A text message.
    Text(String),
    /// A binary message.
    Binary(Vec<u8>),
    /// The peer closed, with this code when it gave one.
    Closed(Option<u16>),
}

/// A client WebSocket (RFC 6455) to the relay.
pub struct WebSocket {
    stream: Stream,
    reader: FrameReader,
    message: Vec<u8>,
    unread: Vec<u8>,
    random: OsRandom,
}

impl WebSocket {
    /// Opens a WebSocket to `url` (`ws` or `wss`).
    ///
    /// # Errors
    /// A network failure or a refused handshake.
    pub fn connect(url: &Url, proxy: Option<&Proxy>) -> Result<Self, Error> {
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
        Ok(Self { stream, reader: FrameReader::new(), message: vec![0_u8; MAX_FRAME_BYTES], unread, random })
    }

    fn send(&mut self, opcode: u8, payload: &[u8]) -> Result<(), Error> {
        let mut buffer = vec![0_u8; payload.len().saturating_add(FRAME_HEADER)];
        let mut writer = Writer::new(&mut buffer);
        ws::write_frame(&mut writer, opcode, payload, &mut self.random).map_err(|_| Error::Limit("a frame"))?;
        let length = writer.len();
        self.stream.write_all(buffer.get(..length).unwrap_or(&[]))
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
        let mut chunk = vec![0_u8; READ_CHUNK];
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
            let Some(count) = self.stream.read_within(&mut chunk, timeout)? else { return Ok(None) };
            if count == 0 {
                return Ok(Some(Incoming::Closed(None)));
            }
            self.unread.extend_from_slice(chunk.get(..count).unwrap_or(&[]));
        }
    }

    /// Closes with a normal close frame.
    pub fn close(&mut self) {
        let mut code = [0_u8; CLOSE_CODE_BYTES];
        code.copy_from_slice(&neoteletext_web::consts::CLOSE_NORMAL.to_be_bytes());
        let _ = self.send(OPCODE_CLOSE, &code);
        self.stream.close();
    }
}
