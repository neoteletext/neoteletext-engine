//! Packet X/28/0 format 1 made from a page's `RE` and `PF` lines
//! (Tables 4 to 6).

use neoteletext_core::consts::{
    CLUT_ENTRIES, CLUT_LEVEL_25_FIRST, COLOUR_MAP_DEFAULTS, COLOUR_WORDS, DESIGNATION_GROUP_SHIFT,
    DESIGNATION_LATIN_ENGLISH, DESIGNATION_PAGE_DATA, NIBBLE_MASK, NO_SECOND_G0, PACKET_PAGE_ENHANCEMENT,
    PAGE_FUNCTION_BASIC, TRIPLETS_PER_PACKET,
};
use neoteletext_core::packet::{Packet, PageFormat, Triplet, encode};
use neoteletext_core::types::{
    ColourWords, DefaultColourFields, Designations, PresentationFields, Rgb12, SidePanelFields,
};

use crate::consts::NOT_TEXT_PAGE_MARKER;
use crate::page::PageFile;

/// The colour words of CLUTs 2 and 3 in Table 30, as X/28/0 format 1
/// carries them. A page-level packet that only states the page's
/// designation sends these, so that it does not redefine those CLUTs.
#[must_use]
pub fn default_clut_2_and_3() -> ColourWords {
    let mut words = [Rgb12::default(); COLOUR_WORDS];
    let first = usize::from(CLUT_LEVEL_25_FIRST).saturating_mul(CLUT_ENTRIES);
    for (word, &value) in words.iter_mut().zip(COLOUR_MAP_DEFAULTS.iter().skip(first)) {
        *word = Rgb12(value);
    }
    words
}

/// Packet X/28/0 format 1 made from `RE` and `PF` when the page has no
/// explicit `OL,28` line with designation code 0, or `None` when none is
/// needed:
/// - a page function other than "basic Level 1 page" is sent as Table 6
///   (and Table 5) give it: the function and coding, the marker of bits 8
///   to 18, and reserved triplets 2 to 13 as 0;
/// - a basic page with `RE`, or with a page coding other than 000, is sent
///   as Table 4 gives it: the default designation is the region above the
///   page's national option index; the second G0 set is Latin English for
///   neoteletext's own designations, whose second set that is
///   (REQ-CHR-012), and none for the standard's; every other
///   field at its default (no side panels, the CLUT 2 and 3 colours of
///   Table 30, screen and row colour 0, no substitution or re-mapping), so
///   that the packet changes nothing but the designation;
/// - a basic page with neither needs no packet: its absence means a basic
///   Level 1 page coded with odd parity (REQ-ACQ-014).
pub(super) fn page_enhancement_packet(page: &PageFile) -> Option<Packet> {
    let explicit =
        page.raw_packets.as_slice().iter().any(|raw| raw.number == PACKET_PAGE_ENHANCEMENT && raw.designation == 0);
    if explicit {
        return None;
    }
    let function = page.function.unwrap_or_default();
    let format = PageFormat { function: function.function & NIBBLE_MASK, coding: function.coding };
    let magazine = page.address.magazine;
    if format.function != PAGE_FUNCTION_BASIC {
        let mut values = [0u32; TRIPLETS_PER_PACKET];
        if let Some(first) = values.first_mut() {
            *first = format.bits() | NOT_TEXT_PAGE_MARKER;
        }
        let triplets = values.map(Triplet::from_value);
        return Some(encode::triplet_packet(magazine, PACKET_PAGE_ENHANCEMENT, DESIGNATION_PAGE_DATA, &triplets));
    }
    if page.region.is_none() && format.coding == 0 {
        return None;
    }
    let region = page.region.unwrap_or(0) & NIBBLE_MASK;
    let default = (region << DESIGNATION_GROUP_SHIFT) | page.control().national_index();
    let fields = PresentationFields {
        designations: Some(Designations { default, second: second_g0_value(default) }),
        side_panels: Some(SidePanelFields { left: false, right: false, status_flag: false, left_columns: 0 }),
        colour_words: Some(default_clut_2_and_3()),
        colours: Some(DefaultColourFields { screen: 0, row: 0, black_background_substitution: false, remapping: 0 }),
    };
    Some(encode::page_enhancement(magazine, DESIGNATION_PAGE_DATA, format, &fields))
}

/// The Table 33 value to send with a default designation: Latin English
/// where the designation implies it (REQ-CHR-012), so that writing it
/// explicitly keeps the page as it reads without the value.
fn second_g0_value(default: u8) -> u8 {
    if neoteletext_core::charset::default_second_g0(default).is_some() {
        DESIGNATION_LATIN_ENGLISH
    } else {
        NO_SECOND_G0
    }
}
