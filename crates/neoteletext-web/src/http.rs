//! HTTP/1.1 as the minimal user agent profile allows (REQ-WEB-014), with
//! strict framing (REQ-SEC-011).
//!
//! - Requests are written into a caller buffer with only the permitted
//!   headers.
//! - The response head is collected up to the header limit, then parsed by
//!   the project's own strict parser (`head`, RFC 9112 sections 2 to 5,
//!   obsolete line folding rejected) and checked: no compression, no
//!   ambiguous framing, a bounded number of headers.
//! - The body is framed by `Content-Length`, by chunked transfer coding
//!   (RFC 9112 section 7.1) or, where the caller allows it, by the end of
//!   the connection. The decoder enforces the caller's size limit.

mod head;

use head::HeadError;

use crate::bytes::{Overflow, TextBuf, Writer, copy_prefix, hex_value, parse_decimal, split_once, trim_ows};
use crate::consts::{
    CHUNKED, CLEAR_COOKIES, CLEAR_EVERYTHING, CONNECTION_CLOSE, CONNECTION_UPGRADE, CRLF, HTTP_MAJOR, HTTP_MINOR_0,
    HTTP_MINOR_1, HTTP_VERSION, IDENTITY, MAX_CHUNK_LINE, MAX_CHUNK_SIZE_DIGITS, MAX_CONTENT_LENGTH_DIGITS,
    MAX_URL_LENGTH, MAX_VALIDATOR_LENGTH, STATUS_INFORMATIONAL_FIRST, STATUS_LIMIT, STATUS_NO_CONTENT,
    STATUS_NOT_MODIFIED, STATUS_SUCCESS_FIRST, STATUS_SWITCHING_PROTOCOLS, UPGRADE_WEBSOCKET, WEBSOCKET_ACCEPT_TEXT,
    cache, header, media,
};

/// Why a response was rejected (REQ-WEB-014, REQ-SEC-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HttpError {
    /// The head exceeded the header size limit.
    HeadTooLarge,
    /// More headers than the limit.
    TooManyHeaders,
    /// The head is not valid HTTP/1.x, including obsolete line folding.
    Malformed,
    /// A protocol version other than HTTP/1.0 or HTTP/1.1.
    Version,
    /// A status code outside the profile.
    Status,
    /// A compressed or otherwise encoded body.
    Compressed,
    /// Both `Content-Length` and `Transfer-Encoding`.
    AmbiguousFraming,
    /// Several `Content-Length` values that disagree, or one that is not a
    /// decimal number.
    BadContentLength,
    /// A transfer coding other than `chunked`.
    BadTransferEncoding,
    /// A malformed chunk-size line or chunk terminator.
    BadChunk,
    /// The body exceeded its size limit.
    BodyTooLarge,
    /// The connection closed before the body was complete.
    Truncated,
    /// The body is delimited only by the connection closing, which the
    /// caller does not accept here.
    NoFraming,
}

/// The request methods of the profile (REQ-WEB-014).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    /// GET
    Get,
    /// POST, for form submission only.
    Post,
}

impl Method {
    /// The method token.
    #[must_use]
    pub const fn token(self) -> &'static [u8] {
        match self {
            Self::Get => b"GET",
            Self::Post => b"POST",
        }
    }
}

/// Writes the request line (RFC 9112 section 3).
pub fn write_request_line(writer: &mut Writer<'_>, method: Method, target: &str) -> Result<(), Overflow> {
    writer.put(method.token())?;
    writer.byte(b' ')?;
    writer.put(target.as_bytes())?;
    writer.byte(b' ')?;
    writer.put(HTTP_VERSION)?;
    writer.put(CRLF)
}

/// Writes one header field (RFC 9112 section 5).
pub fn write_header(writer: &mut Writer<'_>, name: &[u8], value: &[u8]) -> Result<(), Overflow> {
    writer.put(name)?;
    writer.put(b": ")?;
    writer.put(value)?;
    writer.put(CRLF)
}

/// Writes the `Host` header from the address's authority (RFC 9110
/// section 7.2).
pub fn write_host(writer: &mut Writer<'_>, url: &crate::url::Url) -> Result<(), Overflow> {
    write_header(writer, header::HOST, url.authority())
}

/// The response media types the profile knows (REQ-WEB-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MediaType {
    /// `text/x-teletext-tti`
    Tti,
    /// `application/x-teletext-t42`
    T42,
    /// `text/plain`: the caller detects the format (REQ-INP-005).
    TextPlain,
    /// `application/octet-stream`: the caller detects the format.
    OctetStream,
    /// `application/json`
    Json,
    /// `text/event-stream`
    EventStream,
    /// `application/x-font-bdf`, accepted for fonts only (REQ-FNT-003).
    FontBdf,
    /// Anything else, or none.
    Other,
}

