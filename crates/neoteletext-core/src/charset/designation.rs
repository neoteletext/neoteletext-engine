//! Designation (Tables 32, 33 and 34): the G0 and G2 sets a Table 32 or 33
//! value designates, and the Modified G0 and G2 designations of Table 34.

use super::code_table::{ENGLISH_DESIGNATION, NATIONAL_BITS_MASK};
#[cfg(feature = "extended-sets")]
use super::extended_designations::{EXTENDED_SECOND, EXTENDED_TABLE_32};
use super::{Designation, G0, G0Set, G2Set, NationalOption};
use crate::consts::DESIGNATION_GROUP_SHIFT as GROUP_SHIFT;
use crate::consts::NO_SECOND_G0;

/// One defined entry of Table 32: group (bits 3 to 6), national option
/// index (bits 0 to 2), the G0 set with its national option, and the G2 set.
pub(super) type Table32Entry = (u8, u8, G0, G2Set);

pub(super) const fn latin(national: NationalOption) -> G0 {
    G0 { set: G0Set::Latin, national: Some(national) }
}

/// Table 32 note 3: the bracketed options have no sub-set of their own.
pub(super) const fn whole(set: G0Set) -> G0 {
    G0 { set, national: None }
}

/// Table 32. Every entry not listed is reserved (note 2). Table 33 has the
/// same G0 column for every value, so it reads the same entries.
const TABLE_32: [Table32Entry; 36] = {
    use G0Set as S;
    use G2Set::{Arabic, Cyrillic, Greek, Latin};
    use NationalOption as N;
    [
        // Group 0000: Western European.
        (0b0000, 0b000, latin(N::English), Latin),
        (0b0000, 0b001, latin(N::German), Latin),
        (0b0000, 0b010, latin(N::SwedishFinnishHungarian), Latin),
        (0b0000, 0b011, latin(N::Italian), Latin),
        (0b0000, 0b100, latin(N::French), Latin),
        (0b0000, 0b101, latin(N::PortugueseSpanish), Latin),
        (0b0000, 0b110, latin(N::CzechSlovak), Latin),
        // Group 0001: Central European.
        (0b0001, 0b000, latin(N::Polish), Latin),
        (0b0001, 0b001, latin(N::German), Latin),
        (0b0001, 0b010, latin(N::SwedishFinnishHungarian), Latin),
        (0b0001, 0b011, latin(N::Italian), Latin),
        (0b0001, 0b100, latin(N::French), Latin),
        (0b0001, 0b110, latin(N::CzechSlovak), Latin),
        // Group 0010: Southern European.
        (0b0010, 0b000, latin(N::English), Latin),
        (0b0010, 0b001, latin(N::German), Latin),
        (0b0010, 0b010, latin(N::SwedishFinnishHungarian), Latin),
        (0b0010, 0b011, latin(N::Italian), Latin),
        (0b0010, 0b100, latin(N::French), Latin),
        (0b0010, 0b101, latin(N::PortugueseSpanish), Latin),
        (0b0010, 0b110, latin(N::Turkish), Latin),
        // Group 0011: South-East European.
        (0b0011, 0b101, latin(N::SerbianCroatianSlovenian), Latin),
        (0b0011, 0b111, latin(N::Rumanian), Latin),
        // Group 0100: Cyrillic and Baltic.
        (0b0100, 0b000, whole(S::Cyrillic1), Cyrillic),
        (0b0100, 0b001, latin(N::German), Latin),
        (0b0100, 0b010, latin(N::Estonian), Latin),
        (0b0100, 0b011, latin(N::LettishLithuanian), Latin),
        (0b0100, 0b100, whole(S::Cyrillic2), Cyrillic),
        (0b0100, 0b101, whole(S::Cyrillic3), Cyrillic),
        (0b0100, 0b110, latin(N::CzechSlovak), Latin),
        // Group 0110: Turkish and Greek.
        (0b0110, 0b110, latin(N::Turkish), Latin),
        (0b0110, 0b111, whole(S::Greek), Greek),
        // Group 1000: Latin with the Arabic G2 set, and Arabic.
        (0b1000, 0b000, latin(N::English), Arabic),
        (0b1000, 0b100, latin(N::French), Arabic),
        (0b1000, 0b111, whole(S::Arabic), Arabic),
        // Group 1010: Hebrew and Arabic.
        (0b1010, 0b101, whole(S::Hebrew), Arabic),
        (0b1010, 0b111, whole(S::Arabic), Arabic),
    ]
};

