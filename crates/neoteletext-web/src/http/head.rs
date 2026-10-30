//! The response-head parser: the status line and header fields of an
//! HTTP/1.x response (RFC 9112 sections 2 to 5), with the strict handling
//! of REQ-SEC-011.
//!
//! The head is checked whole before any field is handed out, so a
//! malformed head is never partly read (REQ-WEB-014). [`parse`] walks the
//! head once to check it and count its fields, and [`Head::fields`] walks
//! it again with the same line reader, so the fields read are exactly the
//! fields checked. Nothing is stored per field, so the stack does not grow
//! with the header limit. Each walk looks at each byte a bounded number of
//! times (REQ-SEC-012).
//!
//! Where RFC 9112 leaves the recipient a choice:
//! - A bare LF ends a line as CR LF does (RFC 9112 §2.2 allows it). A CR
//!   anywhere else is a forbidden character. The head collector ends a head
//!   only at CR LF CR LF and [`parse`] must use every byte it is given, so
//!   a head whose empty line is a bare LF never passes: the two cannot
//!   disagree about where a head ends.
//! - Empty lines before the status line are skipped (RFC 9112 §2.2's
//!   robustness rule).
//! - The reason phrase may be missing together with the space before it
//!   (RFC 9112 §4: a client should ignore the reason phrase).
//! - The version is read as any `HTTP/` DIGIT `.` DIGIT; the caller decides
//!   which versions it accepts.

use crate::bytes::{parse_decimal, trim_ows};
use crate::consts::{HTTP_NAME, MAX_RESPONSE_HEADERS, STATUS_CODE_DIGITS};

/// Why a response head was rejected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeadError {
    /// The bytes end before the empty line that ends the head.
    Incomplete,
    /// The status line does not start with `HTTP/` DIGIT `.` DIGIT and a
    /// space (RFC 9112 §2.3).
    Version,
    /// The status code is not three digits, or the reason phrase holds a
    /// control character (RFC 9112 §4).
    Status,
    /// A field name is empty, holds a character that is not a token
    /// character, or is followed by white space before its colon (RFC 9112
    /// §5.1).
    FieldName,
    /// A field value holds a control character (RFC 9110 §5.5).
    FieldValue,
    /// A field line starts with white space: obsolete line folding (RFC
    /// 9112 §5.2) or white space before the first field (RFC 9112 §2.2).
    LeadingWhitespace,
    /// More fields than [`MAX_RESPONSE_HEADERS`] (REQ-SEC-011).
    TooManyFields,
    /// Bytes follow the empty line that ends the head.
    TrailingBytes,
}

/// One header field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Field<'a> {
    /// The field name, as received.
    pub name: &'a [u8],
    /// The field value without the optional white space around it (RFC
    /// 9112 §5.1).
    pub value: &'a [u8],
}

/// A checked response head.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Head<'a> {
    /// The major version digit.
    pub major: u8,
    /// The minor version digit.
    pub minor: u8,
    /// The status code, from 0 to 999.
    pub status: u16,
    /// The field lines and the empty line that ends them, all checked.
    field_lines: &'a [u8],
}

impl<'a> Head<'a> {
    /// The header fields, in the order received.
    #[must_use]
    pub const fn fields(&self) -> Fields<'a> {
        Fields { rest: self.field_lines }
    }
}

/// The header fields of a checked head.
#[derive(Clone, Debug)]
pub struct Fields<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for Fields<'a> {
    type Item = Field<'a>;

    fn next(&mut self) -> Option<Field<'a>> {
        // `parse` checked every line, so the walk ends only at the empty
        // line.
        let (found, rest) = next_field(self.rest).ok()?;
        self.rest = if found.is_some() { rest } else { &[] };
        found
    }
}

/// Parses a response head that fills `bytes` exactly, up to and including
/// the empty line that ends it (RFC 9112 §2.1: `status-line *( field-line
/// CRLF ) CRLF`).
pub fn parse(bytes: &[u8]) -> Result<Head<'_>, HeadError> {
    let mut rest = bytes;
    // RFC 9112 §2.2: a recipient may ignore empty lines before the start
    // line.
    let status_line = loop {
        let (line, after) = take_line(rest)?;
        rest = after;
        if !line.is_empty() {
            break line;
        }
    };
    let (major, minor, status) = read_status_line(status_line)?;
    let field_lines = rest;
    let mut count: usize = 0;
    loop {
        let (found, after) = next_field(rest)?;
        rest = after;
        if found.is_none() {
            break;
        }
        if count == MAX_RESPONSE_HEADERS {
            return Err(HeadError::TooManyFields);
        }
        count = count.saturating_add(1);
    }
    if !rest.is_empty() {
        return Err(HeadError::TrailingBytes);
    }
    Ok(Head { major, minor, status, field_lines })
}

