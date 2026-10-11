//! Small byte-string helpers shared by the protocol modules: a bounded
//! writer, a fixed-capacity owned string and ASCII parsing. No `core::fmt`
//! (REQ-GEN-016): numbers are written digit by digit.

/// Decimal digits of the largest `u64`, 18446744073709551615.
pub const MAX_U64_DIGITS: usize = 20;

/// The output buffer was too small.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Overflow;

/// Appends to a caller-supplied buffer, refusing to write past its end.
#[derive(Debug)]
pub struct Writer<'a> {
    buffer: &'a mut [u8],
    len: usize,
    /// Bytes at the end of the buffer kept back for later writes.
    reserve: usize,
}

impl<'a> Writer<'a> {
    /// A writer at the start of `buffer`.
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, len: 0, reserve: 0 }
    }

    /// Keeps `bytes` at the end of the buffer out of reach until the
    /// reserve is set back to 0, so that what must follow still fits.
    pub fn set_reserve(&mut self, bytes: usize) {
        self.reserve = bytes;
    }

    fn capacity(&self) -> usize {
        self.buffer.len().saturating_sub(self.reserve)
    }

    /// Bytes written so far.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True when nothing has been written.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The bytes written.
    #[must_use]
    pub fn written(&self) -> &[u8] {
        self.buffer.get(..self.len).unwrap_or(&[])
    }

    /// Room left.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.capacity().saturating_sub(self.len)
    }

    /// Discards everything written after `len`.
    pub fn truncate(&mut self, len: usize) {
        self.len = self.len.min(len);
    }

    /// Appends bytes, or changes nothing when they do not fit.
    pub fn put(&mut self, bytes: &[u8]) -> Result<(), Overflow> {
        let end = self.len.checked_add(bytes.len()).ok_or(Overflow)?;
        if end > self.capacity() {
            return Err(Overflow);
        }
        copy_prefix(self.buffer.get_mut(self.len..end).ok_or(Overflow)?, bytes);
        self.len = end;
        Ok(())
    }

    /// Appends one byte.
    pub fn byte(&mut self, byte: u8) -> Result<(), Overflow> {
        self.put(&[byte])
    }

    /// Appends a number in decimal.
    pub fn decimal(&mut self, value: u64) -> Result<(), Overflow> {
        let mut digits = [0u8; MAX_U64_DIGITS];
        let text = decimal_digits(value, &mut digits);
        self.put(text)
    }

    /// Appends `value` as `width` uppercase hexadecimal digits.
    pub fn hex(&mut self, value: u32, width: u32) -> Result<(), Overflow> {
        let mut shift = width.saturating_mul(4);
        while shift > 0 {
            shift = shift.saturating_sub(4);
            let nibble = ((value >> shift) & 0xF) as u8;
            self.byte(neoteletext_core::types::hex_digit(nibble))?;
        }
        Ok(())
    }
}

/// Copies as many bytes as both slices hold from the start of `source` to
/// the start of `target`, and returns how many.
///
/// REQ-GEN-019: a loop over the pair, not `copy_from_slice`. Where the
/// optimiser cannot see that two slices cut to the same length are equal,
/// `copy_from_slice` links its length-mismatch panic into the image.
pub(crate) fn copy_prefix(target: &mut [u8], source: &[u8]) -> usize {
    for (to, from) in target.iter_mut().zip(source) {
        *to = *from;
    }
    target.len().min(source.len())
}

/// Writes `value` in decimal into `digits` and returns the used part.
pub fn decimal_digits(value: u64, digits: &mut [u8; MAX_U64_DIGITS]) -> &[u8] {
    let mut remaining = value;
    let mut start = digits.len();
    loop {
        start = start.saturating_sub(1);
        if let Some(slot) = digits.get_mut(start) {
            *slot = b'0'.wrapping_add((remaining % 10) as u8);
        }
        remaining /= 10;
        if remaining == 0 || start == 0 {
            break;
        }
    }
    digits.get(start..).unwrap_or(&[])
}

/// An owned byte string of at most `N` bytes, held inline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextBuf<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Default for TextBuf<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> TextBuf<N> {
    /// An empty string.
    #[must_use]
    pub const fn new() -> Self {
        Self { bytes: [0; N], len: 0 }
    }

    /// A copy of `bytes`, or `None` when they do not fit.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut text = Self::new();
        text.set(bytes).ok()?;
        Some(text)
    }

    /// Replaces the contents, or changes nothing when `bytes` do not fit.
    pub fn set(&mut self, bytes: &[u8]) -> Result<(), Overflow> {
        copy_prefix(self.bytes.get_mut(..bytes.len()).ok_or(Overflow)?, bytes);
        self.len = bytes.len();
        Ok(())
    }

    /// Appends, or changes nothing when `bytes` do not fit.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), Overflow> {
        let end = self.len.checked_add(bytes.len()).ok_or(Overflow)?;
        copy_prefix(self.bytes.get_mut(self.len..end).ok_or(Overflow)?, bytes);
        self.len = end;
        Ok(())
    }

    /// Empties the string.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// The bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// The text, or an empty string if the bytes are not UTF-8 (they always
    /// are when written from `&str` or validated input).
    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or("")
    }

    /// Length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True when empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Removes optional white space (space and horizontal tab) from both ends
