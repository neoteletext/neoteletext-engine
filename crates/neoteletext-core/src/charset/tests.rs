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
/// REQ-82: the Vietnamese set is a whole set, so that X/26 marks find its letters as bases.
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
            (value(0b1101, 0), (S::Latin, Latin)),
            (value(0b1101, 1), (S::Hiragana, Latin)),
            (value(0b1101, 2), (S::Katakana, Latin)),
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
    // Group 1110 is wholly reserved.
    let (found, fallback) = designation_with_fallback(value(0b1110, 0b001));
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
/// REQ-79: a mosaic, which is no letter, converts through its pattern.
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

/// REQ-CHR-016: a Thai or Lao G2 glyph of two marks gives its marks as a
/// combining sequence, vowel sign first, and a font draws it by its
/// private-use key: the requirement's example is Thai G2 5/C, which gives
/// U+0E35 U+0E48. Every sequence is reached from one of the two G2 sets,
/// and only from columns 5 and 6.
/// REQ-79: a character may convert to a short sequence of scalar values.
#[test]
fn thai_and_lao_mark_pairs_give_combining_sequences() {
    let thai = |code| CharRef::new(CharSet::G2(G2Set::Thai), code);
    assert_eq!(to_unicode(thai(0x5C)), Unicode { base: '\u{0E35}', combining: Some('\u{0E48}') });
    let sequences = mark_sequences();
    assert_eq!(sequences.len(), 53);
    for &(key, first, second) in sequences {
        let found: std::vec::Vec<CharRef> = [G2Set::Thai, G2Set::Lao]
            .into_iter()
            .flat_map(|set| (0x20u8..=0x7F).map(move |code| CharRef::new(CharSet::G2(set), code)))
            .filter(|&character| glyph_key(character) == key)
            .collect();
        let [position] = found[..] else { panic!("{key:?} is at {found:?}, not one position") };
        assert!((0x50..=0x6F).contains(&position.code), "{key:?} in columns 5 and 6");
        assert_eq!(to_unicode(position), Unicode { base: first, combining: Some(second) }, "{key:?}");
    }
}

// The table tests keep the layout of the tables they check.
#[rustfmt::skip]
mod tables;

/// REQ-2188 item 1, DEC-2187 item 3: the kana tables. The 75 kana both
/// scripts share are the same sound at the same code; the digits are at
/// 3/0 to 3/9; hiragana keeps ! + - : ? at their ASCII codes; both put
/// 、 and 。 at the comma's and the full stop's.
#[cfg(feature = "extended-sets")]
#[test]
fn kana_tables() {
    let hiragana = CharSet::G0(G0 { set: G0Set::Hiragana, national: None });
    let katakana = CharSet::G0(G0 { set: G0Set::Katakana, national: None });
    let at = |set, code: u8| to_unicode(CharRef::new(set, code)).base;
    let mut shared = 0;
    for code in 0x21..=0x7E_u8 {
        let (h, k) = (at(hiragana, code), at(katakana, code));
        if ('\u{3041}'..='\u{3096}').contains(&h) && ('\u{30A1}'..='\u{30F6}').contains(&k) {
            assert_eq!(u32::from(k) - u32::from(h), 0x60, "{code:#04x}: {h} and {k} are one sound");
            shared += 1;
        }
    }
    assert_eq!(shared, 75);
    for digit in 0..10_u8 {
        assert_eq!(at(hiragana, 0x30 + digit), char::from(b'0' + digit));
        assert_eq!(at(katakana, 0x30 + digit), char::from(b'0' + digit));
    }
    let hiragana_own = [(0x21, '!'), (0x28, '「'), (0x29, '」'), (0x2B, '+'), (0x2D, '-'), (0x3A, ':'), (0x3F, '?')];
    for (code, expected) in hiragana_own {
        assert_eq!(at(hiragana, code), expected);
    }
    let katakana_own =
        [(0x21, 'ァ'), (0x28, 'ィ'), (0x29, 'ゥ'), (0x2B, 'ェ'), (0x2D, 'ー'), (0x3A, 'ォ'), (0x3F, 'ヴ')];
    for (code, expected) in katakana_own {
        assert_eq!(at(katakana, code), expected);
    }
    for set in [hiragana, katakana] {
        assert_eq!((at(set, 0x2C), at(set, 0x2E), at(set, 0x7F)), ('、', '。', '■'));
    }
    assert_eq!((at(hiragana, 0x40), at(katakana, 0x40)), ('あ', 'ア'));
    let romaji = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::JapaneseRomaji) });
    let options: std::string::String = [0x23, 0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x7B, 0x7C, 0x7D, 0x7E]
        .iter()
        .map(|&code| at(romaji, code))
        .collect();
    assert_eq!(options, "¥$ĀĒĪŌŪ#āēīōū");
}

