//! Tests of the character set tables of clause 15.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see `charset/tests.rs`); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use super::super::code_table::{ENGLISH_DESIGNATION, NATIONAL_BITS_MASK, base_unicode};
use super::super::composition::{COMPOSITIONS, EXTENDED_COMPOSITIONS, search};
use super::super::diacritics::{COMBINING_MARKS, diacritic_combining};
use super::super::mosaics::{FULL_BLOCK, MOSAIC_LEFT_COLUMN, MOSAIC_RIGHT_COLUMN, mosaic_unicode};
use crate::consts::{DESIGNATION_GROUP_SHIFT as GROUP_SHIFT, MOSAIC_FULL, NATIONAL_INDEX_MASK, NATIONAL_POSITIONS};
use std::collections::BTreeSet;
use std::vec::Vec;

const ALL_CODES: core::ops::RangeInclusive<u8> = 0x20..=0x7F;

const fn latin(national: Option<NationalOption>) -> CharSet {
    CharSet::G0(G0 { set: G0Set::Latin, national })
}

fn g0(set: G0Set) -> CharSet {
    CharSet::G0(G0 { set, national: None })
}

/// The text of a set's 96 characters, for whole-row comparisons.
fn row(set: CharSet, first: u8) -> std::string::String {
    (first..first + 16).map(|code| base_unicode(set, code)).collect()
}

// --- Table 36 -----------------------------------------------------------

/// Each option's 13 characters, transcribed again as one string in the
/// position order 2/3 2/4 4/0 5/B 5/C 5/D 5/E 5/F 6/0 7/B 7/C 7/D 7/E.
const TABLE_36: [(NationalOption, &str); 13] = [
    (NationalOption::CzechSlovak, "#ůčťžýířéáěúš"),
    (NationalOption::English, "£$@←½→↑#—¼‖¾÷"),
    (NationalOption::Estonian, "#õŠÄÖŽÜÕšäöžü"),
    (NationalOption::French, "éïàëêùî#èâôûç"),
    (NationalOption::German, "#$§ÄÖÜ^_°äöüß"),
    (NationalOption::Italian, "£$é°ç→↑#ùàòèì"),
    (NationalOption::LettishLithuanian, "#$ŠėęŽčūšąųžį"),
    (NationalOption::Polish, "#ńąƵŚŁćóężśłź"),
    (NationalOption::PortugueseSpanish, "ç$¡áéíóú¿üñèà"),
    (NationalOption::Rumanian, "#¤ŢÂŞĂÎıţâşăî"),
    (NationalOption::SerbianCroatianSlovenian, "#ËČĆŽĐŠëčćžđš"),
    (NationalOption::SwedishFinnishHungarian, "#¤ÉÄÖÅÜ_éäöåü"),
    (NationalOption::Turkish, "₺ğİŞÖÇÜĞışöçü"),
];

#[test]
fn every_national_option_matches_table_36() {
    assert_eq!(TABLE_36.len(), NationalOption::ALL.len());
    for (option, expected) in TABLE_36 {
        let actual: std::string::String = NATIONAL_POSITIONS
            .iter()
            .map(|&code| base_unicode(latin(Some(option)), code))
            .collect();
        assert_eq!(actual, expected, "{option:?}");
    }
}

#[test]
fn project_decided_national_glyphs() {
    let english = latin(Some(NationalOption::English));
    let polish = latin(Some(NationalOption::Polish));
    assert_eq!(base_unicode(polish, 0x5B), '\u{01B5}');
    assert_eq!(base_unicode(polish, 0x7D), '\u{0142}');
    assert_eq!(base_unicode(english, 0x60), '\u{2014}');
    assert_eq!(base_unicode(english, 0x7C), '\u{2016}');
    assert_eq!(base_unicode(latin(Some(NationalOption::Turkish)), 0x23), '\u{20BA}');
}

