//! Tests of the error-protection codes (area ECC): parity, Hamming 8/4 and
//! 24/18, bit reversal and the page checksum.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::unwrap_used)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

/// REQ-ECC-003: the sixteen codes listed in the requirement.
const CODES: [u8; 16] =
    [0x15, 0x02, 0x49, 0x5E, 0x64, 0x73, 0x38, 0x2F, 0xD0, 0xC7, 0x8C, 0x9B, 0xA1, 0xB6, 0xFD, 0xEA];

#[test]
fn hamming84_encodes_the_standard_codes() {
    for (value, &code) in CODES.iter().enumerate() {
        assert_eq!(hamming84_encode(value as u8), code, "value {value}");
    }
}

/// REQ-TST-002: all 256 byte values.
#[test]
fn hamming84_exhaustive() {
    for (value, &code) in CODES.iter().enumerate() {
        assert_eq!(hamming84_decode(code), (value as u8, Status::Clean));
        for flip in 0..8 {
            let single = code ^ (1 << flip);
            assert_eq!(hamming84_decode(single), (value as u8, Status::Corrected));
            for second in (flip + 1)..8 {
                let double = single ^ (1 << second);
                assert_eq!(hamming84_decode(double).1, Status::Uncorrectable);
            }
        }
    }
    // Every byte is one of: a code, a single error, or uncorrectable.
    let usable = (0..=255u8).filter(|&byte| hamming84_decode(byte).1.is_usable()).count();
    assert_eq!(usable, 16 * 9);
}

/// REQ-TST-003: every 18-bit value, every single error, sampled doubles.
#[test]
fn hamming2418_exhaustive() {
    let mut lcg: u32 = 0x1234_5678;
    for value in 0..(1u32 << 18) {
        let encoded = hamming2418_encode(value);
        assert_eq!(hamming2418_decode(encoded), (value, Status::Clean));
        if value % 64 == 0 {
            for position in 0..24 {
                let mut corrupted = encoded;
                corrupted[position / 8] ^= 1 << (position % 8);
                assert_eq!(hamming2418_decode(corrupted), (value, Status::Corrected));
            }
            lcg = lcg.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let first = (lcg >> 8) % 24;
            let second = (first + 1 + (lcg >> 16) % 23) % 24;
            let mut double = encoded;
            double[(first / 8) as usize] ^= 1 << (first % 8);
            double[(second / 8) as usize] ^= 1 << (second % 8);
            assert_eq!(hamming2418_decode(double).1, Status::Uncorrectable);
        }
    }
}

/// REQ-ECC-002: the table is the clause 8.2 equations evaluated for every
/// byte. The build runs at compile time, so it is run again here.
#[test]
fn hamming84_table_is_the_equations() {
    let rebuilt = build_hamming84_table();
    for byte in 0..=255u8 {
        let index = usize::from(byte);
        assert_eq!(rebuilt[index], hamming84_decode_slow(byte), "byte {byte:#04x}");
        assert_eq!(HAMMING84_TABLE[index], rebuilt[index], "byte {byte:#04x}");
    }
    // Data bits D1 to D4 are b2, b4, b6 and b8 (clause 8.2), whatever the
    // protection bits hold.
    for value in 0..16u8 {
        assert_eq!(hamming84_data(hamming84_encode(value)), value);
        assert_eq!(hamming84_data(hamming84_encode(value) ^ 0b0101_0101), value);
    }
}

/// REQ-ECC-002: an error in P4 alone fails test D only, and the data is
/// taken unchanged.
#[test]
fn hamming84_error_in_p4_alone_keeps_the_data() {
    let p4 = 1 << 6;
    for (value, &code) in CODES.iter().enumerate() {
        assert_eq!(hamming84_decode_slow(code ^ p4), (value as u8, Status::Corrected));
        assert_eq!(hamming84(code ^ p4), Some(value as u8));
    }
}

