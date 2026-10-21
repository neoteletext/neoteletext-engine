//! WebSocket client framing (RFC 6455), written in the project (decision
//! E.38): the opening handshake and its accept check, client masking with
//! keys from the caller's random source, framing, fragmentation, control
//! frames and size limits.

use crate::base64;
use crate::bytes::{Overflow, Writer, copy_prefix};
use crate::consts::{
    CLOSE_CODE_BYTES, CLOSE_INVALID_DATA, CLOSE_PROTOCOL_ERROR, CLOSE_TOO_BIG, OPCODE_BINARY, OPCODE_CLOSE,
    OPCODE_CONTINUATION, OPCODE_CONTROL_FIRST, OPCODE_PING, OPCODE_PONG, OPCODE_TEXT, STATUS_SWITCHING_PROTOCOLS,
    UPGRADE_WEBSOCKET, WEBSOCKET_ACCEPT_TEXT, WEBSOCKET_GUID, WEBSOCKET_KEY_BYTES, WEBSOCKET_KEY_TEXT,
    WEBSOCKET_VERSION, WS_CONTROL_MAX, WS_FIN, WS_LENGTH_7_MAX, WS_LENGTH_16, WS_LENGTH_64, WS_LENGTH_MASK,
    WS_MASK_BIT, WS_MASK_BYTES, WS_MAX_HEADER, WS_OPCODE_MASK, WS_RSV_MASK, header,
};
use crate::http::{self, Method, ResponseHead};
use crate::port::RandomSource;
use crate::sha1;
use crate::url::Url;

/// Why a WebSocket connection failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WsError {
    /// The handshake response was not a valid acceptance (RFC 6455
    /// section 4.1, client requirements 1 to 6).
    Handshake,
    /// A frame broke the framing rules: a masked server frame, reserved
    /// bits, an unknown opcode, a fragmented or oversized control frame, a
    /// continuation without a start, or a non-minimal length.
    Protocol,
    /// A message larger than the limit.
    TooBig,
    /// A text message that is not UTF-8 (RFC 6455 section 8.1).
    InvalidText,
}

impl WsError {
    /// The close status to send for this error (RFC 6455 section 7.4.1).
    #[must_use]
    pub const fn close_code(self) -> u16 {
        match self {
            Self::Handshake | Self::Protocol => CLOSE_PROTOCOL_ERROR,
            Self::TooBig => CLOSE_TOO_BIG,
            Self::InvalidText => CLOSE_INVALID_DATA,
        }
    }
}

/// The `Sec-WebSocket-Key` text: base64 of 16 random bytes.
pub type HandshakeKey = [u8; WEBSOCKET_KEY_TEXT];

/// Draws a new handshake key from the random source (RFC 6455 section 4.1
/// item 7).
pub fn new_key(random: &mut dyn RandomSource) -> HandshakeKey {
    let mut nonce = [0u8; WEBSOCKET_KEY_BYTES];
    random.fill(&mut nonce);
    let mut key = [0u8; WEBSOCKET_KEY_TEXT];
    // 16 bytes always encode to exactly 24 characters.
    let _ = base64::encode(&nonce, &mut key);
    key
}

/// The `Sec-WebSocket-Accept` value a server must return for `key` (RFC
/// 6455 section 4.2.2 item 5.4).
#[must_use]
pub fn accept_for(key: &[u8]) -> [u8; WEBSOCKET_ACCEPT_TEXT] {
    let digest = sha1::digest(&[key, WEBSOCKET_GUID]);
    let mut accept = [0u8; WEBSOCKET_ACCEPT_TEXT];
    // 20 bytes always encode to exactly 28 characters.
    let _ = base64::encode(&digest, &mut accept);
    accept
}

/// Writes the opening handshake request line and headers (RFC 6455
/// section 4.1): only the headers of the profile (REQ-WEB-014), with
/// `user_agent` (REQ-WEB-031). The caller adds the `Cookie` header, if any,
/// and the empty line that ends the request.
pub fn write_handshake_headers(
    writer: &mut Writer<'_>,
    url: &Url,
    key: &HandshakeKey,
    user_agent: &[u8],
) -> Result<(), Overflow> {
    http::write_request_line(writer, Method::Get, url.target())?;
    http::write_host(writer, url)?;
    http::write_header(writer, header::USER_AGENT, user_agent)?;
    http::write_header(writer, header::UPGRADE, UPGRADE_WEBSOCKET)?;
    http::write_header(writer, header::CONNECTION, crate::consts::CONNECTION_UPGRADE)?;
    http::write_header(writer, header::SEC_WEBSOCKET_KEY, key)?;
    http::write_header(writer, header::SEC_WEBSOCKET_VERSION, WEBSOCKET_VERSION)
}

