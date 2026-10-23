//! The code tables of clause 15: a set as its 96 characters of codes 2/0 to
//! 7/F, the lookup of a code, and the Unicode of a G0, G2 or G3 code.
//!
//! Every code table holds the 96 characters of codes 2/0 to 7/F, column by
//! column, one row of the source table per line. A glyph the standard draws
//! but Unicode cannot represent exactly is mapped to the nearest character
//! and the choice is commented at its position.

use super::arabic_script::{
    ARABICPLAIN_G0, BALOCHI_G0, PASHTO_G0, PERSIAN_G0, SINDHI_G0, SORANI_G0, URDU_G0, UYGHUR_G0,
};
use super::g0_extended::{
    ARMENIAN_G0, BELARUSIAN_G0, GEORGIAN_G0, KYRGYZ_MONGOLIAN_G0, MACEDONIAN_G0, SERBIAN_G0, TAJIK_G0, TATAR_G0,
    TIFINAGH_G0, UKRAINIAN_G0, VIETNAMESE_G0, YORUBA_IGBO_G0,
};
use super::g0_standard::{ARABIC_G0, CYRILLIC_1_G0, CYRILLIC_2_G0, CYRILLIC_3_G0, GREEK_G0, HEBREW_G0};
use super::g2::{ARABIC_G2, ARMENIAN_G2, CYRILLIC_G2, GREEK_G2, LATIN_G2, VIETNAMESE_G2};
use super::g3::G3;
use super::latin::{LATIN_G0, national_char};
use super::thai_lao::{LAO_G0, LAO_G2, THAI_G0, THAI_G2};
use super::{CharSet, Designation, G0, G0Set, G2Set, NationalOption};
use crate::consts::{NATIONAL_INDEX_MASK, SPACE};

/// Mask of the national option bits of a Table 32 value.
pub(super) const NATIONAL_BITS_MASK: u8 = NATIONAL_INDEX_MASK;

/// English Latin G0 with Latin G2: the last-resort fallback.
pub(super) const ENGLISH_DESIGNATION: Designation = Designation {
    g0: G0 { set: G0Set::Latin, national: Some(NationalOption::English) },
    g2: G2Set::Latin,
};

/// The 13 national option positions of Table 36.
pub(super) use crate::consts::NATIONAL_POSITIONS;

/// Codes 2/0 to 7/F: six columns of sixteen.
const SET_SIZE: usize = 96;

/// One character set, indexed by code − 2/0.
pub(super) type CodeTable = [char; SET_SIZE];



fn lookup(table: &CodeTable, code: u8) -> char {
    code.checked_sub(SPACE)
        .and_then(|index| table.get(usize::from(index)))
        .copied()
        .unwrap_or(' ')
}

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
        G0Set::Vietnamese => &VIETNAMESE_G0,
        G0Set::YorubaIgbo => &YORUBA_IGBO_G0,
        G0Set::Ukrainian => &UKRAINIAN_G0,
        G0Set::Belarusian => &BELARUSIAN_G0,
        G0Set::Macedonian => &MACEDONIAN_G0,
        G0Set::Serbian => &SERBIAN_G0,
        G0Set::KyrgyzMongolian => &KYRGYZ_MONGOLIAN_G0,
        G0Set::Tatar => &TATAR_G0,
        G0Set::Tajik => &TAJIK_G0,
        G0Set::Georgian => &GEORGIAN_G0,
        G0Set::Armenian => &ARMENIAN_G0,
        G0Set::Tifinagh => &TIFINAGH_G0,
        G0Set::Persian => &PERSIAN_G0,
        G0Set::Urdu => &URDU_G0,
        G0Set::Pashto => &PASHTO_G0,
        G0Set::Sindhi => &SINDHI_G0,
        G0Set::Sorani => &SORANI_G0,
        G0Set::Uyghur => &UYGHUR_G0,
        G0Set::Balochi => &BALOCHI_G0,
        G0Set::ArabicPlain => &ARABICPLAIN_G0,
        G0Set::Thai => &THAI_G0,
        G0Set::Lao => &LAO_G0,
    };
    lookup(table, code)
}

const fn g2_table(set: G2Set) -> &'static CodeTable {
    match set {
        G2Set::Latin => &LATIN_G2,
        G2Set::Cyrillic => &CYRILLIC_G2,
        G2Set::Greek => &GREEK_G2,
        G2Set::Arabic => &ARABIC_G2,
        G2Set::Vietnamese => &VIETNAMESE_G2,
        G2Set::Armenian => &ARMENIAN_G2,
        G2Set::Thai => &THAI_G2,
        G2Set::Lao => &LAO_G2,
    }
}

pub(super) fn base_unicode(set: CharSet, code: u8) -> char {
    match set {
        CharSet::G0(g0) => g0_unicode(g0, code),
        CharSet::G2(g2) => lookup(g2_table(g2), code),
        CharSet::G3 => lookup(&G3, code),
    }
}
