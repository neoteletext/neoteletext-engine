//! Character sets and their designation (EN 300 706 clause 15, area CHR).
//!
//! Inside the decoder a character is a set identifier and a 7-bit code, never
//! a Unicode value (REQ-CHR-001). Conversion to Unicode happens only at the
//! output stage, through [`to_unicode`].

// Tables keep one source row per line, aligned as in the standard.
#[rustfmt::skip]
mod tables;

use crate::consts::{CODE_MASK_7BIT, DIACRITIC_HOOK_ABOVE, DIACRITIC_LOW_LINE, DIACRITIC_NONE};
use crate::types::{DrcsCharacter, DrcsTable};

/// A G0 primary character set (EN 300 706 clause 15.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum G0Set {
    /// Latin G0 (Table 35), used with a national option sub-set.
    Latin,
    /// Cyrillic G0 option 1, Serbian and Croatian (Table 38).
    Cyrillic1,
    /// Cyrillic G0 option 2, Russian and Bulgarian (Table 39).
    Cyrillic2,
    /// Cyrillic G0 option 3, Ukrainian (Table 40).
    Cyrillic3,
    /// Greek G0 (Table 42).
    Greek,
    /// Arabic G0 (Table 44).
    Arabic,
    /// Hebrew G0 (Table 46).
    Hebrew,
    /// Vietnamese: Latin G0 with the 13 national positions holding the
    /// vowels with a circumflex, breve or horn, and đ (decision E.81).
    Vietnamese,
    /// Yoruba and Igbo: Latin G0 with the 13 national positions holding
    /// the letters with a dot below and ṅ (decision E.81).
    YorubaIgbo,
    /// Ukrainian: Table 40 with Ґ ґ (decision E.78).
    Ukrainian,
    /// Belarusian, built from Table 39.
    Belarusian,
    /// Macedonian, built from Table 38.
    Macedonian,
    /// Serbian: Table 38 with lowercase џ.
    Serbian,
    /// Kyrgyz and Mongolian, built from Table 39.
    KyrgyzMongolian,
    /// Tatar, built from Table 39.
    Tatar,
    /// Tajik, built from Table 39.
    Tajik,
    /// Georgian (Mkhedruli), laid out as Table 46.
    Georgian,
    /// Armenian.
    Armenian,
    /// Tifinagh (the Moroccan alphabet), laid out as Table 46.
    Tifinagh,
}

/// A G2 supplementary character set (EN 300 706 clause 15.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum G2Set {
    /// Latin G2 (Table 37).
    Latin,
    /// Cyrillic G2 (Table 41).
    Cyrillic,
    /// Greek G2 (Table 43).
    Greek,
    /// Arabic G2 (Table 45).
    Arabic,
    /// Vietnamese G2: Latin G2 with the hook above at 4/C (decision E.81).
    Vietnamese,
    /// Armenian G2: Latin G2 with և and the rarer Armenian punctuation in
    /// its unused positions (decision E.78).
    Armenian,
}

/// A Latin national option sub-set (EN 300 706 Table 36).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum NationalOption {
    /// Czech and Slovak.
    CzechSlovak,
    /// English.
    English,
    /// Estonian.
    Estonian,
    /// French.
    French,
    /// German.
    German,
    /// Italian.
    Italian,
    /// Lettish and Lithuanian.
    LettishLithuanian,
    /// Polish.
    Polish,
    /// Portuguese and Spanish.
    PortugueseSpanish,
    /// Rumanian.
    Rumanian,
    /// Serbian, Croatian and Slovenian.
    SerbianCroatianSlovenian,
    /// Swedish, Finnish and Hungarian.
    SwedishFinnishHungarian,
    /// Turkish.
    Turkish,
    /// Hausa and Fula (decision E.78).
    HausaFula,
    /// Akan, Lingala, Bambara and the other West African languages.
    WestAfrican,
    /// Ewe.
    Ewe,
    /// Kabyle and Tamazight in Latin script.
    KabyleTamazight,
    /// Azerbaijani.
    Azerbaijani,
    /// Kanuri and Mossi.
    KanuriMossi,
}

