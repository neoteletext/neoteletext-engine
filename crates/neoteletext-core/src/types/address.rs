//! Addressing (clauses 7.1.2 and 9.3.1, Annex A.1): magazines, page
//! numbers, page addresses, sub-codes and the header control bits.

use crate::consts::{
    DECIMAL_DIGIT_MAX, MAGAZINE_MASK, MAGAZINE_ZERO_MEANS, NIBBLE_MASK, PAGE_TIME_FILLING, SUBCODE_ANY, SUBCODE_MASK,
};

/// A magazine number, 1 to 8 (clause 7.1.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Magazine(u8);

impl Magazine {
    /// Converts the 3-bit transmitted value, where 0 means magazine 8.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        let value = bits & MAGAZINE_MASK;
        Self(if value == 0 { MAGAZINE_ZERO_MEANS } else { value })
    }

    /// A magazine from its number 1 to 8, or `None` outside that range.
    #[must_use]
    pub const fn new(number: u8) -> Option<Self> {
        if number >= 1 && number <= MAGAZINE_ZERO_MEANS { Some(Self(number)) } else { None }
    }

    /// The magazine number, 1 to 8.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }

    /// The 3-bit transmitted form, where magazine 8 is 0.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0 & MAGAZINE_MASK
    }

    /// The zero-based index 0 to 7, for arrays of per-magazine state.
    #[must_use]
    pub const fn index(self) -> usize {
        self.bits() as usize
    }
}

/// A page number within a magazine: two hexadecimal digits, 0x00 to 0xFF.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct PageNumber(pub u8);

impl PageNumber {
    /// The units digit.
    #[must_use]
    pub const fn units(self) -> u8 {
        self.0 & NIBBLE_MASK
    }

    /// The tens digit.
    #[must_use]
    pub const fn tens(self) -> u8 {
        self.0 >> 4
    }

    /// True when both digits are decimal (Table 1 note: other pages are not
    /// for display).
    #[must_use]
    pub const fn is_decimal(self) -> bool {
        self.units() <= DECIMAL_DIGIT_MAX && self.tens() <= DECIMAL_DIGIT_MAX
    }

    /// True for the time-filling page number FF.
    #[must_use]
    pub const fn is_time_filling(self) -> bool {
        self.0 == PAGE_TIME_FILLING
    }
}

/// A page address: magazine and page number, written `mpp`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct PageAddress {
    /// The magazine.
    pub magazine: Magazine,
    /// The page number within it.
    pub page: PageNumber,
}

impl PageAddress {
    /// Builds an address from the three-digit hexadecimal form `0xMPP`, where
    /// the magazine digit is 1 to 8. `None` otherwise.
    #[must_use]
    pub const fn from_mpp(mpp: u16) -> Option<Self> {
        let magazine_digit = (mpp >> 8) as u8;
        match Magazine::new(magazine_digit) {
            Some(magazine) if mpp <= 0x8FF => Some(Self { magazine, page: PageNumber((mpp & 0xFF) as u8) }),
            _ => None,
        }
    }

    /// The three-digit form `0xMPP` with the magazine as 1 to 8.
    #[must_use]
    pub const fn mpp(self) -> u16 {
        ((self.magazine.number() as u16) << 8) | self.page.0 as u16
    }

    /// The three ASCII characters of `mpp`, uppercase hexadecimal.
    #[must_use]
    pub const fn ascii(self) -> [u8; 3] {
        [hex_digit(self.magazine.number()), hex_digit(self.page.tens()), hex_digit(self.page.units())]
    }
}

/// An uppercase hexadecimal ASCII digit for a value 0 to 15.
#[must_use]
pub const fn hex_digit(value: u8) -> u8 {
    let nibble = value & NIBBLE_MASK;
    if nibble < 10 { b'0'.wrapping_add(nibble) } else { b'A'.wrapping_add(nibble.wrapping_sub(10)) }
}

/// A page sub-code, masked to the 13 bits a header can carry (Annex A.1).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
pub struct SubCode(u16);

impl SubCode {
    /// "Any sub-page", 3F7F.
    pub const ANY: Self = Self(SUBCODE_ANY);

    /// A sub-code from its raw value, masked.
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw & SUBCODE_MASK)
    }

    /// The masked value.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }

    /// True for 3F7F.
    #[must_use]
    pub const fn is_any(self) -> bool {
        self.0 == SUBCODE_ANY
    }
}

/// Header control bits C4 to C14; bit n of the word is Cn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct ControlBits(pub u16);

impl ControlBits {
    /// True when every bit in `mask` is set.
    #[must_use]
    pub const fn has(self, mask: u16) -> bool {
        self.0 & mask == mask
    }

    /// The national option index 4×C12 + 2×C13 + C14 (decision E.21).
    #[must_use]
    pub const fn national_index(self) -> u8 {
        let c12 = ((self.0 >> 12) & 1) as u8;
        let c13 = ((self.0 >> 13) & 1) as u8;
        let c14 = ((self.0 >> 14) & 1) as u8;
        (c12 << 2) | (c13 << 1) | c14
    }
}
