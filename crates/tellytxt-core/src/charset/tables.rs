//! Character set tables of EN 300 706 clause 15, transcribed from the glyph
//! drawings of Tables 32 to 48 (area CHR).
//!
//! Every code table holds the 96 characters of codes 2/0 to 7/F, column by
//! column, one row of the source table per line. A glyph the standard draws
//! but Unicode cannot represent exactly is mapped to the nearest character
//! and the choice is commented at its position.

use super::{CharSet, Designation, G0, G0Set, G2Set, NationalOption};
use crate::consts::{MOSAIC_FULL, NATIONAL_INDEX_MASK, SPACE};

/// Mask of the national option bits of a Table 32 value.
pub(super) const NATIONAL_BITS_MASK: u8 = NATIONAL_INDEX_MASK;

/// English Latin G0 with Latin G2: the last-resort fallback.
pub(super) const ENGLISH_DESIGNATION: Designation = Designation {
    g0: G0 { set: G0Set::Latin, national: Some(NationalOption::English) },
    g2: G2Set::Latin,
};

/// The 13 national option positions of Table 36.
pub(super) use crate::consts::NATIONAL_POSITIONS;
use crate::consts::NO_SECOND_G0;

/// Codes 2/0 to 7/F: six columns of sixteen.
const SET_SIZE: usize = 96;

/// One character set, indexed by code − 2/0.
type CodeTable = [char; SET_SIZE];

use crate::consts::DESIGNATION_GROUP_SHIFT as GROUP_SHIFT;


fn lookup(table: &CodeTable, code: u8) -> char {
    code.checked_sub(SPACE)
        .and_then(|index| table.get(usize::from(index)))
        .copied()
        .unwrap_or(' ')
}

// ---------------------------------------------------------------------------
// Designation (Tables 32 and 33)
// ---------------------------------------------------------------------------

/// One defined entry of Table 32: group (bits 3 to 6), national option
/// index (bits 0 to 2), the G0 set with its national option, and the G2 set.
type Table32Entry = (u8, u8, G0, G2Set);

const fn latin(national: NationalOption) -> G0 {
    G0 { set: G0Set::Latin, national: Some(national) }
}

/// Table 32 note 3: the bracketed options have no sub-set of their own.
const fn whole(set: G0Set) -> G0 {
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

fn table_32_entry(value: u8) -> Option<(G0, G2Set)> {
    let group = value.wrapping_shr(GROUP_SHIFT);
    let index = value & NATIONAL_BITS_MASK;
    TABLE_32
        .iter()
        .find(|&&(entry_group, entry_index, _, _)| (entry_group, entry_index) == (group, index))
        .map(|&(_, _, g0, g2)| (g0, g2))
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
}

pub(super) fn second_g0(value: u8) -> Option<G0> {
    if value == NO_SECOND_G0 {
        return None;
    }
    // Table 33 lists the same G0 sets and options as Table 32 for every
    // value; only the G2 column is absent (clause 15.3).
    table_32_entry(value).map(|(g0, _)| g0)
}

// ---------------------------------------------------------------------------
// Latin G0 and the national option sub-sets (Tables 35 and 36)
// ---------------------------------------------------------------------------

/// Table 35. The shaded national option positions hold the base forms used
/// through packet X/26 (REQ-CHR-002). 7/C is drawn as a broken bar, the
/// ISO 646 rendering of the vertical line, so it maps to U+007C. 7/F is the
/// solid block of note 4.
const LATIN_G0: CodeTable = [
    ' ', '!', '"', '#', '¤', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', //
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_', //
    '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', //
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', '~', '■', //
];

/// One national option sub-set: the characters at the 13 positions of
/// [`NATIONAL_POSITIONS`], in that order.
type NationalSubset = [char; 13];

// Table 36, one row per option. Positions:
//  2/3  2/4  4/0  5/B  5/C  5/D  5/E  5/F  6/0  7/B  7/C  7/D  7/E
const CZECH_SLOVAK: NationalSubset =
    ['#', 'ů', 'č', 'ť', 'ž', 'ý', 'í', 'ř', 'é', 'á', 'ě', 'ú', 'š'];
// 6/0 is the SAA5050 long dash (U+2014) and 7/C the double vertical bar
// (U+2016); both are fixed by REQ-CHR-002.
const ENGLISH: NationalSubset =
    ['£', '$', '@', '←', '½', '→', '↑', '#', '—', '¼', '‖', '¾', '÷'];
const ESTONIAN: NationalSubset =
    ['#', 'õ', 'Š', 'Ä', 'Ö', 'Ž', 'Ü', 'Õ', 'š', 'ä', 'ö', 'ž', 'ü'];
const FRENCH: NationalSubset =
    ['é', 'ï', 'à', 'ë', 'ê', 'ù', 'î', '#', 'è', 'â', 'ô', 'û', 'ç'];
const GERMAN: NationalSubset =
    ['#', '$', '§', 'Ä', 'Ö', 'Ü', '^', '_', '°', 'ä', 'ö', 'ü', 'ß'];
const ITALIAN: NationalSubset =
    ['£', '$', 'é', '°', 'ç', '→', '↑', '#', 'ù', 'à', 'ò', 'è', 'ì'];
const LETTISH_LITHUANIAN: NationalSubset =
    ['#', '$', 'Š', 'ė', 'ę', 'Ž', 'č', 'ū', 'š', 'ą', 'ų', 'ž', 'į'];
// 5/B is Z with stroke (U+01B5) and 7/D l with stroke (U+0142), both drawn
// too small to read and fixed by REQ-CHR-002.
const POLISH: NationalSubset =
    ['#', 'ń', 'ą', 'Ƶ', 'Ś', 'Ł', 'ć', 'ó', 'ę', 'ż', 'ś', 'ł', 'ź'];
const PORTUGUESE_SPANISH: NationalSubset =
    ['ç', '$', '¡', 'á', 'é', 'í', 'ó', 'ú', '¿', 'ü', 'ñ', 'è', 'à'];
// The T and S at 4/0, 5/C, 6/0 and 7/C are drawn with a hook below that can
// be read as a cedilla or a comma; the cedilla forms are used, as in the
// pre-2003 Romanian code pages this sub-set was designed alongside.
const RUMANIAN: NationalSubset =
    ['#', '¤', 'Ţ', 'Â', 'Ş', 'Ă', 'Î', 'ı', 'ţ', 'â', 'ş', 'ă', 'î'];
const SERBIAN_CROATIAN_SLOVENIAN: NationalSubset =
    ['#', 'Ë', 'Č', 'Ć', 'Ž', 'Đ', 'Š', 'ë', 'č', 'ć', 'ž', 'đ', 'š'];
const SWEDISH_FINNISH_HUNGARIAN: NationalSubset =
    ['#', '¤', 'É', 'Ä', 'Ö', 'Å', 'Ü', '_', 'é', 'ä', 'ö', 'å', 'ü'];
// 2/3 is the Turkish currency sign of note 1, mapped to the Turkish lira
// sign U+20BA (REQ-CHR-002).
const TURKISH: NationalSubset =
    ['₺', 'ğ', 'İ', 'Ş', 'Ö', 'Ç', 'Ü', 'Ğ', 'ı', 'ş', 'ö', 'ç', 'ü'];

const fn national_subset(option: NationalOption) -> &'static NationalSubset {
    match option {
        NationalOption::CzechSlovak => &CZECH_SLOVAK,
        NationalOption::English => &ENGLISH,
        NationalOption::Estonian => &ESTONIAN,
        NationalOption::French => &FRENCH,
        NationalOption::German => &GERMAN,
        NationalOption::Italian => &ITALIAN,
        NationalOption::LettishLithuanian => &LETTISH_LITHUANIAN,
        NationalOption::Polish => &POLISH,
        NationalOption::PortugueseSpanish => &PORTUGUESE_SPANISH,
        NationalOption::Rumanian => &RUMANIAN,
        NationalOption::SerbianCroatianSlovenian => &SERBIAN_CROATIAN_SLOVENIAN,
        NationalOption::SwedishFinnishHungarian => &SWEDISH_FINNISH_HUNGARIAN,
        NationalOption::Turkish => &TURKISH,
    }
}

