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

mod body;
mod head;
mod response;

pub use body::{BodyDecoder, BodyStep};
pub use response::{HeadCollector, ResponseHead, each_field, parse_head};

use crate::bytes::{Overflow, Writer, split_once, trim_ows};
use crate::consts::{CRLF, HTTP_VERSION, header, media};

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

#[cfg(test)]
mod tests;
