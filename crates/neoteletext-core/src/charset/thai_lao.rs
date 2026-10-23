//! Group 1100: Thai and Lao (REQ-CHR-016, decision E.80).

use super::code_table::CodeTable;
use super::g2::LATIN_G2;

/// Thai G0: the Philips SAA5246AP/S Thai page, Table 23 columns 2 to 7
/// (`docs/spec/design/languages.md` section 7.3). 4/0 (ๅ), 7/9 (ใ),
/// 7/A (ไ) and 7/C (็) are readings still to be confirmed (section 7.8).
pub(super) const THAI_G0: CodeTable = [
    '\u{0020}', '\u{0021}', '\u{0022}', '\u{0023}', '\u{0024}', '\u{0025}', '\u{0026}', '\u{0027}', '\u{0028}', '\u{0029}', '\u{002A}', '\u{002B}', '\u{002C}', '\u{002D}', '\u{002E}', '\u{002F}', //
    '\u{0030}', '\u{0031}', '\u{0032}', '\u{0033}', '\u{0034}', '\u{0035}', '\u{0036}', '\u{0037}', '\u{0038}', '\u{0039}', '\u{003A}', '\u{003B}', '\u{003C}', '\u{003D}', '\u{003E}', '\u{003F}', //
    '\u{0E45}', '\u{0E01}', '\u{0E02}', '\u{0E04}', '\u{0E06}', '\u{0E07}', '\u{0E08}', '\u{0E09}', '\u{0E0A}', '\u{0E0B}', '\u{0E0C}', '\u{0E0D}', '\u{0E0E}', '\u{0E0F}', '\u{0E10}', '\u{0E11}', //
    '\u{0E12}', '\u{0E13}', '\u{0E14}', '\u{0E15}', '\u{0E16}', '\u{0E17}', '\u{0E18}', '\u{0E19}', '\u{0E1A}', '\u{0E1B}', '\u{0E1C}', '\u{0E1D}', '\u{0E1E}', '\u{0E1F}', '\u{0E20}', '\u{0E21}', //
    '\u{0E22}', '\u{0E23}', '\u{0E24}', '\u{0E25}', '\u{0E27}', '\u{0E28}', '\u{0E29}', '\u{0E2A}', '\u{0E2B}', '\u{0E2C}', '\u{0E2D}', '\u{0E2E}', '\u{0E2F}', '\u{0E30}', '\u{0E31}', '\u{0E32}', //
    '\u{0E34}', '\u{0E35}', '\u{0E36}', '\u{0E37}', '\u{0E38}', '\u{0E39}', '\u{0E40}', '\u{0E41}', '\u{0E42}', '\u{0E43}', '\u{0E44}', '\u{0E46}', '\u{0E47}', '\u{0E48}', '\u{0E49}', '\u{25A0}', //
];

/// Lao G0: the 1990 World System Teletext specification's Figure 33
/// (p. 173), read from its drawings (section 7.7). Positions the figure
/// leaves unused, and those whose glyph is not yet identified (2/7 drawn
/// as dots and read as an ellipsis, 5/D "NUL", 6/8 to 6/A, and 7/7, the
/// ligature ຫຼ, which Unicode has no single code point for), are spaces.
pub(super) const LAO_G0: CodeTable = [
    '\u{0020}', '\u{0021}', '\u{0022}', '\u{0023}', '\u{0024}', '\u{0025}', '\u{0EC6}', '\u{2026}', '\u{0028}', '\u{0029}', '\u{002A}', '\u{002B}', '\u{002C}', '\u{002D}', '\u{002E}', '\u{002F}', //
    '\u{0ED0}', '\u{0ED1}', '\u{0ED2}', '\u{0ED3}', '\u{0ED4}', '\u{0ED5}', '\u{0ED6}', '\u{0ED7}', '\u{0ED8}', '\u{0ED9}', '\u{003A}', '\u{003B}', '\u{00A3}', '\u{003D}', '\u{0EAF}', '\u{003F}', //
    '\u{0E99}', '\u{0E87}', '\u{0EA1}', '\u{0E8D}', '\u{0EA7}', '\u{0E9F}', '\u{0EA5}', '\u{0EA3}', '\u{0E8A}', '\u{0EAE}', '\u{0E9E}', '\u{0E97}', '\u{0E84}', '\u{0E9A}', '\u{0E94}', '\u{0020}', //
    '\u{0EB9}', '\u{0EB8}', '\u{0EB1}', '\u{0EB5}', '\u{0EB7}', '\u{0EB4}', '\u{0EB6}', '\u{0EBB}', '\u{0ECD}', '\u{0EC8}', '\u{0EC9}', '\u{0ECA}', '\u{0ECB}', '\u{0020}', '\u{0020}', '\u{0020}', //
    '\u{0EB2}', '\u{0EB0}', '\u{0EC4}', '\u{0EC3}', '\u{0EB3}', '\u{0EC0}', '\u{0EC1}', '\u{0EC2}', '\u{0020}', '\u{0020}', '\u{0020}', '\u{0020}', '\u{0020}', '\u{0020}', '\u{0020}', '\u{0020}', //
    '\u{0EDC}', '\u{0EA2}', '\u{0EDD}', '\u{0E81}', '\u{0E88}', '\u{0E9D}', '\u{0EAD}', '\u{0020}', '\u{0EAA}', '\u{0EAB}', '\u{0E9C}', '\u{0E96}', '\u{0E82}', '\u{0E9B}', '\u{0E95}', '\u{25A0}', //
];