fn national_char(option: NationalOption, code: u8) -> Option<char> {
    let position = NATIONAL_POSITIONS.iter().position(|&candidate| candidate == code)?;
    national_subset(option).get(position).copied()
}

// ---------------------------------------------------------------------------
// Other G0 sets (Tables 38 to 40, 42, 44 and 46)
// ---------------------------------------------------------------------------

/// Table 38: Cyrillic G0 option 1, Serbian and Croatian.
const CYRILLIC_1_G0: CodeTable = [
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    'Ч', 'А', 'Б', 'Ц', 'Д', 'Е', 'Ф', 'Г', 'Х', 'И', 'Ј', 'К', 'Л', 'М', 'Н', 'О', //
    'П', 'Ќ', 'Р', 'С', 'Т', 'У', 'В', 'Ѓ', 'Љ', 'Њ', 'З', 'Ћ', 'Ж', 'Ђ', 'Ш', 'Џ', //
    'ч', 'а', 'б', 'ц', 'д', 'е', 'ф', 'г', 'х', 'и', 'ј', 'к', 'л', 'м', 'н', 'о', //
    'п', 'ќ', 'р', 'с', 'т', 'у', 'в', 'ѓ', 'љ', 'њ', 'з', 'ћ', 'ж', 'ђ', 'ш', '■', //
];

/// Table 39: Cyrillic G0 option 2, Russian and Bulgarian.
const CYRILLIC_2_G0: CodeTable = [
    ' ', '!', '"', '#', '$', '%', 'ы', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    'Ю', 'А', 'Б', 'Ц', 'Д', 'Е', 'Ф', 'Г', 'Х', 'И', 'Й', 'К', 'Л', 'М', 'Н', 'О', //
    'П', 'Я', 'Р', 'С', 'Т', 'У', 'Ж', 'В', 'Ь', 'Ъ', 'З', 'Ш', 'Э', 'Щ', 'Ч', 'Ы', //
    'ю', 'а', 'б', 'ц', 'д', 'е', 'ф', 'г', 'х', 'и', 'й', 'к', 'л', 'м', 'н', 'о', //
    'п', 'я', 'р', 'с', 'т', 'у', 'ж', 'в', 'ь', 'ъ', 'з', 'ш', 'э', 'щ', 'ч', '■', //
];

/// Table 40: Cyrillic G0 option 3, Ukrainian.
const CYRILLIC_3_G0: CodeTable = [
    ' ', '!', '"', '#', '$', '%', 'ї', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    'Ю', 'А', 'Б', 'Ц', 'Д', 'Е', 'Ф', 'Г', 'Х', 'И', 'Й', 'К', 'Л', 'М', 'Н', 'О', //
    'П', 'Я', 'Р', 'С', 'Т', 'У', 'Ж', 'В', 'Ь', 'І', 'З', 'Ш', 'Є', 'Щ', 'Ч', 'Ї', //
    'ю', 'а', 'б', 'ц', 'д', 'е', 'ф', 'г', 'х', 'и', 'й', 'к', 'л', 'м', 'н', 'о', //
    'п', 'я', 'р', 'с', 'т', 'у', 'ж', 'в', 'ь', 'і', 'з', 'ш', 'є', 'щ', 'ч', '■', //
];

/// Table 42: Greek G0. Columns 4 to 7 follow ISO 8859-7. 4/0 and 6/0 are
/// drawn with only the dialytika, as the 12 by 10 matrix has no room for the
/// tonos too; they are read as the ISO 8859-7 characters ΐ and ΰ. 5/2, which
/// ISO 8859-7 leaves unassigned, is drawn as a raised tick and is mapped to
/// the spacing tonos U+0384.
const GREEK_G0: CodeTable = [
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '«', '=', '»', '?', //
    'ΐ', 'Α', 'Β', 'Γ', 'Δ', 'Ε', 'Ζ', 'Η', 'Θ', 'Ι', 'Κ', 'Λ', 'Μ', 'Ν', 'Ξ', 'Ο', //
    'Π', 'Ρ', '΄', 'Σ', 'Τ', 'Υ', 'Φ', 'Χ', 'Ψ', 'Ω', 'Ϊ', 'Ϋ', 'ά', 'έ', 'ή', 'ί', //
    'ΰ', 'α', 'β', 'γ', 'δ', 'ε', 'ζ', 'η', 'θ', 'ι', 'κ', 'λ', 'μ', 'ν', 'ξ', 'ο', //
    'π', 'ρ', 'ς', 'σ', 'τ', 'υ', 'φ', 'χ', 'ψ', 'ω', 'ϊ', 'ϋ', 'ό', 'ύ', 'ώ', '■', //
];

