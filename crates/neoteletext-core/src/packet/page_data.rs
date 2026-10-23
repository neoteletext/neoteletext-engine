//! Page-related packets X/28 and magazine-related packets M/29 (clauses
//! 9.4.2 to 9.5.3; Tables 3, 4, 9, 10, 11 and 13; REQ-PKT-006,
//! REQ-PKT-007, REQ-PKT-012, REQ-PKT-015 and REQ-PKT-016). The DRCS
//! packets X/28/1 and M/29/1 are not decoded, and of X/28/3 only the page
//! function is (decision E.95).
//!
//! Several fields span triplets. They are read from a bit stream formed by
//! the thirteen 18-bit triplet values in order, least significant bit
//! first: stream position 18 × t + b − 1 is bit b of triplet t + 1. Every
//! field group carries its own validity (decision E.44): it is `None` when
//! any triplet it lies in was uncorrectable.

use crate::consts::{
    BLACK_SUBSTITUTION_SHIFT, COLOUR_PRIMARY_BITS, COLOUR_WORD_BITS, COLOUR_WORDS, COLOUR_WORDS_START,
    DEFAULT_ROW_COLOUR_SHIFT, DEFAULT_SCREEN_COLOUR_SHIFT, DESIGNATION_DEFAULT_SHIFT, DESIGNATION_MASK,
    DESIGNATION_SECOND_HIGH_MASK, DESIGNATION_SECOND_LOW_BITS, DESIGNATION_SECOND_LOW_SHIFT, NIBBLE_MASK,
    PAGE_CODING_MASK, PAGE_CODING_SHIFT, PAGE_FUNCTION_MASK, REMAPPING_MASK, REMAPPING_SHIFT, SIDE_PANEL_COLUMNS_SHIFT,
    SIDE_PANEL_LEFT_SHIFT, SIDE_PANEL_RIGHT_SHIFT, SIDE_PANEL_STATUS_SHIFT, TRIPLET_COLOUR_INDEX_MASK,
    TRIPLET_DATA_BITS, TRIPLETS_PER_PACKET,
};
use crate::types::{ColourWords, DefaultColourFields, Designations, PresentationFields, Rgb12, SidePanelFields};

/// The thirteen 18-bit triplet values of a packet, `None` where the
/// Hamming 24/18 check failed.
pub type TripletValues = [Option<u32>; TRIPLETS_PER_PACKET];

/// Bits per triplet, as a stream length.
const TRIPLET_STREAM_BITS: usize = TRIPLET_DATA_BITS as usize;
/// Index of triplets 1, 2 and 13 in a [`TripletValues`].
const T1: usize = 0;
const T2: usize = 1;
const T13: usize = 12;

/// Reads `count` bits (at most 32) of the stream from position `start`,
/// the first bit read becoming the least significant. `None` when any
/// triplet the bits lie in was uncorrectable.
#[must_use]
pub fn stream_bits(values: &TripletValues, start: usize, count: usize) -> Option<u32> {
    let mut out = 0u32;
    for offset in 0..count {
        let position = start.saturating_add(offset);
        let value = values.get(position / TRIPLET_STREAM_BITS).copied().flatten()?;
        let bit = value.wrapping_shr((position % TRIPLET_STREAM_BITS) as u32) & 1;
        out |= bit.wrapping_shl(offset as u32);
    }
    Some(out)
}

/// The page function and page coding, triplet 1 bits 1 to 7 (Table 3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PageFormat {
    /// Bits 1 to 4, the page function: 0000 a basic Level 1 page.
    pub function: u8,
    /// Bits 5 to 7, the page coding: 000 seven bits with odd parity.
    pub coding: u8,
}

impl PageFormat {
    /// The fields of a triplet 1 value.
    #[must_use]
    pub const fn from_triplet(value: u32) -> Self {
        Self {
            function: (value & PAGE_FUNCTION_MASK) as u8,
            coding: ((value >> PAGE_CODING_SHIFT) & PAGE_CODING_MASK) as u8,
        }
    }

    /// Bits 1 to 7 of triplet 1.
    #[must_use]
    pub const fn bits(self) -> u32 {
        (self.function as u32 & PAGE_FUNCTION_MASK) | (self.coding as u32 & PAGE_CODING_MASK) << PAGE_CODING_SHIFT
    }
}

/// Packet X/28/0 format 1 or X/28/4 (Tables 4 and 10). Decision E.44 reads
/// X/28/0 as format 1 on every displayed page.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct PageEnhancement {
    /// The page function and coding; `None` when triplet 1 was
    /// uncorrectable.
    pub format: Option<PageFormat>,
    /// The other field groups. For X/28/4 the colour words are CLUT 0
    /// entries 0 to 7 then CLUT 1 entries 0 to 7, and the word for CLUT 1
    /// entry 0 is to be ignored (REQ-PKT-015).
    pub fields: PresentationFields,
}

fn bit(value: u32, shift: u32) -> bool {
    value.wrapping_shr(shift) & 1 == 1
}