/// Checks the handshake response (RFC 6455 section 4.1, the client's
/// checks on the server's response): status 101, `Upgrade: websocket`,
/// `Connection: Upgrade`, the right accept value, and no extension or
/// subprotocol, since none were requested.
pub fn check_handshake(head: &ResponseHead, key: &HandshakeKey) -> Result<(), WsError> {
    let accepted = head.status == STATUS_SWITCHING_PROTOCOLS
        && head.upgrade_websocket
        && head.connection_upgrade
        && head.websocket_accept == Some(accept_for(key))
        && !head.websocket_extras;
    if accepted { Ok(()) } else { Err(WsError::Handshake) }
}

/// Writes one complete, masked client frame (RFC 6455 section 5.2). The
/// client never fragments what it sends.
pub fn write_frame(
    writer: &mut Writer<'_>,
    opcode: u8,
    payload: &[u8],
    random: &mut dyn RandomSource,
) -> Result<(), Overflow> {
    let mut mask = [0u8; WS_MASK_BYTES];
    random.fill(&mut mask);
    write_frame_with_mask(writer, opcode, payload, mask)
}

/// Writes a frame with a given masking key.
pub fn write_frame_with_mask(
    writer: &mut Writer<'_>,
    opcode: u8,
    payload: &[u8],
    mask: [u8; WS_MASK_BYTES],
) -> Result<(), Overflow> {
    let start = writer.len();
    let result = (|| {
        writer.byte(WS_FIN | (opcode & WS_OPCODE_MASK))?;
        let len = payload.len();
        if len <= WS_LENGTH_7_MAX {
            writer.byte(WS_MASK_BIT | len as u8)?;
        } else if let Ok(short) = u16::try_from(len) {
            writer.byte(WS_MASK_BIT | WS_LENGTH_16)?;
            writer.put(&short.to_be_bytes())?;
        } else {
            writer.byte(WS_MASK_BIT | WS_LENGTH_64)?;
            writer.put(&(len as u64).to_be_bytes())?;
        }
        writer.put(&mask)?;
        for (index, &byte) in payload.iter().enumerate() {
            writer.byte(byte ^ mask.get(index % WS_MASK_BYTES).copied().unwrap_or(0))?;
        }
        Ok(())
    })();
    if result.is_err() {
        writer.truncate(start);
    }
    result
}

/// Writes a close frame with a status code (RFC 6455 section 5.5.1).
pub fn write_close(writer: &mut Writer<'_>, code: u16, random: &mut dyn RandomSource) -> Result<(), Overflow> {
    write_frame(writer, OPCODE_CLOSE, &code.to_be_bytes(), random)
}

/// What a completed frame or message is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WsMessage {
    /// A complete text message of this length in the message buffer,
    /// already checked to be UTF-8.
    Text(usize),
    /// A complete binary message of this length in the message buffer.
    Binary(usize),
    /// A ping; the payload is in [`FrameReader::control_payload`] and must
    /// be answered with a pong.
    Ping,
    /// A pong.
    Pong,
    /// The server closed the connection, with its status if it sent one.
    Close(Option<u16>),
    /// A data message longer than the message buffer, read to its end and
    /// discarded, from a reader made with [`FrameReader::skipping_oversized`].
    Skipped,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ReadState {
    /// Collecting the frame header.
    Header,
    /// Reading payload bytes.
    Payload,
}