impl NationalOption {
    /// neoteletext's own options (decision E.78), in the order of their
    /// designations in group 0101.
    pub const EXTENDED: [Self; 6] =
        [Self::HausaFula, Self::WestAfrican, Self::Ewe, Self::KabyleTamazight, Self::Azerbaijani, Self::KanuriMossi];

    /// Every national option of the standard, in the order of Table 36.
    pub const ALL: [Self; 13] = [
        Self::CzechSlovak,
        Self::English,
        Self::Estonian,
        Self::French,
        Self::German,
        Self::Italian,
        Self::LettishLithuanian,
        Self::Polish,
        Self::PortugueseSpanish,
        Self::Rumanian,
        Self::SerbianCroatianSlovenian,
        Self::SwedishFinnishHungarian,
        Self::Turkish,
    ];
}

/// A G0 set together with the national option applied to it, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct G0 {
    /// The primary set.
    pub set: G0Set,
    /// The national option sub-set; only meaningful with [`G0Set::Latin`].
    pub national: Option<NationalOption>,
}

/// The character set of a stored character (REQ-CHR-001).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CharSet {
    /// A G0 set, with or without national option substitution.
    G0(G0),
    /// The G2 supplementary set.
    G2(G2Set),
    /// The G3 smooth mosaics and line drawing set (Table 48).
    G3,
    /// A dynamically redefinable character (clause 14): the character
    /// number is the [`CharRef`]'s code. It has no Unicode equivalent and
    /// converts to the fallback U+FFFD (REQ-CHR-011).
    Drcs {
        /// The table.
        table: DrcsTable,
        /// The sub-table, 0 to 15.
        subtable: u8,
    },
}

/// One character: a set, a 7-bit code and an optional diacritical mark.
///
/// `diacritic` is the column 4 position of the G2 mark (1 to 15), or
/// [`DIACRITIC_NONE`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct CharRef {
    /// The character set.
    pub set: CharSet,
    /// The 7-bit character code, 0x20 to 0x7F.
    pub code: u8,
    /// The diacritical mark number (REQ-CHR-008), 0 for none.
    pub diacritic: u8,
}

impl CharRef {
    /// A plain character with no mark.
    #[must_use]
    pub const fn new(set: CharSet, code: u8) -> Self {
        Self { set, code: code & CODE_MASK_7BIT, diacritic: DIACRITIC_NONE }
    }
}

/// The default sets of a page: Table 32 resolved (REQ-CHR-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Designation {
    /// The default G0 set and national option.
    pub g0: G0,
    /// The default G2 set.
    pub g2: G2Set,
}

/// A character converted to Unicode: a base scalar and an optional
/// combining mark (REQ-CHR-010, decision E.26).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unicode {
    /// The base character.
    pub base: char,
    /// The combining character to follow it, if any.
    pub combining: Option<char>,
}

/// Looks up Table 32 for a 7-bit designation value: group in bits 3 to 6,
/// national option index in bits 0 to 2. `None` when the entry is reserved.
#[must_use]
pub fn designation(value: u8) -> Option<Designation> {
    tables::designation(value & CODE_MASK_7BIT)
}

/// Resolves Table 32 with the fallback of REQ-CHR-003: a reserved entry is
/// first retried in group 0000 with the same national option bits, and then
/// English is used. The flag is true when a fallback was needed.
#[must_use]
pub fn designation_with_fallback(value: u8) -> (Designation, bool) {
    if let Some(found) = designation(value) {
        return (found, false);
    }
    let national_bits = value & tables::NATIONAL_BITS_MASK;
    if let Some(found) = designation(national_bits) {
        return (found, true);
    }
    (tables::ENGLISH_DESIGNATION, true)
}

/// Looks up Table 33 for a second G0 designation value. `None` when the
/// value is reserved or 0x7F ("no second G0 set required").
#[must_use]
pub fn second_g0(value: u8) -> Option<G0> {
    tables::second_g0(value & CODE_MASK_7BIT)
}

