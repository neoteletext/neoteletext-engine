//! The two checks of the profile: CRC-32 over a record (REQ-AUD-003) and the
//! 16-bit check of an independent data line (REQ-AUD-007).

/// CRC-32/ISO-HDLC's polynomial, reflected (0x04C11DB7 bit-reversed).
const CRC32_POLYNOMIAL_REFLECTED: u32 = 0xEDB8_8320;
/// CRC-32/ISO-HDLC's initial register and final XOR.
const CRC32_INVERT: u32 = 0xFFFF_FFFF;

/// CRC-32/ISO-HDLC of `bytes`: reflected, initial and final XOR all ones.
/// Its check value for ASCII `123456789` is `0xCBF43926`.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    crc32_continue(CRC32_INVERT, bytes) ^ CRC32_INVERT
}

/// CRC-32 over two pieces, as if they were one: a record's header and body,
/// without the CRC field between them.
#[must_use]
pub fn crc32_parts(first: &[u8], second: &[u8]) -> u32 {
    crc32_continue(crc32_continue(CRC32_INVERT, first), second) ^ CRC32_INVERT
}

fn crc32_continue(mut register: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        register ^= u32::from(byte);
        for _ in 0..8 {
            let low = register & 1;
            register >>= 1;
            if low != 0 {
                register ^= CRC32_POLYNOMIAL_REFLECTED;
            }
        }
    }
    register
}

/// EN 300 708 clause 6.5.8.1's polynomial x¹⁶ + x⁹ + x⁷ + x⁴ + 1, without
/// its x¹⁶ term.
const CRC16_POLYNOMIAL: u16 = (1 << 9) | (1 << 7) | (1 << 4) | 1;

/// The 16-bit check of an independent data line (EN 300 708 clause
/// 6.5.8.1): the data as a polynomial, highest degree first, divided by the
/// clause's polynomial from a register of zeros; the remainder is sent
/// highest term first.
#[must_use]
pub fn crc16_idl(bytes: &[u8]) -> u16 {
    let mut register: u16 = 0;
    for &byte in bytes {
        register ^= u16::from(byte) << 8;
        for _ in 0..8 {
            let high = register & 0x8000;
            register <<= 1;
            if high != 0 {
                register ^= CRC16_POLYNOMIAL;
            }
        }
    }
    register
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-003 and REQ-AUD-007.
    use super::*;

    #[test]
    fn crc32_has_the_iso_hdlc_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32_parts(b"1234", b"56789"), 0xCBF4_3926);
    }

    #[test]
    fn crc16_divides_by_the_en_300_708_polynomial() {
        // Appending the remainder, highest term first, leaves a register of
        // zeros: the clause's check (6.5.8.2).
        let data = *b"audio data line";
        let remainder = crc16_idl(&data);
        let mut checked = std::vec::Vec::from(data);
        checked.extend_from_slice(&remainder.to_be_bytes());
        assert_eq!(crc16_idl(&checked), 0);
        // One flipped bit is caught.
        checked[3] ^= 0x10;
        assert_ne!(crc16_idl(&checked), 0);
    }
}