#[test]
fn national_option_leaves_other_positions_alone() {
    for option in NationalOption::ALL {
        for code in ALL_CODES.filter(|code| !NATIONAL_POSITIONS.contains(code)) {
            assert_eq!(
                base_unicode(latin(Some(option)), code),
                base_unicode(latin(None), code),
                "{option:?} {code:#04x}"
            );
        }
    }
}

#[test]
fn latin_g0_base_forms() {
    let base: std::string::String =
        NATIONAL_POSITIONS.iter().map(|&code| base_unicode(latin(None), code)).collect();
    assert_eq!(base, "#¤@[\\]^_`{|}~");
    assert_eq!(row(latin(None), 0x40), "@ABCDEFGHIJKLMNO");
    assert_eq!(row(latin(None), 0x70), "pqrstuvwxyz{|}~■");
    assert_eq!(base_unicode(latin(None), 0x20), ' ');
}

// --- Tables 32 and 33 -------------------------------------------------------

/// A Table 32 or 33 value from its group and national option index.
fn value(group: u8, index: u8) -> u8 {
    group << GROUP_SHIFT | index
}

fn latin_g0(national: NationalOption) -> G0 {
    G0 { set: G0Set::Latin, national: Some(national) }
}

fn whole_g0(set: G0Set) -> G0 {
    G0 { set, national: None }
}

/// Every defined entry of Table 32: (group, index, G0, G2).
fn table_32_defined() -> Vec<(u8, u8, G0, G2Set)> {
    use NationalOption as N;
    let l = |group, index, national| (group, index, latin_g0(national), G2Set::Latin);
    std::vec![
        l(0b0000, 0, N::English),
        l(0b0000, 1, N::German),
        l(0b0000, 2, N::SwedishFinnishHungarian),
        l(0b0000, 3, N::Italian),
        l(0b0000, 4, N::French),
        l(0b0000, 5, N::PortugueseSpanish),
        l(0b0000, 6, N::CzechSlovak),
        l(0b0001, 0, N::Polish),
        l(0b0001, 1, N::German),
        l(0b0001, 2, N::SwedishFinnishHungarian),
        l(0b0001, 3, N::Italian),
        l(0b0001, 4, N::French),
        l(0b0001, 6, N::CzechSlovak),
        l(0b0010, 0, N::English),
        l(0b0010, 1, N::German),
        l(0b0010, 2, N::SwedishFinnishHungarian),
        l(0b0010, 3, N::Italian),
        l(0b0010, 4, N::French),
        l(0b0010, 5, N::PortugueseSpanish),
        l(0b0010, 6, N::Turkish),
        l(0b0011, 5, N::SerbianCroatianSlovenian),
        l(0b0011, 7, N::Rumanian),
        (0b0100, 0, whole_g0(G0Set::Cyrillic1), G2Set::Cyrillic),
        l(0b0100, 1, N::German),
        l(0b0100, 2, N::Estonian),
        l(0b0100, 3, N::LettishLithuanian),
        (0b0100, 4, whole_g0(G0Set::Cyrillic2), G2Set::Cyrillic),
        (0b0100, 5, whole_g0(G0Set::Cyrillic3), G2Set::Cyrillic),
        l(0b0100, 6, N::CzechSlovak),
        l(0b0110, 6, N::Turkish),
        (0b0110, 7, whole_g0(G0Set::Greek), G2Set::Greek),
        (0b1000, 0, latin_g0(N::English), G2Set::Arabic),
        (0b1000, 4, latin_g0(N::French), G2Set::Arabic),
        (0b1000, 7, whole_g0(G0Set::Arabic), G2Set::Arabic),
        (0b1010, 5, whole_g0(G0Set::Hebrew), G2Set::Arabic),
        (0b1010, 7, whole_g0(G0Set::Arabic), G2Set::Arabic),
    ]
}