/// Reads server frames and assembles messages (RFC 6455 sections 5 and 6.2).
#[derive(Clone, Copy, Debug)]
pub struct FrameReader {
    state: ReadState,
    header: [u8; WS_MAX_HEADER],
    header_len: usize,
    opcode: u8,
    fin: bool,
    remaining: u64,
    /// The opcode of the data message being assembled, if one is open.
    message_opcode: Option<u8>,
    message_len: usize,
    control: [u8; WS_CONTROL_MAX],
    control_len: usize,
    /// Discard data messages that do not fit the buffer instead of failing.
    skip_oversized: bool,
    /// The message being read does not fit and is being discarded.
    skipping: bool,
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameReader {
    /// A reader at the start of a connection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ReadState::Header,
            header: [0; WS_MAX_HEADER],
            header_len: 0,
            opcode: 0,
            fin: false,
            remaining: 0,
            message_opcode: None,
            message_len: 0,
            control: [0; WS_CONTROL_MAX],
            control_len: 0,
            skip_oversized: false,
            skipping: false,
        }
    }

    /// A reader that discards a data message longer than the message
    /// buffer and reports it as [`WsMessage::Skipped`], for a feed whose
    /// large messages the client does not need. Memory stays bounded by the
    /// buffer (REQ-SEC-012); the bytes are read and dropped.
    #[must_use]
    pub const fn skipping_oversized() -> Self {
        let mut reader = Self::new();
        reader.skip_oversized = true;
        reader
    }

    /// The payload of the last ping or close frame.
    #[must_use]
    pub fn control_payload(&self) -> &[u8] {
        self.control.get(..self.control_len).unwrap_or(&[])
    }

    /// The header length needed so far: 2, then 4 or 10, plus nothing for
    /// the mask (server frames are never masked).
    fn header_needed(&self) -> usize {
        match self.header.get(1).map(|b| b & WS_LENGTH_MASK) {
            Some(WS_LENGTH_16) if self.header_len >= 2 => 4,
            Some(WS_LENGTH_64) if self.header_len >= 2 => 10,
            _ => 2,
        }
    }

    /// Feeds bytes. Returns the bytes used and, when a message or control
    /// frame completed, what it was. Data messages are assembled in
    /// `message`, whose length is the message size limit.
    pub fn feed(&mut self, message: &mut [u8], input: &[u8]) -> Result<(usize, Option<WsMessage>), WsError> {
        let mut used = 0usize;
        loop {
            let rest = input.get(used..).unwrap_or(&[]);
            match self.state {
                ReadState::Header => {
                    let needed = self.header_needed();
                    let take = needed.saturating_sub(self.header_len).min(rest.len());
                    let end = self.header_len.saturating_add(take);
                    if let Some(target) = self.header.get_mut(self.header_len..end) {
                        copy_prefix(target, rest);
                    }
                    self.header_len = end;
                    used = used.saturating_add(take);
                    if self.header_len < self.header_needed() {
                        if rest.len() == take {
                            return Ok((used, None));
                        }
                        continue;
                    }
                    self.begin_frame()?;
                    if self.remaining == 0
                        && let Some(done) = self.end_frame(message)?
                    {
                        return Ok((used, Some(done)));
                    }
                }
                ReadState::Payload => {
                    if rest.is_empty() {
                        return Ok((used, None));
                    }
                    let take = usize::try_from(self.remaining).unwrap_or(usize::MAX).min(rest.len());
                    let data = rest.get(..take).unwrap_or(&[]);
                    if self.opcode >= OPCODE_CONTROL_FIRST {
                        let end = self.control_len.saturating_add(take);
                        let target = self.control.get_mut(self.control_len..end).ok_or(WsError::Protocol)?;
                        copy_prefix(target, data);
                        self.control_len = end;
                    } else if !self.skipping {
                        let end = self.message_len.saturating_add(take);
                        match message.get_mut(self.message_len..end) {
                            Some(target) => {
                                copy_prefix(target, data);
                                self.message_len = end;
                            }
                            None if self.skip_oversized => self.skipping = true,
                            None => return Err(WsError::TooBig),
                        }
                    }
                    used = used.saturating_add(take);
                    self.remaining = self.remaining.saturating_sub(take as u64);
                    if self.remaining == 0
                        && let Some(done) = self.end_frame(message)?
                    {
                        return Ok((used, Some(done)));
                    }
                }
            }
        }
    }

    /// Checks a complete frame header and prepares to read its payload.
    fn begin_frame(&mut self) -> Result<(), WsError> {
        let first = self.header.first().copied().unwrap_or(0);
        let second = self.header.get(1).copied().unwrap_or(0);
        self.header_len = 0;
        self.state = ReadState::Payload;
        if first & WS_RSV_MASK != 0 || second & WS_MASK_BIT != 0 {
            // No extension was negotiated, and server frames are never
            // masked (RFC 6455 section 5.1).
            return Err(WsError::Protocol);
        }
        self.fin = first & WS_FIN != 0;
        self.opcode = first & WS_OPCODE_MASK;
        let short = second & WS_LENGTH_MASK;
        let length = match short {
            WS_LENGTH_16 => {
                let value = u16::from_be_bytes([
                    self.header.get(2).copied().unwrap_or(0),
                    self.header.get(3).copied().unwrap_or(0),
                ]);
                // RFC 6455 section 5.2: the minimal number of bytes must be used.
                if usize::from(value) <= WS_LENGTH_7_MAX {
                    return Err(WsError::Protocol);
                }
                u64::from(value)
            }
            WS_LENGTH_64 => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(self.header.get(2..10).unwrap_or(&[0; 8]));
                let value = u64::from_be_bytes(bytes);
                // The most significant bit must be 0, and a value that fits
                // in 16 bits must use the 16-bit form.
                if value >> 63 != 0 || u16::try_from(value).is_ok() {
                    return Err(WsError::Protocol);
                }
                value
            }
            _ => u64::from(short),
        };
        self.remaining = length;
        match self.opcode {
            OPCODE_CLOSE | OPCODE_PING | OPCODE_PONG => {
                if !self.fin || length > WS_CONTROL_MAX as u64 {
                    return Err(WsError::Protocol);
                }
                self.control_len = 0;
            }
            OPCODE_TEXT | OPCODE_BINARY => {
                if self.message_opcode.is_some() {
                    return Err(WsError::Protocol);
                }
                self.message_opcode = Some(self.opcode);
                self.message_len = 0;
                self.skipping = false;
            }
            OPCODE_CONTINUATION => {
                if self.message_opcode.is_none() {
                    return Err(WsError::Protocol);
                }
            }
            _ => return Err(WsError::Protocol),
        }
        Ok(())
    }

    /// Finishes a frame whose payload is complete.
    fn end_frame(&mut self, message: &[u8]) -> Result<Option<WsMessage>, WsError> {
        self.state = ReadState::Header;
        match self.opcode {
            OPCODE_PING => Ok(Some(WsMessage::Ping)),
            OPCODE_PONG => Ok(Some(WsMessage::Pong)),
            OPCODE_CLOSE => {
                let payload = self.control_payload();
                let code = match payload.len() {
                    0 => None,
                    1 => return Err(WsError::Protocol),
                    _ => Some(u16::from_be_bytes([
                        payload.first().copied().unwrap_or(0),
                        payload.get(1).copied().unwrap_or(0),
                    ])),
                };
                if core::str::from_utf8(payload.get(CLOSE_CODE_BYTES..).unwrap_or(&[])).is_err() {
                    return Err(WsError::InvalidText);
                }
                Ok(Some(WsMessage::Close(code)))
            }
            _ if !self.fin => Ok(None),
            _ => {
                let len = self.message_len;
                let opcode = self.message_opcode.take();
                self.message_len = 0;
                if core::mem::take(&mut self.skipping) {
                    return Ok(Some(WsMessage::Skipped));
                }
                if opcode == Some(OPCODE_TEXT) {
                    let text = message.get(..len).unwrap_or(&[]);
                    if core::str::from_utf8(text).is_err() {
                        return Err(WsError::InvalidText);
                    }
                    Ok(Some(WsMessage::Text(len)))
                } else {
                    Ok(Some(WsMessage::Binary(len)))
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;

    struct Counter(u8);
    impl RandomSource for Counter {
        fn fill(&mut self, bytes: &mut [u8]) {
            for byte in bytes {
                *byte = self.0;
                self.0 = self.0.wrapping_add(1);
            }
        }
    }

    #[test]
    fn rfc6455_accept_example() {
        // RFC 6455 section 1.3.
        assert_eq!(&accept_for(b"dGhlIHNhbXBsZSBub25jZQ=="), b"s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn key_is_base64_of_sixteen_random_bytes() {
        let key = new_key(&mut Counter(0));
        let mut decoded = [0u8; 18];
        assert_eq!(base64::decode(&key, &mut decoded).unwrap(), 16);
        assert_eq!(&decoded[..16], &core::array::from_fn::<u8, 16, _>(|i| i as u8));
    }

    #[test]
    fn handshake_request_has_only_profile_headers() {
        let url = Url::parse("wss://zxnet.co.uk/ws/teefax").unwrap();
        let key = *b"dGhlIHNhbXBsZSBub25jZQ==";
        let mut buffer = [0u8; 512];
        let mut writer = Writer::new(&mut buffer);
        write_handshake_headers(&mut writer, &url, &key, b"NeoTeletext/0.1 (macOS)").unwrap();
        writer.put(crate::consts::CRLF).unwrap();
        let text = core::str::from_utf8(writer.written()).unwrap();
        assert_eq!(
            text,
            "GET /ws/teefax HTTP/1.1\r\nHost: zxnet.co.uk\r\nUser-Agent: NeoTeletext/0.1 (macOS)\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
    }

    #[test]
    fn handshake_response_checks() {
        let key = *b"dGhlIHNhbXBsZSBub25jZQ==";
        let good = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
                     Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
        let head = http::parse_head(good, &mut |_| {}).unwrap();
        assert_eq!(check_handshake(&head, &key), Ok(()));
        let bad: [&[u8]; 5] = [
            b"HTTP/1.1 200 OK\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: AAAALMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nSec-WebSocket-Extensions: permessage-deflate\r\n\r\n",
        ];
        for text in bad {
            let head = http::parse_head(text, &mut |_| {}).unwrap();
            assert_eq!(check_handshake(&head, &key), Err(WsError::Handshake));
        }
    }

    #[test]
    fn masking_rfc6455_example() {
        // RFC 6455 section 5.7: a masked "Hello".
        let mut buffer = [0u8; 16];
        let mut writer = Writer::new(&mut buffer);
        write_frame_with_mask(&mut writer, OPCODE_TEXT, b"Hello", [0x37, 0xfa, 0x21, 0x3d]).unwrap();
        assert_eq!(writer.written(), &[0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58]);
        let mut writer = Writer::new(&mut buffer);
        write_frame(&mut writer, OPCODE_PING, b"", &mut Counter(9)).unwrap();
        assert_eq!(writer.written(), &[0x89, 0x80, 9, 10, 11, 12]);
    }

    #[test]
    fn long_lengths_are_encoded_minimally() {
        let payload = [0u8; 300];
        let mut buffer = [0u8; 400];
        let mut writer = Writer::new(&mut buffer);
        write_frame_with_mask(&mut writer, OPCODE_BINARY, &payload, [0; 4]).unwrap();
        assert_eq!(&writer.written()[..4], &[0x82, 0xFE, 0x01, 0x2C]);
        assert_eq!(writer.len(), 2 + 2 + 4 + 300);
    }

    fn read_all(
        input: &[u8],
        limit: usize,
        step: usize,
    ) -> Result<std::vec::Vec<(WsMessage, std::vec::Vec<u8>)>, WsError> {
        let mut reader = FrameReader::new();
        let mut message = std::vec![0u8; limit];
        let mut out = std::vec::Vec::new();
        for piece in input.chunks(step) {
            let mut rest = piece;
            loop {
                let (used, done) = reader.feed(&mut message, rest)?;
                rest = &rest[used..];
                if let Some(done) = done {
                    let data = match done {
                        WsMessage::Text(n) | WsMessage::Binary(n) => message[..n].to_vec(),
                        _ => reader.control_payload().to_vec(),
                    };
                    out.push((done, data));
                }
                if rest.is_empty() {
                    break;
                }
            }
        }
        Ok(out)
    }

    #[test]
    fn rfc6455_unmasked_examples() {
        // Section 5.7: single-frame text, fragmented text, ping, 256-byte binary.
        let mut stream = std::vec::Vec::new();
        stream.extend_from_slice(&[0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]);
        stream.extend_from_slice(&[0x01, 0x03, 0x48, 0x65, 0x6c]);
        stream.extend_from_slice(&[0x89, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]); // interleaved ping
        stream.extend_from_slice(&[0x80, 0x02, 0x6c, 0x6f]);
        stream.extend_from_slice(&[0x82, 0x7E, 0x01, 0x00]);
        stream.extend_from_slice(&[7u8; 256]);
        stream.extend_from_slice(&[0x88, 0x02, 0x03, 0xE8]);
        for step in [1, 2, 3, 5, stream.len()] {
            let messages = read_all(&stream, 512, step).unwrap();
            assert_eq!(messages.len(), 5, "step {step}");
            assert_eq!(messages[0], (WsMessage::Text(5), b"Hello".to_vec()));
            assert_eq!(messages[1], (WsMessage::Ping, b"Hello".to_vec()));
            assert_eq!(messages[2], (WsMessage::Text(5), b"Hello".to_vec()));
            assert_eq!(messages[3], (WsMessage::Binary(256), std::vec![7u8; 256]));
            assert_eq!(messages[4].0, WsMessage::Close(Some(1000)));
        }
    }

    #[test]
    fn framing_violations() {
        let cases: [(&[u8], WsError); 11] = [
            (&[0x81, 0x85, 0, 0, 0, 0, 1, 2, 3, 4, 5], WsError::Protocol), // masked server frame
            (&[0xC1, 0x00], WsError::Protocol),                            // RSV1 without extension
            (&[0x83, 0x00], WsError::Protocol),                            // reserved opcode
            (&[0x8B, 0x00], WsError::Protocol),                            // reserved control opcode
            (&[0x09, 0x00], WsError::Protocol),                            // fragmented ping
            (&[0x89, 0x7E, 0x00, 0x7E], WsError::Protocol),                // control frame over 125
            (&[0x80, 0x00], WsError::Protocol),                            // continuation without start
            (&[0x01, 0x01, b'a', 0x81, 0x00], WsError::Protocol),          // new message inside fragmented one
            (&[0x82, 0x7E, 0x00, 0x05, 1, 2, 3, 4, 5], WsError::Protocol), // non-minimal 16-bit length
            (&[0x81, 0x02, 0xC3, 0x28], WsError::InvalidText),             // invalid UTF-8
            (&[0x88, 0x01, 0x03], WsError::Protocol),                      // one-byte close payload
        ];
        for (input, error) in cases {
            assert_eq!(read_all(input, 64, input.len()).map(|m| m.len()), Err(error), "{input:02x?}");
        }
        let mut huge = std::vec![0x82, 0x7F];
        huge.extend_from_slice(&(1u64 << 63).to_be_bytes());
        assert_eq!(read_all(&huge, 64, 64).map(|m| m.len()), Err(WsError::Protocol));
    }

    #[test]
    fn oversize_messages() {
        let mut frame = std::vec![0x82, 0x7E, 0x01, 0x00];
        frame.extend_from_slice(&[0u8; 256]);
        assert_eq!(read_all(&frame, 255, 64).map(|m| m.len()), Err(WsError::TooBig));
        // Fragments together exceeding the limit.
        let mut fragments = std::vec![0x02, 0x7E, 0x00, 0x80];
        fragments.extend_from_slice(&[0u8; 128]);
        fragments.extend_from_slice(&[0x80, 0x7E, 0x00, 0x80]);
        fragments.extend_from_slice(&[0u8; 128]);
        assert_eq!(read_all(&fragments, 200, 64).map(|m| m.len()), Err(WsError::TooBig));
        assert_eq!(read_all(&fragments, 256, 64).map(|m| m.len()), Ok(1));
    }

    /// An oversized data message: the strict reader fails with `TooBig`; the
    /// skipping reader drops it, fragments included, and reads the next.
    #[test]
    fn oversized_messages_fail_or_are_skipped() {
        let big = [b'a'; 200];
        let mut stream = std::vec![OPCODE_TEXT, 126];
        stream.extend_from_slice(&200u16.to_be_bytes());
        stream.extend_from_slice(&big);
        // The same, split into a first frame and a final continuation.
        stream.extend_from_slice(&[OPCODE_TEXT, 100]);
        stream.extend_from_slice(&big[..100]);
        stream.extend_from_slice(&[WS_FIN | OPCODE_CONTINUATION, 100]);
        stream.extend_from_slice(&big[..100]);
        stream.extend_from_slice(&[WS_FIN | OPCODE_TEXT, 2, b'h', b'i']);
        stream[0] |= WS_FIN;
        let mut buffer = [0u8; 16];
        assert_eq!(FrameReader::new().feed(&mut buffer, &stream), Err(WsError::TooBig));
        let mut reader = FrameReader::skipping_oversized();
        let mut rest = stream.as_slice();
        let mut got = std::vec::Vec::new();
        while !rest.is_empty() {
            let (used, message) = reader.feed(&mut buffer, rest).unwrap();
            got.extend(message);
            rest = &rest[used..];
        }
        assert_eq!(got, [WsMessage::Skipped, WsMessage::Skipped, WsMessage::Text(2)]);
        assert_eq!(&buffer[..2], b"hi");
    }
}
