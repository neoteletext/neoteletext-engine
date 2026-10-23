//! The application messages inside sealed frames (`protocol.ts`
//! `ApplicationMessage`): the byte 0x10, then a JSON object. Each carries
//! an 11-character base64url id, which the screen's acknowledgement
//! echoes.

use crate::crypto::SEALED_OVERHEAD;
use crate::json::Value;
use crate::{Error, MAX_FRAME_BYTES};

/// The first plaintext byte of an application message (`MESSAGE_TAG`).
pub const MESSAGE_TAG: u8 = 0x10;
/// The largest plaintext that fits one relay frame once sealed
/// (`MAX_PLAINTEXT_BYTES`).
pub const MAX_PLAINTEXT_BYTES: usize = MAX_FRAME_BYTES - SEALED_OVERHEAD;
/// A message id's length: 64 bits in unpadded base64url
/// (`MESSAGE_ID_LENGTH`).
pub const MESSAGE_ID_LENGTH: usize = 11;
/// The longest text the phone's box sends at once, in code points
/// (`MAX_ENTRY_CODE_POINTS`).
pub const MAX_ENTRY_CODE_POINTS: usize = 10_000;

/// The live-mode key codes a text box sends for its special keys
/// (`SPECIAL_KEYS`), with their W3C names.
pub const SPECIAL_KEYS: [(u64, &str); 8] = [
    (8, "Backspace"),
    (9, "Tab"),
    (10, "Enter"),
    (27, "Escape"),
    (37, "ArrowLeft"),
    (38, "ArrowUp"),
    (39, "ArrowRight"),
    (40, "ArrowDown"),
];

/// A message from the phone, checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FromPhone {
    /// Text typed in live mode.
    Text(String),
    /// A special key of the text box (W3C name), `count` times: a bulk
    /// Backspace repeats.
    Special {
        /// Its W3C `KeyboardEvent.key` name.
        key: &'static str,
        /// How many times, at least 1.
        count: usize,
    },
    /// A key of the layout went down.
    Down(String),
    /// A key of the layout came up.
    Up(String),
    /// The phone asks to end the session.
    Finish,
    /// Something this screen does not take: an action, a context question,
    /// a submitted text; refused in its acknowledgement.
    Other,
}

/// One message as read: its id and what it says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The id to acknowledge.
    pub id: String,
    /// What it says.
    pub body: FromPhone,
}

/// Whether `id` is a message id (`MESSAGE_ID_PATTERN`).
#[must_use]
pub fn is_id(id: &str) -> bool {
    id.len() == MESSAGE_ID_LENGTH && id.bytes().all(is_id_byte)
}

/// Whether `byte` is in a message id's alphabet, base64url's
/// (`MESSAGE_ID_PATTERN`, RFC 4648 section 5).
#[must_use]
pub const fn is_id_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// Reads the plaintext of a sealed frame from the phone; `None` for
/// anything malformed, which is dropped.
#[must_use]
pub fn read(plaintext: &[u8]) -> Option<Message> {
    let (&tag, json) = plaintext.split_first()?;
    if tag != MESSAGE_TAG || plaintext.len() > MAX_PLAINTEXT_BYTES {
        return None;
    }
    let value = Value::parse(json).ok()?;
    let id = value.get("id")?.as_str()?.to_owned();
    if !is_id(&id) {
        return None;
    }
    let text = value.get("text").and_then(Value::as_str);
    let key = value.get("key").and_then(Value::as_str);
    let body = match value.get("type")?.as_str()? {
        "key" => match (text, value.get("code").and_then(Value::as_u64)) {
            (Some(text), None) => FromPhone::Text(text.to_owned()),
            (None, Some(code)) => {
                let (_, key) = SPECIAL_KEYS.iter().find(|(special, _)| *special == code)?;
                let count = value.get("count").and_then(Value::as_u64).unwrap_or(1);
                let count = usize::try_from(count).unwrap_or(MAX_ENTRY_CODE_POINTS).clamp(1, MAX_ENTRY_CODE_POINTS);
                FromPhone::Special { key, count }
            }
            _ => return None,
        },
        "down" => FromPhone::Down(key?.to_owned()),
        "up" => FromPhone::Up(key?.to_owned()),
        "finish" => FromPhone::Finish,
        _ => FromPhone::Other,
    };
    Some(Message { id, body })
}

/// A message's plaintext: the tag, then its JSON.
///
/// # Errors
/// When it would not fit one relay frame once sealed.
pub fn pack(message: &Value) -> Result<Vec<u8>, Error> {
    let json = message.text();
    let mut plaintext = Vec::with_capacity(json.len().saturating_add(1));
    plaintext.push(MESSAGE_TAG);
    plaintext.extend_from_slice(json.as_bytes());
    if plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(Error::Limit("a message longer than a relay frame"));
    }
    Ok(plaintext)
}