/// Splits off one line: the line without its CR LF or bare LF, and the
/// bytes after it (RFC 9112 §2.2).
fn take_line(bytes: &[u8]) -> Result<(&[u8], &[u8]), HeadError> {
    let end = bytes.iter().position(|&byte| byte == b'\n').ok_or(HeadError::Incomplete)?;
    let (line, rest) = bytes.split_at_checked(end).ok_or(HeadError::Incomplete)?;
    let (_, rest) = rest.split_first().ok_or(HeadError::Incomplete)?;
    let line = match line.split_last() {
        Some((b'\r', before)) => before,
        _ => line,
    };
    Ok((line, rest))
}

/// Reads the next field line, or the empty line that ends the head: the
/// field, `None` at the end, and the bytes after the line.
fn next_field(bytes: &[u8]) -> Result<(Option<Field<'_>>, &[u8]), HeadError> {
    let (line, rest) = take_line(bytes)?;
    if line.is_empty() { Ok((None, rest)) } else { Ok((Some(read_field(line)?), rest)) }
}

/// RFC 9112 §4: `status-line = HTTP-version SP status-code SP
/// [ reason-phrase ]`, the version per §2.3: `HTTP-name "/" DIGIT "."
/// DIGIT`. Returns the major and minor digits and the code.
fn read_status_line(line: &[u8]) -> Result<(u8, u8, u16), HeadError> {
    let rest = line.strip_prefix(HTTP_NAME).ok_or(HeadError::Version)?;
    let [major, b'.', minor, b' ', rest @ ..] = rest else {
        return Err(HeadError::Version);
    };
    let major = digit_value(*major).ok_or(HeadError::Version)?;
    let minor = digit_value(*minor).ok_or(HeadError::Version)?;
    let (code, rest) = rest.split_first_chunk::<STATUS_CODE_DIGITS>().ok_or(HeadError::Status)?;
    let status =
        parse_decimal(code, STATUS_CODE_DIGITS).and_then(|value| u16::try_from(value).ok()).ok_or(HeadError::Status)?;
    let reason = match rest {
        [] => rest,
        [b' ', reason @ ..] => reason,
        _ => return Err(HeadError::Status),
    };
    // RFC 9112 §4: `reason-phrase = 1*( HTAB / SP / VCHAR / obs-text )`.
    if !reason.iter().all(|&byte| is_field_text(byte)) {
        return Err(HeadError::Status);
    }
    Ok((major, minor, status))
}

/// RFC 9112 §5: `field-line = field-name ":" OWS field-value OWS`.
fn read_field(line: &[u8]) -> Result<Field<'_>, HeadError> {
    if let [b' ' | b'\t', ..] = line {
        return Err(HeadError::LeadingWhitespace);
    }
    let colon = line.iter().position(|&byte| byte == b':').ok_or(HeadError::FieldName)?;
    let (name, rest) = line.split_at_checked(colon).ok_or(HeadError::FieldName)?;
    let (_, raw_value) = rest.split_first().ok_or(HeadError::FieldName)?;
    // RFC 9112 §5.1: no white space is allowed between the name and the
    // colon; a space is not a token character, so this rejects it.
    if name.is_empty() || !name.iter().all(|&byte| is_token_character(byte)) {
        return Err(HeadError::FieldName);
    }
    if !raw_value.iter().all(|&byte| is_field_text(byte)) {
        return Err(HeadError::FieldValue);
    }
    Ok(Field { name, value: trim_ows(raw_value) })
}

/// The value of an ASCII decimal digit.
const fn digit_value(byte: u8) -> Option<u8> {
    if byte.is_ascii_digit() { Some(byte.wrapping_sub(b'0')) } else { None }
}

/// RFC 9110 §5.6.2: a `tchar` is a letter, a digit, or one of
/// ``! # $ % & ' * + - . ^ _ ` | ~``.
pub(crate) const fn is_token_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
        )
}

/// A character a field value or a reason phrase may hold: HTAB, SP, VCHAR
/// (0x21 to 0x7E) or obs-text (0x80 to 0xFF) (RFC 9110 §5.5, RFC 9112 §4).
/// Every other control character, CR, LF, NUL and DEL among them, is
/// rejected.
pub(super) const fn is_field_text(byte: u8) -> bool {
    matches!(byte, b'\t' | b' '..=b'~' | 0x80..=0xFF)
}
