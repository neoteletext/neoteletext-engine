//! Error-protection codes (EN 300 706 clause 8, area ECC).
//!
//! Bytes are in the internal packet form of REQ-ARCH-002: the first
//! transmitted bit is bit 0. In the standard's numbering, bit b1 is bit 0 and
//! bit b8 is bit 7.

use crate::consts::{CODE_MASK_7BIT, NIBBLE_MASK, TRIPLET_DATA_MASK};

/// The outcome of decoding a protected value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// Every parity test passed.
    Clean,
    /// A single error was found and corrected.
    Corrected,
    /// The value could not be decoded.
    Uncorrectable,
}

impl Status {
    /// True unless the value was uncorrectable.
    #[must_use]
    pub const fn is_usable(self) -> bool {
        !matches!(self, Self::Uncorrectable)
    }
}

/// True when a byte has odd parity: an odd number of 1 bits (clause 8.1).
#[must_use]
pub const fn has_odd_parity(byte: u8) -> bool {
    byte.count_ones() % 2 == 1
}

/// Checks an odd-parity byte (REQ-ECC-001): its 7-bit character value and
/// whether it passed.
#[must_use]
pub const fn check_parity(byte: u8) -> (u8, bool) {
    (byte & CODE_MASK_7BIT, has_odd_parity(byte))
}

/// Adds the odd-parity bit to a 7-bit value.
#[must_use]
pub const fn with_odd_parity(value: u8) -> u8 {
    let seven = value & CODE_MASK_7BIT;
    if has_odd_parity(seven) { seven } else { seven | crate::consts::PARITY_BIT }
}

const fn bit(byte: u8, index: u32) -> u8 {
    (byte >> index) & 1
}

/// Encodes a 4-bit value as a Hamming 8/4 byte with the equations of
/// clause 8.2 (REQ-ECC-003). Data bits D1 to D4 are b2, b4, b6 and b8;
/// protection bits P1 to P4 are b1, b3, b5 and b7.
#[must_use]
pub const fn hamming84_encode(value: u8) -> u8 {
    let d1 = value & 1;
    let d2 = (value >> 1) & 1;
    let d3 = (value >> 2) & 1;
    let d4 = (value >> 3) & 1;
    // Clause 8.2: each of tests A, B and C covers four bits and must give odd
    // parity; test D covers all eight.
    let p1 = 1 ^ d1 ^ d3 ^ d4;
    let p2 = 1 ^ d1 ^ d2 ^ d4;
    let p3 = 1 ^ d1 ^ d2 ^ d3;
    let p4 = 1 ^ p1 ^ d1 ^ p2 ^ d2 ^ p3 ^ d3 ^ d4;
    p1 | (d1 << 1) | (p2 << 2) | (d2 << 3) | (p3 << 4) | (d3 << 5) | (p4 << 6) | (d4 << 7)
}

const fn hamming84_data(byte: u8) -> u8 {
    bit(byte, 1) | (bit(byte, 3) << 1) | (bit(byte, 5) << 2) | (bit(byte, 7) << 3)
}

/// Decodes one byte by the parity tests of clause 8.2.
const fn hamming84_decode_slow(byte: u8) -> (u8, Status) {
    let a = bit(byte, 0) ^ bit(byte, 1) ^ bit(byte, 5) ^ bit(byte, 7);
    let b = bit(byte, 1) ^ bit(byte, 2) ^ bit(byte, 3) ^ bit(byte, 7);
    let c = bit(byte, 1) ^ bit(byte, 3) ^ bit(byte, 4) ^ bit(byte, 5);
    let d = byte.count_ones() & 1;
    let abc_pass = a == 1 && b == 1 && c == 1;
    if d == 1 {
        // Overall parity correct: either no error, or a double error.
        if abc_pass { (hamming84_data(byte), Status::Clean) } else { (0, Status::Uncorrectable) }
    } else if abc_pass {
        // Only test D failed: the error is in P4, the data is intact.
        (hamming84_data(byte), Status::Corrected)
    } else {
        // Single error: the failing tests name the bit (clause 8.2).
        let failing = (1 ^ a) | ((1 ^ b) << 1) | ((1 ^ c) << 2);
        let flip: u8 = match failing {
            0b001 => 1 << 0, // P1: test A only
            0b010 => 1 << 2, // P2: test B only
            0b100 => 1 << 4, // P3: test C only
            0b111 => 1 << 1, // D1: tests A, B and C
            0b110 => 1 << 3, // D2: tests B and C
            0b101 => 1 << 5, // D3: tests A and C
            _ => 1 << 7,     // D4: tests A and B
        };
        (hamming84_data(byte ^ flip), Status::Corrected)
    }
}

