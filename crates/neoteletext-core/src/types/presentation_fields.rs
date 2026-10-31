//! The field groups that X/28 and M/29 packets and the Magazine Organization
//! Table carry (Tables 4, 10, 11, 13 and 21): colours, designations, side
//! panels, default colours and the fallback defaults.

use crate::consts::{COLOUR_PRIMARY_BITS, COLOUR_WORDS, NIBBLE_MASK};

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

/// The side-panel fields of Table 4, triplet 2 bits 4 to 10, kept only so
/// that the packets round-trip; they have no effect (REQ-1644).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct SidePanelFields {
    /// Bit 4: the left side panel is displayed.
    pub left: bool,
    /// Bit 5: the right side panel is displayed.
    pub right: bool,
    /// Bit 6, the Side Panel Status Flag: 1 when side panels are required
    /// at Levels 2.5 and 3.5, 0 when at Level 3.5 only.
    pub status_flag: bool,
    /// Bits 7 to 10: the number of columns of the left panel, 0 to 15.
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

/// The defaults that the fallback bits of a Magazine Organization Table
/// POP link give (Table 21, byte N+4), in the form of the X/28 fields they
/// stand in for (decision E.53).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct FallbackFields {
    /// The side panels, with the status flag set, kept only so that the
    /// packets round-trip; they have no effect (REQ-1644).
    pub side_panels: SidePanelFields,
    /// Black Background Colour Substitution.
    pub black_background_substitution: bool,
}