#[cfg(not(feature = "extended-sets"))]
const EXTENDED_SECOND: [(u8, G0); 0] = [];

#[cfg(not(feature = "extended-sets"))]
const EXTENDED_TABLE_32: [Table32Entry; 0] = [];

fn find_entry(table: &[Table32Entry], value: u8) -> Option<(G0, G2Set)> {
    let group = value.wrapping_shr(GROUP_SHIFT);
    let index = value & NATIONAL_BITS_MASK;
    table
        .iter()
        .find(|&&(entry_group, entry_index, _, _)| (entry_group, entry_index) == (group, index))
        .map(|&(_, _, g0, g2)| (g0, g2))
}

fn table_32_entry(value: u8) -> Option<(G0, G2Set)> {
    find_entry(&TABLE_32, value).or_else(|| find_entry(&EXTENDED_TABLE_32, value))
}

/// The second G0 set of one of neoteletext's own designations when no
/// Table 33 designation is received: Latin with the English option, so
/// that ESC reaches English text (REQ-CHR-012, decision E.78 item 4).
pub(super) fn default_second_g0(value: u8) -> Option<G0> {
    let second = EXTENDED_SECOND.iter().find(|(entry, _)| *entry == value).map_or(ENGLISH_DESIGNATION.g0, |(_, g0)| *g0);
    find_entry(&EXTENDED_TABLE_32, value).map(|_| second)
}

pub(super) fn designation(value: u8) -> Option<Designation> {
    table_32_entry(value).map(|(g0, g2)| Designation { g0, g2 })
}

/// Table 34, the Modified G0 and G2 Character Set Designation triplet:
/// group (data bits 18 to 15) and the low three bits, `None` for "any",
/// with the G0 and G2 sets. Entries not listed are reserved (its note).
/// No national option applies to a G0 set reached by a triplet (clause
/// 15.4 note), so the Latin rows name only the set.
const TABLE_34: [(u8, Option<u8>, G0Set, G2Set); 18] = {
    use G0Set as S;
    use G2Set::{Arabic, Cyrillic, Greek, Latin};
    [
        (0b0000, None,        S::Latin,     Latin),
        (0b0001, None,        S::Latin,     Latin),
        (0b0010, None,        S::Latin,     Latin),
        (0b0011, None,        S::Latin,     Latin),
        (0b0100, Some(0b000), S::Cyrillic1, Cyrillic),
        (0b0100, Some(0b001), S::Latin,     Latin),
        (0b0100, Some(0b010), S::Latin,     Latin),
        (0b0100, Some(0b011), S::Latin,     Latin),
        (0b0100, Some(0b100), S::Cyrillic2, Cyrillic),
        (0b0100, Some(0b101), S::Cyrillic3, Cyrillic),
        (0b0100, Some(0b110), S::Latin,     Latin),
        (0b0110, Some(0b110), S::Latin,     Latin),
        (0b0110, Some(0b111), S::Greek,     Greek),
        (0b1000, Some(0b000), S::Latin,     Arabic),
        (0b1000, Some(0b100), S::Latin,     Arabic),
        (0b1000, Some(0b111), S::Arabic,    Arabic),
        (0b1010, Some(0b101), S::Hebrew,    Arabic),
        (0b1010, Some(0b111), S::Arabic,    Arabic),
    ]
};

pub(super) fn modified_designation(value: u8) -> Option<(G0Set, G2Set)> {
    let group = value.wrapping_shr(GROUP_SHIFT);
    let index = value & NATIONAL_BITS_MASK;
    TABLE_34
        .iter()
        .find(|&&(entry_group, entry_index, _, _)| entry_group == group && entry_index.is_none_or(|low| low == index))
        .map(|&(_, _, g0, g2)| (g0, g2))
        // Table 34 accepts neoteletext's own values as Table 32 does
        // (REQ-CHR-012); a triplet's G0 set takes no national option.
        .or_else(|| find_entry(&EXTENDED_TABLE_32, value).map(|(g0, g2)| (g0.set, g2)))
}

pub(super) fn second_g0(value: u8) -> Option<G0> {
    if value == NO_SECOND_G0 {
        return None;
    }
    // Table 33 lists the same G0 sets and options as Table 32 for every
    // value; only the G2 column is absent (clause 15.3).
    table_32_entry(value).map(|(g0, _)| g0)
}