/// The mark a "G0 character with diacritical mark" triplet names, taken
/// from column 4 of the page's G2 set (REQ-CHR-014): the Vietnamese set's
/// 4/C is the hook above; every other mark is Latin G2's.
#[must_use]
pub fn page_mark(g2: G2Set, diacritic: u8) -> u8 {
    if g2 == G2Set::Vietnamese && diacritic == DIACRITIC_LOW_LINE { DIACRITIC_HOOK_ABOVE } else { diacritic }
}

/// The second G0 set a designation value implies when no Table 33
/// designation is received: Latin English for neoteletext's own values
/// (REQ-CHR-012), none for the standard's.
#[must_use]
pub fn default_second_g0(value: u8) -> Option<G0> {
    tables::default_second_g0(value & CODE_MASK_7BIT)
}

/// Looks up Table 34 for the data of a Modified G0 and G2 Character Set
/// Designation triplet (Table 29 mode 01000, clause 15.4): the G0 and G2
/// sets, or `None` when the value is reserved. A G0 set chosen this way is
/// used without national option (clause 15.4 note).
#[must_use]
pub fn modified_designation(value: u8) -> Option<(G0Set, G2Set)> {
    tables::modified_designation(value & CODE_MASK_7BIT)
}

impl CharRef {
    /// A DRCS character as a character reference.
    #[must_use]
    pub const fn drcs(character: DrcsCharacter) -> Self {
        Self::new(CharSet::Drcs { table: character.table, subtable: character.subtable }, character.character)
    }

    /// The DRCS character this refers to, if it is one.
    #[must_use]
    pub const fn drcs_character(self) -> Option<DrcsCharacter> {
        match self.set {
            CharSet::Drcs { table, subtable } => Some(DrcsCharacter { table, subtable, character: self.code }),
            CharSet::G0(_) | CharSet::G2(_) | CharSet::G3 => None,
        }
    }
}

/// Converts a character to Unicode (REQ-CHR-010). Codes outside 0x20 to 0x7F
/// give a space. Characters with no Unicode equivalent give the documented
/// fallback U+FFFD.
#[must_use]
pub fn to_unicode(character: CharRef) -> Unicode {
    let base = tables::base_unicode(character.set, character.code);
    let combining =
        if character.diacritic == DIACRITIC_NONE { None } else { tables::diacritic_combining(character.diacritic) };
    Unicode { base, combining }
}

/// Converts a block mosaic pattern to Unicode (REQ-CHR-005). `pattern` is
/// the 6-bit block pattern n, bit 0 the top-left block.
#[must_use]
pub fn mosaic_to_unicode(pattern: u8, separated: bool) -> char {
    tables::mosaic_unicode(pattern, separated)
}

/// The 6-bit block pattern of a G1 mosaic code (REQ-CHR-005):
/// n = (code AND 0x1F) OR ((code AND 0x40) >> 1).
#[must_use]
pub const fn mosaic_pattern(code: u8) -> u8 {
    (code & crate::consts::MOSAIC_LOW_BITS) | ((code & crate::consts::MOSAIC_BIT6) >> 1)
}

/// True when the national option of a G0 set may substitute this code:
/// the 13 positions of Table 36.
#[must_use]
pub fn is_national_position(code: u8) -> bool {
    tables::NATIONAL_POSITIONS.contains(&(code & CODE_MASK_7BIT))
}

/// Precomposes a base letter and a combining mark where Unicode has a
/// single code point for the pair (decision E.26, Normalization Form C).
#[must_use]
pub fn compose(base: char, combining: char) -> Option<char> {
    tables::compose(base, combining)
}

/// Text decoded with the Latin G0 set and no national option, for data
/// that carries no character set designation, such as the 8/30 status
/// message (REQ-SVC-005). Control codes show as spaces.
#[must_use]
pub fn latin_text<const N: usize>(codes: &[u8; N]) -> [char; N] {
    let latin = CharSet::G0(G0 { set: G0Set::Latin, national: None });
    codes.map(|code| to_unicode(CharRef::new(latin, code)).base)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
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
}