impl MediaType {
    /// Classifies a `Content-Type` value, ignoring parameters (RFC 9110
    /// section 8.3.1).
    #[must_use]
    pub fn classify(value: &[u8]) -> Self {
        let (essence, _) = split_once(value, b';');
        let essence = trim_ows(essence);
        let known = [
            (media::TTI, Self::Tti),
            (media::T42, Self::T42),
            (media::TEXT_PLAIN, Self::TextPlain),
            (media::OCTET_STREAM, Self::OctetStream),
            (media::JSON, Self::Json),
            (media::EVENT_STREAM, Self::EventStream),
            (media::FONT_BDF, Self::FontBdf),
        ];
        known.into_iter().find(|(name, _)| essence.eq_ignore_ascii_case(name)).map_or(Self::Other, |(_, kind)| kind)
    }

    /// True for the media types a page response may have (REQ-WEB-003).
    #[must_use]
    pub const fn is_page(self) -> bool {
        matches!(self, Self::Tti | Self::T42 | Self::TextPlain | Self::OctetStream)
    }
}

/// How the body of a response is delimited (RFC 9112 section 6.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Framing {
    /// No body: 1xx, 204 and 304.
    None,
    /// Exactly this many bytes.
    Length(u64),
    /// Chunked transfer coding.
    Chunked,
    /// Until the connection closes.
    UntilClose,
}

/// What the profile reads from a response head (REQ-WEB-014).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// Each flag is a separate header fact of REQ-WEB-014's table.
#[allow(clippy::struct_excessive_bools)]
pub struct ResponseHead {
    /// The status code.
    pub status: u16,
    /// True for HTTP/1.1, false for HTTP/1.0.
    pub http11: bool,
    /// The body framing.
    pub framing: Framing,
    /// The `Content-Type`.
    pub media: MediaType,
    /// The `Location`, when present and short enough to be an address.
    pub location: Option<TextBuf<MAX_URL_LENGTH>>,
    /// The `ETag`, when present and short enough to keep.
    pub etag: Option<TextBuf<MAX_VALIDATOR_LENGTH>>,
    /// The `Last-Modified`, when present and short enough to keep.
    pub last_modified: Option<TextBuf<MAX_VALIDATOR_LENGTH>>,
    /// `Cache-Control: max-age`, in seconds.
    pub max_age: Option<u32>,
    /// `Cache-Control: no-cache`.
    pub no_cache: bool,
    /// `Cache-Control: no-store`.
    pub no_store: bool,
    /// `Retry-After` in seconds (the delay form only).
    pub retry_after: Option<u32>,
    /// The connection cannot be reused.
    pub close: bool,
    /// `Upgrade: websocket`.
    pub upgrade_websocket: bool,
    /// `Connection: Upgrade`.
    pub connection_upgrade: bool,
    /// `Sec-WebSocket-Accept`.
    pub websocket_accept: Option<[u8; WEBSOCKET_ACCEPT_TEXT]>,
    /// `Sec-WebSocket-Extensions` or `Sec-WebSocket-Protocol` present,
    /// neither of which this client asks for (RFC 6455 section 4.1).
    pub websocket_extras: bool,
    /// `Clear-Site-Data` names `"cookies"` or `"*"` (decision E.104).
    pub clear_cookies: bool,
}

/// True when a comma-separated header value contains `token`, ignoring
/// letter case (RFC 9110 section 5.6.1).
fn has_token(value: &[u8], token: &[u8]) -> bool {
    value.split(|&b| b == b',').any(|item| trim_ows(item).eq_ignore_ascii_case(token))
}

/// Parses a `Cache-Control` value into the response head.
fn apply_cache_control(head: &mut ResponseHead, value: &[u8]) {
    for directive in value.split(|&b| b == b',') {
        let (name, argument) = split_once(trim_ows(directive), b'=');
        let name = trim_ows(name);
        if name.eq_ignore_ascii_case(cache::NO_CACHE) {
            head.no_cache = true;
        } else if name.eq_ignore_ascii_case(cache::NO_STORE) {
            head.no_store = true;
        } else if name.eq_ignore_ascii_case(cache::MAX_AGE) {
            // RFC 9111 section 1.2.2: delta-seconds; larger values saturate.
            head.max_age = argument
                .map(trim_ows)
                .and_then(|digits| parse_decimal(digits, MAX_CONTENT_LENGTH_DIGITS))
                .map(|seconds| u32::try_from(seconds).unwrap_or(u32::MAX));
        }
    }
}