/// REQ-CHR-012's table, written out again from the requirement.
fn extended_defined() -> Vec<(u8, u8, G0, G2Set)> {
    use NationalOption as N;
    if !cfg!(feature = "extended-sets") {
        return Vec::new();
    }
    let l = |index, national| (0b0101, index, latin_g0(national), G2Set::Latin);
    std::vec![
        l(0, N::HausaFula),
        l(1, N::WestAfrican),
        l(2, N::Ewe),
        l(3, N::KabyleTamazight),
        l(4, N::Azerbaijani),
        l(5, N::KanuriMossi),
        (0b0101, 6, whole_g0(G0Set::YorubaIgbo), G2Set::Latin),
        (0b0101, 7, whole_g0(G0Set::Vietnamese), G2Set::Vietnamese),
        (0b0111, 0, whole_g0(G0Set::Ukrainian), G2Set::Cyrillic),
        (0b0111, 1, whole_g0(G0Set::Belarusian), G2Set::Cyrillic),
        (0b0111, 2, whole_g0(G0Set::Macedonian), G2Set::Cyrillic),
        (0b0111, 3, whole_g0(G0Set::Serbian), G2Set::Cyrillic),
        (0b0111, 5, whole_g0(G0Set::KyrgyzMongolian), G2Set::Cyrillic),
        (0b0111, 6, whole_g0(G0Set::Tatar), G2Set::Cyrillic),
        (0b0111, 7, whole_g0(G0Set::Tajik), G2Set::Cyrillic),
        (0b1001, 0, whole_g0(G0Set::Persian), G2Set::Arabic),
        (0b1001, 1, whole_g0(G0Set::Urdu), G2Set::Arabic),
        (0b1001, 2, whole_g0(G0Set::Pashto), G2Set::Arabic),
        (0b1001, 3, whole_g0(G0Set::Sindhi), G2Set::Arabic),
        (0b1001, 4, whole_g0(G0Set::Sorani), G2Set::Arabic),
        (0b1001, 5, whole_g0(G0Set::Uyghur), G2Set::Arabic),
        (0b1001, 6, whole_g0(G0Set::Balochi), G2Set::Arabic),
        (0b1001, 7, whole_g0(G0Set::ArabicPlain), G2Set::Arabic),
        (0b1011, 0, whole_g0(G0Set::Georgian), G2Set::Latin),
        (0b1011, 1, whole_g0(G0Set::Armenian), G2Set::Armenian),
        (0b1011, 2, whole_g0(G0Set::Tifinagh), G2Set::Latin),
        (0b1100, 0, latin_g0(N::English), G2Set::Thai),
        (0b1100, 2, whole_g0(G0Set::Thai), G2Set::Thai),
        (0b1100, 4, whole_g0(G0Set::Lao), G2Set::Lao),
    ]
}

/// REQ-CHR-012: Table 34 gives neoteletext's own values' sets, and each
/// implies Latin English as the second set; the standard's none.
#[test]
fn extended_values_in_tables_33_and_34() {
    for (group, index, g0, g2) in extended_defined() {
        assert_eq!(modified_designation(value(group, index)), Some((g0.set, g2)));
        let second = if (group, index) == (0b1100, 0) { whole_g0(G0Set::Thai) } else { ENGLISH_DESIGNATION.g0 };
        assert_eq!(default_second_g0(value(group, index)), Some(second), "{group:04b} {index:03b}");
    }
    for (group, index, _, _) in table_32_defined() {
        assert_eq!(default_second_g0(value(group, index)), None);
    }
    let reserved = value(0b0111, 0b100);
    assert_eq!((designation(reserved), modified_designation(reserved), default_second_g0(reserved)), (None, None, None));
}

#[test]
fn table_32_every_entry() {
    let mut defined = table_32_defined();
    defined.extend(extended_defined());
    let mut seen = 0;
    for candidate in 0..=0x7F_u8 {
        let expected = defined
            .iter()
            .find(|(group, index, _, _)| value(*group, *index) == candidate)
            .map(|&(_, _, g0, g2)| Designation { g0, g2 });
        assert_eq!(designation(candidate), expected, "value {candidate:#09b}");
        seen += usize::from(expected.is_some());
    }
    assert_eq!(seen, defined.len());
}

