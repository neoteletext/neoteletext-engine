//! The other G0 sets of the standard (Tables 38 to 40, 42, 44 and 46).

use super::code_table::CodeTable;

/// Table 38: Cyrillic G0 option 1, Serbian and Croatian.
pub(super) const CYRILLIC_1_G0: CodeTable = [
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    'Ч', 'А', 'Б', 'Ц', 'Д', 'Е', 'Ф', 'Г', 'Х', 'И', 'Ј', 'К', 'Л', 'М', 'Н', 'О', //
    'П', 'Ќ', 'Р', 'С', 'Т', 'У', 'В', 'Ѓ', 'Љ', 'Њ', 'З', 'Ћ', 'Ж', 'Ђ', 'Ш', 'Џ', //
    'ч', 'а', 'б', 'ц', 'д', 'е', 'ф', 'г', 'х', 'и', 'ј', 'к', 'л', 'м', 'н', 'о', //
    'п', 'ќ', 'р', 'с', 'т', 'у', 'в', 'ѓ', 'љ', 'њ', 'з', 'ћ', 'ж', 'ђ', 'ш', '■', //
];

/// Table 39: Cyrillic G0 option 2, Russian and Bulgarian.
pub(super) const CYRILLIC_2_G0: CodeTable = [
    ' ', '!', '"', '#', '$', '%', 'ы', '\'', '(', ')', '*', '+', ',', '-', '.', '/', //
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', //
    'Ю', 'А', 'Б', 'Ц', 'Д', 'Е', 'Ф', 'Г', 'Х', 'И', 'Й', 'К', 'Л', 'М', 'Н', 'О', //
    'П', 'Я', 'Р', 'С', 'Т', 'У', 'Ж', 'В', 'Ь', 'Ъ', 'З', 'Ш', 'Э', 'Щ', 'Ч', 'Ы', //
    'ю', 'а', 'б', 'ц', 'д', 'е', 'ф', 'г', 'х', 'и', 'й', 'к', 'л', 'м', 'н', 'о', //
    'п', 'я', 'р', 'с', 'т', 'у', 'ж', 'в', 'ь', 'ъ', 'з', 'ш', 'э', 'щ', 'ч', '■', //
];

/// Table 40: Cyrillic G0 option 3, Ukrainian.
pub(super) const CYRILLIC_3_G0: CodeTable = [
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
pub(super) const GREEK_G0: CodeTable = [
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
pub(super) const ARABIC_G0: CodeTable = [
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
pub(super) const HEBREW_G0: CodeTable = [
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