/// The framing headers seen so far.
#[derive(Default)]
struct FramingHeaders {
    content_length: Option<u64>,
    chunked: bool,
    transfer_encodings: u8,
}

/// Reads one header field into the head.
fn read_header(
    head: &mut ResponseHead,
    framing: &mut FramingHeaders,
    name: &[u8],
    raw_value: &[u8],
    on_set_cookie: &mut dyn FnMut(&[u8]),
) -> Result<(), HttpError> {
    let value = trim_ows(raw_value);
    let is = |known: &[u8]| name.eq_ignore_ascii_case(known);
    if is(header::CONTENT_LENGTH) {
        let length = parse_decimal(value, MAX_CONTENT_LENGTH_DIGITS).ok_or(HttpError::BadContentLength)?;
        if framing.content_length.is_some_and(|previous| previous != length) {
            return Err(HttpError::BadContentLength);
        }
        framing.content_length = Some(length);
    } else if is(header::TRANSFER_ENCODING) {
        framing.transfer_encodings = framing.transfer_encodings.saturating_add(1);
        if !value.eq_ignore_ascii_case(CHUNKED) || framing.transfer_encodings > 1 || !head.http11 {
            return Err(HttpError::BadTransferEncoding);
        }
        framing.chunked = true;
    } else if is(header::CONTENT_ENCODING) {
        if !value.eq_ignore_ascii_case(IDENTITY) {
            return Err(HttpError::Compressed);
        }
    } else if is(header::CONTENT_TYPE) {
        head.media = MediaType::classify(value);
    } else if is(header::LOCATION) {
        head.location = TextBuf::from_bytes(value);
    } else if is(header::ETAG) {
        head.etag = TextBuf::from_bytes(value);
    } else if is(header::LAST_MODIFIED) {
        head.last_modified = TextBuf::from_bytes(value);
    } else if is(header::CACHE_CONTROL) {
        apply_cache_control(head, value);
    } else if is(header::RETRY_AFTER) {
        head.retry_after = parse_decimal(value, MAX_CONTENT_LENGTH_DIGITS).and_then(|s| u32::try_from(s).ok());
    } else if is(header::CONNECTION) {
        head.close |= has_token(value, CONNECTION_CLOSE);
        head.connection_upgrade |= has_token(value, CONNECTION_UPGRADE);
    } else if is(header::UPGRADE) {
        head.upgrade_websocket = value.eq_ignore_ascii_case(UPGRADE_WEBSOCKET);
    } else if is(header::SEC_WEBSOCKET_ACCEPT) {
        head.websocket_accept = value.try_into().ok();
    } else if is(header::SEC_WEBSOCKET_EXTENSIONS) || is(header::SEC_WEBSOCKET_PROTOCOL) {
        head.websocket_extras = true;
    } else if is(header::SET_COOKIE) {
        on_set_cookie(raw_value);
    } else if is(header::CLEAR_SITE_DATA) {
        // A list of quoted types; an unknown type is ignored (W3C Clear
        // Site Data section 3.1).
        head.clear_cookies |=
            value.split(|&b| b == b',').map(trim_ows).any(|item| item == CLEAR_COOKIES || item == CLEAR_EVERYTHING);
    }
    Ok(())
}

