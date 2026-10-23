//! Latin G0 and the national option sub-sets (Tables 35 and 36).

use super::NationalOption;
use super::code_table::CodeTable;
use crate::consts::NATIONAL_POSITIONS;

/// Table 35. The shaded national option positions hold the base forms used
/// through packet X/26 (REQ-CHR-002). 7/C is drawn as a broken bar, the
/// ISO 646 rendering of the vertical line, so it maps to U+007C. 7/F is the
/// solid block of note 4.
pub(super) const LATIN_G0: CodeTable = [
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

// neoteletext's own options (decision E.78, docs/spec/design/languages.md
// section 8.1): capitals at 4/0 and 5/B to 5/F, small letters at 6/0, 7/B
// to 7/E and 2/3, so a capital and its small letter differ only in bit 6
// wherever the positions allow; 2/4 holds a further letter or keeps ¤.
const HAUSA_FULA: NationalSubset =
    ['ɲ', 'ʼ', 'Ɓ', 'Ɗ', 'Ƙ', 'Ƴ', 'Ŋ', 'Ɲ', 'ɓ', 'ɗ', 'ƙ', 'ƴ', 'ŋ'];

const WEST_AFRICAN: NationalSubset =
    ['ɖ', '¤', 'Ɛ', 'Ɔ', 'Ŋ', 'Ɲ', 'Ɣ', 'Ɖ', 'ɛ', 'ɔ', 'ŋ', 'ɲ', 'ɣ'];

const EWE: NationalSubset =
    ['ʋ', '¤', 'Ɖ', 'Ɛ', 'Ƒ', 'Ɣ', 'Ɔ', 'Ʋ', 'ɖ', 'ɛ', 'ƒ', 'ɣ', 'ɔ'];

const KANURI_MOSSI: NationalSubset =
    ['ŋ', '¤', 'Ǝ', 'Ɍ', 'Ɛ', 'Ɩ', 'Ʋ', 'Ŋ', 'ǝ', 'ɍ', 'ɛ', 'ɩ', 'ʋ'];

const KABYLE_TAMAZIGHT: NationalSubset =
    ['ṣ', 'ṭ', 'Ɛ', 'Ɣ', 'Č', 'Ǧ', 'ḍ', 'ḥ', 'ɛ', 'ɣ', 'č', 'ǧ', 'ṛ'];

const AZERBAIJANI: NationalSubset =
    ['ı', 'ü', 'Ə', 'Ğ', 'İ', 'Ç', 'Ö', 'Ş', 'ə', 'ğ', 'ç', 'ö', 'ş'];


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
        NationalOption::HausaFula => &HAUSA_FULA,
        NationalOption::WestAfrican => &WEST_AFRICAN,
        NationalOption::Ewe => &EWE,
        NationalOption::KabyleTamazight => &KABYLE_TAMAZIGHT,
        NationalOption::Azerbaijani => &AZERBAIJANI,
        NationalOption::KanuriMossi => &KANURI_MOSSI,
    }
}

pub(super) fn national_char(option: NationalOption, code: u8) -> Option<char> {
    let position = NATIONAL_POSITIONS.iter().position(|&candidate| candidate == code)?;
    national_subset(option).get(position).copied()
}
