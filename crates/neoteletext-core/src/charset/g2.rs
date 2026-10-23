//! The G2 sets (Tables 37, 41, 43 and 45).

use super::code_table::CodeTable;

// Column 4 of the Latin, Cyrillic and Greek G2 sets holds the diacritical
// marks (REQ-CHR-008). Shown on their own they use the spacing form of each
// mark. Dot below (4/9) has no spacing form in Unicode; the full stop is
// the character that looks like the drawn low dot. Low line (4/C) uses the
// spacing low line, the counterpart of the combining low line U+0332.

/// Table 37: Latin G2. 5/6 is drawn as the old "CE" sign, but note 4 says
/// it is the European currency symbol, so it maps to the euro sign. 5/8 is
/// the Greek alpha. 5/9, 5/A, 5/B and 6/5 must not be transmitted (note 5)
/// and display as a space (REQ-CHR-006). 7/F is the solid block of note 6.
pub(super) const LATIN_G2: CodeTable = [
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
pub(super) const CYRILLIC_G2: CodeTable = [
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
pub(super) const GREEK_G2: CodeTable = [
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
pub(super) const ARABIC_G2: CodeTable = [
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

/// Position of 4/C in a code table: the low line in Latin G2.
const HOOK_POSITION: usize = 0x4C - 0x20;
/// The Vietnamese G2 set (decision E.81): Latin G2 with the hook above at
/// 4/C in place of the low line. Shown on its own it is U+02C0, the spacing
/// character nearest the hook; as a mark it is U+0309 (REQ-CHR-014).
pub(super) const VIETNAMESE_G2: CodeTable = {
    let mut table = LATIN_G2;
    table[HOOK_POSITION] = '\u{02C0}';
    table
};
/// The Armenian G2 set (decision E.78): Latin G2 with the ligature և and
/// the rarer Armenian punctuation at the four positions Latin G2 leaves
/// untransmitted (5/9, 5/A, 5/B and 6/5, note 5).
pub(super) const ARMENIAN_G2: CodeTable = {
    let mut table = LATIN_G2;
    table[0x59 - 0x20] = '\u{0587}';
    table[0x5A - 0x20] = '\u{055B}';
    table[0x5B - 0x20] = '\u{055E}';
    table[0x65 - 0x20] = '\u{055C}';
    table
};
