//! The standard's organisation of character sets, for the font browser
//! (REQ-BRW-010, REQ-BRW-011, decision E.75): the entries of Table 32 by
//! group, each a G0 set with its national option and the G2 set paired
//! with it, and the names the browser shows for groups and sets.

use neoteletext_core::charset::{CharSet, designation};
use neoteletext_core::consts::DESIGNATION_GROUP_SHIFT;
use neoteletext_core::glyphs::conformance::font_set_of;

/// The highest seven-bit Table 32 value.
const LAST_VALUE: u8 = 0x7F;

/// The browser's name for each set of `FONT_SETS`, in its order.
pub const SET_LABELS: [&str; 24] = [
    "G0 Latin",
    "G0 Latin, English",
    "G0 Latin, German",
    "G0 Latin, Swedish/Finnish/Hungarian",
    "G0 Latin, Italian",
    "G0 Latin, French",
    "G0 Latin, Portuguese/Spanish",
    "G0 Latin, Czech/Slovak",
    "G0 Latin, Polish",
    "G0 Latin, Turkish",
    "G0 Latin, Serbian/Croatian/Slovenian",
    "G0 Latin, Rumanian",
    "G0 Latin, Estonian",
    "G0 Latin, Lettish/Lithuanian",
    "G0 Cyrillic 1, Serbian/Croatian",
    "G0 Cyrillic 2, Russian/Bulgarian",
    "G0 Cyrillic 3, Ukrainian",
    "G0 Greek",
    "G0 Arabic",
    "G0 Hebrew",
    "G2 Latin",
    "G2 Cyrillic",
    "G2 Greek",
    "G2 Arabic",
];

/// The browser's name for a Table 32 group, by the sets its entries name.
#[must_use]
pub const fn group_name(group: u8) -> &'static str {
    match group {
        0b0000 => "West European",
        0b0001 => "West European with Polish",
        0b0010 => "West European with Turkish",
        0b0011 => "Serbian/Croatian/Slovenian and Rumanian",
        0b0100 => "Cyrillic and Baltic",
        0b0110 => "Greek and Turkish",
        0b1000 => "Arabic with English and French",
        0b1010 => "Hebrew and Arabic",
        _ => "reserved",
    }
}

/// The browser's name for a set, by its place in `FONT_SETS`.
#[must_use]
pub fn set_label(place: usize) -> &'static str {
    SET_LABELS.get(place).copied().unwrap_or("unknown set")
}

/// One entry of Table 32 that is not reserved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    /// The four-bit group.
    pub group: u8,
    /// The three-bit national option index.
    pub national: u8,
    /// The G0 set with its national option, by its place in `FONT_SETS`.
    pub g0: usize,
    /// The paired G2 set, by its place in `FONT_SETS`.
    pub g2: usize,
}

/// Every entry of Table 32 that is not reserved, in value order, from the
/// core's own table.
#[must_use]
pub fn entries() -> Vec<Entry> {
    (0..=LAST_VALUE)
        .filter_map(|value| {
            let found = designation(value)?;
            Some(Entry {
                group: value >> DESIGNATION_GROUP_SHIFT,
                national: value & ((1 << DESIGNATION_GROUP_SHIFT) - 1),
                g0: font_set_of(CharSet::G0(found.g0))?,
                g2: font_set_of(CharSet::G2(found.g2))?,
            })
        })
        .collect()
}

/// The groups of Table 32 with at least one entry, in order.
#[must_use]
pub fn groups() -> Vec<u8> {
    let mut groups: Vec<u8> = entries().iter().map(|entry| entry.group).collect();
    groups.dedup();
    groups
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use neoteletext_core::glyphs::conformance::FONT_SETS;

    /// A name for every set, in the order of `FONT_SETS`.
    #[test]
    fn every_set_is_named() {
        assert_eq!(SET_LABELS.len(), FONT_SETS.len());
        for (place, (name, _)) in FONT_SETS.iter().enumerate() {
            let label = set_label(place);
            assert_eq!(&name[..2], &label[..2], "{name} and {label}");
        }
    }

    /// Table 32 as the core has it: 36 entries in 8 groups, each named,
    /// Hebrew paired with G2 Arabic.
    #[test]
    fn table_32() {
        let all = entries();
        assert_eq!(all.len(), 36);
        assert_eq!(groups(), [0b0000, 0b0001, 0b0010, 0b0011, 0b0100, 0b0110, 0b1000, 0b1010]);
        assert!(groups().iter().all(|&group| group_name(group) != "reserved"));
        let hebrew = all.iter().find(|entry| set_label(entry.g0) == "G0 Hebrew").unwrap();
        assert_eq!(set_label(hebrew.g2), "G2 Arabic");
        let first = all[0];
        assert_eq!((first.group, first.national, set_label(first.g0)), (0, 0, "G0 Latin, English"));
    }
}