/// Each G2 glyph that is two marks in one cell: its private-use key, the
/// vowel sign and the tone or other mark (REQ-CHR-016). `to_unicode` gives
/// the two marks; fonts draw the key. Thai from U+F0100, Lao from U+F0140.
const MARK_SEQUENCES: [(char, char, char); 53] = [
    ('\u{F0100}', '\u{0E31}', '\u{0E48}'),
    ('\u{F0101}', '\u{0E31}', '\u{0E49}'),
    ('\u{F0102}', '\u{0E31}', '\u{0E4A}'),
    ('\u{F0103}', '\u{0E31}', '\u{0E4B}'),
    ('\u{F0104}', '\u{0E34}', '\u{0E48}'),
    ('\u{F0105}', '\u{0E34}', '\u{0E49}'),
    ('\u{F0106}', '\u{0E34}', '\u{0E4A}'),
    ('\u{F0107}', '\u{0E34}', '\u{0E4B}'),
    ('\u{F0108}', '\u{0E34}', '\u{0E4C}'),
    ('\u{F0109}', '\u{0E35}', '\u{0E48}'),
    ('\u{F010A}', '\u{0E35}', '\u{0E49}'),
    ('\u{F010B}', '\u{0E35}', '\u{0E4A}'),
    ('\u{F010C}', '\u{0E35}', '\u{0E4B}'),
    ('\u{F010D}', '\u{0E36}', '\u{0E48}'),
    ('\u{F010E}', '\u{0E36}', '\u{0E49}'),
    ('\u{F010F}', '\u{0E36}', '\u{0E4A}'),
    ('\u{F0110}', '\u{0E36}', '\u{0E4B}'),
    ('\u{F0111}', '\u{0E37}', '\u{0E48}'),
    ('\u{F0112}', '\u{0E37}', '\u{0E49}'),
    ('\u{F0113}', '\u{0E37}', '\u{0E4A}'),
    ('\u{F0114}', '\u{0E37}', '\u{0E4B}'),
    ('\u{F0115}', '\u{0E4D}', '\u{0E48}'),
    ('\u{F0116}', '\u{0E4D}', '\u{0E49}'),
    ('\u{F0117}', '\u{0E4D}', '\u{0E4A}'),
    ('\u{F0118}', '\u{0E4D}', '\u{0E4B}'),
    ('\u{F0140}', '\u{0EB1}', '\u{0EC8}'),
    ('\u{F0141}', '\u{0EB1}', '\u{0EC9}'),
    ('\u{F0142}', '\u{0EB1}', '\u{0ECA}'),
    ('\u{F0143}', '\u{0EB1}', '\u{0ECB}'),
    ('\u{F0144}', '\u{0EB4}', '\u{0EC8}'),
    ('\u{F0145}', '\u{0EB4}', '\u{0EC9}'),
    ('\u{F0146}', '\u{0EB4}', '\u{0ECA}'),
    ('\u{F0147}', '\u{0EB4}', '\u{0ECB}'),
    ('\u{F0148}', '\u{0EB5}', '\u{0EC8}'),
    ('\u{F0149}', '\u{0EB5}', '\u{0EC9}'),
    ('\u{F014A}', '\u{0EB5}', '\u{0ECA}'),
    ('\u{F014B}', '\u{0EB5}', '\u{0ECB}'),
    ('\u{F014C}', '\u{0EB6}', '\u{0EC8}'),
    ('\u{F014D}', '\u{0EB6}', '\u{0EC9}'),
    ('\u{F014E}', '\u{0EB6}', '\u{0ECA}'),
    ('\u{F014F}', '\u{0EB6}', '\u{0ECB}'),
    ('\u{F0150}', '\u{0EB7}', '\u{0EC8}'),
    ('\u{F0151}', '\u{0EB7}', '\u{0EC9}'),
    ('\u{F0152}', '\u{0EB7}', '\u{0ECA}'),
    ('\u{F0153}', '\u{0EB7}', '\u{0ECB}'),
    ('\u{F0154}', '\u{0EBB}', '\u{0EC8}'),
    ('\u{F0155}', '\u{0EBB}', '\u{0EC9}'),
    ('\u{F0156}', '\u{0EBB}', '\u{0ECA}'),
    ('\u{F0157}', '\u{0EBB}', '\u{0ECB}'),
    ('\u{F0158}', '\u{0ECD}', '\u{0EC8}'),
    ('\u{F0159}', '\u{0ECD}', '\u{0EC9}'),
    ('\u{F015A}', '\u{0ECD}', '\u{0ECA}'),
    ('\u{F015B}', '\u{0ECD}', '\u{0ECB}'),
];