/// REQ-ECC-004: three errors can give an error position above 24, which
/// is reported as uncorrectable rather than used.
#[test]
fn hamming2418_error_position_above_24_is_uncorrectable() {
    // Positions 1, 8 and 16 are P1, P4 and P5: they fail tests A, D and E,
    // so the position is 1 + 8 + 16 = 25, and test F fails (three flips).
    let positions = [1u32, 8, 16];
    assert_eq!(positions.iter().fold(0, |sum, &p| sum ^ p), 25);
    for value in [0u32, 1, 0x2_AAAA, TRIPLET_DATA_MASK] {
        let mut word = triplet_word(hamming2418_encode(value));
        for &position in &positions {
            word ^= 1 << (position - 1);
        }
        let [low, middle, high, _] = word.to_le_bytes();
        assert_eq!(hamming2418_decode([low, middle, high]), (0, Status::Uncorrectable), "value {value:#x}");
        assert_eq!(hamming2418([low, middle, high]), None);
    }
}

/// The register of clause 9.6.1 modelled as a bit sequence: each new
/// stage-1 bit is the input XOR the bits that entered 7, 9, 12 and 16
/// clocks earlier.
fn checksum_by_recurrence(bytes: &[u8]) -> u16 {
    let mut stream = std::vec::Vec::new();
    for &byte in bytes {
        for bit in (0..8).rev() {
            let input = (byte >> bit) & 1;
            let tap = |stage: usize| if stream.len() >= stage { stream[stream.len() - stage] } else { 0 };
            let next = input ^ tap(7) ^ tap(9) ^ tap(12) ^ tap(16);
            stream.push(next);
        }
    }
    stream.iter().rev().take(16).enumerate().fold(0u16, |register, (stage, &bit)| register | (u16::from(bit) << stage))
}

/// REQ-ECC-007: a zero register, bit 8 first, feedback from stages 7, 9,
/// 12 and 16.
#[test]
fn page_checksum_shift_register() {
    assert_eq!(PageChecksum::new(), PageChecksum::default());
    assert_eq!(PageChecksum::new().value(), 0);
    let mut zeros = PageChecksum::new();
    zeros.update_all(&[0; 64]);
    assert_eq!(zeros.value(), 0, "a zero register stays zero on zero input");

    // Bit 1 is applied last, so it lands in stage 1 with nothing shifted.
    let mut one = PageChecksum::new();
    one.update(0x01);
    assert_eq!(one.value(), 0x0001);
    // Bit 8 is applied first: after seven more clocks it has reached
    // stage 8, passing stage 7 on the last clock, which feeds back a 1.
    let mut eight = PageChecksum::new();
    eight.update(0x80);
    assert_eq!(eight.value(), 0x0081);

    // A run of bytes is the bytes applied one by one, and matches the
    // recurrence over long inputs, where every tap has come into play.
    let message: std::vec::Vec<u8> = (0..=255u8).chain([0x80, 0, 0, 0, 0x55, 0xAA]).collect();
    let mut whole = PageChecksum::new();
    whole.update_all(&message);
    let mut one_by_one = PageChecksum::new();
    for (index, &byte) in message.iter().enumerate() {
        one_by_one.update(byte);
        assert_eq!(one_by_one.value(), checksum_by_recurrence(&message[..=index]), "after byte {index}");
    }
    assert_eq!(whole, one_by_one);
    assert_eq!(whole.value(), checksum_by_recurrence(&message));
}

#[test]
fn parity_over_all_bytes() {
    for byte in 0..=255u8 {
        let (value, ok) = check_parity(byte);
        assert_eq!(value, byte & 0x7F);
        assert_eq!(ok, byte.count_ones() % 2 == 1);
        assert!(has_odd_parity(with_odd_parity(byte)));
        assert_eq!(with_odd_parity(byte) & 0x7F, byte & 0x7F);
    }
}

#[test]
fn bit_reversal_over_all_bytes() {
    for byte in 0..=255u8 {
        assert_eq!(reverse_bits(reverse_bits(byte)), byte);
        for index in 0..8 {
            assert_eq!((byte >> index) & 1, (reverse_bits(byte) >> (7 - index)) & 1);
        }
    }
}