#[test]
fn table_32_decision_e21_examples() {
    // France 2: C12 = 1, C13 = 0, C14 = 0 gives index 4.
    let french = designation(0b100).unwrap();
    assert_eq!(french.g0, latin_g0(NationalOption::French));
    // Italian: C12 = 0, C13 = 1, C14 = 1 gives index 3.
    let italian = designation(0b011).unwrap();
    assert_eq!(italian.g0, latin_g0(NationalOption::Italian));
    assert_eq!(designation(0).unwrap(), ENGLISH_DESIGNATION);
    assert_eq!(designation(1).unwrap().g0, latin_g0(NationalOption::German));
    assert_eq!(designation(value(0b0000, 0b111)), None);
    assert_eq!(designation(value(0b1101, 0b000)), None);
    assert_eq!(designation(0x7F), None);
}

#[test]
fn table_33_spot_checks() {
    assert_eq!(second_g0(0x7F), None);
    assert_eq!(second_g0(value(0b0100, 0b000)), Some(whole_g0(G0Set::Cyrillic1)));
    assert_eq!(second_g0(value(0b0100, 0b100)), Some(whole_g0(G0Set::Cyrillic2)));
    assert_eq!(second_g0(value(0b0110, 0b111)), Some(whole_g0(G0Set::Greek)));
    assert_eq!(second_g0(value(0b1000, 0b111)), Some(whole_g0(G0Set::Arabic)));
    assert_eq!(second_g0(value(0b1010, 0b101)), Some(whole_g0(G0Set::Hebrew)));
    assert_eq!(second_g0(value(0b0001, 0b000)), Some(latin_g0(NationalOption::Polish)));
    assert_eq!(second_g0(value(0b0010, 0b110)), Some(latin_g0(NationalOption::Turkish)));
    assert_eq!(second_g0(value(0b1101, 0b011)), None);
    assert_eq!(second_g0(value(0b0011, 0b000)), None);
    for (group, index, g0, _) in table_32_defined().into_iter().chain(extended_defined()) {
        assert_eq!(second_g0(value(group, index)), Some(g0));
    }
}

#[test]
fn national_bits_mask_is_the_index_mask() {
    assert_eq!(NATIONAL_BITS_MASK, NATIONAL_INDEX_MASK);
    assert_eq!(1_u8 << GROUP_SHIFT, NATIONAL_BITS_MASK + 1);
}

// --- Code tables --------------------------------------------------------

fn every_set() -> Vec<CharSet> {
    let mut sets = std::vec![latin(None)];
    sets.extend(NationalOption::ALL.iter().map(|&option| latin(Some(option))));
    sets.extend(
        [
            G0Set::Cyrillic1,
            G0Set::Cyrillic2,
            G0Set::Cyrillic3,
            G0Set::Greek,
            G0Set::Arabic,
            G0Set::Hebrew,
        ]
        .map(g0),
    );
    sets.extend(
        [G2Set::Latin, G2Set::Cyrillic, G2Set::Greek, G2Set::Arabic].map(CharSet::G2),
    );
    sets.push(CharSet::G3);
    sets
}

#[test]
fn no_replacement_characters_in_any_set() {
    for set in every_set() {
        for code in ALL_CODES {
            let found = base_unicode(set, code);
            assert_ne!(found, char::REPLACEMENT_CHARACTER, "{set:?} {code:#04x}");
            assert!(!found.is_control(), "{set:?} {code:#04x}");
        }
    }
}

#[test]
fn codes_outside_the_table_are_spaces() {
    for set in every_set() {
        for code in (0..0x20).chain(0x80..=0xFF) {
            assert_eq!(base_unicode(set, code), ' ', "{set:?} {code:#04x}");
        }
    }
}

#[test]
fn space_is_space_in_every_alphanumeric_set() {
    // G3 2/0 is a smooth mosaic, not a space.
    for set in every_set().into_iter().filter(|&set| set != CharSet::G3) {
        assert_eq!(base_unicode(set, SPACE), ' ', "{set:?}");
    }
}

