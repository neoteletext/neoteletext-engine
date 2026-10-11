//! Base64 with the standard alphabet and padding (RFC 4648 section 4),
//! written here rather than taken as a dependency (decision E.38).
//!
//! The decoder is strict: the length must be a multiple of four, padding
//! appears only at the end, and the unused bits of the last character must
//! be zero (RFC 4648 section 3.5 permits rejecting such data).

use crate::consts::{BASE64_ALPHABET, BASE64_BITS, BASE64_GROUP_BYTES, BASE64_GROUP_CHARS, BASE64_PAD};

/// Why base64 text was rejected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Base64Error {
    /// The length is not a multiple of four.
    Length,
    /// A character outside the alphabet, or misplaced padding.
    Character,
    /// The unused bits of the final group are not zero.
    NonCanonical,
    /// The output buffer is too small.
    Overflow,
}

/// The number of characters `bytes` bytes encode to.
#[must_use]
pub const fn encoded_len(bytes: usize) -> usize {
    bytes.div_ceil(BASE64_GROUP_BYTES).saturating_mul(BASE64_GROUP_CHARS)
}

/// The largest number of bytes `chars` characters can decode to.
#[must_use]
pub const fn decoded_len_max(chars: usize) -> usize {
    (chars / BASE64_GROUP_CHARS).saturating_mul(BASE64_GROUP_BYTES)
}

fn alphabet(index: u32) -> u8 {
    BASE64_ALPHABET.get((index & 0x3F) as usize).copied().unwrap_or(BASE64_PAD)
}

/// Encodes `input` into `output`, returning the number of characters
/// written.
pub fn encode(input: &[u8], output: &mut [u8]) -> Result<usize, Base64Error> {
    let needed = encoded_len(input.len());
    let target = output.get_mut(..needed).ok_or(Base64Error::Overflow)?;
    for (group, chars) in input.chunks(BASE64_GROUP_BYTES).zip(target.chunks_mut(BASE64_GROUP_CHARS)) {
        // Three bytes, most significant first, are four 6-bit groups.
        let value = group.iter().enumerate().fold(0u32, |value, (index, &byte)| {
            value | (u32::from(byte) << BYTE_BITS.saturating_mul(2u32.saturating_sub(index as u32)))
        });
        for (index, slot) in chars.iter_mut().enumerate() {
            // A group of n bytes gives n + 1 characters; the rest are pads.
            *slot = if index <= group.len() {
                alphabet(value >> BASE64_BITS.saturating_mul(3u32.saturating_sub(index as u32)))
            } else {
                BASE64_PAD
            };
        }
    }
    Ok(needed)
}

/// Bits in a byte.
const BYTE_BITS: u32 = 8;

fn sextet(character: u8) -> Option<u32> {
    let value = match character {
        b'A'..=b'Z' => character.wrapping_sub(b'A'),
        b'a'..=b'z' => character.wrapping_sub(b'a').wrapping_add(26),
        b'0'..=b'9' => character.wrapping_sub(b'0').wrapping_add(52),
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(value))
}

/// Decodes one group of four characters: the three bytes and how many of
/// them are real.
fn decode_group(chars: [u8; 4], last: bool) -> Result<([u8; 3], usize), Base64Error> {
    let [c0, c1, c2, c3] = chars;
    let pad2 = c2 == BASE64_PAD;
    let pad3 = c3 == BASE64_PAD;
    // Padding only at the end of the last group, and "x=" never before a
    // character.
    if (pad2 && !pad3) || (pad3 && !last) {
        return Err(Base64Error::Character);
    }
    let s0 = sextet(c0).ok_or(Base64Error::Character)?;
    let s1 = sextet(c1).ok_or(Base64Error::Character)?;
    let s2 = if pad2 { 0 } else { sextet(c2).ok_or(Base64Error::Character)? };
    let s3 = if pad3 { 0 } else { sextet(c3).ok_or(Base64Error::Character)? };
    let value = (s0 << (3 * BASE64_BITS)) | (s1 << (2 * BASE64_BITS)) | (s2 << BASE64_BITS) | s3;
    let bytes = [(value >> (2 * BYTE_BITS)) as u8, (value >> BYTE_BITS) as u8, value as u8];
    let count = if pad2 {
        1
    } else if pad3 {
        2
    } else {
        3
    };
    // RFC 4648 section 3.5: the bits after the last real byte must be zero.
    let unused_bits = BYTE_BITS.saturating_mul(3u32.saturating_sub(count as u32));
    if unused_bits == 0 || value.trailing_zeros() >= unused_bits {
        Ok((bytes, count))
    } else {
        Err(Base64Error::NonCanonical)
    }
}