/// Table 44: Arabic G0.
///
/// The table draws each letter in the cell forms a fixed-pitch display
/// needs. A glyph that joins on its left (an initial or medial form), or
/// that is explicitly a joined final form beside a separate unjoined form,
/// maps to its Arabic Presentation Forms-B code point, so that the form the
/// broadcaster chose survives; NFKC normalisation recovers the plain
/// letter. A glyph that serves as both isolated and final form maps to the
/// plain letter. Rows are in visual order, so the mirrored brackets and
/// comparison signs at 2/8, 2/9, 3/C and 3/E map to the character drawn.
/// Western digits are drawn in column 3.
const ARABIC_G0: CodeTable = [
    // 2/6 is the bowl that completes a final seen, sheen, sad or dad after
    // their initial forms (5/3 to 5/6); Unicode has no such fragment, and
    // the identical-looking final noon ghunna U+FB9F is used. 2/C is drawn
    // as a comma and read as the Arabic comma.
    ' ', '!', '"', '£', '$', '%', '\u{FB9F}', '\u{FEF2}', ')', '(', '*', '+', '\u{060C}', '-',
    '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', '\u{061B}', '>', '=', '<',
    '\u{061F}', //
    // teh marbuta final, hamza, beh medial, beh, teh medial, teh, alef final,
    // alef, beh initial, teh marbuta, teh initial, theh initial, jeem
    // initial, hah initial, khah initial, dal.
    '\u{FE94}', '\u{0621}', '\u{FE92}', '\u{0628}', '\u{FE98}', '\u{062A}', '\u{FE8E}',
    '\u{0627}', '\u{FE91}', '\u{0629}', '\u{FE97}', '\u{FE9B}', '\u{FE9F}', '\u{FEA3}',
    '\u{FEA7}', '\u{062F}', //
    // thal, reh, zain, seen initial, sheen initial, sad initial, dad
    // initial, tah initial, zah initial, ain initial, ghain initial, theh
    // medial, jeem medial, hah medial, khah medial, number sign.
    '\u{0630}', '\u{0631}', '\u{0632}', '\u{FEB3}', '\u{FEB7}', '\u{FEBB}', '\u{FEBF}',
    '\u{FEC3}', '\u{FEC7}', '\u{FECB}', '\u{FECF}', '\u{FE9C}', '\u{FEA0}', '\u{FEA4}',
    '\u{FEA8}', '#', //
    // tatweel, feh initial, qaf initial, kaf initial (drawn joined on both
    // sides, as the medial form also would be), lam initial, meem initial,
    // noon initial, heh initial, waw, alef maksura final, yeh initial, theh,
    // jeem, hah, khah, yeh medial.
    '\u{0640}', '\u{FED3}', '\u{FED7}', '\u{FEDB}', '\u{FEDF}', '\u{FEE3}', '\u{FEE7}',
    '\u{FEEB}', '\u{0648}', '\u{FEF0}', '\u{FEF3}', '\u{062B}', '\u{062C}', '\u{062D}',
    '\u{062E}', '\u{FEF4}', //
    // alef maksura, ain medial, ghain medial, feh medial, feh, qaf medial,
    // qaf, kaf, lam medial, lam, meem medial, meem, noon medial, noon, lam
    // with alef isolated, solid block.
    '\u{0649}', '\u{FECC}', '\u{FED0}', '\u{FED4}', '\u{0641}', '\u{FED8}', '\u{0642}',
    '\u{0643}', '\u{FEE0}', '\u{0644}', '\u{FEE4}', '\u{0645}', '\u{FEE8}', '\u{0646}',
    '\u{FEFB}', '■', //
];

/// Table 46: Hebrew G0. Column 4 and 5/0 to 5/A are Latin capitals; the
/// Hebrew letters run from 6/0 in alphabetical order. 7/B is the new shekel
/// sign of note 4 and 7/C the double vertical bar of the English sub-set.
const HEBREW_G0: CodeTable = [
    ' ', '!', '"', '£', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', //
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '←', '½', '→', '↑', '#', //
    // alef to final nun.
    '\u{05D0}', '\u{05D1}', '\u{05D2}', '\u{05D3}', '\u{05D4}', '\u{05D5}', '\u{05D6}',
    '\u{05D7}', '\u{05D8}', '\u{05D9}', '\u{05DA}', '\u{05DB}', '\u{05DC}', '\u{05DD}',
    '\u{05DE}', '\u{05DF}', //
    // nun to tav, then the shekel sign.
    '\u{05E0}', '\u{05E1}', '\u{05E2}', '\u{05E3}', '\u{05E4}', '\u{05E5}', '\u{05E6}',
    '\u{05E7}', '\u{05E8}', '\u{05E9}', '\u{05EA}', '\u{20AA}', '‖', '¾', '÷', '■', //
];

fn g0_unicode(g0: G0, code: u8) -> char {
    let table = match g0.set {
        G0Set::Latin => {
            if let Some(found) = g0.national.and_then(|option| national_char(option, code)) {
                return found;
            }
            &LATIN_G0
        }
        G0Set::Cyrillic1 => &CYRILLIC_1_G0,
        G0Set::Cyrillic2 => &CYRILLIC_2_G0,
        G0Set::Cyrillic3 => &CYRILLIC_3_G0,
        G0Set::Greek => &GREEK_G0,
        G0Set::Arabic => &ARABIC_G0,
        G0Set::Hebrew => &HEBREW_G0,
    };
    lookup(table, code)
}

// ---------------------------------------------------------------------------
// G2 sets (Tables 37, 41, 43 and 45)
// ---------------------------------------------------------------------------

// Column 4 of the Latin, Cyrillic and Greek G2 sets holds the diacritical
// marks (REQ-CHR-008). Shown on their own they use the spacing form of each
// mark. Dot below (4/9) has no spacing form in Unicode; the full stop is
// the character that looks like the drawn low dot. Low line (4/C) uses the
// spacing low line, the counterpart of the combining low line U+0332.

/// Table 37: Latin G2. 5/6 is drawn as the old "CE" sign, but note 4 says
/// it is the European currency symbol, so it maps to the euro sign. 5/8 is
/// the Greek alpha. 5/9, 5/A, 5/B and 6/5 must not be transmitted (note 5)
/// and display as a space (REQ-CHR-006). 7/F is the solid block of note 6.
const LATIN_G2: CodeTable = [
    ' ', '¡', '¢', '£', '$', '¥', '#', '§', '¤', '‘', '“', '«', '←', '↑', '→', '↓', //
    '°', '±', '²', '³', '×', 'µ', '¶', '·', '÷', '’', '”', '»', '¼', '½', '¾', '¿', //
    ' ', '`', '´', 'ˆ', '˜', '¯', '˘', '˙', '¨', '.', '˚', '¸', '_', '˝', '˛', 'ˇ', //
    '―', '¹', '®', '©', '™', '♪', '€', '‰', 'α', ' ', ' ', ' ', '⅛', '⅜', '⅝', '⅞', //
    'Ω', 'Æ', 'Đ', 'ª', 'Ħ', ' ', 'Ĳ', 'Ŀ', 'Ł', 'Ø', 'Œ', 'º', 'Þ', 'Ŧ', 'Ŋ', 'ŉ', //
    'ĸ', 'æ', 'đ', 'ð', 'ħ', 'ı', 'ĳ', 'ŀ', 'ł', 'ø', 'œ', 'ß', 'þ', 'ŧ', 'ŋ', '■', //
];

/// Table 41: Cyrillic G2. 2/6 and 2/8 must not be transmitted (note 2) and
/// display as a space. 5/A is drawn as the Polish 7/D glyph and is read the
/// same way, as l with stroke; 5/B is the sharp s, drawn as in Table 37 7/B.
/// Columns 6 and 7 hold the Latin letters that the Cyrillic G0 sets lack.
const CYRILLIC_G2: CodeTable = [
    ' ', '¡', '¢', '£', '$', '¥', ' ', '§', ' ', '‘', '“', '«', '←', '↑', '→', '↓', //
    '°', '±', '²', '³', '×', 'µ', '¶', '·', '÷', '’', '”', '»', '¼', '½', '¾', '¿', //
    ' ', '`', '´', 'ˆ', '˜', '¯', '˘', '˙', '¨', '.', '˚', '¸', '_', '˝', '˛', 'ˇ', //
    '―', '¹', '®', '©', '™', '♪', '€', '‰', 'α', 'Ł', 'ł', 'ß', '⅛', '⅜', '⅝', '⅞', //
    'D', 'E', 'F', 'G', 'I', 'J', 'K', 'L', 'N', 'Q', 'R', 'S', 'U', 'V', 'W', 'Z', //
    'd', 'e', 'f', 'g', 'i', 'j', 'k', 'l', 'n', 'q', 'r', 's', 'u', 'v', 'w', 'z', //
];

