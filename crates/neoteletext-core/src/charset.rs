//! Character sets and their designation (EN 300 706 clause 15, area CHR).
//!
//! Inside the decoder a character is a set identifier and a 7-bit code, never
//! a Unicode value (REQ-CHR-001). Conversion to Unicode happens only at the
//! output stage, through [`to_unicode`].

// Tables keep one source row per line, aligned as in the standard.
#[rustfmt::skip]
mod arabic_script;
#[rustfmt::skip]
mod code_table;
#[rustfmt::skip]
mod composition;
#[rustfmt::skip]
mod designation;
#[rustfmt::skip]
mod diacritics;
#[cfg(feature = "extended-sets")]
#[rustfmt::skip]
mod extended_designations;
#[rustfmt::skip]
mod g0_extended;
#[rustfmt::skip]
mod g0_standard;
#[rustfmt::skip]
mod g2;
#[rustfmt::skip]
mod g3;
#[rustfmt::skip]
mod latin;
#[rustfmt::skip]
mod mosaics;
#[rustfmt::skip]
mod thai_lao;

use crate::consts::{CODE_MASK_7BIT, DIACRITIC_HOOK_ABOVE, DIACRITIC_LOW_LINE, DIACRITIC_NONE};

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
    /// Persian and Dari in plain letters, joined by the receiver
    /// (decision E.79).
    Persian,
    /// Urdu in plain letters, joined by the receiver.
    Urdu,
    /// Pashto in plain letters, joined by the receiver.
    Pashto,
    /// Sindhi in plain letters, joined by the receiver.
    Sindhi,
    /// Sorani Kurdish in plain letters, joined by the receiver.
    Sorani,
    /// Uyghur in plain letters, joined by the receiver.
    Uyghur,
    /// Balochi in plain letters, joined by the receiver.
    Balochi,
    /// Arabic in plain letters, joined by the receiver.
    ArabicPlain,
    /// Thai: the Philips SAA5246AP/S Thai page (decision E.80).
    Thai,
    /// Lao: the 1990 World System Teletext specification's Figure 33.
    Lao,
}

impl G0Set {
    /// True for the sets of group 1001, whose letters the receiver joins
    /// (REQ-CHR-015).
    #[must_use]
    pub const fn is_joined(self) -> bool {
        // Thai and Lao marks are ordinary characters in their own cells;
        // only group 1001 is joined.
        matches!(
            self,
            Self::Persian
                | Self::Urdu
                | Self::Pashto
                | Self::Sindhi
                | Self::Sorani
                | Self::Uyghur
                | Self::Balochi
                | Self::ArabicPlain
        )
    }
}

/// How a letter joins its neighbours (Unicode `ArabicShaping.txt`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum JoiningType {
    /// Joins on both sides.
    Dual,
    /// Joins only the letter before it.
    Right,
    /// Joins neither.
    None,
    /// The tatweel: joins both sides and causes its neighbours to join.
    Causing,
}

impl JoiningType {
    /// Can join the letter before it in reading order.
    #[must_use]
    pub const fn joins_previous(self) -> bool {
        matches!(self, Self::Dual | Self::Right | Self::Causing)
    }

    /// Can join the letter after it in reading order.
    #[must_use]
    pub const fn joins_next(self) -> bool {
        matches!(self, Self::Dual | Self::Causing)
    }
}

/// A joined letter's form (REQ-CHR-015).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum JoinForm {
    /// Joining neither neighbour; every character that is not a joined
    /// letter has this form.
    #[default]
    Isolated,
    /// Joining only the letter before it.
    Final,
    /// Joining only the letter after it.
    Initial,
    /// Joining both.
    Medial,
}

/// The joining type of a letter of the group 1001 sets; `None` for any
/// other character.
#[must_use]
pub fn joining_type(letter: char) -> Option<JoiningType> {
    arabic_script::joining_type(letter)
}

/// The code point drawn for a letter in a form (REQ-CHR-015): its Arabic
/// Presentation Form, a private-use code point where Unicode has none, or
/// the letter itself when isolated or not a joining letter.
#[must_use]
pub fn joined_form(letter: char, form: JoinForm) -> char {
    arabic_script::joined_form(letter, form).unwrap_or(letter)
}

