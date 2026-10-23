#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The tests of HTTP/1.1 as the profile reads and writes it, and of the
//! response-head parser (`super::head`), with its property tests.

use super::head::{Head, HeadError, is_field_text, is_token_character, parse};
use super::*;
use crate::consts::{HTTP_MAJOR, HTTP_MINOR_1, HTTP_NAME, MAX_RESPONSE_HEADERS};
use proptest::prelude::*;
use std::vec::Vec;

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
    assert_eq!(collector.feed(&mut buffer, b"HTTP/1.1 200 OK\r\nX: yyyyyyyy\r\n\r\n"), Err(HttpError::HeadTooLarge));
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
        (b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n", HttpError::AmbiguousFraming),
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 5\r\n\r\n", HttpError::AmbiguousFraming),
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