#[test]
fn letters_are_distinct_within_each_g0_set() {
    for set in [G0Set::Cyrillic1, G0Set::Cyrillic2, G0Set::Cyrillic3, G0Set::Greek]
        .map(g0)
    {
        let letters: Vec<char> = (0x40..0x7F).map(|code| base_unicode(set, code)).collect();
        let distinct: BTreeSet<char> = letters.iter().copied().collect();
        assert_eq!(distinct.len(), letters.len(), "{set:?}");
    }
}

#[test]
fn cyrillic_rows() {
    assert_eq!(row(g0(G0Set::Cyrillic1), 0x40), "ЧАБЦДЕФГХИЈКЛМНО");
    assert_eq!(row(g0(G0Set::Cyrillic1), 0x50), "ПЌРСТУВЃЉЊЗЋЖЂШЏ");
    assert_eq!(row(g0(G0Set::Cyrillic2), 0x50), "ПЯРСТУЖВЬЪЗШЭЩЧЫ");
    assert_eq!(row(g0(G0Set::Cyrillic2), 0x70), "пярстужвьъзшэщч■");
    assert_eq!(row(g0(G0Set::Cyrillic3), 0x50), "ПЯРСТУЖВЬІЗШЄЩЧЇ");
    assert_eq!(base_unicode(g0(G0Set::Cyrillic2), 0x26), 'ы');
    assert_eq!(base_unicode(g0(G0Set::Cyrillic3), 0x26), 'ї');
    assert_eq!(base_unicode(g0(G0Set::Cyrillic1), 0x26), '&');
    // Each lower-case letter is its capital two columns to the left;
    // 7/F is the solid block, so the last capital has no partner.
    for set in [G0Set::Cyrillic1, G0Set::Cyrillic2, G0Set::Cyrillic3].map(g0) {
        for code in 0x40..0x5F {
            let upper = base_unicode(set, code);
            let lower = base_unicode(set, code + 0x20);
            assert_eq!(upper.to_lowercase().next().unwrap(), lower, "{set:?} {code:#04x}");
        }
    }
}

#[test]
fn greek_rows() {
    let greek = g0(G0Set::Greek);
    assert_eq!(row(greek, 0x40), "ΐΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟ");
    assert_eq!(row(greek, 0x50), "ΠΡ΄ΣΤΥΦΧΨΩΪΫάέήί");
    assert_eq!(row(greek, 0x60), "ΰαβγδεζηθικλμνξο");
    assert_eq!(row(greek, 0x70), "πρςστυφχψωϊϋόύώ■");
    assert_eq!(row(CharSet::G2(G2Set::Greek), 0x60), "CDFGJLQRSUVWYZΆΉ");
}

#[test]
fn hebrew_rows() {
    let hebrew = g0(G0Set::Hebrew);
    assert_eq!(row(hebrew, 0x60), "אבגדהוזחטיךכלםמן");
    assert_eq!(row(hebrew, 0x70), "נסעףפץצקרשת₪‖¾÷■");
    assert_eq!(row(hebrew, 0x50), "PQRSTUVWXYZ←½→↑#");
}

#[test]
fn arabic_spot_checks() {
    let arabic = g0(G0Set::Arabic);
    assert_eq!(base_unicode(arabic, 0x41), '\u{0621}'); // hamza
    assert_eq!(base_unicode(arabic, 0x47), '\u{0627}'); // alef
    assert_eq!(base_unicode(arabic, 0x60), '\u{0640}'); // tatweel
    assert_eq!(base_unicode(arabic, 0x7E), '\u{FEFB}'); // lam with alef
    assert_eq!(base_unicode(arabic, 0x3F), '\u{061F}'); // question mark
    assert_eq!(base_unicode(arabic, 0x5F), '#');
    let arabic_g2 = CharSet::G2(G2Set::Arabic);
    let digits: std::string::String =
        (0x30..0x3A).map(|code| base_unicode(arabic_g2, code)).collect();
    assert_eq!(digits, "٠١٢٣٤٥٦٧٨٩");
    assert_eq!(row(arabic_g2, 0x70), "pqrstuvwxyzâôûç ");
    // Every Arabic G0 letter position holds an Arabic character.
    for code in 0x40..0x7F {
        if code == 0x5F {
            continue;
        }
        let found = u32::from(base_unicode(arabic, code));
        assert!(
            (0x0600..=0x06FF).contains(&found) || (0xFB50..=0xFEFF).contains(&found),
            "{code:#04x}"
        );
    }
}