/// Decodes `input` into `output`, returning the number of bytes written.
pub fn decode(input: &[u8], output: &mut [u8]) -> Result<usize, Base64Error> {
    let (groups, rest) = input.as_chunks::<BASE64_GROUP_CHARS>();
    if !rest.is_empty() {
        return Err(Base64Error::Length);
    }
    let mut written = 0usize;
    for (index, chars) in groups.iter().enumerate() {
        let (bytes, count) = decode_group(*chars, index.saturating_add(1) == groups.len())?;
        let end = written.saturating_add(count);
        let target = output.get_mut(written..end).ok_or(Base64Error::Overflow)?;
        target.copy_from_slice(bytes.get(..count).unwrap_or(&[]));
        written = end;
    }
    Ok(written)
}

/// Decodes base64 held in `buffer[..len]` in place, returning the decoded
/// length. Each group of four characters becomes at most three bytes
/// written at or before its own position, so no input is overwritten
/// before it is read.
pub fn decode_in_place(buffer: &mut [u8], len: usize) -> Result<usize, Base64Error> {
    if !len.is_multiple_of(BASE64_GROUP_CHARS) || len > buffer.len() {
        return Err(Base64Error::Length);
    }
    let groups = len / BASE64_GROUP_CHARS;
    let mut written = 0usize;
    for index in 0..groups {
        let start = index.saturating_mul(BASE64_GROUP_CHARS);
        let mut chars = [0u8; 4];
        for (offset, slot) in chars.iter_mut().enumerate() {
            *slot = buffer.get(start.saturating_add(offset)).copied().unwrap_or(0);
        }
        let (bytes, count) = decode_group(chars, index.saturating_add(1) == groups)?;
        for &byte in bytes.get(..count).unwrap_or(&[]) {
            if let Some(slot) = buffer.get_mut(written) {
                *slot = byte;
            }
            written = written.saturating_add(1);
        }
    }
    Ok(written)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;

    /// RFC 4648 section 10 test vectors.
    const VECTORS: [(&[u8], &[u8]); 7] = [
        (b"", b""),
        (b"f", b"Zg=="),
        (b"fo", b"Zm8="),
        (b"foo", b"Zm9v"),
        (b"foob", b"Zm9vYg=="),
        (b"fooba", b"Zm9vYmE="),
        (b"foobar", b"Zm9vYmFy"),
    ];

    #[test]
    fn rfc4648_vectors_encode_and_decode() {
        for (plain, encoded) in VECTORS {
            let mut out = [0u8; 16];
            let n = encode(plain, &mut out).unwrap();
            assert_eq!(&out[..n], encoded);
            let mut back = [0u8; 16];
            let n = decode(encoded, &mut back).unwrap();
            assert_eq!(&back[..n], plain);
            let mut in_place = [0u8; 16];
            in_place[..encoded.len()].copy_from_slice(encoded);
            let n = decode_in_place(&mut in_place, encoded.len()).unwrap();
            assert_eq!(&in_place[..n], plain);
        }
    }

    #[test]
    fn strict_decoding() {
        let mut out = [0u8; 16];
        assert_eq!(decode(b"Zg=", &mut out), Err(Base64Error::Length));
        assert_eq!(decode(b"Zg==Zg==", &mut out), Err(Base64Error::Character));
        assert_eq!(decode(b"Z=g=", &mut out), Err(Base64Error::Character));
        assert_eq!(decode(b"Zh==", &mut out), Err(Base64Error::NonCanonical));
        assert_eq!(decode(b"Zm9 ", &mut out), Err(Base64Error::Character));
        assert_eq!(decode(b"Zm9v", &mut out[..2]), Err(Base64Error::Overflow));
    }

    #[test]
    fn all_byte_values_round_trip() {
        let mut input = [0u8; 256];
        for (i, b) in input.iter_mut().enumerate() {
            *b = i as u8;
        }
        let mut text = [0u8; 344];
        let n = encode(&input, &mut text).unwrap();
        assert_eq!(n, encoded_len(256));
        let mut back = [0u8; 256];
        assert_eq!(decode(&text[..n], &mut back).unwrap(), 256);
        assert_eq!(back, input);
    }
}
