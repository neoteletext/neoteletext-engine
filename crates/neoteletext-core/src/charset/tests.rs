//! Tests of character sets and their designation (area CHR): Vietnamese
//! tones, Table 34, the fallback, mosaics and national positions. The tests
//! of the code tables are in tests/tables.rs.
#![allow(clippy::unwrap_used, clippy::panic)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::consts::{DESIGNATION_GROUP_SHIFT, SPACE};

/// REQ-CHR-014: every tone on every Vietnamese vowel, placed by X/26 on
/// the Vietnamese G0 set with marks from the Vietnamese G2 set,
/// composes to its precomposed letter. Tones: grave 4/1, acute 4/2,
/// tilde 4/4, dot below 4/9, hook above 4/C.
#[test]
fn every_vietnamese_tone_composes() {
    const TONES: [u8; 5] = [0x1, 0x2, 0x4, 0x9, 0xC];
    let expected = [
        ('a', "àáãạả"),
        ('ă', "ằắẵặẳ"),
        ('â', "ầấẫậẩ"),
        ('e', "èéẽẹẻ"),
        ('ê', "ềếễệể"),
        ('i', "ìíĩịỉ"),
        ('o', "òóõọỏ"),
        ('ô', "ồốỗộổ"),
        ('ơ', "ờớỡợở"),
        ('u', "ùúũụủ"),
        ('ư', "ừứữựử"),
        ('y', "ỳýỹỵỷ"),
        ('A', "ÀÁÃẠẢ"),
        ('Ă', "ẰẮẴẶẲ"),
        ('Â', "ẦẤẪẬẨ"),
        ('E', "ÈÉẼẸẺ"),
        ('Ê', "ỀẾỄỆỂ"),
        ('I', "ÌÍĨỊỈ"),
        ('O', "ÒÓÕỌỎ"),
        ('Ô', "ỒỐỖỘỔ"),
        ('Ơ', "ỜỚỠỢỞ"),
        ('U', "ÙÚŨỤỦ"),
        ('Ư', "ỪỨỮỰỬ"),
        ('Y', "ỲÝỸỴỶ"),
    ];
    let set = CharSet::G0(G0 { set: G0Set::Vietnamese, national: None });
    for (vowel, toned) in expected {
        let code = (SPACE..0x7F)
            .find(|&code| to_unicode(CharRef::new(set, code)).base == vowel)
            .unwrap_or_else(|| panic!("{vowel} is in the Vietnamese set"));
        for (tone, want) in TONES.into_iter().zip(toned.chars()) {
            let mut character = CharRef::new(set, code);
            character.diacritic = page_mark(G2Set::Vietnamese, tone);
            let unicode = to_unicode(character);
            assert_eq!(compose(unicode.base, unicode.combining.unwrap()), Some(want), "{vowel} tone {tone:X}");
        }
    }
}

/// REQ-CHR-014: 4/C is the hook above only in the Vietnamese G2 set;
/// pages with the standard's sets keep the low line.
#[test]
fn the_hook_above_is_vietnamese_only() {
    let hook = to_unicode(CharRef {
        diacritic: page_mark(G2Set::Vietnamese, 0xC),
        ..CharRef::new(CharSet::G0(G0 { set: G0Set::Latin, national: None }), b'a')
    });
    assert_eq!(hook.combining, Some('\u{0309}'));
    for g2 in [G2Set::Latin, G2Set::Cyrillic, G2Set::Greek, G2Set::Arabic, G2Set::Armenian] {
        for mark in 0..=0xF {
            assert_eq!(page_mark(g2, mark), mark);
        }
    }
    for mark in (0..=0xF).filter(|&mark| mark != 0xC) {
        assert_eq!(page_mark(G2Set::Vietnamese, mark), mark);
    }
}

fn value(group: u8, national: u8) -> u8 {
    (group << DESIGNATION_GROUP_SHIFT) | national
}

fn latin(national: NationalOption) -> G0 {
    G0 { set: G0Set::Latin, national: Some(national) }
}

