//! WebSocket client framing (RFC 6455), written in the project (decision
//! E.38): the opening handshake and its accept check, client masking with
//! keys from the caller's random source, framing, fragmentation, control
//! frames and size limits.

use crate::base64;
use crate::bytes::{Overflow, Writer, copy_prefix};
use crate::consts::{
    CLOSE_CODE_BYTES, CLOSE_INVALID_DATA, CLOSE_PROTOCOL_ERROR, CLOSE_RESERVED, CLOSE_TOO_BIG, OPCODE_BINARY,
    OPCODE_CLOSE, OPCODE_CONTINUATION, OPCODE_CONTROL_FIRST, OPCODE_PING, OPCODE_PONG, OPCODE_TEXT,
    STATUS_SWITCHING_PROTOCOLS, UPGRADE_WEBSOCKET, WEBSOCKET_ACCEPT_TEXT, WEBSOCKET_GUID, WEBSOCKET_KEY_BYTES,
    WEBSOCKET_KEY_TEXT, WEBSOCKET_VERSION, WS_CONTROL_MAX, WS_FIN, WS_LENGTH_7_MAX, WS_LENGTH_16, WS_LENGTH_64,
    WS_LENGTH_MASK, WS_MASK_BIT, WS_MASK_BYTES, WS_MAX_HEADER, WS_OPCODE_MASK, WS_RSV_MASK, header,
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
                // RFC 6455 section 7.4.1: these are for use outside frames,
                // so one in a frame is the server's protocol error, and is
                // never echoed (decision E.114 item 8, finding F-92).
                if code.is_some_and(|code| CLOSE_RESERVED.contains(&code)) {
                    return Err(WsError::Protocol);
                }
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
mod tests;