/// REQ-2181: the language table. Every entry's value resolves to a set;
/// tags match on their language and script subtags, ignoring case and
/// region; a tag naming no script takes the language's default; an
/// unknown language has no set.
#[cfg(feature = "extended-sets")]
#[test]
fn the_language_table() {
    use super::language::{LANGUAGES, language, language_value, region_for_iso_639_2};
    for entry in LANGUAGES {
        assert!(designation(entry.value).is_some(), "{}-{}", entry.language, entry.script);
        if let Some(standard) = entry.standard {
            assert!(designation(standard).is_some(), "{}-{}", entry.language, entry.script);
        }
    }
    let value_of = |tag: &str| language(tag).map(|found| found.value);
    assert_eq!(value_of("sr"), Some(value(0b0111, 0b011)), "Serbian in its Cyrillic set");
    assert_eq!(value_of("sr-Latn"), Some(value(0b0011, 0b101)));
    assert_eq!(value_of("SR-cyrl-RS"), Some(value(0b0111, 0b011)));
    assert_eq!(value_of("kk"), Some(value(0b0010, 0b110)), "Kazakh in the Turkish option, DEC-602");
    assert_eq!(value_of("en-GB"), Some(0));
    assert_eq!(value_of("ja"), Some(value(0b1101, 0b001)));
    assert_eq!(value_of("ja-Kana"), Some(value(0b1101, 0b010)));
    assert_eq!(value_of("ja-Latn"), Some(value(0b1101, 0b000)));
    assert_eq!(value_of("xx"), None);
    // REQ-2215: each entry's English name, direction and mark rows, for
    // the server's list of languages.
    let entry = |tag: &str| language(tag).unwrap();
    assert_eq!((entry("ru").name, entry("sr-Latn").name), ("Russian", "Serbian (Latin script)"));
    assert!(entry("ar").right_to_left() && entry("ur").right_to_left() && entry("he").right_to_left());
    assert!(!entry("en").right_to_left() && !entry("th").right_to_left());
    assert_eq!((entry("th").mark_rows(), entry("lo").mark_rows(), entry("vi").mark_rows()), (1, 1, 0));
    assert_eq!((entry("sr").tag_parts(), entry("sr-Latn").tag_parts()), (("sr", None), ("sr", Some("Latn"))));
    assert!(LANGUAGES.iter().all(|entry| !entry.name.is_empty()));
    assert_eq!(value_of("ru-Latn"), None, "no Latin Russian is written");
    // DEC-2184, DEC-2187 item 9: for Level 3.5 and below, rōmaji is Latin
    // English and kana are refused; other languages keep their sets.
    assert_eq!(language_value("ja-Latn", false), Some(0));
    assert_eq!((language_value("ja", false), language_value("ja-Kana", false)), (None, None));
    assert_eq!(language_value("yo", false), Some(value(0b0101, 0b110)));
    // REQ-2181 item 5: the subtitle region of every code the input knew
    // before is unchanged.
    let before: [(&[u8; 3], u8); 21] = [
        (b"pol", 1),
        (b"tur", 2),
        (b"srp", 3),
        (b"scc", 3),
        (b"hrv", 3),
        (b"scr", 3),
        (b"slv", 3),
        (b"ron", 3),
        (b"RUM", 3),
        (b"rus", 4),
        (b"bul", 4),
        (b"ukr", 4),
        (b"est", 4),
        (b"lav", 4),
        (b"lit", 4),
        (b"gre", 6),
        (b"ell", 6),
        (b"ara", 8),
        (b"heb", 10),
        (b"eng", 0),
        (b"zzz", 0),
    ];
    for (code, region) in before {
        assert_eq!(region_for_iso_639_2(*code), region, "{}", core::str::from_utf8(code).unwrap_or("?"));
    }
}