/// Columns 5 and 6 of the Thai G2 set: the chip's columns 8 and 9 (section
/// 7.4). 6/D to 6/F are not yet identified.
const THAI_G2_COLUMNS_5_AND_6: [char; 32] = [
    '\u{0E4A}', '\u{0E4B}', '\u{0E4C}', '\u{F0100}', '\u{F0101}', '\u{F0102}', '\u{F0103}', '\u{F0104}', '\u{F0105}', '\u{F0106}', '\u{F0107}', '\u{F0108}', '\u{F0109}', '\u{F010A}', '\u{F010B}', '\u{F010C}', '\u{F010D}', '\u{F010E}', '\u{F010F}', '\u{F0110}', '\u{F0111}', '\u{F0112}', '\u{F0113}', '\u{F0114}', '\u{F0115}', '\u{F0116}', '\u{F0117}', '\u{F0118}', '\u{0E4D}', '\u{0020}', '\u{0020}', '\u{0020}',
];

/// Columns 5 and 6 of the Lao G2 set: each upper vowel sign with each tone
/// mark, then the cancellation mark, the subscript lo and the semivowel yo.
const LAO_G2_COLUMNS_5_AND_6: [char; 32] = [
    '\u{F0140}', '\u{F0141}', '\u{F0142}', '\u{F0143}', '\u{F0144}', '\u{F0145}', '\u{F0146}', '\u{F0147}', '\u{F0148}', '\u{F0149}', '\u{F014A}', '\u{F014B}', '\u{F014C}', '\u{F014D}', '\u{F014E}', '\u{F014F}', '\u{F0150}', '\u{F0151}', '\u{F0152}', '\u{F0153}', '\u{F0154}', '\u{F0155}', '\u{F0156}', '\u{F0157}', '\u{F0158}', '\u{F0159}', '\u{F015A}', '\u{F015B}', '\u{0ECC}', '\u{0EBC}', '\u{0EBD}', '\u{0020}',
];

/// Latin G2 (Table 37) with columns 5 and 6 replaced.
// Evaluated only in constants, where an index out of range or an overflow
// fails the build rather than panicking at run time.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn latin_g2_with(columns: [char; 32]) -> CodeTable {
    let mut table = LATIN_G2;
    let mut index = 0;
    while index < columns.len() {
        table[COLUMN_5_START + index] = columns[index];
        index += 1;
    }
    table
}

/// The index of 5/0 in a code table.
const COLUMN_5_START: usize = 0x50 - 0x20;

pub(super) const THAI_G2: CodeTable = latin_g2_with(THAI_G2_COLUMNS_5_AND_6);
pub(super) const LAO_G2: CodeTable = latin_g2_with(LAO_G2_COLUMNS_5_AND_6);

/// The two marks of a mark sequence's private-use key.
pub(super) fn mark_sequence(key: char) -> Option<(char, char)> {
    MARK_SEQUENCES.iter().find(|&&(entry, _, _)| entry == key).map(|&(_, first, second)| (first, second))
}

/// Every mark sequence: key, first mark, second mark.
pub(super) const fn mark_sequences() -> &'static [(char, char, char)] {
    &MARK_SEQUENCES
}
