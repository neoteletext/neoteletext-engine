//! The response head as the profile reads it (REQ-WEB-014, REQ-SEC-011):
//! the collector that finds where a head ends in arriving bytes, and the
//! reader that turns the parsed head (`super::head`) into the facts the
//! agent acts on, with the framing rules of RFC 9112 section 6.3.

use super::head::{self, HeadError};
use super::{Framing, HttpError, MediaType};
use crate::bytes::{TextBuf, copy_prefix, parse_decimal, split_once, trim_ows};
use crate::consts::{
    CHUNKED, CLEAR_COOKIES, CLEAR_EVERYTHING, CONNECTION_CLOSE, CONNECTION_UPGRADE, HTTP_MAJOR, HTTP_MINOR_0,
    HTTP_MINOR_1, IDENTITY, MAX_CONTENT_LENGTH_DIGITS, MAX_URL_LENGTH, MAX_VALIDATOR_LENGTH,
    STATUS_INFORMATIONAL_FIRST, STATUS_LIMIT, STATUS_NO_CONTENT, STATUS_NOT_MODIFIED, STATUS_SUCCESS_FIRST,
    STATUS_SWITCHING_PROTOCOLS, UPGRADE_WEBSOCKET, WEBSOCKET_ACCEPT_TEXT, cache, header,
};

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

/// True when the complete head in `bytes` is an interim response, a 1xx
/// other than 101: RFC 9110 section 15.2 asks a client to read past any
/// number of them before the final response, and their fields are
/// ignored (finding F-75). A head that does not parse is not interim, so
/// [`parse_head`] reports it.
#[must_use]
pub fn is_interim(bytes: &[u8]) -> bool {
    head::parse(bytes).is_ok_and(|parsed| {
        (STATUS_INFORMATIONAL_FIRST..STATUS_SUCCESS_FIRST).contains(&parsed.status)
            && parsed.status != STATUS_SWITCHING_PROTOCOLS
    })
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
    // Interim responses are read past before this (`is_interim`); 101 is
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
