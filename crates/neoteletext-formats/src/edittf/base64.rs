//! The base64url codec of edit.tf addresses (RFC 4648 section 5): seven-bit
//! codes packed six bits to a character, and the hexadecimal fields.

use neoteletext_core::consts::CODE_MASK_7BIT;
use neoteletext_core::types::hex_digit;

use crate::consts::{BASE64_BITS, BASE64URL_ALPHABET, EDITTF_CODE_BITS};

use super::BASE64_MASK;

/// The value of a base64url character.
pub(super) fn base64_value(character: u8) -> Option<u32> {
    BASE64URL_ALPHABET.iter().position(|&known| known == character).and_then(|value| u32::try_from(value).ok())
}

/// Reads 7-bit codes, most significant bit first, from base64url text into
/// `codes`. Returns false on a character outside the alphabet.
pub(super) fn unpack_codes(text: &[u8], codes: &mut [u8]) -> bool {
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut output = codes.iter_mut();
    for &character in text {
        let Some(value) = base64_value(character) else { return false };
        bits = (bits << BASE64_BITS) | value;
        held = held.saturating_add(u32::try_from(BASE64_BITS).unwrap_or(0));
        let code_bits = u32::try_from(EDITTF_CODE_BITS).unwrap_or(0);
        while held >= code_bits {
            held = held.saturating_sub(code_bits);
            if let Some(slot) = output.next() {
                *slot = ((bits >> held) as u8) & CODE_MASK_7BIT;
            }
        }
        bits &= 1u32.checked_shl(held).map_or(0, |bit| bit.wrapping_sub(1));
    }
    true
}

/// Writes 7-bit codes as base64url text, most significant bit first,
/// padding the last character with zeros.
pub(super) fn pack_codes(codes: &[u8], out: &mut dyn FnMut(&[u8])) {
    let mut bits = 0u32;
    let mut held = 0u32;
    let base64_bits = u32::try_from(BASE64_BITS).unwrap_or(0);
    let emit = |value: u32, out: &mut dyn FnMut(&[u8])| {
        let index = usize::try_from(value & BASE64_MASK).unwrap_or(0);
        if let Some(character) = BASE64URL_ALPHABET.get(index) {
            out(core::slice::from_ref(character));
        }
    };
    for &code in codes {
        bits = (bits << EDITTF_CODE_BITS) | u32::from(code & CODE_MASK_7BIT);
        held = held.saturating_add(u32::try_from(EDITTF_CODE_BITS).unwrap_or(0));
        while held >= base64_bits {
            held = held.saturating_sub(base64_bits);
            emit(bits >> held, out);
        }
        bits &= 1u32.checked_shl(held).map_or(0, |bit| bit.wrapping_sub(1));
    }
    if held > 0 {
        emit(bits << base64_bits.saturating_sub(held), out);
    }
}

pub(super) fn hex_value(text: &[u8]) -> Option<u32> {
    if text.is_empty() || text.len() > 8 {
        return None;
    }
    text.iter().try_fold(0u32, |value, &byte| Some((value << 4) | char::from(byte).to_digit(16)?))
}

pub(super) fn write_hex(value: u32, digits: u32, out: &mut dyn FnMut(&[u8])) {
    for index in (0..digits).rev() {
        let digit = (value >> index.saturating_mul(4)) as u8;
        out(&[hex_digit(digit)]);
    }
}
