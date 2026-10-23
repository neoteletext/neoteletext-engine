//! A small, bounded JSON pull reader (RFC 8259), used for the service
//! descriptor (REQ-WEB-002) and the zxnet feed's messages (REQ-INP-022).
//! Written in the project rather than taken from serde (decision E.38).
//!
//! The reader validates the whole grammar as it goes: UTF-8 (section 8.1),
//! string escapes and surrogate pairs (section 7), the number grammar
//! (section 6) and nesting, which is limited to `MAX_JSON_DEPTH` levels
//! (section 9 permits a limit). Work is linear in the input length.

use crate::bytes::{Overflow, copy_prefix};
use crate::consts::MAX_JSON_DEPTH;

/// Why a document was rejected, with the byte offset where it happened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JsonError {
    /// Byte offset of the fault.
    pub offset: usize,
    /// What was wrong.
    pub kind: JsonErrorKind,
}

/// The kinds of JSON fault.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JsonErrorKind {
    /// Not valid UTF-8.
    Encoding,
    /// A byte that cannot start or continue the expected token.
    Unexpected,
    /// The input ended inside a value.
    Truncated,
    /// A bad escape, control character or unpaired surrogate in a string.
    BadString,
    /// A number outside the RFC 8259 grammar.
    BadNumber,
    /// Nesting deeper than the limit.
    TooDeep,
    /// Content after the top-level value.
    TrailingData,
}

/// A string as it appears in the document, escapes still encoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JsonStr<'a> {
    raw: &'a [u8],
    escaped: bool,
}

impl<'a> JsonStr<'a> {
    /// The raw text between the quotes when it has no escapes.
    #[must_use]
    pub const fn plain(&self) -> Option<&'a [u8]> {
        if self.escaped { None } else { Some(self.raw) }
    }

    /// The length of the raw text: an upper bound on the decoded length.
    #[must_use]
    pub const fn raw_len(&self) -> usize {
        self.raw.len()
    }

    /// Decodes the string as UTF-8 into `output`, returning its length.
    /// The escapes were validated when the string was read.
    pub fn decode_into(&self, output: &mut [u8]) -> Result<usize, Overflow> {
        let mut written = 0usize;
        let mut index = 0usize;
        let mut put = |bytes: &[u8], written: &mut usize| -> Result<(), Overflow> {
            let end = written.checked_add(bytes.len()).ok_or(Overflow)?;
            copy_prefix(output.get_mut(*written..end).ok_or(Overflow)?, bytes);
            *written = end;
            Ok(())
        };
        while let Some(&byte) = self.raw.get(index) {
            if byte != b'\\' {
                put(&[byte], &mut written)?;
                index = index.saturating_add(1);
                continue;
            }
            let escape = self.raw.get(index.saturating_add(1)).copied().unwrap_or(0);
            index = index.saturating_add(2);
            let simple = match escape {
                b'"' => Some(b'"'),
                b'\\' => Some(b'\\'),
                b'/' => Some(b'/'),
                b'b' => Some(0x08),
                b'f' => Some(0x0C),
                b'n' => Some(b'\n'),
                b'r' => Some(b'\r'),
                b't' => Some(b'\t'),
                _ => None,
            };
            if let Some(value) = simple {
                put(&[value], &mut written)?;
                continue;
            }
            let high = hex4(self.raw.get(index..index.saturating_add(4)).unwrap_or(&[])).unwrap_or(0);
            index = index.saturating_add(4);
            let code = if is_high_surrogate(high) {
                let low =
                    hex4(self.raw.get(index.saturating_add(2)..index.saturating_add(6)).unwrap_or(&[])).unwrap_or(0);
                index = index.saturating_add(6);
                combine_surrogates(high, low)
            } else {
                u32::from(high)
            };
            let character = char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER);
            let mut utf8 = [0u8; 4];
            put(character.encode_utf8(&mut utf8).as_bytes(), &mut written)?;
        }
        Ok(written)
    }

    /// True when the decoded string equals `text`.
    #[must_use]
    pub fn equals(&self, text: &[u8]) -> bool {
        if let Some(raw) = self.plain() {
            return raw == text;
        }
        let mut buffer = [0u8; 64];
        self.decode_into(&mut buffer).is_ok_and(|len| buffer.get(..len).is_some_and(|decoded| decoded == text))
    }
}

