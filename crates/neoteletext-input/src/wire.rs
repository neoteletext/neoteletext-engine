//! The encoding primitives of the input protocol (`neoteletext-common/INPUT.md`
//! section 5.2, REQ-INT-102): unsigned and signed varints, and
//! length-prefixed strings, over caller-provided buffers.

/// The most bytes a varint takes: 32 bits in groups of 7.
pub const MAX_VARINT: usize = 5;
/// The bits of a value each varint byte carries.
const GROUP_BITS: u32 = 7;
/// The low seven bits of a byte.
const GROUP: u32 = 0x7F;
/// The largest value a varint holds in one byte.
pub const ONE_BYTE_MAX: u32 = GROUP;
/// The bit that says another byte follows.
const MORE: u8 = 0x80;
/// The bits of a 32-bit value, for the zigzag mapping's sign.
const SIGN_SHIFT: u32 = 31;

/// Why bytes were not a well-formed primitive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireError {
    /// The bytes end before the value does.
    Incomplete,
    /// A varint longer than [`MAX_VARINT`] bytes, beyond 32 bits, or not in
    /// its shortest form.
    Varint,
    /// A string longer than its field allows.
    TooLong,
    /// A string that is not UTF-8.
    Utf8,
    /// The output buffer is too small.
    Full,
}

/// Writes into a caller's buffer.
pub struct Writer<'a> {
    out: &'a mut [u8],
    used: usize,
}

impl<'a> Writer<'a> {
    /// A writer at the start of `out`.
    pub fn new(out: &'a mut [u8]) -> Self {
        Self { out, used: 0 }
    }

    /// The bytes written.
    #[must_use]
    pub fn len(&self) -> usize {
        self.used
    }

    /// Whether nothing was written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.used == 0
    }

    /// Writes one byte.
    ///
    /// # Errors
    /// When the buffer is full.
    pub fn byte(&mut self, byte: u8) -> Result<(), WireError> {
        let slot = self.out.get_mut(self.used).ok_or(WireError::Full)?;
        *slot = byte;
        self.used = self.used.saturating_add(1);
        Ok(())
    }

    /// Writes bytes.
    ///
    /// # Errors
    /// When the buffer is full.
    pub fn bytes(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        let end = self.used.checked_add(bytes.len()).ok_or(WireError::Full)?;
        self.out.get_mut(self.used..end).ok_or(WireError::Full)?.copy_from_slice(bytes);
        self.used = end;
        Ok(())
    }

    /// Writes an unsigned varint.
    ///
    /// # Errors
    /// When the buffer is full.
    pub fn uv(&mut self, mut value: u32) -> Result<(), WireError> {
        loop {
            let low = (value & GROUP) as u8;
            value >>= GROUP_BITS;
            if value == 0 {
                return self.byte(low);
            }
            self.byte(low | MORE)?;
        }
    }

    /// Writes a signed varint, zigzag-mapped.
    ///
    /// # Errors
    /// When the buffer is full.
    pub fn sv(&mut self, value: i32) -> Result<(), WireError> {
        self.uv(zigzag(value))
    }

    /// Writes a string: its byte length as a varint, then its bytes.
    ///
    /// # Errors
    /// When the string is longer than `limit` or the buffer is full.
    pub fn str(&mut self, text: &str, limit: usize) -> Result<(), WireError> {
        if text.len() > limit {
            return Err(WireError::TooLong);
        }
        self.uv(u32::try_from(text.len()).map_err(|_| WireError::TooLong)?)?;
        self.bytes(text.as_bytes())
    }
}

/// Reads from bytes that may end early: [`WireError::Incomplete`] then means
/// more is needed, not that the bytes are wrong.
pub struct Reader<'a> {
    bytes: &'a [u8],
    used: usize,
}