#[test]
fn latin_g2_rows() {
    let set = CharSet::G2(G2Set::Latin);
    assert_eq!(row(set, 0x20), " ¡¢£$¥#§¤‘“«←↑→↓");
    assert_eq!(row(set, 0x30), "°±²³×µ¶·÷’”»¼½¾¿");
    assert_eq!(row(set, 0x50), "―¹®©™♪€‰α   ⅛⅜⅝⅞");
    assert_eq!(row(set, 0x60), "ΩÆĐªĦ ĲĿŁØŒºÞŦŊŉ");
    assert_eq!(row(set, 0x70), "ĸæđðħıĳŀłøœßþŧŋ■");
    // REQ-CHR-006: undefined positions are spaces.
    for code in [0x59, 0x5A, 0x5B, 0x65] {
        assert_eq!(base_unicode(set, code), ' ');
    }
}

#[test]
fn g2_column_4_is_the_diacritic_column() {
    let spacing = " `´ˆ˜¯˘˙¨.˚¸_˝˛ˇ";
    for set in [G2Set::Latin, G2Set::Cyrillic, G2Set::Greek] {
        assert_eq!(row(CharSet::G2(set), 0x40), spacing, "{set:?}");
    }
    assert_eq!(row(CharSet::G2(G2Set::Arabic), 0x40), "àABCDEFGHIJKLMNO");
}

#[test]
fn cyrillic_g2_rows() {
    let set = CharSet::G2(G2Set::Cyrillic);
    assert_eq!(base_unicode(set, 0x26), ' ');
    assert_eq!(base_unicode(set, 0x28), ' ');
    assert_eq!(row(set, 0x50), "―¹®©™♪€‰αŁłß⅛⅜⅝⅞");
    assert_eq!(row(set, 0x60), "DEFGIJKLNQRSUVWZ");
    assert_eq!(row(set, 0x70), "defgijklnqrsuvwz");
}

/// REQ-CHR-011: every G3 position against the mapping of L2/19-025 and
/// `TELTXTG3.TXT`, as `research/notes-level25-drcs.md` section 9.3
/// lists it; 5/F and the unassigned positions are spaces.
#[test]
fn g3_positions() {
    const MAPPING: [u32; 96] = [
        0x1FB3C, 0x1FB3D, 0x1FB3E, 0x1FB3F, 0x1FB40, 0x25E3, 0x1FB41, 0x1FB42, // 2/0
        0x1FB43, 0x1FB44, 0x1FB45, 0x1FB46, 0x1FB68, 0x1FB69, 0x1FB70, 0x2592, // 2/8
        0x1FB47, 0x1FB48, 0x1FB49, 0x1FB4A, 0x1FB4B, 0x25E2, 0x1FB4C, 0x1FB4D, // 3/0
        0x1FB4E, 0x1FB4F, 0x1FB50, 0x1FB51, 0x1FB6A, 0x1FB6B, 0x1FB75, 0x2588, // 3/8
        0x2537, 0x252F, 0x251D, 0x2525, 0x1FBA4, 0x1FBA5, 0x1FBA6, 0x1FBA7, // 4/0
        0x1FBA0, 0x1FBA1, 0x1FBA2, 0x1FBA3, 0x253F, 0x2022, 0x25CF, 0x25CB, // 4/8
        0x2502, 0x2500, 0x250C, 0x2510, 0x2514, 0x2518, 0x251C, 0x2524, // 5/0
        0x252C, 0x2534, 0x253C, 0x2B62, 0x2B60, 0x2B61, 0x2B63, 0x0020, // 5/8
        0x1FB52, 0x1FB53, 0x1FB54, 0x1FB55, 0x1FB56, 0x25E5, 0x1FB57, 0x1FB58, // 6/0
        0x1FB59, 0x1FB5A, 0x1FB5B, 0x1FB5C, 0x1FB6C, 0x1FB6D, 0x0020, 0x0020, // 6/8
        0x1FB5D, 0x1FB5E, 0x1FB5F, 0x1FB60, 0x1FB61, 0x25E4, 0x1FB62, 0x1FB63, // 7/0
        0x1FB64, 0x1FB65, 0x1FB66, 0x1FB67, 0x1FB6E, 0x1FB6F, 0x0020, 0x0020, // 7/8
    ];
    for (code, &expected) in ALL_CODES.zip(MAPPING.iter()) {
        assert_eq!(u32::from(base_unicode(CharSet::G3, code)), expected, "G3 {code:#04x}");
    }
    assert_eq!(base_unicode(CharSet::G3, 0x3F), FULL_BLOCK);
    // Every assigned position is distinct.
    let shapes: Vec<char> = ALL_CODES
        .map(|code| base_unicode(CharSet::G3, code))
        .filter(|&found| found != ' ')
        .collect();
    let distinct: BTreeSet<char> = shapes.iter().copied().collect();
    assert_eq!(shapes.len(), 91);
    assert_eq!(distinct.len(), shapes.len());
}

