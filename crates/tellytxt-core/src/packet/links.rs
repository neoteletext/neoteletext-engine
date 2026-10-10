//! Compositional link packets X/27/4 and X/27/5, Format 1 (clause 9.6.2,
//! Figure 14, Table 15; REQ-PKT-011). Each packet carries six links of two
//! triplets; triplet 13 is reserved. The roles of the links (Table 16) are
//! applied by the object layer (REQ-OBJ-030, decision E.52).

use crate::consts::{
    LINK_FUNCTION_MASK, LINK_MAGAZINE_SHIFT, LINK_MARKER_FIRST_SHIFT, LINK_MARKER_SECOND_MASK, LINK_NULL_PAGE,
    LINK_SUBPAGES_MASK, LINK_SUBPAGES_SHIFT, LINK_TENS_SHIFT, LINK_UNITS_SHIFT, LINK_VALIDITY_MASK,
    LINK_VALIDITY_SHIFT, MAGAZINE_MASK, NIBBLE_MASK,
};
use crate::types::{Magazine, PageNumber};

/// One link of X/27/4 or X/27/5 (Table 15).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct CompositionalLink {
    /// Triplet 1 bits 2 and 1, the link function: 00 GPOP, 01 POP,
    /// 10 GDRCS, 11 DRCS.
    pub function: u8,
    /// Triplet 1 bits 4 and 3, the page validity: 01 Level 2.5 only, 10
    /// Level 3.5 only, 11 both, 00 reserved.
    pub validity: u8,
    /// The linked magazine: the packet's magazine with each bit complemented
    /// where the relative magazine bit (triplet 1 bits 12 to 14) is 1.
    pub magazine: Magazine,
    /// The page number: tens in triplet 1 bits 15 to 18, units in bits 7
    /// to 10.
    pub page: PageNumber,
    /// Triplet 2 bits 3 to 18: bit k set when the sub-page with S1 = k is
    /// required.
    pub subpages: u16,
    /// Triplet 1 bit 11, which Format 1 sets to 1.
    pub marker_first: bool,
    /// Triplet 2 bit 1, which Format 1 sets to 0.
    pub marker_second: bool,
}

impl CompositionalLink {
    /// True when the Format 1 marker bits are as Table 15 requires: a link
    /// is accepted only then (decision E.52).
    #[must_use]
    pub const fn is_format_1(&self) -> bool {
        self.marker_first && !self.marker_second
    }

    /// True for "no link": page validity 00, no sub-page flag set, or page
    /// number FF (decision E.52).
    #[must_use]
    pub const fn is_null(&self) -> bool {
        self.validity == 0 || self.subpages == 0 || self.page.0 == LINK_NULL_PAGE
    }

    /// Decodes a link from its two triplet values in a packet of
    /// `packet_magazine`.
    #[must_use]
    pub const fn from_values(packet_magazine: Magazine, first: u32, second: u32) -> Self {
        let units = (first >> LINK_UNITS_SHIFT) as u8 & NIBBLE_MASK;
        let tens = (first >> LINK_TENS_SHIFT) as u8 & NIBBLE_MASK;
        let relative = (first >> LINK_MAGAZINE_SHIFT) as u8 & MAGAZINE_MASK;
        Self {
            function: (first & LINK_FUNCTION_MASK) as u8,
            validity: ((first >> LINK_VALIDITY_SHIFT) & LINK_VALIDITY_MASK) as u8,
            magazine: Magazine::from_bits(packet_magazine.bits() ^ relative),
            page: PageNumber(tens << 4 | units),
            subpages: ((second >> LINK_SUBPAGES_SHIFT) & LINK_SUBPAGES_MASK) as u16,
            marker_first: (first >> LINK_MARKER_FIRST_SHIFT) & 1 == 1,
            marker_second: second & LINK_MARKER_SECOND_MASK != 0,
        }
    }

    /// The two triplet values of the link in a packet of `packet_magazine`;
    /// reserved bits are 0.
    #[must_use]
    pub const fn to_values(&self, packet_magazine: Magazine) -> (u32, u32) {
        let relative = (packet_magazine.bits() ^ self.magazine.bits()) & MAGAZINE_MASK;
        let first = (self.function as u32 & LINK_FUNCTION_MASK)
            | (self.validity as u32 & LINK_VALIDITY_MASK) << LINK_VALIDITY_SHIFT
            | (self.page.units() as u32) << LINK_UNITS_SHIFT
            | (self.marker_first as u32) << LINK_MARKER_FIRST_SHIFT
            | (relative as u32) << LINK_MAGAZINE_SHIFT
            | (self.page.tens() as u32) << LINK_TENS_SHIFT;
        let second = (self.marker_second as u32) | (self.subpages as u32) << LINK_SUBPAGES_SHIFT;
        (first, second)
    }
}