/// One token of the document.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Token<'a> {
    /// `{`
    BeginObject,
    /// `}`
    EndObject,
    /// `[`
    BeginArray,
    /// `]`
    EndArray,
    /// An object member name; its value follows.
    Key(JsonStr<'a>),
    /// A string value.
    Str(JsonStr<'a>),
    /// A number, as its validated text.
    Number(&'a [u8]),
    /// `true` or `false`.
    Bool(bool),
    /// `null`
    Null,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// A value is required (document start, after `:` or after `,` in an
    /// array).
    Value,
    /// After `[`: a value or `]`.
    ArrayFirst,
    /// After `{`: a member name or `}`.
    ObjectFirst,
    /// After `,` in an object: a member name.
    ObjectKey,
    /// After a value: `,`, a closing bracket or the end.
    AfterValue,
    /// The top-level value is complete.
    Done,
}

/// The pull reader.
#[derive(Debug)]
pub struct JsonReader<'a> {
    input: &'a [u8],
    position: usize,
    /// True for an object, false for an array, at each open level.
    stack: [bool; MAX_JSON_DEPTH],
    depth: usize,
    state: State,
}

impl<'a> JsonReader<'a> {
    /// A reader over `input`, which must be valid UTF-8 (RFC 8259 section
    /// 8.1).
    pub fn new(input: &'a [u8]) -> Result<Self, JsonError> {
        core::str::from_utf8(input)
            .map_err(|error| JsonError { offset: error.valid_up_to(), kind: JsonErrorKind::Encoding })?;
        Ok(Self { input, position: 0, stack: [false; MAX_JSON_DEPTH], depth: 0, state: State::Value })
    }

    /// The current nesting depth.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    fn error(&self, kind: JsonErrorKind) -> JsonError {
        JsonError { offset: self.position, kind }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn advance(&mut self, count: usize) {
        self.position = self.position.saturating_add(count);
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.advance(1);
        }
    }

    fn top_is_object(&self) -> bool {
        self.depth.checked_sub(1).and_then(|top| self.stack.get(top)).copied().unwrap_or(false)
    }

    fn push(&mut self, object: bool) -> Result<(), JsonError> {
        let slot =
            self.stack.get_mut(self.depth).ok_or(JsonError { offset: self.position, kind: JsonErrorKind::TooDeep })?;
        *slot = object;
        self.depth = self.depth.saturating_add(1);
        Ok(())
    }

    fn pop(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        self.state = if self.depth == 0 { State::Done } else { State::AfterValue };
    }

    fn value_done(&mut self) {
        self.state = if self.depth == 0 { State::Done } else { State::AfterValue };
    }

    /// The next token, `None` at the end of a complete document.
    pub fn next_token(&mut self) -> Result<Option<Token<'a>>, JsonError> {
        loop {
            self.skip_whitespace();
            match self.state {
                State::Done => {
                    return if self.peek().is_some() { Err(self.error(JsonErrorKind::TrailingData)) } else { Ok(None) };
                }
                State::Value => return self.value().map(Some),
                State::ArrayFirst => {
                    if self.peek() == Some(b']') {
                        self.advance(1);
                        self.pop();
                        return Ok(Some(Token::EndArray));
                    }
                    return self.value().map(Some);
                }
                State::ObjectFirst | State::ObjectKey => {
                    if self.state == State::ObjectFirst && self.peek() == Some(b'}') {
                        self.advance(1);
                        self.pop();
                        return Ok(Some(Token::EndObject));
                    }
                    if self.peek() != Some(b'"') {
                        return Err(self.unexpected());
                    }
                    let key = self.string()?;
                    self.skip_whitespace();
                    if self.peek() != Some(b':') {
                        return Err(self.unexpected());
                    }
                    self.advance(1);
                    self.state = State::Value;
                    return Ok(Some(Token::Key(key)));
                }
                State::AfterValue => match self.peek() {
                    Some(b',') => {
                        self.advance(1);
                        self.state = if self.top_is_object() { State::ObjectKey } else { State::Value };
                    }
                    Some(b']') if !self.top_is_object() => {
                        self.advance(1);
                        self.pop();
                        return Ok(Some(Token::EndArray));
                    }
                    Some(b'}') if self.top_is_object() => {
                        self.advance(1);
                        self.pop();
                        return Ok(Some(Token::EndObject));
                    }
                    _ => return Err(self.unexpected()),
                },
            }
        }
    }

    fn unexpected(&self) -> JsonError {
        self.error(if self.peek().is_none() { JsonErrorKind::Truncated } else { JsonErrorKind::Unexpected })
    }

    fn value(&mut self) -> Result<Token<'a>, JsonError> {
        match self.peek() {
            Some(b'{') => {
                self.push(true)?;
                self.advance(1);
                self.state = State::ObjectFirst;
                Ok(Token::BeginObject)
            }
            Some(b'[') => {
                self.push(false)?;
                self.advance(1);
                self.state = State::ArrayFirst;
                Ok(Token::BeginArray)
            }
            Some(b'"') => {
                let text = self.string()?;
                self.value_done();
                Ok(Token::Str(text))
            }
            Some(b'-' | b'0'..=b'9') => {
                let number = self.number()?;
                self.value_done();
                Ok(Token::Number(number))
            }
            Some(b't') => self.literal(b"true", Token::Bool(true)),
            Some(b'f') => self.literal(b"false", Token::Bool(false)),
            Some(b'n') => self.literal(b"null", Token::Null),
            _ => Err(self.unexpected()),
        }
    }

    fn literal(&mut self, text: &[u8], token: Token<'a>) -> Result<Token<'a>, JsonError> {
        let end = self.position.saturating_add(text.len());
        match self.input.get(self.position..end) {
            Some(found) if found == text => {
                self.position = end;
                self.value_done();
                Ok(token)
            }
            Some(_) => Err(self.error(JsonErrorKind::Unexpected)),
            None => Err(self.error(JsonErrorKind::Truncated)),
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.position;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.advance(1);
        }
        self.position.saturating_sub(start)
    }

    /// RFC 8259 section 6: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`.
    fn number(&mut self) -> Result<&'a [u8], JsonError> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.advance(1);
        }
        match self.peek() {
            Some(b'0') => {
                self.advance(1);
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error(JsonErrorKind::BadNumber));
                }
            }
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return Err(self.error(JsonErrorKind::BadNumber)),
        }
        if self.peek() == Some(b'.') {
            self.advance(1);
            if self.digits() == 0 {
                return Err(self.error(JsonErrorKind::BadNumber));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.advance(1);
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.advance(1);
            }
            if self.digits() == 0 {
                return Err(self.error(JsonErrorKind::BadNumber));
            }
        }
        Ok(self.input.get(start..self.position).unwrap_or(&[]))
    }

    /// RFC 8259 section 7. The opening quote is at the current position.
    fn string(&mut self) -> Result<JsonStr<'a>, JsonError> {
        self.advance(1);
        let start = self.position;
        let mut escaped = false;
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error(JsonErrorKind::Truncated));
            };
            match byte {
                b'"' => {
                    let raw = self.input.get(start..self.position).unwrap_or(&[]);
                    self.advance(1);
                    return Ok(JsonStr { raw, escaped });
                }
                b'\\' => {
                    escaped = true;
                    self.escape()?;
                }
                0x00..=0x1F => return Err(self.error(JsonErrorKind::BadString)),
                _ => self.advance(1),
            }
        }
    }

    fn escape(&mut self) -> Result<(), JsonError> {
        let kind = self.input.get(self.position.saturating_add(1)).copied();
        match kind {
            Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                self.advance(2);
                Ok(())
            }
            Some(b'u') => {
                let unit = self.unicode_escape()?;
                if is_high_surrogate(unit) {
                    if self.input.get(self.position..self.position.saturating_add(2)) != Some(b"\\u") {
                        return Err(self.error(JsonErrorKind::BadString));
                    }
                    let low = self.unicode_escape()?;
                    if !is_low_surrogate(low) {
                        return Err(self.error(JsonErrorKind::BadString));
                    }
                } else if is_low_surrogate(unit) {
                    return Err(self.error(JsonErrorKind::BadString));
                }
                Ok(())
            }
            None => Err(self.error(JsonErrorKind::Truncated)),
            Some(_) => Err(self.error(JsonErrorKind::BadString)),
        }
    }

    /// Reads `\uXXXX` at the current position.
    fn unicode_escape(&mut self) -> Result<u16, JsonError> {
        let digits_start = self.position.saturating_add(2);
        let digits = self.input.get(digits_start..digits_start.saturating_add(4));
        let unit = digits.and_then(hex4).ok_or_else(|| self.error(JsonErrorKind::BadString))?;
        self.advance(6);
        Ok(unit)
    }

    /// Skips the value whose first token was just returned (`first`), so a
    /// member the caller does not use costs no more than reading it.
    pub fn skip(&mut self, first: Token<'a>) -> Result<(), JsonError> {
        if !matches!(first, Token::BeginObject | Token::BeginArray) {
            return Ok(());
        }
        let target = self.depth.saturating_sub(1);
        while self.depth > target {
            if self.next_token()?.is_none() {
                return Err(self.error(JsonErrorKind::Truncated));
            }
        }
        Ok(())
    }
}