/// The field groups shared by X/28/0 format 1, X/28/4, M/29/0 and M/29/4
/// (Table 4, REQ-PKT-012).
#[must_use]
pub fn presentation_fields(values: &TripletValues) -> PresentationFields {
    let first = values.get(T1).copied().flatten();
    let second = values.get(T2).copied().flatten();
    let last = values.get(T13).copied().flatten();
    let designations = match (first, second) {
        (Some(first), Some(second)) => Some(Designations {
            default: ((first >> DESIGNATION_DEFAULT_SHIFT) & DESIGNATION_MASK) as u8,
            second: (((first >> DESIGNATION_SECOND_LOW_SHIFT) & u32::from(NIBBLE_MASK))
                | (second & DESIGNATION_SECOND_HIGH_MASK) << DESIGNATION_SECOND_LOW_BITS) as u8,
        }),
        _ => None,
    };
    let side_panels = second.map(|second| SidePanelFields {
        left: bit(second, SIDE_PANEL_LEFT_SHIFT),
        right: bit(second, SIDE_PANEL_RIGHT_SHIFT),
        status_flag: bit(second, SIDE_PANEL_STATUS_SHIFT),
        left_columns: (second >> SIDE_PANEL_COLUMNS_SHIFT) as u8 & NIBBLE_MASK,
    });
    let colours = last.map(|last| DefaultColourFields {
        screen: (last >> DEFAULT_SCREEN_COLOUR_SHIFT) as u8 & TRIPLET_COLOUR_INDEX_MASK,
        row: (last >> DEFAULT_ROW_COLOUR_SHIFT) as u8 & TRIPLET_COLOUR_INDEX_MASK,
        black_background_substitution: bit(last, BLACK_SUBSTITUTION_SHIFT),
        remapping: ((last >> REMAPPING_SHIFT) & REMAPPING_MASK) as u8,
    });
    PresentationFields { designations, side_panels, colour_words: colour_words(values), colours }
}

/// The sixteen colour words: each is red, green and blue, four bits each,
/// each group least significant bit first (Table 4).
fn colour_words(values: &TripletValues) -> Option<ColourWords> {
    let mut words = [Rgb12::default(); COLOUR_WORDS];
    for (index, word) in words.iter_mut().enumerate() {
        let start = COLOUR_WORDS_START.saturating_add(index.saturating_mul(COLOUR_WORD_BITS));
        let bits = stream_bits(values, start, COLOUR_WORD_BITS)?;
        let primary = |shift: u32| (bits.wrapping_shr(shift) as u8) & NIBBLE_MASK;
        *word = Rgb12::new(primary(0), primary(COLOUR_PRIMARY_BITS), primary(2 * COLOUR_PRIMARY_BITS));
    }
    Some(words)
}

/// X/28/0 format 1 or X/28/4.
#[must_use]
pub fn page_enhancement(values: &TripletValues) -> PageEnhancement {
    PageEnhancement {
        format: values.get(T1).copied().flatten().map(PageFormat::from_triplet),
        fields: presentation_fields(values),
    }
}

/// The page function and coding of X/28/3, the only part of it decoded
/// (Table 9, REQ-PKT-014, decision E.95): a page that declares itself a pattern
/// download page this way is still never shown as text.
#[must_use]
pub fn page_function(values: &TripletValues) -> Option<PageFormat> {
    values.get(T1).copied().flatten().map(PageFormat::from_triplet)
}

/// Encoders for the packets above: the inverse of each decoder, as 18-bit
/// triplet values.
pub(super) mod encode {
    use super::{T1, T2, T13, set_stream_bits};
    use crate::consts::{
        BLACK_SUBSTITUTION_SHIFT, COLOUR_PRIMARY_BITS, COLOUR_WORD_BITS, COLOUR_WORDS_START, DEFAULT_ROW_COLOUR_SHIFT,
        DEFAULT_SCREEN_COLOUR_SHIFT, DESIGNATION_DEFAULT_SHIFT, DESIGNATION_DOWNLOAD_FUNCTION, DESIGNATION_MASK,
        DESIGNATION_SECOND_HIGH_MASK, DESIGNATION_SECOND_LOW_BITS, DESIGNATION_SECOND_LOW_SHIFT, NIBBLE_MASK,
        PACKET_MAGAZINE_ENHANCEMENT, PACKET_PAGE_ENHANCEMENT, REMAPPING_MASK, REMAPPING_SHIFT,
        SIDE_PANEL_COLUMNS_SHIFT, SIDE_PANEL_LEFT_SHIFT, SIDE_PANEL_RIGHT_SHIFT, SIDE_PANEL_STATUS_SHIFT,
        TRIPLET_COLOUR_INDEX_MASK, TRIPLETS_PER_PACKET,
    };
    use crate::types::{Magazine, PresentationFields};

    use super::super::triplets::encode::value_packet;
    use super::super::{Packet, PageFormat, Triplet};