/// Table 43: Greek G2. Columns 2, 3, 6 and 7 add the Latin letters whose
/// shapes the Greek G0 set lacks. 3/4 is the multiplication sign, drawn as
/// in Table 37, and 3/F the letter x. 2/8 is drawn as two stacked dots and
/// read as a colon. 5/0 is the Latin question mark (the Greek question mark
/// is the G0 semicolon). 5/9 to 5/B, 6/E, 6/F and 7/E are capitals with
/// tonos.
const GREEK_G2: CodeTable = [
    ' ', 'a', 'b', '£', 'e', 'h', 'i', '§', ':', '‘', '“', 'k', '←', '↑', '→', '↓', //
    '°', '±', '²', '³', '×', 'm', 'n', 'p', '÷', '’', '”', 't', '¼', '½', '¾', 'x', //
    ' ', '`', '´', 'ˆ', '˜', '¯', '˘', '˙', '¨', '.', '˚', '¸', '_', '˝', '˛', 'ˇ', //
    '?', '¹', '®', '©', '™', '♪', '€', '‰', 'α', 'Ί', 'Ύ', 'Ώ', '⅛', '⅜', '⅝', '⅞', //
    'C', 'D', 'F', 'G', 'J', 'L', 'Q', 'R', 'S', 'U', 'V', 'W', 'Y', 'Z', 'Ά', 'Ή', //
    'c', 'd', 'f', 'g', 'j', 'l', 'q', 'r', 's', 'u', 'v', 'w', 'y', 'z', 'Έ', '■', //
];

/// Table 45: Arabic G2, used with the Arabic and Hebrew G0 sets. Columns 2
/// and 3 add hamza forms, the Persian letters peh, tcheh, jeh and gaf,
/// further letter forms and the Arabic-Indic digits; columns 4 to 7 are a
/// Latin alphabet with French accented letters. Letter forms follow the
/// rule of [`ARABIC_G0`]. 5/F is drawn as the final ain, the one ain form
/// missing from Table 44. 7/F is drawn empty and displays as a space.
const ARABIC_G2: CodeTable = [
    // space, ain, alef with madda, alef with hamza above, waw with hamza,
    // alef with hamza below, yeh with hamza initial, yeh with hamza final,
    // tcheh initial, tcheh medial, tcheh, peh initial, peh medial, peh, jeh,
    // gaf initial (drawn joined on both sides like kaf at Table 44 6/3).
    ' ', '\u{0639}', '\u{0622}', '\u{0623}', '\u{0624}', '\u{0625}', '\u{FE8B}', '\u{FE8A}',
    '\u{FB7C}', '\u{FB7D}', '\u{0686}', '\u{FB58}', '\u{FB59}', '\u{067E}', '\u{0698}',
    '\u{FB94}', //
    // Arabic-Indic digits zero to nine, ghain final, ghain, lam with alef
    // final, heh medial, heh final, heh.
    '\u{0660}', '\u{0661}', '\u{0662}', '\u{0663}', '\u{0664}', '\u{0665}', '\u{0666}',
    '\u{0667}', '\u{0668}', '\u{0669}', '\u{FECE}', '\u{063A}', '\u{FEFC}', '\u{FEEC}',
    '\u{FEEA}', '\u{0647}', //
    'à', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', //
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'ë', 'ê', 'ù', 'î', '\u{FECA}', //
    'é', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', //
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'â', 'ô', 'û', 'ç', ' ', //
];

const fn g2_table(set: G2Set) -> &'static CodeTable {
    match set {
        G2Set::Latin => &LATIN_G2,
        G2Set::Cyrillic => &CYRILLIC_G2,
        G2Set::Greek => &GREEK_G2,
        G2Set::Arabic => &ARABIC_G2,
    }
}

// ---------------------------------------------------------------------------
// G3 smooth mosaics and line drawing (Table 48)
// ---------------------------------------------------------------------------

/// Table 48. The smooth mosaics map to the matching shapes of Symbols for
/// Legacy Computing (U+1FB3C to U+1FB6F, U+1FB70, U+1FB75, U+1FB95) and of
/// Geometric Shapes; the diagonal line pieces of column 4 to U+1FBA0 to
/// U+1FBA7. Line pieces that join a thick bar to a thin line (4/0 to 4/3
/// and 4/C) use the box drawing characters with a heavy horizontal or
/// right/left arm and light other arms; all other lines are light. 4/D,
/// 4/E and 4/F are a small dot, a large dot and a ring. 5/F is the
/// background-colour space of note 1, and 6/E, 6/F, 7/E and 7/F are
/// unassigned (note 2); all four display as a space (REQ-CHR-007).
const G3: CodeTable = [
    '\u{1FB3C}', '\u{1FB3D}', '\u{1FB3E}', '\u{1FB3F}', '\u{1FB40}', '\u{25E3}', '\u{1FB41}',
    '\u{1FB42}', '\u{1FB43}', '\u{1FB44}', '\u{1FB45}', '\u{1FB46}', '\u{1FB68}', '\u{1FB69}',
    '\u{1FB70}', '\u{1FB95}', //
    '\u{1FB47}', '\u{1FB48}', '\u{1FB49}', '\u{1FB4A}', '\u{1FB4B}', '\u{25E2}', '\u{1FB4C}',
    '\u{1FB4D}', '\u{1FB4E}', '\u{1FB4F}', '\u{1FB50}', '\u{1FB51}', '\u{1FB6A}', '\u{1FB6B}',
    '\u{1FB75}', '\u{2588}', //
    '\u{2537}', '\u{252F}', '\u{251D}', '\u{2525}', '\u{1FBA4}', '\u{1FBA5}', '\u{1FBA6}',
    '\u{1FBA7}', '\u{1FBA0}', '\u{1FBA1}', '\u{1FBA2}', '\u{1FBA3}', '\u{253F}', '\u{2022}',
    '\u{25CF}', '\u{25CB}', //
    '\u{2502}', '\u{2500}', '\u{250C}', '\u{2510}', '\u{2514}', '\u{2518}', '\u{251C}',
    '\u{2524}', '\u{252C}', '\u{2534}', '\u{253C}', '\u{2192}', '\u{2190}', '\u{2191}',
    '\u{2193}', ' ', //
    '\u{1FB52}', '\u{1FB53}', '\u{1FB54}', '\u{1FB55}', '\u{1FB56}', '\u{25E5}', '\u{1FB57}',
    '\u{1FB58}', '\u{1FB59}', '\u{1FB5A}', '\u{1FB5B}', '\u{1FB5C}', '\u{1FB6C}', '\u{1FB6D}',
    ' ', ' ', //
    '\u{1FB5D}', '\u{1FB5E}', '\u{1FB5F}', '\u{1FB60}', '\u{1FB61}', '\u{25E4}', '\u{1FB62}',
    '\u{1FB63}', '\u{1FB64}', '\u{1FB65}', '\u{1FB66}', '\u{1FB67}', '\u{1FB6E}', '\u{1FB6F}',
    ' ', ' ', //
];

