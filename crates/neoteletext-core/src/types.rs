//! Small shared types and fixed-capacity collections (Foundation).

use crate::consts::{
    CELL_FIELD_LINES, CELL_HEIGHT, COLOUR_PRIMARY_BITS, COLOUR_WORDS, DECIMAL_DIGIT_MAX, MAGAZINE_MASK,
    MAGAZINE_ZERO_MEANS, NIBBLE_MASK, PAGE_TIME_FILLING, SUBCODE_ANY, SUBCODE_MASK,
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

/// A 12-bit colour of the Colour Map: four bits each of red, green and
/// blue, from 0 (zero intensity) to 15 (full), not gamma corrected
/// (Table 30). Held as `0xRGB`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct Rgb12(pub u16);

/// Shift of the red nibble in an [`Rgb12`].
const RED_SHIFT: u32 = 2 * COLOUR_PRIMARY_BITS;
/// Shift of the green nibble in an [`Rgb12`].
const GREEN_SHIFT: u32 = COLOUR_PRIMARY_BITS;

impl Rgb12 {
    /// A colour from its three 4-bit primaries; higher bits are dropped.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self(
            ((red & NIBBLE_MASK) as u16) << RED_SHIFT
                | ((green & NIBBLE_MASK) as u16) << GREEN_SHIFT
                | (blue & NIBBLE_MASK) as u16,
        )
    }

    /// The red primary, 0 to 15.
    #[must_use]
    pub const fn red(self) -> u8 {
        (self.0 >> RED_SHIFT) as u8 & NIBBLE_MASK
    }

    /// The green primary, 0 to 15.
    #[must_use]
    pub const fn green(self) -> u8 {
        (self.0 >> GREEN_SHIFT) as u8 & NIBBLE_MASK
    }

    /// The blue primary, 0 to 15.
    #[must_use]
    pub const fn blue(self) -> u8 {
        self.0 as u8 & NIBBLE_MASK
    }
}

/// The sixteen colour words of X/28/0 format 1, X/28/4, M/29/0 or M/29/4:
/// two CLUTs of eight entries, in transmission order (Tables 4 and 10).
pub type ColourWords = [Rgb12; COLOUR_WORDS];

/// The default G0 and G2 designation and the second G0 designation of
/// Table 4, as their 7-bit Table 32 and Table 33 values.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Designations {
    /// Default G0 and G2 designation and national option (Table 32).
    pub default: u8,
    /// Second G0 designation and national option (Table 33).
    pub second: u8,
}

/// The side-panel fields of Table 4, triplet 2 bits 4 to 10.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct SidePanelFields {
    /// Bit 4: the left side panel is displayed.
    pub left: bool,
    /// Bit 5: the right side panel is displayed.
    pub right: bool,
    /// Bit 6, the Side Panel Status Flag: 1 when side panels are required
    /// at Levels 2.5 and 3.5, 0 when at Level 3.5 only.
    pub status_flag: bool,
    /// Bits 7 to 10: the number of columns of the left panel, 0 to 15
    /// (decision E.49 gives the widths).
    pub left_columns: u8,
}

/// The default screen and row colours, Black Background Colour
/// Substitution and Colour Table Re-mapping: Table 4, triplet 13 bits 5 to
/// 18, one field group of decision E.44.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct DefaultColourFields {
    /// Bits 5 to 9: the default screen colour, a Colour Map index.
    pub screen: u8,
    /// Bits 10 to 14: the default row colour, a Colour Map index.
    pub row: u8,
    /// Bit 15: Black Background Colour Substitution.
    pub black_background_substitution: bool,
    /// Bits 16 to 18: the Colour Table Re-mapping value, 0 to 7.
    pub remapping: u8,
}

/// The field groups that X/28/0 format 1, X/28/4, M/29/0 and M/29/4 share
/// (Tables 4, 10, 11 and 13). Each group is `None` when any triplet that
/// carries it was uncorrectable, so that it is treated as absent and the
/// next source in the order of decision E.44 applies.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct PresentationFields {
    /// The character set designations (triplets 1 and 2).
    pub designations: Option<Designations>,
    /// The side-panel fields (triplet 2).
    pub side_panels: Option<SidePanelFields>,
    /// The sixteen colour words (triplets 2 to 13): CLUTs 2 and 3 for
    /// X/28/0 and M/29/0, CLUTs 0 and 1 for X/28/4 and M/29/4.
    pub colour_words: Option<ColourWords>,
    /// The default colours, substitution and re-mapping (triplet 13).
    pub colours: Option<DefaultColourFields>,
}

/// The side-panel and substitution defaults that the fallback bits of a
/// Magazine Organization Table POP link give (Table 21, byte N+4), in the
/// form of the X/28 fields they stand in for (decision E.53).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct FallbackFields {
    /// The side panels, with the status flag set: the link belongs to the
    /// level whose packets carry it.
    pub side_panels: SidePanelFields,
    /// Black Background Colour Substitution.
    pub black_background_substitution: bool,
}

/// The DRCS table a character comes from: Table 29 mode 01101, data bit
/// D6.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum DrcsTable {
    /// D6 = 0: the Global DRCS page (GDRCS).
    Global,
    /// D6 = 1: the normal DRCS page.
    Normal,
}

/// A DRCS character as a cell names it: table, sub-table and character
/// (decision E.42, REQ-L25-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DrcsCharacter {
    /// The table.
    pub table: DrcsTable,
    /// The sub-table: the S1 of the DRCS sub-page, 0 to 15.
    pub subtable: u8,
    /// The character number, 0 to 47 (48 to 63 are invalid).
    pub character: u8,
}

