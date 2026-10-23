//! Structured field values (RFC 9651), as much as the two session headers
//! use: a cursor over a header value that reads strings, tokens, keys and
//! parameters, and steps over the rest.

use super::MAX_SESSION_URL;
use crate::bytes::TextBuf;

/// A cursor over a header value.
pub(super) struct Fields<'a> {
    input: &'a [u8],
    at: usize,
}

impl<'a> Fields<'a> {
    pub(super) const fn new(input: &'a [u8]) -> Self {
        Self { input, at: 0 }
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.input.get(self.at).copied()
    }

    pub(super) fn bump(&mut self) {
        self.at = self.at.saturating_add(1);
    }

    pub(super) fn spaces(&mut self) {
        while self.peek() == Some(b' ') {
            self.bump();
        }
    }

    pub(super) fn optional_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.bump();
        }
    }

    pub(super) fn at_end(&self) -> bool {
        self.at >= self.input.len()
    }

    /// An sf-string (RFC 9651 section 4.2.5), decoded into `out`.
    pub(super) fn string<const N: usize>(&mut self, out: &mut TextBuf<N>) -> Option<()> {
        if self.peek() != Some(b'"') {
            return None;
        }
        self.bump();
        out.clear();
        loop {
            let byte = self.peek()?;
            self.bump();
            match byte {
                b'"' => return Some(()),
                b'\\' => {
                    let escaped = self.peek().filter(|&next| next == b'"' || next == b'\\')?;
                    self.bump();
                    out.push(&[escaped]).ok()?;
                }
                0x20..=0x7E => out.push(&[byte]).ok()?,
                _ => return None,
            }
        }
    }

    /// An sf-token (section 4.2.6), as the bytes it is.
    pub(super) fn token(&mut self) -> Option<&'a [u8]> {
        let start = self.at;
        if !self.peek().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'*') {
            return None;
        }
        while self.peek().is_some_and(|byte| is_tchar(byte) || byte == b':' || byte == b'/') {
            self.bump();
        }
        self.input.get(start..self.at)
    }

    /// A key (section 4.2.3.3).
    fn key(&mut self) -> Option<&'a [u8]> {
        let start = self.at;
        if !self.peek().is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'*') {
            return None;
        }
        while self.peek().is_some_and(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.' | b'*')
        }) {
            self.bump();
        }
        self.input.get(start..self.at)
    }

    /// Steps over a bare item of any kind but a string, which the caller
    /// reads: a token, a number, a boolean or a byte sequence.
    fn skip_other_item(&mut self) -> Option<()> {
        match self.peek()? {
            b'?' => {
                self.bump();
                self.peek().filter(|&byte| byte == b'0' || byte == b'1')?;
                self.bump();
            }
            b':' => {
                self.bump();
                while self.peek()? != b':' {
                    self.bump();
                }
                self.bump();
            }
            byte if byte == b'-' || byte.is_ascii_digit() => {
                self.bump();
                while self.peek().is_some_and(|byte| byte.is_ascii_digit() || byte == b'.') {
                    self.bump();
                }
            }
            _ => {
                self.token()?;
            }
        }
        Some(())
    }

    /// Parameters (section 4.2.3.2), each given to `each` with its value
    /// when that is a string: `each(key, Some(fields))` lets the caller read
    /// the string; anything else is stepped over.
    pub(super) fn parameters(&mut self, mut each: impl FnMut(&[u8], &mut Self) -> Option<bool>) -> Option<()> {
        while self.peek() == Some(b';') {
            self.bump();
            self.spaces();
            let key = self.key()?;
            if self.peek() != Some(b'=') {
                continue;
            }
            self.bump();
            if self.peek() == Some(b'"') {
                // The caller takes the string, or it is stepped over.
                let mut skipped = TextBuf::<MAX_SESSION_URL>::new();
                if !each(key, self)? {
                    self.string(&mut skipped)?;
                }
            } else {
                self.skip_other_item()?;
            }
        }
        Some(())
    }
}

/// RFC 9110 section 5.6.2's token characters.
const fn is_tchar(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
        )
}