/// The documented fallback of REQ-CHR-010 for characters with no Unicode
/// equivalent: U+FFFD REPLACEMENT CHARACTER.
const NO_EQUIVALENT: char = '\u{FFFD}';

pub(super) fn base_unicode(set: CharSet, code: u8) -> char {
    match set {
        CharSet::G0(g0) => g0_unicode(g0, code),
        CharSet::G2(g2) => lookup(g2_table(g2), code),
        CharSet::G3 => lookup(&G3, code),
        // REQ-CHR-011: a DRCS character has no Unicode equivalent.
        CharSet::Drcs { .. } => NO_EQUIVALENT,
    }
}

// ---------------------------------------------------------------------------
// Diacritical marks (G2 column 4, REQ-CHR-008)
// ---------------------------------------------------------------------------

/// The combining character of each column 4 mark, indexed by mark number.
/// 4/0 is "no mark".
const COMBINING_MARKS: [Option<char>; 16] = [
    None,
    Some('\u{0300}'), // 4/1 grave
    Some('\u{0301}'), // 4/2 acute
    Some('\u{0302}'), // 4/3 circumflex
    Some('\u{0303}'), // 4/4 tilde
    Some('\u{0304}'), // 4/5 macron
    Some('\u{0306}'), // 4/6 breve
    Some('\u{0307}'), // 4/7 dot above
    Some('\u{0308}'), // 4/8 diaeresis
    Some('\u{0323}'), // 4/9 dot below
    Some('\u{030A}'), // 4/A ring above
    Some('\u{0327}'), // 4/B cedilla
    Some('\u{0332}'), // 4/C low line
    Some('\u{030B}'), // 4/D double acute
    Some('\u{0328}'), // 4/E ogonek
    Some('\u{030C}'), // 4/F caron
];

pub(super) fn diacritic_combining(mark: u8) -> Option<char> {
    COMBINING_MARKS.get(usize::from(mark)).copied().flatten()
}

// ---------------------------------------------------------------------------
// Block mosaics (Table 47, REQ-CHR-005)
// ---------------------------------------------------------------------------

/// BLOCK SEXTANT-1 (Symbols for Legacy Computing): the first contiguous
/// sextant, for pattern 1.
const SEXTANT_FIRST: u32 = 0x1_FB00;
/// SEPARATED BLOCK SEXTANT-1 is one above this (Symbols for Legacy
/// Computing Supplement, Unicode 16), so pattern n maps to this + n.
const SEPARATED_SEXTANT_BASE: u32 = 0x1_CE50;
/// The three left-hand blocks: bits 0, 2 and 4 of the pattern.
const MOSAIC_LEFT_COLUMN: u8 = 0b01_0101;
/// The three right-hand blocks: bits 1, 3 and 5 of the pattern.
const MOSAIC_RIGHT_COLUMN: u8 = 0b10_1010;
/// LEFT HALF BLOCK, the contiguous form of [`MOSAIC_LEFT_COLUMN`].
const LEFT_HALF_BLOCK: char = '\u{258C}';
/// RIGHT HALF BLOCK, the contiguous form of [`MOSAIC_RIGHT_COLUMN`].
const RIGHT_HALF_BLOCK: char = '\u{2590}';
/// FULL BLOCK, the contiguous form of [`MOSAIC_FULL`].
const FULL_BLOCK: char = '\u{2588}';

pub(super) fn mosaic_unicode(pattern: u8, separated: bool) -> char {
    let pattern = pattern & MOSAIC_FULL;
    if pattern == 0 {
        return ' ';
    }
    if separated {
        let scalar = SEPARATED_SEXTANT_BASE.wrapping_add(u32::from(pattern));
        return char::from_u32(scalar).unwrap_or(char::REPLACEMENT_CHARACTER);
    }
    match pattern {
        MOSAIC_LEFT_COLUMN => LEFT_HALF_BLOCK,
        MOSAIC_RIGHT_COLUMN => RIGHT_HALF_BLOCK,
        MOSAIC_FULL => FULL_BLOCK,
        _ => {
            // The sextant block skips the patterns that already exist as
            // half and full blocks; pattern 0 is the space.
            let skipped =
                1_u8.wrapping_add(u8::from(pattern > MOSAIC_LEFT_COLUMN))
                    .wrapping_add(u8::from(pattern > MOSAIC_RIGHT_COLUMN));
            let offset = u32::from(pattern.wrapping_sub(skipped));
            char::from_u32(SEXTANT_FIRST.wrapping_add(offset))
                .unwrap_or(char::REPLACEMENT_CHARACTER)
        }
    }
}

// ---------------------------------------------------------------------------
// Composition (decision E.26)
// ---------------------------------------------------------------------------