/// Parses and checks a complete response head held in `bytes`.
/// `on_set_cookie` receives each `Set-Cookie` value in order.
pub fn parse_head(bytes: &[u8], on_set_cookie: &mut dyn FnMut(&[u8])) -> Result<ResponseHead, HttpError> {
    let parsed = head::parse(bytes).map_err(|error| match error {
        HeadError::TooManyFields => HttpError::TooManyHeaders,
        // A partial head, trailing bytes, or any syntax error, obsolete line
        // folding included.
        HeadError::Incomplete
        | HeadError::Version
        | HeadError::Status
        | HeadError::FieldName
        | HeadError::FieldValue
        | HeadError::LeadingWhitespace
        | HeadError::TrailingBytes => HttpError::Malformed,
    })?;
    let http11 = match (parsed.major, parsed.minor) {
        (HTTP_MAJOR, HTTP_MINOR_1) => true,
        (HTTP_MAJOR, HTTP_MINOR_0) => false,
        _ => return Err(HttpError::Version),
    };
    let status = parsed.status;
    if !(STATUS_INFORMATIONAL_FIRST..STATUS_LIMIT).contains(&status) {
        return Err(HttpError::Status);
    }
    // No request this client sends asks for an interim response; 101 is
    // the WebSocket upgrade.
    if status < STATUS_SUCCESS_FIRST && status != STATUS_SWITCHING_PROTOCOLS {
        return Err(HttpError::Status);
    }
    let mut head = ResponseHead {
        status,
        http11,
        framing: Framing::UntilClose,
        media: MediaType::Other,
        location: None,
        etag: None,
        last_modified: None,
        max_age: None,
        no_cache: false,
        no_store: false,
        retry_after: None,
        close: !http11,
        upgrade_websocket: false,
        connection_upgrade: false,
        websocket_accept: None,
        websocket_extras: false,
        clear_cookies: false,
    };
    let mut framing = FramingHeaders::default();
    for field in parsed.fields() {
        read_header(&mut head, &mut framing, field.name, field.value, on_set_cookie)?;
    }
    let FramingHeaders { content_length, chunked, .. } = framing;
    if chunked && content_length.is_some() {
        return Err(HttpError::AmbiguousFraming);
    }
    head.framing = if status == STATUS_NO_CONTENT || status == STATUS_NOT_MODIFIED || status < STATUS_SUCCESS_FIRST {
        Framing::None
    } else if chunked {
        Framing::Chunked
    } else if let Some(length) = content_length {
        Framing::Length(length)
    } else {
        head.close = true;
        Framing::UntilClose
    };
    Ok(head)
}

/// Calls `each` with the value of every field of a complete response head
/// named `name` (compared without regard to case), in the order received;
/// a head that does not parse gives none. For the fields [`parse_head`]
/// does not keep, such as those of Device Bound Session Credentials.
pub fn each_field(bytes: &[u8], name: &[u8], each: &mut dyn FnMut(&[u8])) {
    let Ok(parsed) = head::parse(bytes) else { return };
    for field in parsed.fields().filter(|field| field.name.eq_ignore_ascii_case(name)) {
        each(field.value);
    }
}

/// The empty line that ends a head (RFC 9112 section 2.1).
const HEAD_END: [u8; 4] = *b"\r\n\r\n";

/// Collects a response head from arriving bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeadCollector {
    len: usize,
    /// The last four bytes collected, to find the end across pieces.
    window: [u8; 4],
}