    /// Triplet values for the shared field groups; absent groups are 0.
    #[must_use]
    pub fn presentation_values(first_bits: u32, fields: &PresentationFields) -> [u32; TRIPLETS_PER_PACKET] {
        let mut values = [0u32; TRIPLETS_PER_PACKET];
        let mut first = first_bits;
        let mut second = 0u32;
        if let Some(designations) = fields.designations {
            let second_designation = u32::from(designations.second);
            first |= (u32::from(designations.default) & DESIGNATION_MASK) << DESIGNATION_DEFAULT_SHIFT
                | (second_designation & u32::from(NIBBLE_MASK)) << DESIGNATION_SECOND_LOW_SHIFT;
            second |= (second_designation >> DESIGNATION_SECOND_LOW_BITS) & DESIGNATION_SECOND_HIGH_MASK;
        }
        if let Some(panels) = fields.side_panels {
            second |= u32::from(panels.left) << SIDE_PANEL_LEFT_SHIFT
                | u32::from(panels.right) << SIDE_PANEL_RIGHT_SHIFT
                | u32::from(panels.status_flag) << SIDE_PANEL_STATUS_SHIFT
                | u32::from(panels.left_columns & NIBBLE_MASK) << SIDE_PANEL_COLUMNS_SHIFT;
        }
        if let Some(slot) = values.get_mut(T1) {
            *slot = first;
        }
        if let Some(slot) = values.get_mut(T2) {
            *slot = second;
        }
        if let Some(words) = fields.colour_words {
            for (index, word) in words.iter().enumerate() {
                let bits = u32::from(word.red())
                    | u32::from(word.green()) << COLOUR_PRIMARY_BITS
                    | u32::from(word.blue()) << (2 * COLOUR_PRIMARY_BITS);
                let start = COLOUR_WORDS_START.saturating_add(index.saturating_mul(COLOUR_WORD_BITS));
                set_stream_bits(&mut values, start, COLOUR_WORD_BITS, bits);
            }
        }
        if let Some(colours) = fields.colours
            && let Some(last) = values.get_mut(T13)
        {
            *last |= u32::from(colours.screen & TRIPLET_COLOUR_INDEX_MASK) << DEFAULT_SCREEN_COLOUR_SHIFT
                | u32::from(colours.row & TRIPLET_COLOUR_INDEX_MASK) << DEFAULT_ROW_COLOUR_SHIFT
                | u32::from(colours.black_background_substitution) << BLACK_SUBSTITUTION_SHIFT
                | (u32::from(colours.remapping) & REMAPPING_MASK) << REMAPPING_SHIFT;
        }
        values
    }

    /// Triplets 1 and 2 of X/28/0 format 1 or M/29/0 for a basic Level 1
    /// page: page function 0, page coding 0 (Table 4).
    #[must_use]
    pub fn designation_triplets(default_designation: u8, second_designation: u8) -> [Triplet; 2] {
        let first = (u32::from(default_designation & 0x7F) << 7) | (u32::from(second_designation & 0xF) << 14);
        let second = u32::from((second_designation >> 4) & 0x7);
        [Triplet::from_value(first), Triplet::from_value(second)]
    }

    /// Packet X/28/0 format 1 (designation 0) or X/28/4 (designation 4)
    /// (Tables 4 and 10). Absent field groups are written as 0.
    #[must_use]
    pub fn page_enhancement(
        magazine: Magazine,
        designation: u8,
        format: PageFormat,
        fields: &PresentationFields,
    ) -> Packet {
        value_packet(magazine, PACKET_PAGE_ENHANCEMENT, designation, &presentation_values(format.bits(), fields))
    }

    /// Packet M/29/0 (designation 0) or M/29/4 (designation 4) with packet
    /// function 0000000 (Tables 11 and 13).
    #[must_use]
    pub fn magazine_enhancement(magazine: Magazine, designation: u8, fields: &PresentationFields) -> Packet {
        value_packet(magazine, PACKET_MAGAZINE_ENHANCEMENT, designation, &presentation_values(0, fields))
    }

    /// Packet X/28/3 with the page function and coding only (Table 9), as
    /// a pattern download page sends it.
    #[must_use]
    pub fn page_function(magazine: Magazine, format: PageFormat) -> Packet {
        let mut values = [0; TRIPLETS_PER_PACKET];
        if let Some(first) = values.first_mut() {
            *first = format.bits();
        }
        value_packet(magazine, PACKET_PAGE_ENHANCEMENT, DESIGNATION_DOWNLOAD_FUNCTION, &values)
    }
}

/// Writes the low `count` bits of `bits` into the stream from `start`.
fn set_stream_bits(values: &mut [u32; TRIPLETS_PER_PACKET], start: usize, count: usize, bits: u32) {
    for offset in 0..count {
        let position = start.saturating_add(offset);
        if let Some(value) = values.get_mut(position / TRIPLET_STREAM_BITS) {
            let mask = 1u32.wrapping_shl((position % TRIPLET_STREAM_BITS) as u32);
            if bits.wrapping_shr(offset as u32) & 1 == 1 {
                *value |= mask;
            } else {
                *value &= !mask;
            }
        }
    }
}