/// Every Unicode Normalization Form C precomposition of a Latin letter A to
/// Z or a to z (and dotless i, which has none) with one of the fifteen
/// combining marks of [`COMBINING_MARKS`]: (base, mark, composed). Generated
/// with Python's `unicodedata.normalize("NFC", base + mark)` (Unicode 16.0)
/// and sorted by base, then mark, for binary search.
const COMPOSITIONS: [(char, char, char); 289] = [
    ('A', '\u{0300}', '\u{00C0}'), // À
    ('A', '\u{0301}', '\u{00C1}'), // Á
    ('A', '\u{0302}', '\u{00C2}'), // Â
    ('A', '\u{0303}', '\u{00C3}'), // Ã
    ('A', '\u{0304}', '\u{0100}'), // Ā
    ('A', '\u{0306}', '\u{0102}'), // Ă
    ('A', '\u{0307}', '\u{0226}'), // Ȧ
    ('A', '\u{0308}', '\u{00C4}'), // Ä
    ('A', '\u{030A}', '\u{00C5}'), // Å
    ('A', '\u{030C}', '\u{01CD}'), // Ǎ
    ('A', '\u{0323}', '\u{1EA0}'), // Ạ
    ('A', '\u{0328}', '\u{0104}'), // Ą
    ('B', '\u{0307}', '\u{1E02}'), // Ḃ
    ('B', '\u{0323}', '\u{1E04}'), // Ḅ
    ('C', '\u{0301}', '\u{0106}'), // Ć
    ('C', '\u{0302}', '\u{0108}'), // Ĉ
    ('C', '\u{0307}', '\u{010A}'), // Ċ
    ('C', '\u{030C}', '\u{010C}'), // Č
    ('C', '\u{0327}', '\u{00C7}'), // Ç
    ('D', '\u{0307}', '\u{1E0A}'), // Ḋ
    ('D', '\u{030C}', '\u{010E}'), // Ď
    ('D', '\u{0323}', '\u{1E0C}'), // Ḍ
    ('D', '\u{0327}', '\u{1E10}'), // Ḑ
    ('E', '\u{0300}', '\u{00C8}'), // È
    ('E', '\u{0301}', '\u{00C9}'), // É
    ('E', '\u{0302}', '\u{00CA}'), // Ê
    ('E', '\u{0303}', '\u{1EBC}'), // Ẽ
    ('E', '\u{0304}', '\u{0112}'), // Ē
    ('E', '\u{0306}', '\u{0114}'), // Ĕ
    ('E', '\u{0307}', '\u{0116}'), // Ė
    ('E', '\u{0308}', '\u{00CB}'), // Ë
    ('E', '\u{030C}', '\u{011A}'), // Ě
    ('E', '\u{0323}', '\u{1EB8}'), // Ẹ
    ('E', '\u{0327}', '\u{0228}'), // Ȩ
    ('E', '\u{0328}', '\u{0118}'), // Ę
    ('F', '\u{0307}', '\u{1E1E}'), // Ḟ
    ('G', '\u{0301}', '\u{01F4}'), // Ǵ
    ('G', '\u{0302}', '\u{011C}'), // Ĝ
    ('G', '\u{0304}', '\u{1E20}'), // Ḡ
    ('G', '\u{0306}', '\u{011E}'), // Ğ
    ('G', '\u{0307}', '\u{0120}'), // Ġ
    ('G', '\u{030C}', '\u{01E6}'), // Ǧ
    ('G', '\u{0327}', '\u{0122}'), // Ģ
    ('H', '\u{0302}', '\u{0124}'), // Ĥ
    ('H', '\u{0307}', '\u{1E22}'), // Ḣ
    ('H', '\u{0308}', '\u{1E26}'), // Ḧ
    ('H', '\u{030C}', '\u{021E}'), // Ȟ
    ('H', '\u{0323}', '\u{1E24}'), // Ḥ
    ('H', '\u{0327}', '\u{1E28}'), // Ḩ
    ('I', '\u{0300}', '\u{00CC}'), // Ì
    ('I', '\u{0301}', '\u{00CD}'), // Í
    ('I', '\u{0302}', '\u{00CE}'), // Î
    ('I', '\u{0303}', '\u{0128}'), // Ĩ
    ('I', '\u{0304}', '\u{012A}'), // Ī
    ('I', '\u{0306}', '\u{012C}'), // Ĭ
    ('I', '\u{0307}', '\u{0130}'), // İ
    ('I', '\u{0308}', '\u{00CF}'), // Ï
    ('I', '\u{030C}', '\u{01CF}'), // Ǐ
    ('I', '\u{0323}', '\u{1ECA}'), // Ị
    ('I', '\u{0328}', '\u{012E}'), // Į
    ('J', '\u{0302}', '\u{0134}'), // Ĵ
    ('K', '\u{0301}', '\u{1E30}'), // Ḱ
    ('K', '\u{030C}', '\u{01E8}'), // Ǩ
    ('K', '\u{0323}', '\u{1E32}'), // Ḳ
    ('K', '\u{0327}', '\u{0136}'), // Ķ
    ('L', '\u{0301}', '\u{0139}'), // Ĺ
    ('L', '\u{030C}', '\u{013D}'), // Ľ
    ('L', '\u{0323}', '\u{1E36}'), // Ḷ
    ('L', '\u{0327}', '\u{013B}'), // Ļ
    ('M', '\u{0301}', '\u{1E3E}'), // Ḿ
    ('M', '\u{0307}', '\u{1E40}'), // Ṁ
    ('M', '\u{0323}', '\u{1E42}'), // Ṃ
    ('N', '\u{0300}', '\u{01F8}'), // Ǹ
    ('N', '\u{0301}', '\u{0143}'), // Ń
    ('N', '\u{0303}', '\u{00D1}'), // Ñ
    ('N', '\u{0307}', '\u{1E44}'), // Ṅ
    ('N', '\u{030C}', '\u{0147}'), // Ň
    ('N', '\u{0323}', '\u{1E46}'), // Ṇ
    ('N', '\u{0327}', '\u{0145}'), // Ņ
    ('O', '\u{0300}', '\u{00D2}'), // Ò
    ('O', '\u{0301}', '\u{00D3}'), // Ó
    ('O', '\u{0302}', '\u{00D4}'), // Ô
    ('O', '\u{0303}', '\u{00D5}'), // Õ
    ('O', '\u{0304}', '\u{014C}'), // Ō
    ('O', '\u{0306}', '\u{014E}'), // Ŏ
    ('O', '\u{0307}', '\u{022E}'), // Ȯ
    ('O', '\u{0308}', '\u{00D6}'), // Ö
    ('O', '\u{030B}', '\u{0150}'), // Ő
    ('O', '\u{030C}', '\u{01D1}'), // Ǒ
    ('O', '\u{0323}', '\u{1ECC}'), // Ọ
    ('O', '\u{0328}', '\u{01EA}'), // Ǫ
    ('P', '\u{0301}', '\u{1E54}'), // Ṕ
    ('P', '\u{0307}', '\u{1E56}'), // Ṗ
    ('R', '\u{0301}', '\u{0154}'), // Ŕ
    ('R', '\u{0307}', '\u{1E58}'), // Ṙ
    ('R', '\u{030C}', '\u{0158}'), // Ř
    ('R', '\u{0323}', '\u{1E5A}'), // Ṛ
    ('R', '\u{0327}', '\u{0156}'), // Ŗ
    ('S', '\u{0301}', '\u{015A}'), // Ś
    ('S', '\u{0302}', '\u{015C}'), // Ŝ
    ('S', '\u{0307}', '\u{1E60}'), // Ṡ
    ('S', '\u{030C}', '\u{0160}'), // Š
    ('S', '\u{0323}', '\u{1E62}'), // Ṣ
    ('S', '\u{0327}', '\u{015E}'), // Ş
    ('T', '\u{0307}', '\u{1E6A}'), // Ṫ
    ('T', '\u{030C}', '\u{0164}'), // Ť
    ('T', '\u{0323}', '\u{1E6C}'), // Ṭ
    ('T', '\u{0327}', '\u{0162}'), // Ţ
    ('U', '\u{0300}', '\u{00D9}'), // Ù
    ('U', '\u{0301}', '\u{00DA}'), // Ú
    ('U', '\u{0302}', '\u{00DB}'), // Û
    ('U', '\u{0303}', '\u{0168}'), // Ũ
    ('U', '\u{0304}', '\u{016A}'), // Ū
    ('U', '\u{0306}', '\u{016C}'), // Ŭ
    ('U', '\u{0308}', '\u{00DC}'), // Ü
    ('U', '\u{030A}', '\u{016E}'), // Ů
    ('U', '\u{030B}', '\u{0170}'), // Ű
    ('U', '\u{030C}', '\u{01D3}'), // Ǔ
    ('U', '\u{0323}', '\u{1EE4}'), // Ụ
    ('U', '\u{0328}', '\u{0172}'), // Ų
    ('V', '\u{0303}', '\u{1E7C}'), // Ṽ
    ('V', '\u{0323}', '\u{1E7E}'), // Ṿ
    ('W', '\u{0300}', '\u{1E80}'), // Ẁ
    ('W', '\u{0301}', '\u{1E82}'), // Ẃ
    ('W', '\u{0302}', '\u{0174}'), // Ŵ
    ('W', '\u{0307}', '\u{1E86}'), // Ẇ
    ('W', '\u{0308}', '\u{1E84}'), // Ẅ
    ('W', '\u{0323}', '\u{1E88}'), // Ẉ
    ('X', '\u{0307}', '\u{1E8A}'), // Ẋ
    ('X', '\u{0308}', '\u{1E8C}'), // Ẍ
    ('Y', '\u{0300}', '\u{1EF2}'), // Ỳ
    ('Y', '\u{0301}', '\u{00DD}'), // Ý
    ('Y', '\u{0302}', '\u{0176}'), // Ŷ
    ('Y', '\u{0303}', '\u{1EF8}'), // Ỹ
    ('Y', '\u{0304}', '\u{0232}'), // Ȳ
    ('Y', '\u{0307}', '\u{1E8E}'), // Ẏ
    ('Y', '\u{0308}', '\u{0178}'), // Ÿ
    ('Y', '\u{0323}', '\u{1EF4}'), // Ỵ
    ('Z', '\u{0301}', '\u{0179}'), // Ź
    ('Z', '\u{0302}', '\u{1E90}'), // Ẑ
    ('Z', '\u{0307}', '\u{017B}'), // Ż
    ('Z', '\u{030C}', '\u{017D}'), // Ž
    ('Z', '\u{0323}', '\u{1E92}'), // Ẓ
    ('a', '\u{0300}', '\u{00E0}'), // à
    ('a', '\u{0301}', '\u{00E1}'), // á
    ('a', '\u{0302}', '\u{00E2}'), // â
    ('a', '\u{0303}', '\u{00E3}'), // ã
    ('a', '\u{0304}', '\u{0101}'), // ā
    ('a', '\u{0306}', '\u{0103}'), // ă
    ('a', '\u{0307}', '\u{0227}'), // ȧ
    ('a', '\u{0308}', '\u{00E4}'), // ä
    ('a', '\u{030A}', '\u{00E5}'), // å
    ('a', '\u{030C}', '\u{01CE}'), // ǎ
    ('a', '\u{0323}', '\u{1EA1}'), // ạ
    ('a', '\u{0328}', '\u{0105}'), // ą
    ('b', '\u{0307}', '\u{1E03}'), // ḃ
    ('b', '\u{0323}', '\u{1E05}'), // ḅ
    ('c', '\u{0301}', '\u{0107}'), // ć
    ('c', '\u{0302}', '\u{0109}'), // ĉ
    ('c', '\u{0307}', '\u{010B}'), // ċ
    ('c', '\u{030C}', '\u{010D}'), // č
    ('c', '\u{0327}', '\u{00E7}'), // ç
    ('d', '\u{0307}', '\u{1E0B}'), // ḋ
    ('d', '\u{030C}', '\u{010F}'), // ď
    ('d', '\u{0323}', '\u{1E0D}'), // ḍ
    ('d', '\u{0327}', '\u{1E11}'), // ḑ
    ('e', '\u{0300}', '\u{00E8}'), // è
    ('e', '\u{0301}', '\u{00E9}'), // é
    ('e', '\u{0302}', '\u{00EA}'), // ê
    ('e', '\u{0303}', '\u{1EBD}'), // ẽ
    ('e', '\u{0304}', '\u{0113}'), // ē
    ('e', '\u{0306}', '\u{0115}'), // ĕ
    ('e', '\u{0307}', '\u{0117}'), // ė
    ('e', '\u{0308}', '\u{00EB}'), // ë
    ('e', '\u{030C}', '\u{011B}'), // ě
    ('e', '\u{0323}', '\u{1EB9}'), // ẹ
    ('e', '\u{0327}', '\u{0229}'), // ȩ
    ('e', '\u{0328}', '\u{0119}'), // ę
    ('f', '\u{0307}', '\u{1E1F}'), // ḟ
    ('g', '\u{0301}', '\u{01F5}'), // ǵ
    ('g', '\u{0302}', '\u{011D}'), // ĝ
    ('g', '\u{0304}', '\u{1E21}'), // ḡ
    ('g', '\u{0306}', '\u{011F}'), // ğ
    ('g', '\u{0307}', '\u{0121}'), // ġ
    ('g', '\u{030C}', '\u{01E7}'), // ǧ
    ('g', '\u{0327}', '\u{0123}'), // ģ
    ('h', '\u{0302}', '\u{0125}'), // ĥ
    ('h', '\u{0307}', '\u{1E23}'), // ḣ
    ('h', '\u{0308}', '\u{1E27}'), // ḧ
    ('h', '\u{030C}', '\u{021F}'), // ȟ
    ('h', '\u{0323}', '\u{1E25}'), // ḥ
    ('h', '\u{0327}', '\u{1E29}'), // ḩ
    ('i', '\u{0300}', '\u{00EC}'), // ì
    ('i', '\u{0301}', '\u{00ED}'), // í
    ('i', '\u{0302}', '\u{00EE}'), // î
    ('i', '\u{0303}', '\u{0129}'), // ĩ
    ('i', '\u{0304}', '\u{012B}'), // ī
    ('i', '\u{0306}', '\u{012D}'), // ĭ
    ('i', '\u{0308}', '\u{00EF}'), // ï
    ('i', '\u{030C}', '\u{01D0}'), // ǐ
    ('i', '\u{0323}', '\u{1ECB}'), // ị
    ('i', '\u{0328}', '\u{012F}'), // į
    ('j', '\u{0302}', '\u{0135}'), // ĵ
    ('j', '\u{030C}', '\u{01F0}'), // ǰ
    ('k', '\u{0301}', '\u{1E31}'), // ḱ
    ('k', '\u{030C}', '\u{01E9}'), // ǩ
    ('k', '\u{0323}', '\u{1E33}'), // ḳ
    ('k', '\u{0327}', '\u{0137}'), // ķ
    ('l', '\u{0301}', '\u{013A}'), // ĺ
    ('l', '\u{030C}', '\u{013E}'), // ľ
    ('l', '\u{0323}', '\u{1E37}'), // ḷ
    ('l', '\u{0327}', '\u{013C}'), // ļ
    ('m', '\u{0301}', '\u{1E3F}'), // ḿ
    ('m', '\u{0307}', '\u{1E41}'), // ṁ
    ('m', '\u{0323}', '\u{1E43}'), // ṃ
    ('n', '\u{0300}', '\u{01F9}'), // ǹ
    ('n', '\u{0301}', '\u{0144}'), // ń
    ('n', '\u{0303}', '\u{00F1}'), // ñ
    ('n', '\u{0307}', '\u{1E45}'), // ṅ
    ('n', '\u{030C}', '\u{0148}'), // ň
    ('n', '\u{0323}', '\u{1E47}'), // ṇ
    ('n', '\u{0327}', '\u{0146}'), // ņ
    ('o', '\u{0300}', '\u{00F2}'), // ò
    ('o', '\u{0301}', '\u{00F3}'), // ó
    ('o', '\u{0302}', '\u{00F4}'), // ô
    ('o', '\u{0303}', '\u{00F5}'), // õ
    ('o', '\u{0304}', '\u{014D}'), // ō
    ('o', '\u{0306}', '\u{014F}'), // ŏ
    ('o', '\u{0307}', '\u{022F}'), // ȯ
    ('o', '\u{0308}', '\u{00F6}'), // ö
    ('o', '\u{030B}', '\u{0151}'), // ő
    ('o', '\u{030C}', '\u{01D2}'), // ǒ
    ('o', '\u{0323}', '\u{1ECD}'), // ọ
    ('o', '\u{0328}', '\u{01EB}'), // ǫ
    ('p', '\u{0301}', '\u{1E55}'), // ṕ
    ('p', '\u{0307}', '\u{1E57}'), // ṗ
    ('r', '\u{0301}', '\u{0155}'), // ŕ
    ('r', '\u{0307}', '\u{1E59}'), // ṙ
    ('r', '\u{030C}', '\u{0159}'), // ř
    ('r', '\u{0323}', '\u{1E5B}'), // ṛ
    ('r', '\u{0327}', '\u{0157}'), // ŗ
    ('s', '\u{0301}', '\u{015B}'), // ś
    ('s', '\u{0302}', '\u{015D}'), // ŝ
    ('s', '\u{0307}', '\u{1E61}'), // ṡ
    ('s', '\u{030C}', '\u{0161}'), // š
    ('s', '\u{0323}', '\u{1E63}'), // ṣ
    ('s', '\u{0327}', '\u{015F}'), // ş
    ('t', '\u{0307}', '\u{1E6B}'), // ṫ
    ('t', '\u{0308}', '\u{1E97}'), // ẗ
    ('t', '\u{030C}', '\u{0165}'), // ť
    ('t', '\u{0323}', '\u{1E6D}'), // ṭ
    ('t', '\u{0327}', '\u{0163}'), // ţ
    ('u', '\u{0300}', '\u{00F9}'), // ù
    ('u', '\u{0301}', '\u{00FA}'), // ú
    ('u', '\u{0302}', '\u{00FB}'), // û
    ('u', '\u{0303}', '\u{0169}'), // ũ
    ('u', '\u{0304}', '\u{016B}'), // ū
    ('u', '\u{0306}', '\u{016D}'), // ŭ
    ('u', '\u{0308}', '\u{00FC}'), // ü
    ('u', '\u{030A}', '\u{016F}'), // ů
    ('u', '\u{030B}', '\u{0171}'), // ű
    ('u', '\u{030C}', '\u{01D4}'), // ǔ
    ('u', '\u{0323}', '\u{1EE5}'), // ụ
    ('u', '\u{0328}', '\u{0173}'), // ų
    ('v', '\u{0303}', '\u{1E7D}'), // ṽ
    ('v', '\u{0323}', '\u{1E7F}'), // ṿ
    ('w', '\u{0300}', '\u{1E81}'), // ẁ
    ('w', '\u{0301}', '\u{1E83}'), // ẃ
    ('w', '\u{0302}', '\u{0175}'), // ŵ
    ('w', '\u{0307}', '\u{1E87}'), // ẇ
    ('w', '\u{0308}', '\u{1E85}'), // ẅ
    ('w', '\u{030A}', '\u{1E98}'), // ẘ
    ('w', '\u{0323}', '\u{1E89}'), // ẉ
    ('x', '\u{0307}', '\u{1E8B}'), // ẋ
    ('x', '\u{0308}', '\u{1E8D}'), // ẍ
    ('y', '\u{0300}', '\u{1EF3}'), // ỳ
    ('y', '\u{0301}', '\u{00FD}'), // ý
    ('y', '\u{0302}', '\u{0177}'), // ŷ
    ('y', '\u{0303}', '\u{1EF9}'), // ỹ
    ('y', '\u{0304}', '\u{0233}'), // ȳ
    ('y', '\u{0307}', '\u{1E8F}'), // ẏ
    ('y', '\u{0308}', '\u{00FF}'), // ÿ
    ('y', '\u{030A}', '\u{1E99}'), // ẙ
    ('y', '\u{0323}', '\u{1EF5}'), // ỵ
    ('z', '\u{0301}', '\u{017A}'), // ź
    ('z', '\u{0302}', '\u{1E91}'), // ẑ
    ('z', '\u{0307}', '\u{017C}'), // ż
    ('z', '\u{030C}', '\u{017E}'), // ž
    ('z', '\u{0323}', '\u{1E93}'), // ẓ
];