/// Every joined form the core draws: (letter, form, code point), sorted by
/// letter and form.
#[must_use]
pub fn joined_forms() -> &'static [(char, JoinForm, char)] {
    arabic_script::JOINED_FORMS.as_slice()
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
    /// Thai G2: Latin G2 with columns 5 and 6 holding the SAA5246AP/S's
    /// columns 8 and 9, the rarer marks and the vowel-and-tone pairs
    /// (decision E.80).
    Thai,
    /// Lao G2: Latin G2 with columns 5 and 6 holding the vowel-and-tone
    /// pairs and the rarer marks.
    Lao,
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
    /// The joined form of a letter of a group 1001 set, given after the
    /// page is composed (REQ-CHR-015); isolated for every other character.
    pub form: JoinForm,
}

impl CharRef {
    /// A plain character with no mark.
    #[must_use]
    pub const fn new(set: CharSet, code: u8) -> Self {
        Self { set, code: code & CODE_MASK_7BIT, diacritic: DIACRITIC_NONE, form: JoinForm::Isolated }
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
    designation::designation(value & CODE_MASK_7BIT)
}

/// Resolves Table 32 with the fallback of REQ-CHR-003: a reserved entry is
/// first retried in group 0000 with the same national option bits, and then
/// English is used. The flag is true when a fallback was needed.
#[must_use]
pub fn designation_with_fallback(value: u8) -> (Designation, bool) {
    if let Some(found) = designation(value) {
        return (found, false);
    }
    let national_bits = value & code_table::NATIONAL_BITS_MASK;
    if let Some(found) = designation(national_bits) {
        return (found, true);
    }
    (code_table::ENGLISH_DESIGNATION, true)
}

/// Looks up Table 33 for a second G0 designation value. `None` when the
/// value is reserved or 0x7F ("no second G0 set required").
#[must_use]
pub fn second_g0(value: u8) -> Option<G0> {
    designation::second_g0(value & CODE_MASK_7BIT)
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
    designation::default_second_g0(value & CODE_MASK_7BIT)
}

/// Looks up Table 34 for the data of a Modified G0 and G2 Character Set
/// Designation triplet (Table 29 mode 01000, clause 15.4): the G0 and G2
/// sets, or `None` when the value is reserved. A G0 set chosen this way is
/// used without national option (clause 15.4 note).
#[must_use]
pub fn modified_designation(value: u8) -> Option<(G0Set, G2Set)> {
    designation::modified_designation(value & CODE_MASK_7BIT)
}

/// Converts a character to Unicode (REQ-CHR-010). Codes outside 0x20 to 0x7F
/// give a space. Every character of the sets has a Unicode equivalent.
#[must_use]
pub fn to_unicode(character: CharRef) -> Unicode {
    let base = code_table::base_unicode(character.set, character.code);
    // REQ-CHR-016: a Thai or Lao G2 glyph of two marks gives both.
    if let Some((first, second)) = thai_lao::mark_sequence(base) {
        return Unicode { base: first, combining: Some(second) };
    }
    let combining =
        if character.diacritic == DIACRITIC_NONE { None } else { diacritics::diacritic_combining(character.diacritic) };
    Unicode { base, combining }
}

/// The code point a font draws a character by: the table's character, in
/// its joined form for a joined letter (REQ-CHR-015), or the private-use
/// key of a two-mark glyph (REQ-CHR-016). Without a diacritical mark it is
/// what glyph lookup uses; [`to_unicode`] is for text.
#[must_use]
pub fn glyph_key(character: CharRef) -> char {
    joined_form(code_table::base_unicode(character.set, character.code), character.form)
}

/// Every two-mark glyph of the Thai and Lao G2 sets: private-use key, first
/// mark, second mark.
#[must_use]
pub fn mark_sequences() -> &'static [(char, char, char)] {
    thai_lao::mark_sequences()
}

/// Converts a block mosaic pattern to Unicode (REQ-CHR-005). `pattern` is
/// the 6-bit block pattern n, bit 0 the top-left block.
#[must_use]
pub fn mosaic_to_unicode(pattern: u8, separated: bool) -> char {
    mosaics::mosaic_unicode(pattern, separated)
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
    code_table::NATIONAL_POSITIONS.contains(&(code & CODE_MASK_7BIT))
}

/// Precomposes a base letter and a combining mark where Unicode has a
/// single code point for the pair (decision E.26, Normalization Form C).
#[must_use]
pub fn compose(base: char, combining: char) -> Option<char> {
    composition::compose(base, combining)
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
mod tests;