/// The screen's acknowledgement of message `id`.
#[must_use]
pub fn ack(id: &str, ok: bool) -> Value {
    Value::object(&[("type", Value::string("ack")), ("id", Value::string(id)), ("ok", Value::Bool(ok))])
}

/// A fresh message id: 64 random bits in unpadded base64url
/// (`messageId`).
///
/// # Errors
/// When the operating system has no random numbers.
pub fn new_id() -> Result<String, Error> {
    let mut bytes = [0_u8; 8];
    crate::random(&mut bytes)?;
    Ok(base64url(&bytes))
}

/// The base64url alphabet (RFC 4648 section 5, Table 2).
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
/// Six bits a symbol.
const SIX_BITS: u8 = 0x3F;

/// Unpadded base64url (RFC 4648 section 5).
#[must_use]
pub fn base64url(bytes: &[u8]) -> String {
    let symbol = |value: u8| char::from(ALPHABET.get(usize::from(value & SIX_BITS)).copied().unwrap_or(b'A'));
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let byte = |index: usize| chunk.get(index).copied().unwrap_or(0);
        let (a, b, c) = (byte(0), byte(1), byte(2));
        let symbols = [a >> 2, ((a & 0x03) << 4) | (b >> 4), ((b & 0x0F) << 2) | (c >> 6), c];
        // One byte gives two symbols, two give three, three give four.
        for &value in symbols.iter().take(chunk.len().saturating_add(1)) {
            out.push(symbol(value));
        }
    }
    out
}

/// Reads unpadded base64url; `None` for any other character.
#[must_use]
pub fn from_base64url(text: &str) -> Option<Vec<u8>> {
    let value = |byte: u8| -> Option<u8> {
        ALPHABET.iter().position(|&symbol| symbol == byte).and_then(|place| u8::try_from(place).ok())
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        if chunk.len() == 1 {
            return None;
        }
        let mut values = [0_u8; 4];
        for (target, &byte) in values.iter_mut().zip(chunk) {
            *target = value(byte)?;
        }
        let [a, b, c, d] = values;
        let bytes = [(a << 2) | (b >> 4), (b << 4) | (c >> 2), (c << 6) | d];
        out.extend_from_slice(bytes.get(..chunk.len().saturating_sub(1)).unwrap_or(&[]));
    }
    Some(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn plaintext(json: &str) -> Vec<u8> {
        [&[MESSAGE_TAG], json.as_bytes()].concat()
    }

    #[test]
    fn the_phones_messages_are_read() {
        let id = "AAAAAAAAAAA";
        assert_eq!(
            read(&plaintext(r#"{"type":"down","id":"AAAAAAAAAAA","key":"ColorF0Red"}"#)),
            Some(Message { id: id.into(), body: FromPhone::Down("ColorF0Red".into()) })
        );
        assert_eq!(
            read(&plaintext(r#"{"type":"key","id":"AAAAAAAAAAA","text":"hé"}"#)).unwrap().body,
            FromPhone::Text("hé".into())
        );
        assert_eq!(
            read(&plaintext(r#"{"type":"key","id":"AAAAAAAAAAA","code":8,"count":3}"#)).unwrap().body,
            FromPhone::Special { key: "Backspace", count: 3 }
        );
        assert_eq!(
            read(&plaintext(r#"{"type":"action","id":"AAAAAAAAAAA","action":"x"}"#)).unwrap().body,
            FromPhone::Other
        );
        assert_eq!(read(&plaintext(r#"{"type":"finish","id":"AAAAAAAAAAA"}"#)).unwrap().body, FromPhone::Finish);
    }

    #[test]
    fn malformed_messages_are_dropped() {
        for json in [
            r#"{"type":"down","id":"short","key":"a"}"#,
            r#"{"type":"down","id":"AAAAAAAAAAA"}"#,
            r#"{"type":"key","id":"AAAAAAAAAAA","code":99}"#,
            r#"{"type":"key","id":"AAAAAAAAAAA","code":8,"text":"x"}"#,
            r#"["not an object"]"#,
        ] {
            assert_eq!(read(&plaintext(json)), None, "{json}");
        }
        assert_eq!(read(b"{}"), None, "no tag");
        let long = format!(r#"{{"type":"key","id":"AAAAAAAAAAA","text":"{}"}}"#, "x".repeat(MAX_PLAINTEXT_BYTES));
        assert_eq!(read(&plaintext(&long)), None);
    }

    #[test]
    fn base64url_round_trips() {
        for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob", &[0xFB, 0xFF, 0xBF]] {
            let text = base64url(bytes);
            assert!(!text.contains('='));
            assert_eq!(from_base64url(&text).unwrap(), bytes);
        }
        assert_eq!(base64url(&[0xFB, 0xFF]), "-_8");
        assert!(is_id(&new_id().unwrap()));
        assert_eq!(from_base64url("a+b"), None);
    }
}