/// (RFC 9110 section 5.6.3).
#[must_use]
pub fn trim_ows(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| b != b' ' && b != b'\t').unwrap_or(bytes.len());
    let end = bytes.iter().rposition(|&b| b != b' ' && b != b'\t').map_or(start, |last| last.saturating_add(1));
    bytes.get(start..end.max(start)).unwrap_or(&[])
}

/// Parses an unsigned decimal number of at most `max_digits` digits, with
/// no sign, space or other character.
#[must_use]
pub fn parse_decimal(bytes: &[u8], max_digits: usize) -> Option<u64> {
    if bytes.is_empty() || bytes.len() > max_digits {
        return None;
    }
    let mut value: u64 = 0;
    for &byte in bytes {
        let digit = byte.checked_sub(b'0').filter(|d| *d <= 9)?;
        value = value.checked_mul(10)?.checked_add(u64::from(digit))?;
    }
    Some(value)
}

/// A number written in hexadecimal digits, at most four of them; `None`
/// for anything else. The standard library's `from_str_radix` carries a
/// panic path for its radix argument, which REQ-GEN-019 keeps out of the
/// linked image.
#[must_use]
pub fn hex_number(text: &[u8]) -> Option<u16> {
    if text.is_empty() || text.len() > 4 {
        return None;
    }
    text.iter().try_fold(0u16, |value, &digit| Some((value << 4) | u16::from(hex_value(digit)?)))
}

/// A number written in decimal digits, at most four of them; `None` for
/// anything else.
#[must_use]
pub fn decimal_number(text: &[u8]) -> Option<u16> {
    if text.is_empty() || text.len() > 4 {
        return None;
    }
    text.iter().try_fold(0u16, |value, &digit| {
        let digit = digit.checked_sub(b'0').filter(|digit| *digit <= 9)?;
        value.checked_mul(10)?.checked_add(u16::from(digit))
    })
}

/// The value of an ASCII hexadecimal digit.
#[must_use]
pub const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte.wrapping_sub(b'0')),
        b'a'..=b'f' => Some(byte.wrapping_sub(b'a').wrapping_add(10)),
        b'A'..=b'F' => Some(byte.wrapping_sub(b'A').wrapping_add(10)),
        _ => None,
    }
}

/// True when `bytes` starts with `prefix`, ignoring ASCII letter case.
#[must_use]
pub fn starts_with_ignore_case(bytes: &[u8], prefix: &[u8]) -> bool {
    bytes.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

/// Splits at the first `separator`: the part before and the part after.
#[must_use]
pub fn split_once(bytes: &[u8], separator: u8) -> (&[u8], Option<&[u8]>) {
    match bytes.iter().position(|&b| b == separator) {
        Some(index) => (bytes.get(..index).unwrap_or(&[]), bytes.get(index.saturating_add(1)..)),
        None => (bytes, None),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn writer_refuses_overflow() {
        let mut buffer = [0u8; 4];
        let mut writer = Writer::new(&mut buffer);
        assert!(writer.put(b"abc").is_ok());
        assert_eq!(writer.put(b"de"), Err(Overflow));
        assert_eq!(writer.written(), b"abc");
        assert!(writer.decimal(7).is_ok());
        assert_eq!(writer.written(), b"abc7");
    }

    #[test]
    fn numbers() {
        let mut buffer = [0u8; 32];
        let mut writer = Writer::new(&mut buffer);
        writer.decimal(0).unwrap();
        writer.byte(b' ').unwrap();
        writer.decimal(u64::MAX).unwrap();
        writer.byte(b' ').unwrap();
        writer.hex(0x3F7F, 4).unwrap();
        assert_eq!(writer.written(), b"0 18446744073709551615 3F7F");
        assert_eq!(parse_decimal(b"65535", 5), Some(65535));
        assert_eq!(parse_decimal(b"+1", 5), None);
        assert_eq!(parse_decimal(b"", 5), None);
        assert_eq!(parse_decimal(b"123456", 5), None);
    }

    #[test]
    fn trimming() {
        assert_eq!(trim_ows(b" \t a b \t"), b"a b");
        assert_eq!(trim_ows(b"   "), b"");
        assert_eq!(split_once(b"a=b=c", b'='), (&b"a"[..], Some(&b"b=c"[..])));
    }
}