// Runs only during compilation: an out-of-range index or overflow here stops
// the build, so it cannot become a run-time panic (REQ-GEN-006).
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn build_hamming84_table() -> [(u8, Status); 256] {
    let mut table = [(0, Status::Uncorrectable); 256];
    let mut index = 0;
    while index < 256 {
        table[index] = hamming84_decode_slow(index as u8);
        index += 1;
    }
    table
}

/// The 256-entry Hamming 8/4 decoding table, generated at compile time from
/// the equations, not typed in (REQ-ECC-002).
static HAMMING84_TABLE: [(u8, Status); 256] = build_hamming84_table();

/// Decodes a Hamming 8/4 byte (REQ-ECC-002).
#[must_use]
pub fn hamming84_decode(byte: u8) -> (u8, Status) {
    HAMMING84_TABLE.get(usize::from(byte)).copied().unwrap_or((0, Status::Uncorrectable))
}

/// Decodes a Hamming 8/4 byte, giving `None` when uncorrectable.
#[must_use]
pub fn hamming84(byte: u8) -> Option<u8> {
    let (value, status) = hamming84_decode(byte);
    if status.is_usable() { Some(value & NIBBLE_MASK) } else { None }
}

/// Bit shifts of data bits D1 to D18 within a Hamming 24/18 word, where
/// position B1 is shift 0 (clause 8.3): every position that is not a power
/// of two and not B24.
const TRIPLET_DATA_SHIFTS: [u32; 18] = [2, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 21, 22];
/// Shifts of protection bits P1 to P5 (positions 1, 2, 4, 8 and 16), which
/// make tests A to E odd; test n covers every position whose 1-based index
/// has bit n set.
const TRIPLET_TEST_BITS: [u32; 5] = [0, 1, 2, 3, 4];
/// Shift of P6 at position 24, which makes test F odd.
const TRIPLET_P6_SHIFT: u32 = 23;
/// Positions in a triplet word.
const TRIPLET_POSITIONS: u32 = 24;

fn triplet_word(bytes: [u8; 3]) -> u32 {
    let [low, middle, high] = bytes;
    u32::from(low) | (u32::from(middle) << 8) | (u32::from(high) << 16)
}

/// Parity of the positions covered by test `test` (A to E are 0 to 4), or
/// of all 24 positions for test F when `test` is `None` (clause 8.3).
fn test_parity(word: u32, test: Option<u32>) -> u32 {
    (0..TRIPLET_POSITIONS)
        .filter(|&shift| match test {
            // Tests A to E cover B1 to B23 only; B24 (P6) is in test F alone.
            Some(bit) => shift != TRIPLET_P6_SHIFT && (shift.wrapping_add(1) >> bit) & 1 == 1,
            None => true,
        })
        .fold(0, |parity, shift| parity ^ ((word >> shift) & 1))
}

fn triplet_data(word: u32) -> u32 {
    TRIPLET_DATA_SHIFTS.iter().enumerate().fold(0, |value, (index, &shift)| value | (((word >> shift) & 1) << index))
}