/// REQ-L25-012: every row of Table 34, and its reserved values.
#[test]
fn table_34_modified_designation() {
    use G0Set as S;
    use G2Set::{Arabic, Cyrillic, Greek, Latin};
    let mut expected = std::vec::Vec::new();
    for group in 0..4 {
        for low in 0..8 {
            expected.push((value(group, low), (S::Latin, Latin)));
        }
    }
    expected.extend([
        (value(0b0100, 0), (S::Cyrillic1, Cyrillic)),
        (value(0b0100, 1), (S::Latin, Latin)),
        (value(0b0100, 2), (S::Latin, Latin)),
        (value(0b0100, 3), (S::Latin, Latin)),
        (value(0b0100, 4), (S::Cyrillic2, Cyrillic)),
        (value(0b0100, 5), (S::Cyrillic3, Cyrillic)),
        (value(0b0100, 6), (S::Latin, Latin)),
        (value(0b0110, 6), (S::Latin, Latin)),
        (value(0b0110, 7), (S::Greek, Greek)),
        (value(0b1000, 0), (S::Latin, Arabic)),
        (value(0b1000, 4), (S::Latin, Arabic)),
        (value(0b1000, 7), (S::Arabic, Arabic)),
        (value(0b1010, 5), (S::Hebrew, Arabic)),
        (value(0b1010, 7), (S::Arabic, Arabic)),
    ]);
    #[cfg(feature = "extended-sets")]
    {
        use G2Set::{Armenian, Vietnamese};
        for low in 0..=5 {
            expected.push((value(0b0101, low), (S::Latin, Latin)));
        }
        expected.extend([
            (value(0b0101, 6), (S::YorubaIgbo, Latin)),
            (value(0b0101, 7), (S::Vietnamese, Vietnamese)),
            (value(0b0111, 0), (S::Ukrainian, Cyrillic)),
            (value(0b0111, 1), (S::Belarusian, Cyrillic)),
            (value(0b0111, 2), (S::Macedonian, Cyrillic)),
            (value(0b0111, 3), (S::Serbian, Cyrillic)),
            (value(0b0111, 5), (S::KyrgyzMongolian, Cyrillic)),
            (value(0b0111, 6), (S::Tatar, Cyrillic)),
            (value(0b0111, 7), (S::Tajik, Cyrillic)),
            (value(0b1001, 0), (S::Persian, Arabic)),
            (value(0b1001, 1), (S::Urdu, Arabic)),
            (value(0b1001, 2), (S::Pashto, Arabic)),
            (value(0b1001, 3), (S::Sindhi, Arabic)),
            (value(0b1001, 4), (S::Sorani, Arabic)),
            (value(0b1001, 5), (S::Uyghur, Arabic)),
            (value(0b1001, 6), (S::Balochi, Arabic)),
            (value(0b1001, 7), (S::ArabicPlain, Arabic)),
            (value(0b1100, 0), (S::Latin, G2Set::Thai)),
            (value(0b1100, 2), (S::Thai, G2Set::Thai)),
            (value(0b1100, 4), (S::Lao, G2Set::Lao)),
            (value(0b1011, 0), (S::Georgian, Latin)),
            (value(0b1011, 1), (S::Armenian, Armenian)),
            (value(0b1011, 2), (S::Tifinagh, Latin)),
        ]);
    }
    for data in 0..=0x7F {
        let want = expected.iter().find(|(entry, _)| *entry == data).map(|(_, sets)| *sets);
        assert_eq!(modified_designation(data), want, "value {data:#04x}");
    }
    assert_eq!(modified_designation(value(0b0100, 7)), None, "0100111 is blank, so reserved");
}

/// REQ-CHR-003: a defined Table 32 entry is used as it is.
#[test]
fn defined_designation_needs_no_fallback() {
    let (found, fallback) = designation_with_fallback(value(0b0011, 0b111));
    assert_eq!(found, Designation { g0: latin(NationalOption::Rumanian), g2: G2Set::Latin });
    assert!(!fallback);
}

/// REQ-CHR-003 step 1: a reserved entry retries the same national
/// option bits in group 0000.
#[test]
fn reserved_designation_retries_group_0000() {
    // Group 0011 defines only indexes 101 and 111.
    let (found, fallback) = designation_with_fallback(value(0b0011, 0b110));
    assert_eq!(found, Designation { g0: latin(NationalOption::CzechSlovak), g2: G2Set::Latin });
    assert!(fallback);
    // Group 1101 is wholly reserved.
    let (found, fallback) = designation_with_fallback(value(0b1101, 0b001));
    assert_eq!(found.g0, latin(NationalOption::German));
    assert!(fallback);
}

/// REQ-CHR-003 step 2: when group 0000 is reserved too, English.
#[test]
fn doubly_reserved_designation_is_english() {
    let english = Designation { g0: latin(NationalOption::English), g2: G2Set::Latin };
    for reserved in [value(0b1101, 0b111), value(0b1111, 0b111), value(0b0000, 0b111)] {
        assert_eq!(designation(reserved), None);
        assert_eq!(designation_with_fallback(reserved), (english, true), "value {reserved:#09b}");
    }
}

/// REQ-CHR-005: G1 codes through the pattern to Unicode.
#[test]
fn mosaic_codes_to_unicode() {
    // 2/0 has no blocks; 7/F has all six.
    assert_eq!(mosaic_to_unicode(mosaic_pattern(SPACE), false), ' ');
    assert_eq!(mosaic_to_unicode(mosaic_pattern(CODE_MASK_7BIT), false), '\u{2588}');
    // 3/5: blocks 1, 3 and 5, the left column: LEFT HALF BLOCK.
    assert_eq!(mosaic_to_unicode(mosaic_pattern(b'5'), false), '\u{258C}');
    // 6/A: blocks 2, 4 and 6, the right column: RIGHT HALF BLOCK.
    assert_eq!(mosaic_to_unicode(mosaic_pattern(b'j'), false), '\u{2590}');
    // Separated mosaics are U+1CE50 plus the pattern.
    assert_eq!(mosaic_to_unicode(mosaic_pattern(b'5'), true), '\u{1CE65}');
    assert_eq!(mosaic_to_unicode(mosaic_pattern(SPACE), true), ' ');
}

/// The 13 positions of Table 36 are exactly the codes where some
/// national option differs from another; the parity bit is ignored.
#[test]
fn national_positions_are_where_the_options_differ() {
    let mut count = 0;
    for code in SPACE..=CODE_MASK_7BIT {
        let first = to_unicode(CharRef::new(CharSet::G0(latin(NationalOption::ALL[0])), code)).base;
        let varies = NationalOption::ALL
            .iter()
            .any(|&option| to_unicode(CharRef::new(CharSet::G0(latin(option)), code)).base != first);
        assert_eq!(is_national_position(code), varies, "code {code:#04x}");
        assert_eq!(is_national_position(code | !CODE_MASK_7BIT), varies, "code {code:#04x} with bit 8 set");
        count += usize::from(varies);
    }
    assert_eq!(count, 13);
}

/// REQ-SVC-005: 2/3 and 2/4 are Latin G0's "#" and currency sign, not
/// a national option's characters, and control codes show as spaces.
#[test]
fn latin_text_has_no_national_option() {
    let text: [char; 8] = latin_text(b"BBC1 #$\x07");
    assert_eq!(text, ['B', 'B', 'C', '1', ' ', '#', '\u{A4}', ' ']);
}

// The table tests keep the layout of the tables they check.
#[rustfmt::skip]
mod tables;