pub(super) fn compose(base: char, combining: char) -> Option<char> {
    COMPOSITIONS
        .binary_search_by(|&(entry_base, entry_mark, _)| {
            (entry_base, entry_mark).cmp(&(base, combining))
        })
        .ok()
        .and_then(|index| COMPOSITIONS.get(index))
        .map(|&(_, _, composed)| composed)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
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

    #[test]
    fn table_32_every_entry() {
        let defined = table_32_defined();
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
        assert_eq!(designation(value(0b0101, 0b000)), None);
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
        assert_eq!(second_g0(value(0b0101, 0b011)), None);
        assert_eq!(second_g0(value(0b0011, 0b000)), None);
        for (group, index, g0, _) in table_32_defined() {
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

    #[test]
    fn g3_positions() {
        // The four positions used at Level 1.5 (REQ-CHR-007).
        assert_eq!(base_unicode(CharSet::G3, 0x51), '\u{2500}');
        assert_eq!(base_unicode(CharSet::G3, 0x5B), '\u{2192}');
        assert_eq!(base_unicode(CharSet::G3, 0x5C), '\u{2190}');
        assert_eq!(base_unicode(CharSet::G3, 0x5D), '\u{2191}');
        for code in [0x5F, 0x6E, 0x6F, 0x7E, 0x7F] {
            assert_eq!(base_unicode(CharSet::G3, code), ' ');
        }
        assert_eq!(base_unicode(CharSet::G3, 0x3F), FULL_BLOCK);
        assert_eq!(base_unicode(CharSet::G3, 0x2F), '\u{1FB95}');
        // Every other position is distinct.
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
            0x0327, 0x0332, 0x030B, 0x0328, 0x030C,
        ];
        assert_eq!(diacritic_combining(0), None);
        for (mark, scalar) in (1..=15).zip(expected) {
            assert_eq!(diacritic_combining(mark), char::from_u32(scalar), "mark {mark}");
        }
        for mark in 16..=0xFF {
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
}