impl HeadCollector {
    /// Starts collecting a new head.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Bytes collected.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True when nothing has been collected.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends from `input` into `buffer` up to the end of the head (the
    /// empty line, CR LF CR LF). Returns the bytes of `input` used and
    /// whether the head is complete. Each byte is looked at once, so the
    /// work is linear in the head's length (REQ-SEC-012).
    pub fn feed(&mut self, buffer: &mut [u8], input: &[u8]) -> Result<(usize, bool), HttpError> {
        let mut found = None;
        for (index, &byte) in input.iter().enumerate() {
            self.window.rotate_left(1);
            if let Some(last) = self.window.last_mut() {
                *last = byte;
            }
            if self.window == HEAD_END {
                found = Some(index.saturating_add(1));
                break;
            }
        }
        let take = found.unwrap_or(input.len());
        let end = self.len.checked_add(take).ok_or(HttpError::HeadTooLarge)?;
        copy_prefix(buffer.get_mut(self.len..end).ok_or(HttpError::HeadTooLarge)?, input);
        self.len = end;
        Ok((take, found.is_some()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ChunkState {
    /// Reading a chunk-size line.
    Size,
    /// Reading chunk data.
    Data,
    /// Expecting the CR LF after chunk data.
    DataEnd,
    /// Reading trailer lines after the last chunk.
    Trailer,
}

/// One step of body decoding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BodyStep<'a> {
    /// Bytes of the input used.
    pub consumed: usize,
    /// Body data found in them, if any.
    pub data: &'a [u8],
    /// The body is complete.
    pub done: bool,
}

/// Decodes a response body according to its framing, enforcing a limit.
#[derive(Clone, Copy, Debug)]
pub struct BodyDecoder {
    framing: Framing,
    limit: usize,
    received: usize,
    remaining: u64,
    chunk: ChunkState,
    line: [u8; MAX_CHUNK_LINE],
    line_len: usize,
    done: bool,
}

impl BodyDecoder {
    /// A decoder for `framing` that accepts at most `limit` body bytes.
    pub fn new(framing: Framing, limit: usize) -> Result<Self, HttpError> {
        let (remaining, done) = match framing {
            Framing::None => (0, true),
            Framing::Length(length) => {
                if usize::try_from(length).map_or(true, |length| length > limit) {
                    return Err(HttpError::BodyTooLarge);
                }
                (length, length == 0)
            }
            Framing::Chunked | Framing::UntilClose => (0, false),
        };
        Ok(Self {
            framing,
            limit,
            received: 0,
            remaining,
            chunk: ChunkState::Size,
            line: [0; MAX_CHUNK_LINE],
            line_len: 0,
            done,
        })
    }

    /// True when the body is complete.
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.done
    }

    /// Body bytes delivered so far.
    #[must_use]
    pub const fn received(&self) -> usize {
        self.received
    }

    fn count(&mut self, bytes: usize) -> Result<(), HttpError> {
        self.received = self.received.checked_add(bytes).ok_or(HttpError::BodyTooLarge)?;
        if self.received > self.limit { Err(HttpError::BodyTooLarge) } else { Ok(()) }
    }

    /// The connection closed: completes a close-delimited body, and is an
    /// error for any other unfinished body.
    pub fn finish_at_close(&mut self) -> Result<(), HttpError> {
        if self.done {
            return Ok(());
        }
        if self.framing == Framing::UntilClose {
            self.done = true;
            Ok(())
        } else {
            Err(HttpError::Truncated)
        }
    }

    /// Decodes from `input`, returning at most one run of body data.
    pub fn feed<'a>(&mut self, input: &'a [u8]) -> Result<BodyStep<'a>, HttpError> {
        if self.done {
            return Ok(BodyStep { consumed: 0, data: &[], done: true });
        }
        match self.framing {
            Framing::None => {
                self.done = true;
                Ok(BodyStep { consumed: 0, data: &[], done: true })
            }
            Framing::UntilClose => {
                self.count(input.len())?;
                Ok(BodyStep { consumed: input.len(), data: input, done: false })
            }
            Framing::Length(_) => {
                let take = usize::try_from(self.remaining).unwrap_or(usize::MAX).min(input.len());
                self.count(take)?;
                self.remaining = self.remaining.saturating_sub(take as u64);
                self.done = self.remaining == 0;
                Ok(BodyStep { consumed: take, data: input.get(..take).unwrap_or(&[]), done: self.done })
            }
            Framing::Chunked => self.feed_chunked(input),
        }
    }

    /// Reads up to a line feed into the line buffer. Returns the bytes used
    /// and whether the line is complete.
    fn read_line(&mut self, input: &[u8]) -> Result<(usize, bool), HttpError> {
        let end = input.iter().position(|&b| b == b'\n');
        let take = end.map_or(input.len(), |i| i.saturating_add(1));
        let new_len = self.line_len.checked_add(take).ok_or(HttpError::BadChunk)?;
        copy_prefix(self.line.get_mut(self.line_len..new_len).ok_or(HttpError::BadChunk)?, input);
        self.line_len = new_len;
        Ok((take, end.is_some()))
    }

    /// The completed line without its CR LF, which must both be present
    /// (RFC 9112 section 7.1).
    fn take_line(&mut self) -> Result<([u8; MAX_CHUNK_LINE], usize), HttpError> {
        let len = self.line_len;
        self.line_len = 0;
        let line = self.line.get(..len).unwrap_or(&[]);
        let content = line.strip_suffix(CRLF).ok_or(HttpError::BadChunk)?;
        if content.contains(&b'\r') {
            return Err(HttpError::BadChunk);
        }
        Ok((self.line, content.len()))
    }

