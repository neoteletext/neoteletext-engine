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
const fn is_token_character(byte: u8) -> bool {
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
const fn is_field_text(byte: u8) -> bool {
    matches!(byte, b'\t' | b' '..=b'~' | 0x80..=0xFF)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;
    use crate::consts::{HTTP_MAJOR, HTTP_MINOR_1, HTTP_VERSION};
    use proptest::prelude::*;
    use std::vec::Vec;

    /// The fields of a head as (name, value) pairs.
    fn pairs<'a>(head: &Head<'a>) -> Vec<(&'a [u8], &'a [u8])> {
        head.fields().map(|field| (field.name, field.value)).collect()
    }

    #[test]
    fn the_version_written_in_requests_is_one_this_parser_reads() {
        assert!(HTTP_VERSION.starts_with(HTTP_NAME));
        let mut text = Vec::from(HTTP_VERSION);
        text.extend_from_slice(b" 200 OK\r\n\r\n");
        let head = parse(&text).unwrap();
        assert_eq!((head.major, head.minor), (HTTP_MAJOR, HTTP_MINOR_1));
    }

    #[test]
    fn valid_heads() {
        let head = parse(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-A: b c\r\n\r\n").unwrap();
        assert_eq!((head.major, head.minor, head.status), (1, 1, 200));
        assert_eq!(pairs(&head), [(&b"Content-Length"[..], &b"5"[..]), (b"X-A", b"b c")]);

        let head = parse(b"HTTP/1.0 404 Not Found\r\n\r\n").unwrap();
        assert_eq!((head.major, head.minor, head.status), (1, 0, 404));
        assert_eq!(head.fields().count(), 0);

        // No reason phrase, with and without the space before it.
        assert_eq!(parse(b"HTTP/1.1 204\r\n\r\n").unwrap().status, 204);
        assert_eq!(parse(b"HTTP/1.1 204 \r\n\r\n").unwrap().status, 204);
        // A reason phrase with tabs and obs-text.
        assert_eq!(parse(b"HTTP/1.1 200 O\tK \xE9\r\n\r\n").unwrap().status, 200);
        // Versions the caller rejects are still read.
        let head = parse(b"HTTP/2.0 200 OK\r\n\r\n").unwrap();
        assert_eq!((head.major, head.minor), (2, 0));
        // Every token character in a name.
        let head = parse(b"HTTP/1.1 200 OK\r\n!#$%&'*+-.^_`|~09AZaz: v\r\n\r\n").unwrap();
        assert_eq!(pairs(&head), [(&b"!#$%&'*+-.^_`|~09AZaz"[..], &b"v"[..])]);
    }

    #[test]
    fn bare_line_feeds_and_leading_empty_lines() {
        let head = parse(b"\r\n\nHTTP/1.1 200 OK\nA: 1\nB: 2\r\n\r\n").unwrap();
        assert_eq!(head.status, 200);
        assert_eq!(pairs(&head), [(&b"A"[..], &b"1"[..]), (b"B", b"2")]);
        assert_eq!(parse(b"HTTP/1.1 200 OK\n\n").unwrap().status, 200);
    }

    #[test]
    fn white_space_is_trimmed_and_values_may_be_empty() {
        let head = parse(b"HTTP/1.1 200 OK\r\nA:  \t x y \t \r\nB:\r\nC: \r\nD:\t \t\r\nE:z\r\n\r\n").unwrap();
        assert_eq!(pairs(&head), [(&b"A"[..], &b"x y"[..]), (b"B", b""), (b"C", b""), (b"D", b""), (b"E", b"z")]);
    }

    #[test]
    fn values_keep_obs_text_and_inner_characters() {
        let head = parse(b"HTTP/1.1 200 OK\r\nA: \x80\xFF ;,=\"x\"\r\n\r\n").unwrap();
        assert_eq!(pairs(&head), [(&b"A"[..], &b"\x80\xFF ;,=\"x\""[..])]);
    }

    #[test]
    fn rejections() {
        let cases: &[(&[u8], HeadError)] = &[
            (b"HTTP/2 200 OK\r\n\r\n", HeadError::Version),
            (b"HTTP/1.1  200 OK\r\n\r\n", HeadError::Status),
            (b"http/1.1 200 OK\r\n\r\n", HeadError::Version),
            (b"HTTP/1.x 200 OK\r\n\r\n", HeadError::Version),
            (b"HTTP/11.1 200 OK\r\n\r\n", HeadError::Version),
            (b"HTTP/1.1\r\n\r\n", HeadError::Version),
            (b"HTTP/1.1\t200 OK\r\n\r\n", HeadError::Version),
            (b"ICY 200 OK\r\n\r\n", HeadError::Version),
            (b"HTTP/1.1 20 OK\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 2000 OK\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 2x0 OK\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 +20 OK\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 200\tOK\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 200 O\x00K\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 200 O\rK\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 200 OK\x7F\r\n\r\n", HeadError::Status),
            (b"HTTP/1.1 200 OK\r\nA b: c\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\nA : c\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\nA\t: c\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\n: c\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\nA(b): c\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\nA\"b: c\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\n\xC3\xA9: c\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\nNo colon\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\n\rA: b\r\n\r\n", HeadError::FieldName),
            (b"HTTP/1.1 200 OK\r\nA: b\x00c\r\n\r\n", HeadError::FieldValue),
            (b"HTTP/1.1 200 OK\r\nA: b\x01\r\n\r\n", HeadError::FieldValue),
            (b"HTTP/1.1 200 OK\r\nA: b\x1Fc\r\n\r\n", HeadError::FieldValue),
            (b"HTTP/1.1 200 OK\r\nA: b\x7Fc\r\n\r\n", HeadError::FieldValue),
            (b"HTTP/1.1 200 OK\r\nA: b\rc\r\n\r\n", HeadError::FieldValue),
            (b"HTTP/1.1 200 OK\r\nA: b\r\r\n\r\n", HeadError::FieldValue),
            (b"HTTP/1.1 200 OK\r\nA: b\r\n c\r\n\r\n", HeadError::LeadingWhitespace),
            (b"HTTP/1.1 200 OK\r\nA: b\r\n\tc\r\n\r\n", HeadError::LeadingWhitespace),
            (b"HTTP/1.1 200 OK\r\n A: b\r\n\r\n", HeadError::LeadingWhitespace),
            (b"HTTP/1.1 200 OK\r\nA: b\r\n\r\nbody", HeadError::TrailingBytes),
            (b"HTTP/1.1 200 OK\n\nA: b\r\n\r\n", HeadError::TrailingBytes),
            (b"HTTP/1.1 200 OK\r\nA: b\r\n", HeadError::Incomplete),
            (b"HTTP/1.1 200 OK\r\nA: b\r\n\r", HeadError::Incomplete),
            (b"HTTP/1.1 200 OK\r\n", HeadError::Incomplete),
            (b"HTTP/1.1 200 OK", HeadError::Incomplete),
            (b"\r\n\r\n", HeadError::Incomplete),
            (b"", HeadError::Incomplete),
        ];
        for (text, error) in cases {
            assert_eq!(parse(text).map(|head| head.status), Err(*error), "{}", text.escape_ascii());
        }
    }

    /// A head with `count` fields `X: y`.
    fn head_with_fields(count: usize) -> Vec<u8> {
        let mut text = Vec::from(&b"HTTP/1.1 200 OK\r\n"[..]);
        for _ in 0..count {
            text.extend_from_slice(b"X: y\r\n");
        }
        text.extend_from_slice(b"\r\n");
        text
    }

    #[test]
    fn field_count_limit() {
        assert_eq!(parse(&head_with_fields(MAX_RESPONSE_HEADERS)).unwrap().fields().count(), MAX_RESPONSE_HEADERS);
        assert_eq!(parse(&head_with_fields(MAX_RESPONSE_HEADERS + 1)), Err(HeadError::TooManyFields));
    }

    #[test]
    fn oversized_heads_are_bounded_by_the_caller() {
        // The parser has no size limit of its own: the head collector
        // refuses a head larger than its buffer (`HeadTooLarge`). A long
        // value is read whole.
        let mut text = Vec::from(&b"HTTP/1.1 200 OK\r\nA: "[..]);
        text.resize(text.len() + 100_000, b'v');
        text.extend_from_slice(b"\r\n\r\n");
        let head = parse(&text).unwrap();
        assert_eq!(head.fields().next().unwrap().value.len(), 100_000);
    }

    /// Token characters (RFC 9110 §5.6.2).
    fn token_characters() -> Vec<u8> {
        (0..=u8::MAX).filter(|&byte| is_token_character(byte)).collect()
    }

    /// Characters a value may start and end with: VCHAR and obs-text.
    fn visible_characters() -> Vec<u8> {
        (0..=u8::MAX).filter(|&byte| is_field_text(byte) && byte != b' ' && byte != b'\t').collect()
    }

    /// Optional white space (RFC 9110 §5.6.3).
    fn ows() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(prop::sample::select(&b" \t"[..]), 0..3)
    }

    /// A field value without white space at either end: words of visible
    /// characters separated by runs of white space.
    fn value() -> impl Strategy<Value = Vec<u8>> {
        let word = prop::collection::vec(prop::sample::select(visible_characters()), 1..8);
        let gap = prop::collection::vec(prop::sample::select(&b" \t"[..]), 1..3);
        prop::collection::vec((word, gap), 0..4).prop_map(|parts| {
            let mut value = Vec::new();
            for (index, (word, gap)) in parts.iter().enumerate() {
                if index > 0 {
                    value.extend_from_slice(gap);
                }
                value.extend_from_slice(word);
            }
            value
        })
    }

    /// The line endings a head may use: CR LF or a bare LF.
    static LINE_ENDS: [&[u8]; 2] = [b"\r\n", b"\n"];

    /// A line ending.
    fn line_end() -> impl Strategy<Value = &'static [u8]> {
        prop::sample::select(&LINE_ENDS[..])
    }

    /// A generated field: its name and value, the white space before and
    /// after the value, and the line ending.
    type GeneratedField = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, &'static [u8]);

    /// A field.
    fn field() -> impl Strategy<Value = GeneratedField> {
        (prop::collection::vec(prop::sample::select(token_characters()), 1..16), value(), ows(), ows(), line_end())
    }

    /// A reason phrase: any field text.
    fn reason() -> impl Strategy<Value = Vec<u8>> {
        let text: Vec<u8> = (0..=u8::MAX).filter(|&byte| is_field_text(byte)).collect();
        prop::collection::vec(prop::sample::select(text), 0..12)
    }

    proptest! {
        #[test]
        fn never_panics(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
            if let Ok(head) = parse(&bytes) {
                prop_assert!(head.fields().count() <= MAX_RESPONSE_HEADERS);
            }
        }

        #[test]
        fn heads_built_from_valid_parts_give_those_parts(
            leading in prop::collection::vec(line_end(), 0..2),
            major in 0u8..10,
            minor in 0u8..10,
            status in 0u16..1000,
            reason in reason(),
            status_end in line_end(),
            fields in prop::collection::vec(field(), 0..=MAX_RESPONSE_HEADERS),
        ) {
            let mut text = Vec::new();
            for end in &leading {
                text.extend_from_slice(end);
            }
            text.extend_from_slice(HTTP_NAME);
            text.extend_from_slice(&[b'0' + major, b'.', b'0' + minor, b' ']);
            text.extend_from_slice(std::format!("{status:03} ").as_bytes());
            text.extend_from_slice(&reason);
            text.extend_from_slice(status_end);
            for (name, value, before, after, end) in &fields {
                text.extend_from_slice(name);
                text.push(b':');
                text.extend_from_slice(before);
                text.extend_from_slice(value);
                text.extend_from_slice(after);
                text.extend_from_slice(end);
            }
            text.extend_from_slice(b"\r\n");
            let head = parse(&text).unwrap();
            prop_assert_eq!((head.major, head.minor, head.status), (major, minor, status));
            let expected: Vec<(&[u8], &[u8])> =
                fields.iter().map(|(name, value, ..)| (&name[..], &value[..])).collect();
            prop_assert_eq!(pairs(&head), expected);
            // Every line is checked only once its line ending has
            // arrived, so every proper prefix is incomplete.
            for cut in 0..text.len() {
                prop_assert_eq!(parse(&text[..cut]), Err(HeadError::Incomplete));
            }
        }
    }
}