fn hex4(digits: &[u8]) -> Option<u16> {
    if digits.len() != 4 {
        return None;
    }
    digits.iter().try_fold(0u16, |value, &d| Some((value << 4) | u16::from(crate::bytes::hex_value(d)?)))
}

/// UTF-16 surrogate ranges (RFC 8259 section 7, RFC 2781 section 2).
const HIGH_SURROGATES: core::ops::RangeInclusive<u16> = 0xD800..=0xDBFF;
const LOW_SURROGATES: core::ops::RangeInclusive<u16> = 0xDC00..=0xDFFF;
/// The first code point a surrogate pair encodes.
const SUPPLEMENTARY_BASE: u32 = 0x1_0000;
/// Bits carried by each half of a surrogate pair.
const SURROGATE_BITS: u32 = 10;

fn is_high_surrogate(unit: u16) -> bool {
    HIGH_SURROGATES.contains(&unit)
}

fn is_low_surrogate(unit: u16) -> bool {
    LOW_SURROGATES.contains(&unit)
}

fn combine_surrogates(high: u16, low: u16) -> u32 {
    let high_bits = u32::from(high.wrapping_sub(*HIGH_SURROGATES.start()));
    let low_bits = u32::from(low.wrapping_sub(*LOW_SURROGATES.start()));
    SUPPLEMENTARY_BASE.wrapping_add((high_bits << SURROGATE_BITS) | low_bits)
}

/// Parses an integer number token: digits with an optional minus sign and
/// no fraction or exponent.
#[must_use]
pub fn integer(number: &[u8]) -> Option<i64> {
    let (negative, digits) = match number.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, number),
    };
    // i64 holds every 18-digit number.
    let magnitude = i64::try_from(crate::bytes::parse_decimal(digits, 18)?).ok()?;
    Some(if negative { magnitude.checked_neg()? } else { magnitude })
}

#[cfg(test)]
mod tests;