    fn feed_chunked<'a>(&mut self, input: &'a [u8]) -> Result<BodyStep<'a>, HttpError> {
        let mut used = 0usize;
        loop {
            let rest = input.get(used..).unwrap_or(&[]);
            if rest.is_empty() {
                return Ok(BodyStep { consumed: used, data: &[], done: false });
            }
            match self.chunk {
                ChunkState::Size => {
                    let (take, complete) = self.read_line(rest)?;
                    used = used.saturating_add(take);
                    if complete {
                        let (line, len) = self.take_line()?;
                        let size = parse_chunk_size(line.get(..len).unwrap_or(&[]))?;
                        if size == 0 {
                            self.chunk = ChunkState::Trailer;
                        } else {
                            if usize::try_from(size)
                                .map_or(true, |size| size > self.limit.saturating_sub(self.received))
                            {
                                return Err(HttpError::BodyTooLarge);
                            }
                            self.remaining = size;
                            self.chunk = ChunkState::Data;
                        }
                    }
                }
                ChunkState::Data => {
                    let take = usize::try_from(self.remaining).unwrap_or(usize::MAX).min(rest.len());
                    self.count(take)?;
                    self.remaining = self.remaining.saturating_sub(take as u64);
                    if self.remaining == 0 {
                        self.chunk = ChunkState::DataEnd;
                    }
                    let data = rest.get(..take).unwrap_or(&[]);
                    return Ok(BodyStep { consumed: used.saturating_add(take), data, done: false });
                }
                ChunkState::DataEnd => {
                    let (take, complete) = self.read_line(rest)?;
                    used = used.saturating_add(take);
                    if complete {
                        let (_, len) = self.take_line()?;
                        if len != 0 {
                            return Err(HttpError::BadChunk);
                        }
                        self.chunk = ChunkState::Size;
                    } else if self.line_len > CRLF.len() {
                        return Err(HttpError::BadChunk);
                    }
                }
                ChunkState::Trailer => {
                    let (take, complete) = self.read_line(rest)?;
                    used = used.saturating_add(take);
                    if complete {
                        let (_, len) = self.take_line()?;
                        // Trailer fields are read and ignored (RFC 9112
                        // section 7.1.2); the empty line ends the body.
                        if len == 0 {
                            self.done = true;
                            return Ok(BodyStep { consumed: used, data: &[], done: true });
                        }
                    }
                }
            }
        }
    }
}