/// Encodes an 18-bit value as a Hamming 24/18 triplet (REQ-ECC-005), with
/// D1 the least significant bit of `value`.
#[must_use]
pub fn hamming2418_encode(value: u32) -> [u8; 3] {
    let data = value & TRIPLET_DATA_MASK;
    let mut word = TRIPLET_DATA_SHIFTS
        .iter()
        .enumerate()
        .fold(0u32, |word, (index, &shift)| word | (((data >> index) & 1) << shift));
    for &test in &TRIPLET_TEST_BITS {
        if test_parity(word, Some(test)) == 0 {
            // P1..P5 sit at position 2^test, which is shift 2^test - 1.
            word |= 1 << (1u32 << test).wrapping_sub(1);
        }
    }
    if test_parity(word, None) == 0 {
        word |= 1 << TRIPLET_P6_SHIFT;
    }
    let bytes = word.to_le_bytes();
    let [low, middle, high, _] = bytes;
    [low, middle, high]
}

/// Decodes a Hamming 24/18 triplet exactly as clause 8.3 and REQ-ECC-004
/// specify. The value has D1 as its least significant bit.
#[must_use]
pub fn hamming2418_decode(bytes: [u8; 3]) -> (u32, Status) {
    let word = triplet_word(bytes);
    let syndrome = TRIPLET_TEST_BITS.iter().fold(0u32, |syndrome, &test| {
        if test_parity(word, Some(test)) == 0 { syndrome | (1 << test) } else { syndrome }
    });
    if syndrome == 0 {
        // Tests A to E pass: the data is accepted. A failing test F alone is
        // an error in P6.
        let status = if test_parity(word, None) == 1 { Status::Clean } else { Status::Corrected };
        return (triplet_data(word), status);
    }
    if test_parity(word, None) == 1 {
        // Tests A to E disagree but F passes: a double error.
        return (0, Status::Uncorrectable);
    }
    if syndrome > TRIPLET_POSITIONS {
        return (0, Status::Uncorrectable);
    }
    let corrected = word ^ (1 << syndrome.wrapping_sub(1));
    (triplet_data(corrected), Status::Corrected)
}

/// Decodes a triplet, giving `None` when uncorrectable.
#[must_use]
pub fn hamming2418(bytes: [u8; 3]) -> Option<u32> {
    let (value, status) = hamming2418_decode(bytes);
    if status.is_usable() { Some(value) } else { None }
}

/// Reverses the bit order of a byte (REQ-ECC-006), converting between the
/// internal order and the order of EN 300 472.
#[must_use]
pub const fn reverse_bits(byte: u8) -> u8 {
    byte.reverse_bits()
}

/// The page checksum shift register of clause 9.6.1 (REQ-ECC-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PageChecksum {
    register: u16,
}

impl PageChecksum {
    /// A register initialised to zero.
    #[must_use]
    pub const fn new() -> Self {
        Self { register: 0 }
    }

    /// Applies one byte, from bit 8 down to bit 1. The feedback is taken from
    /// stages 7, 9, 12 and 16 (clause 9.6.1), which are register bits 6, 8,
    /// 11 and 15.
    pub fn update(&mut self, byte: u8) {
        let mut index = 8u32;
        while index > 0 {
            index = index.wrapping_sub(1);
            let input = u16::from((byte >> index) & 1);
            let feedback = (self.register >> 15) ^ (self.register >> 11) ^ (self.register >> 8) ^ (self.register >> 6);
            self.register = (self.register << 1) | ((input ^ feedback) & 1);
        }
    }

    /// Applies a run of bytes.
    pub fn update_all(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.update(byte);
        }
    }

    /// The checksum so far.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.register
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::unwrap_used)]
mod tests {
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
        stream
            .iter()
            .rev()
            .take(16)
            .enumerate()
            .fold(0u16, |register, (stage, &bit)| register | (u16::from(bit) << stage))
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
}