// --- Diacritical marks and composition --------------------------------

#[test]
fn diacritic_combining_marks() {
    let expected = [
        0x0300, 0x0301, 0x0302, 0x0303, 0x0304, 0x0306, 0x0307, 0x0308, 0x0323, 0x030A,
        0x0327, 0x0332, 0x030B, 0x0328, 0x030C, 0x0309,
    ];
    assert_eq!(diacritic_combining(0), None);
    for (mark, scalar) in (1..=16).zip(expected) {
        assert_eq!(diacritic_combining(mark), char::from_u32(scalar), "mark {mark}");
    }
    for mark in 17..=0xFF {
        assert_eq!(diacritic_combining(mark), None);
    }
}

#[test]
fn compose_spot_checks() {
    let acute = diacritic_combining(2).unwrap();
    let dot_above = diacritic_combining(7).unwrap();
    let caron = diacritic_combining(15).unwrap();
    assert_eq!(compose('A', acute), Some('Á'));
    assert_eq!(compose('c', caron), Some('č'));
    assert_eq!(compose('Z', dot_above), Some('Ż'));
    assert_eq!(compose('x', acute), None);
    assert_eq!(compose('o', diacritic_combining(13).unwrap()), Some('ő'));
    assert_eq!(compose('s', diacritic_combining(11).unwrap()), Some('ş'));
    assert_eq!(compose('e', diacritic_combining(14).unwrap()), Some('ę'));
    assert_eq!(compose('\u{0131}', acute), None);
    assert_eq!(compose('A', 'B'), None);
}

#[test]
fn composition_table_is_sorted_and_uses_known_marks() {
    for pair in COMPOSITIONS.windows(2) {
        assert!((pair[0].0, pair[0].1) < (pair[1].0, pair[1].1), "{pair:?}");
    }
    let marks: BTreeSet<char> = COMBINING_MARKS.iter().flatten().copied().collect();
    for (base, mark, composed) in COMPOSITIONS {
        assert!(base.is_ascii_alphabetic());
        assert!(marks.contains(&mark));
        assert_eq!(compose(base, mark), Some(composed));
        // Composition keeps the case of the base letter.
        assert_eq!(composed.is_uppercase(), base.is_uppercase(), "{composed}");
    }
    for pair in EXTENDED_COMPOSITIONS.windows(2) {
        assert!((pair[0].0, pair[0].1) < (pair[1].0, pair[1].1), "{pair:?}");
    }
    for (base, mark, composed) in EXTENDED_COMPOSITIONS {
        assert!(marks.contains(&mark));
        assert_eq!(search(&COMPOSITIONS, base, mark), None, "{composed} is in one table only");
        assert_eq!(compose(base, mark), Some(composed));
        assert_eq!(composed.is_uppercase(), base.is_uppercase(), "{composed}");
    }
}