impl<'a> Reader<'a> {
    /// A reader at the start of `bytes`.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, used: 0 }
    }

    /// The bytes read so far.
    #[must_use]
    pub const fn used(&self) -> usize {
        self.used
    }

    /// Reads one byte.
    ///
    /// # Errors
    /// When the bytes end.
    pub fn byte(&mut self) -> Result<u8, WireError> {
        let byte = *self.bytes.get(self.used).ok_or(WireError::Incomplete)?;
        self.used = self.used.saturating_add(1);
        Ok(byte)
    }

    /// Reads `count` bytes.
    ///
    /// # Errors
    /// When the bytes end.
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], WireError> {
        let end = self.used.checked_add(count).ok_or(WireError::Incomplete)?;
        let taken = self.bytes.get(self.used..end).ok_or(WireError::Incomplete)?;
        self.used = end;
        Ok(taken)
    }

    /// Reads an unsigned varint, refusing one longer than it need be.
    ///
    /// # Errors
    /// When the bytes end, or the varint breaks section 5.2.
    pub fn uv(&mut self) -> Result<u32, WireError> {
        let mut value: u32 = 0;
        for index in 0..MAX_VARINT {
            let byte = self.byte()?;
            let group = u32::from(byte) & GROUP;
            let shift = GROUP_BITS.saturating_mul(u32::try_from(index).unwrap_or(u32::MAX));
            let shifted = group.checked_shl(shift).ok_or(WireError::Varint)?;
            // The bits shifted out must be zero: nothing beyond 32 bits.
            if shifted.checked_shr(shift) != Some(group) {
                return Err(WireError::Varint);
            }
            value |= shifted;
            if byte & MORE == 0 {
                // The shortest form: a last group of zero only for zero.
                if index > 0 && group == 0 {
                    return Err(WireError::Varint);
                }
                return Ok(value);
            }
        }
        Err(WireError::Varint)
    }

    /// Reads a signed varint.
    ///
    /// # Errors
    /// As [`Reader::uv`].
    pub fn sv(&mut self) -> Result<i32, WireError> {
        self.uv().map(unzigzag)
    }

    /// Reads a string of at most `limit` bytes.
    ///
    /// # Errors
    /// When the bytes end, the length is over `limit`, or it is not UTF-8.
    pub fn str(&mut self, limit: usize) -> Result<&'a str, WireError> {
        let length = usize::try_from(self.uv()?).map_err(|_| WireError::TooLong)?;
        if length > limit {
            return Err(WireError::TooLong);
        }
        core::str::from_utf8(self.take(length)?).map_err(|_| WireError::Utf8)
    }
}

/// Zigzag: small magnitudes of either sign become small unsigned values.
#[must_use]
pub const fn zigzag(value: i32) -> u32 {
    // The casts reinterpret bits, as the mapping is defined.
    #[allow(clippy::cast_sign_loss, reason = "zigzag is defined on the bits")]
    let mapped = ((value << 1) ^ (value >> SIGN_SHIFT)) as u32;
    mapped
}

/// The inverse of [`zigzag`].
#[must_use]
pub const fn unzigzag(value: u32) -> i32 {
    #[allow(clippy::cast_possible_wrap, reason = "zigzag is defined on the bits")]
    let mapped = ((value >> 1) as i32) ^ 0_i32.wrapping_sub((value & 1) as i32);
    mapped
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
    use super::*;

    fn uv(value: u32) -> std::vec::Vec<u8> {
        let mut out = [0; MAX_VARINT];
        let mut writer = Writer::new(&mut out);
        writer.uv(value).unwrap();
        let used = writer.len();
        out[..used].to_vec()
    }

    // REQ-INT-102.
    #[test]
    fn varints_take_one_byte_to_127_and_round_trip() {
        assert_eq!(uv(0), [0]);
        assert_eq!(uv(127), [0x7F]);
        assert_eq!(uv(128), [0x80, 0x01]);
        assert_eq!(uv(0x100), [0x80, 0x02]);
        assert_eq!(uv(u32::MAX).len(), MAX_VARINT);
        for value in [0, 1, 127, 128, 255, 300, 16_383, 16_384, 0x8FF, u32::MAX] {
            assert_eq!(Reader::new(&uv(value)).uv(), Ok(value));
        }
    }

    // REQ-INT-102.
    #[test]
    fn a_varint_too_long_or_not_shortest_is_refused() {
        assert_eq!(Reader::new(&[0x80, 0x00]).uv(), Err(WireError::Varint), "zero in two bytes");
        assert_eq!(Reader::new(&[0xFF, 0xFF, 0xFF, 0xFF, 0x7F]).uv(), Err(WireError::Varint), "beyond 32 bits");
        assert_eq!(Reader::new(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x01]).uv(), Err(WireError::Varint), "six bytes");
        assert_eq!(Reader::new(&[0x80]).uv(), Err(WireError::Incomplete));
    }

    // REQ-INT-102.
    #[test]
    fn signed_varints_are_zigzagged() {
        assert_eq!(zigzag(0), 0);
        assert_eq!(zigzag(-1), 1);
        assert_eq!(zigzag(1), 2);
        assert_eq!(zigzag(-64), 127, "−64 to 63 in one byte");
        assert_eq!(zigzag(120), 240);
        for value in [0, -1, 1, -64, 63, 120, -120, i32::MIN, i32::MAX] {
            assert_eq!(unzigzag(zigzag(value)), value);
        }
    }

    // REQ-INT-102.
    #[test]
    fn strings_are_bounded_and_utf8() {
        let mut out = [0; 16];
        let mut writer = Writer::new(&mut out);
        writer.str("é!", 8).unwrap();
        assert_eq!(writer.str("too long for it", 4), Err(WireError::TooLong));
        let used = writer.len();
        assert_eq!(Reader::new(&out[..used]).str(8), Ok("é!"));
        assert_eq!(Reader::new(&[2, 0xC3, 0x28]).str(8), Err(WireError::Utf8));
        assert_eq!(Reader::new(&[9]).str(8), Err(WireError::TooLong));
    }
}