/// One character of a loaded font (REQ-FNT-002, decisions E.72 and E.77):
/// its Unicode code point and the lines of its cell, 12 by 10 or 12 by 20,
/// top line first, bit 11 the leftmost dot.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct FontGlyph {
    /// The Unicode code point.
    pub code_point: u32,
    /// The lines of the cell; bits 12 to 15 are zero. A 12 by 10 glyph uses
    /// the first ten and leaves the rest zero.
    pub rows: [u16; CELL_HEIGHT],
    /// The glyph is 12 by 20 (decision E.77); otherwise it is 12 by 10.
    pub tall: bool,
}

impl FontGlyph {
    /// An empty slot of a glyph table.
    pub const EMPTY: Self = Self { code_point: 0, rows: [0; CELL_HEIGHT], tall: false };

    /// A 12 by 10 glyph.
    #[must_use]
    pub const fn small(code_point: u32, small: [u16; CELL_FIELD_LINES]) -> Self {
        let [l0, l1, l2, l3, l4, l5, l6, l7, l8, l9] = small;
        let rows = [l0, l1, l2, l3, l4, l5, l6, l7, l8, l9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        Self { code_point, rows, tall: false }
    }

    /// A 12 by 20 glyph (decision E.77).
    #[must_use]
    pub const fn tall(code_point: u32, rows: [u16; CELL_HEIGHT]) -> Self {
        Self { code_point, rows, tall: true }
    }

    /// The glyph's lines: ten for a 12 by 10 glyph, twenty for a 12 by 20.
    #[must_use]
    pub fn lines(&self) -> &[u16] {
        let count = if self.tall { CELL_HEIGHT } else { CELL_FIELD_LINES };
        self.rows.get(..count).unwrap_or(&[])
    }
}

/// A point in time in microseconds, from a monotonic clock supplied by the
/// caller (REQ-GEN-005).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
pub struct Instant(pub u64);

impl Instant {
    /// Microseconds from `earlier` to `self`, zero if `earlier` is later.
    #[must_use]
    pub const fn since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

/// A vector with fixed capacity, held inline: the Foundation's
/// fixed-capacity collection (no allocation, REQ-GEN-002). Equality
/// compares the items only, not the unused slots.
#[derive(Clone, Copy, Debug)]
pub struct FixedVec<T: Copy, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy + PartialEq, const N: usize> PartialEq for FixedVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Copy + Eq, const N: usize> Eq for FixedVec<T, N> {}

impl<T: Copy, const N: usize> FixedVec<T, N> {
    /// An empty vector whose unused slots hold `filler`.
    #[must_use]
    pub const fn new(filler: T) -> Self {
        Self { items: [filler; N], len: 0 }
    }

    /// Appends an item. Returns false, changing nothing, when full.
    pub fn push(&mut self, item: T) -> bool {
        match self.items.get_mut(self.len) {
            Some(slot) => {
                *slot = item;
                self.len = self.len.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// Removes and returns the last item.
    pub fn pop(&mut self) -> Option<T> {
        let last = self.len.checked_sub(1)?;
        self.len = last;
        self.items.get(last).copied()
    }

    /// Removes the item at `index`, shifting later items down.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let removed = self.as_slice().get(index).copied()?;
        let len = self.len;
        // A rotation of an empty slice panics (REQ-GEN-006).
        if let Some(tail) = self.items.get_mut(index..len).filter(|tail| !tail.is_empty()) {
            tail.rotate_left(1);
        }
        self.len = len.saturating_sub(1);
        Some(removed)
    }

    /// Empties the vector.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Number of items.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// True when empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True when no more items fit.
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len >= N
    }

    /// The items as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        self.items.get(..self.len).unwrap_or(&[])
    }

    /// The items as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.len;
        self.items.get_mut(..len).unwrap_or(&mut [])
    }

    /// Shortens the vector to at most `len` items.
    pub fn truncate(&mut self, len: usize) {
        self.len = self.len.min(len);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn magazine_zero_is_eight() {
        assert_eq!(Magazine::from_bits(0).number(), 8);
        assert_eq!(Magazine::from_bits(3).number(), 3);
        assert_eq!(Magazine::new(8).unwrap().bits(), 0);
        assert!(Magazine::new(0).is_none());
        assert!(Magazine::new(9).is_none());
    }

    #[test]
    fn page_addresses_round_trip() {
        let address = PageAddress::from_mpp(0x8FF).unwrap();
        assert_eq!(address.mpp(), 0x8FF);
        assert_eq!(&address.ascii(), b"8FF");
        assert!(!address.page.is_decimal());
        assert!(PageAddress::from_mpp(0x100).unwrap().page.is_decimal());
        assert!(PageAddress::from_mpp(0x900).is_none());
        assert!(PageAddress::from_mpp(0x0A0).is_none());
    }

    #[test]
    fn national_index_follows_e21() {
        // France 2 page 888: C12=1, C13=0, C14=0 gives index 4 (French).
        assert_eq!(ControlBits(1 << 12).national_index(), 4);
        // Italian subtitles: C12=0, C13=1, C14=1 gives index 3.
        assert_eq!(ControlBits((1 << 13) | (1 << 14)).national_index(), 3);
    }

    #[test]
    fn fixed_vec_basics() {
        let mut vec: FixedVec<u8, 3> = FixedVec::new(0);
        assert!(vec.push(1) && vec.push(2) && vec.push(3));
        assert!(!vec.push(4));
        assert_eq!(vec.remove(0), Some(1));
        assert_eq!(vec.as_slice(), &[2, 3]);
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.len(), 1);
    }
}