#[test]
fn every_latin_g0_letter_with_every_mark() {
    // Each Latin G0 letter with each mark either has a precomposed form
    // or none; the pairs national option sub-sets need must compose.
    let needed = [
        ('u', 10, 'ů'),
        ('c', 15, 'č'),
        ('t', 15, 'ť'),
        ('o', 4, 'õ'),
        ('e', 7, 'ė'),
        ('a', 14, 'ą'),
        ('u', 5, 'ū'),
        ('n', 2, 'ń'),
        ('T', 11, 'Ţ'),
        ('g', 6, 'ğ'),
        ('I', 7, 'İ'),
        ('u', 13, 'ű'),
    ];
    for (base, mark, composed) in needed {
        assert_eq!(compose(base, diacritic_combining(mark).unwrap()), Some(composed));
    }
    let mut count = 0;
    for code in (0x41..=0x5A).chain(0x61..=0x7A) {
        let base = base_unicode(latin(None), code);
        for mark in 1..=15 {
            count += usize::from(compose(base, diacritic_combining(mark).unwrap()).is_some());
        }
    }
    assert_eq!(count, COMPOSITIONS.len());
}

// --- Mosaics --------------------------------------------------------------

#[test]
fn mosaic_fixed_points() {
    assert_eq!(MOSAIC_LEFT_COLUMN | MOSAIC_RIGHT_COLUMN, MOSAIC_FULL);
    assert_eq!(MOSAIC_LEFT_COLUMN, 21);
    assert_eq!(MOSAIC_RIGHT_COLUMN, 42);
    assert_eq!(mosaic_unicode(0, false), ' ');
    assert_eq!(mosaic_unicode(21, false), '\u{258C}');
    assert_eq!(mosaic_unicode(42, false), '\u{2590}');
    assert_eq!(mosaic_unicode(63, false), '\u{2588}');
    assert_eq!(mosaic_unicode(1, false), '\u{1FB00}');
    assert_eq!(mosaic_unicode(20, false), '\u{1FB13}');
    assert_eq!(mosaic_unicode(22, false), '\u{1FB14}');
    assert_eq!(mosaic_unicode(62, false), '\u{1FB3B}');
    assert_eq!(mosaic_unicode(0, true), ' ');
    assert_eq!(mosaic_unicode(1, true), '\u{1CE51}');
    assert_eq!(mosaic_unicode(63, true), '\u{1CE8F}');
}

#[test]
fn all_64_mosaics_contiguous() {
    let fixed = [0, 21, 42, 63];
    let sextants: Vec<u32> = (0..64_u8)
        .filter(|n| !fixed.contains(n))
        .map(|n| u32::from(mosaic_unicode(n, false)))
        .collect();
    assert_eq!(sextants.len(), 60);
    assert_eq!(sextants.first(), Some(&0x1FB00));
    assert_eq!(sextants.last(), Some(&0x1FB3B));
    // Strictly increasing by one: monotonic, distinct and gap-free.
    for pair in sextants.windows(2) {
        assert_eq!(pair[1], pair[0] + 1);
    }
    let all: BTreeSet<char> = (0..64).map(|n| mosaic_unicode(n, false)).collect();
    assert_eq!(all.len(), 64);
}

#[test]
fn all_64_mosaics_separated() {
    for n in 1..64_u8 {
        assert_eq!(u32::from(mosaic_unicode(n, true)), 0x1CE50 + u32::from(n));
    }
    let all: BTreeSet<char> = (0..64).map(|n| mosaic_unicode(n, true)).collect();
    assert_eq!(all.len(), 64);
}

#[test]
fn mosaic_pattern_ignores_high_bits() {
    for n in 0..64_u8 {
        assert_eq!(mosaic_unicode(n | 0xC0, false), mosaic_unicode(n, false));
        assert_eq!(mosaic_unicode(n | 0xC0, true), mosaic_unicode(n, true));
    }
}
