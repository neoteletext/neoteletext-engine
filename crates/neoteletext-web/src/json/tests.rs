#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The tests of the bounded JSON pull reader (RFC 8259).

use super::*;

fn tokens(input: &[u8]) -> Result<usize, JsonError> {
    let mut reader = JsonReader::new(input)?;
    let mut count = 0;
    while reader.next_token()?.is_some() {
        count += 1;
    }
    Ok(count)
}

#[test]
fn valid_documents() {
    assert_eq!(tokens(b"[]").unwrap(), 2);
    assert_eq!(tokens(b" {\"a\" : [1, -2.5e+3, true, false, null, \"x\"]} ").unwrap(), 11);
    assert_eq!(tokens(b"\"\\u00e9\\ud83d\\ude00\"").unwrap(), 1);
    assert_eq!(tokens(b"0").unwrap(), 1);
}

#[test]
fn invalid_documents() {
    let cases: [(&[u8], JsonErrorKind); 13] = [
        (b"[1,]", JsonErrorKind::Unexpected),
        (b"{\"a\" 1}", JsonErrorKind::Unexpected),
        (b"[01]", JsonErrorKind::BadNumber),
        (b"[1.]", JsonErrorKind::BadNumber),
        (b"[-]", JsonErrorKind::BadNumber),
        (b"[\"a\nb\"]", JsonErrorKind::BadString),
        (b"[\"\\x\"]", JsonErrorKind::BadString),
        (b"[\"\\ud800\"]", JsonErrorKind::BadString),
        (b"[\"\\udc00\"]", JsonErrorKind::BadString),
        (b"[1] 2", JsonErrorKind::TrailingData),
        (b"[tru]", JsonErrorKind::Unexpected),
        (b"{\"a\":1", JsonErrorKind::Truncated),
        (b"{\"a\":1]", JsonErrorKind::Unexpected),
    ];
    for (input, kind) in cases {
        assert_eq!(tokens(input).unwrap_err().kind, kind, "{:?}", core::str::from_utf8(input));
    }
    assert_eq!(tokens(b"[\xff]").unwrap_err().kind, JsonErrorKind::Encoding);
}

#[test]
fn nesting_is_limited() {
    let mut deep = [b'['; 40];
    deep[39] = b']';
    assert_eq!(tokens(&deep).unwrap_err().kind, JsonErrorKind::TooDeep);
}

#[test]
fn strings_decode() {
    let mut reader = JsonReader::new(b"[\"a\\\"b\\u00e9\\ud83d\\ude00\"]").unwrap();
    reader.next_token().unwrap();
    let Some(Token::Str(text)) = reader.next_token().unwrap() else { panic!() };
    let mut out = [0u8; 16];
    let n = text.decode_into(&mut out).unwrap();
    assert_eq!(core::str::from_utf8(&out[..n]).unwrap(), "a\"b\u{e9}\u{1F600}");
    assert!(text.equals("a\"b\u{e9}\u{1F600}".as_bytes()));
}

#[test]
fn skipping_values() {
    let mut reader = JsonReader::new(b"{\"x\":{\"y\":[1,{\"z\":2}]},\"w\":3}").unwrap();
    assert_eq!(reader.next_token().unwrap(), Some(Token::BeginObject));
    assert!(matches!(reader.next_token().unwrap(), Some(Token::Key(_))));
    let first = reader.next_token().unwrap().unwrap();
    reader.skip(first).unwrap();
    let Some(Token::Key(key)) = reader.next_token().unwrap() else { panic!() };
    assert!(key.equals(b"w"));
    assert_eq!(reader.next_token().unwrap(), Some(Token::Number(b"3")));
}

#[test]
fn integers() {
    assert_eq!(integer(b"-12"), Some(-12));
    assert_eq!(integer(b"1.5"), None);
    assert_eq!(integer(b"1e3"), None);
}