/// Parses a chunk-size line (RFC 9112 section 7.1): hexadecimal digits,
/// then optionally `;` and extensions, which are ignored. No white space.
fn parse_chunk_size(line: &[u8]) -> Result<u64, HttpError> {
    let digits_end = line.iter().position(|&b| b == b';').unwrap_or(line.len());
    let digits = line.get(..digits_end).unwrap_or(&[]);
    if digits.is_empty() || digits.len() > MAX_CHUNK_SIZE_DIGITS {
        return Err(HttpError::BadChunk);
    }
    let extension = line.get(digits_end..).unwrap_or(&[]);
    if extension.iter().any(|&b| b < 0x20 || b == 0x7F) {
        return Err(HttpError::BadChunk);
    }
    digits.iter().try_fold(0u64, |value, &d| Some((value << 4) | u64::from(hex_value(d)?))).ok_or(HttpError::BadChunk)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;
    use crate::consts::MAX_RESPONSE_HEADERS;

    fn head(text: &[u8]) -> Result<ResponseHead, HttpError> {
        parse_head(text, &mut |_| {})
    }

    /// Decision E.104: `Clear-Site-Data` clears cookies when it names
    /// `"cookies"` or `"*"`, quoted and exactly; anything else does not.
    #[test]
    fn clear_site_data_names_cookies() {
        let clears = |value: &str| {
            let text = std::format!("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nClear-Site-Data: {value}\r\n\r\n");
            head(text.as_bytes()).unwrap().clear_cookies
        };
        for value in ["\"cookies\"", "\"cache\", \"cookies\"", "\"*\"", " \"storage\" ,\"cookies\" "] {
            assert!(clears(value), "{value}");
        }
        for value in ["cookies", "\"Cookies\"", "\"cache\"", "\"cookies", "\"\"", "\"cookies\"x"] {
            assert!(!clears(value), "{value}");
        }
        assert!(!head(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n").unwrap().clear_cookies);
    }

    #[test]
    fn head_collector_finds_end_across_pieces() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi";
        for split in 1..response.len() {
            let mut buffer = [0u8; 128];
            let mut collector = HeadCollector::default();
            let (first, done1) = collector.feed(&mut buffer, &response[..split]).unwrap();
            let total = if done1 {
                first
            } else {
                let (second, done2) = collector.feed(&mut buffer, &response[split..]).unwrap();
                assert!(done2, "split {split}");
                first + second
            };
            assert_eq!(total, response.len() - 2, "split {split}");
            assert_eq!(collector.len(), response.len() - 2);
        }
        // Byte by byte.
        let mut buffer = [0u8; 128];
        let mut collector = HeadCollector::default();
        let mut done_at = None;
        for (index, byte) in response.iter().enumerate() {
            let (used, done) = collector.feed(&mut buffer, core::slice::from_ref(byte)).unwrap();
            assert_eq!(used, 1);
            if done {
                done_at = Some(index);
                break;
            }
        }
        assert_eq!(done_at, Some(response.len() - 3));
    }

    #[test]
    fn head_size_limit() {
        let mut buffer = [0u8; 16];
        let mut collector = HeadCollector::default();
        assert_eq!(
            collector.feed(&mut buffer, b"HTTP/1.1 200 OK\r\nX: yyyyyyyy\r\n\r\n"),
            Err(HttpError::HeadTooLarge)
        );
    }

    #[test]
    fn framing_rules() {
        assert_eq!(head(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n").unwrap().framing, Framing::Length(5));
        assert_eq!(head(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap().framing, Framing::Chunked);
        let until_close = head(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
        assert_eq!(until_close.framing, Framing::UntilClose);
        assert!(until_close.close);
        assert_eq!(head(b"HTTP/1.1 304 Not Modified\r\nContent-Length: 9\r\n\r\n").unwrap().framing, Framing::None);
        assert_eq!(head(b"HTTP/1.1 204 No Content\r\n\r\n").unwrap().framing, Framing::None);
        assert_eq!(
            head(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\n").unwrap().framing,
            Framing::Length(5)
        );
        assert!(head(b"HTTP/1.0 200 OK\r\nContent-Length: 1\r\n\r\n").unwrap().close);
    }

    #[test]
    fn framing_attacks_are_rejected() {
        let cases: [(&[u8], HttpError); 13] = [
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n",
                HttpError::AmbiguousFraming,
            ),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\n",
                HttpError::AmbiguousFraming,
            ),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\n", HttpError::BadContentLength),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 5, 5\r\n\r\n", HttpError::BadContentLength),
            (b"HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\n", HttpError::BadContentLength),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 0x10\r\n\r\n", HttpError::BadContentLength),
            (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, chunked\r\n\r\n", HttpError::BadTransferEncoding),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n\r\n",
                HttpError::BadTransferEncoding,
            ),
            (b"HTTP/1.0 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n", HttpError::BadTransferEncoding),
            (b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\n\r\n", HttpError::Compressed),
            (b"HTTP/1.1 200 OK\r\nX-Folded: a\r\n b\r\n\r\n", HttpError::Malformed),
            (b"HTTP/2 200 OK\r\n\r\n", HttpError::Malformed),
            (b"HTTP/1.1 100 Continue\r\n\r\n", HttpError::Status),
        ];
        for (text, error) in cases {
            assert_eq!(head(text).map(|h| h.status), Err(error), "{:?}", core::str::from_utf8(text));
        }
        assert_eq!(head(b"HTTP/1.1 200 OK\r\nContent-Encoding: identity\r\n\r\n").map(|h| h.status), Ok(200));
    }

    #[test]
    fn versions_outside_the_profile_are_rejected() {
        assert!(head(b"HTTP/1.1 200 OK\r\n\r\n").unwrap().http11);
        assert!(!head(b"HTTP/1.0 200 OK\r\n\r\n").unwrap().http11);
        for text in [&b"HTTP/1.2 200 OK\r\n\r\n"[..], b"HTTP/2.0 200 OK\r\n\r\n", b"HTTP/0.9 200 OK\r\n\r\n"] {
            assert_eq!(head(text).map(|h| h.status), Err(HttpError::Version));
        }
    }

    #[test]
    fn a_head_with_bytes_after_it_is_rejected() {
        assert_eq!(head(b"HTTP/1.1 200 OK\r\n\r\nX").map(|h| h.status), Err(HttpError::Malformed));
        assert_eq!(head(b"HTTP/1.1 200 OK\r\n").map(|h| h.status), Err(HttpError::Malformed));
    }

    #[test]
    fn header_count_limit() {
        let mut text = [0u8; 2048];
        let mut writer = Writer::new(&mut text);
        writer.put(b"HTTP/1.1 200 OK\r\n").unwrap();
        for _ in 0..=MAX_RESPONSE_HEADERS {
            writer.put(b"X: y\r\n").unwrap();
        }
        writer.put(b"\r\n").unwrap();
        let len = writer.len();
        assert_eq!(head(&text[..len]).map(|h| h.status), Err(HttpError::TooManyHeaders));
    }

    #[test]
    fn profile_headers_are_read() {
        let mut cookies = 0;
        let parsed = parse_head(
            b"HTTP/1.1 301 Moved\r\nLocation: /p/101\r\nETag: \"v1\"\r\nLast-Modified: Sat, 10 Oct 2026 10:00:00 GMT\r\n\
              Cache-Control: max-age=60, no-cache\r\nRetry-After: 5\r\nContent-Type: text/x-teletext-tti; charset=utf-8\r\n\
              Set-Cookie: a=1\r\nSet-Cookie: b=2\r\nContent-Length: 0\r\n\r\n",
            &mut |_| cookies += 1,
        )
        .unwrap();
        assert_eq!(cookies, 2);
        assert_eq!(parsed.location.unwrap().as_bytes(), b"/p/101");
        assert_eq!(parsed.etag.unwrap().as_bytes(), b"\"v1\"");
        assert!(parsed.last_modified.is_some());
        assert_eq!(parsed.max_age, Some(60));
        assert!(parsed.no_cache && !parsed.no_store);
        assert_eq!(parsed.retry_after, Some(5));
        assert_eq!(parsed.media, MediaType::Tti);
    }

    fn decode_all(framing: Framing, limit: usize, input: &[u8], step: usize) -> Result<([u8; 256], usize), HttpError> {
        let mut decoder = BodyDecoder::new(framing, limit)?;
        let mut out = [0u8; 256];
        let mut len = 0;
        let mut offset = 0;
        while offset < input.len() && !decoder.is_done() {
            let end = (offset + step).min(input.len());
            let mut piece = &input[offset..end];
            while !piece.is_empty() && !decoder.is_done() {
                let result = decoder.feed(piece)?;
                out[len..len + result.data.len()].copy_from_slice(result.data);
                len += result.data.len();
                piece = &piece[result.consumed..];
                offset += result.consumed;
                if result.consumed == 0 {
                    break;
                }
            }
        }
        decoder.finish_at_close()?;
        Ok((out, len))
    }

    #[test]
    fn chunked_bodies() {
        let body = b"4\r\nWiki\r\n5;name=value\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\nTrailer: x\r\n\r\n";
        for step in [1, 2, 3, 7, body.len()] {
            let (out, len) = decode_all(Framing::Chunked, 100, body, step).unwrap();
            assert_eq!(&out[..len], b"Wikipedia in\r\n\r\nchunks.", "step {step}");
        }
    }

    #[test]
    fn bad_chunks_are_rejected() {
        let cases: [&[u8]; 8] = [
            b"g\r\nx\r\n0\r\n\r\n",
            b" 4\r\nWiki\r\n0\r\n\r\n",
            b"4 \r\nWiki\r\n0\r\n\r\n",
            b"4\nWiki\r\n0\r\n\r\n",
            b"4\r\nWikiX\r\n0\r\n\r\n",
            b"123456789\r\n",
            b"-1\r\n",
            b"\r\n",
        ];
        for case in cases {
            assert_eq!(decode_all(Framing::Chunked, 100, case, 64).map(|r| r.1), Err(HttpError::BadChunk), "{case:?}");
        }
        assert_eq!(decode_all(Framing::Chunked, 100, b"4\r\nWi", 64).map(|r| r.1), Err(HttpError::Truncated));
    }

    #[test]
    fn size_limits() {
        assert_eq!(BodyDecoder::new(Framing::Length(11), 10).map(|_| ()), Err(HttpError::BodyTooLarge));
        assert_eq!(
            decode_all(Framing::Chunked, 3, b"4\r\nWiki\r\n0\r\n\r\n", 64).map(|r| r.1),
            Err(HttpError::BodyTooLarge)
        );
        assert_eq!(decode_all(Framing::UntilClose, 3, b"abcd", 64).map(|r| r.1), Err(HttpError::BodyTooLarge));
        let (out, len) = decode_all(Framing::Length(3), 10, b"abcdef", 64).unwrap();
        assert_eq!(&out[..len], b"abc");
        assert_eq!(decode_all(Framing::Length(5), 10, b"abc", 64).map(|r| r.1), Err(HttpError::Truncated));
    }

    #[test]
    fn media_types() {
        assert_eq!(MediaType::classify(b"Application/X-Teletext-T42"), MediaType::T42);
        assert_eq!(MediaType::classify(b"text/plain; charset=us-ascii"), MediaType::TextPlain);
        assert_eq!(MediaType::classify(b"text/html"), MediaType::Other);
        assert!(!MediaType::Json.is_page());
    }

    #[test]
    fn request_lines() {
        let mut buffer = [0u8; 128];
        let mut writer = Writer::new(&mut buffer);
        let url = crate::url::Url::parse("http://a.b:81/x?y").unwrap();
        write_request_line(&mut writer, Method::Get, url.target()).unwrap();
        write_host(&mut writer, &url).unwrap();
        assert_eq!(writer.written(), b"GET /x?y HTTP/1.1\r\nHost: a.b:81\r\n");
    }
}
