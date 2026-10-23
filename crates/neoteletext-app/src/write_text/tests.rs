#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use super::rtl::break_line_rtl;
use super::sets::{Cell, Sets, encode, page_sets};
use super::thai_lao::{marked_lines, marked_rows};
use super::*;
use neoteletext_core::charset::{CharRef, G2Set, compose, default_second_g0, page_mark, to_unicode};
use neoteletext_core::consts::{DESIGNATION_GROUP_SHIFT, ESC};

/// What a decoder shows for cells written in `name`'s sets, ESC as a
/// space (REQ-CHR-009): codes through the set in force, marks composed
/// and G2 characters from the page's G2 set.
fn shown(name: &str, cells: &[Cell]) -> String {
    let (_, sets) = page_sets(name).unwrap();
    let mut in_second = false;
    cells
        .iter()
        .map(|cell| match *cell {
            Cell::Code(ESC) => {
                in_second = !in_second;
                ' '
            }
            Cell::Code(code) => {
                let set = if in_second { sets.second.unwrap() } else { sets.default };
                to_unicode(CharRef::new(set, code)).base
            }
            Cell::Marked { code, mark } => {
                let placed = CharRef { diacritic: page_mark(sets.g2, mark), ..CharRef::new(sets.base, code) };
                let unicode = to_unicode(placed);
                compose(unicode.base, unicode.combining.unwrap()).unwrap()
            }
            Cell::G2 { code, .. } => to_unicode(CharRef::new(CharSet::G2(sets.g2), code)).base,
        })
        .collect()
}

fn cells(name: &str, text: &str) -> Vec<Cell> {
    let (_, sets) = page_sets(name).unwrap();
    let characters: Vec<char> = text.chars().collect();
    encode(&sets, &characters, false, 1).unwrap().0
}

#[test]
fn every_set_name_designates_its_set() {
    for (name, group, option) in SETS {
        let (value, sets) = page_sets(name).unwrap();
        assert_eq!(value, (group << DESIGNATION_GROUP_SHIFT) | option);
        let own = default_second_g0(value).is_some();
        assert_eq!(sets.second.is_some(), own, "{name}: only neoteletext's own sets imply English second");
    }
    assert_eq!(page_sets("klingon").err(), Some(WriteError::UnknownSet("klingon".into())));
}

/// Each set writes a sample and reads it back unchanged.
#[test]
fn samples_round_trip() {
    let samples = [
        ("vietnamese", "Tiếng Việt có dấu, Hà Nội và Đà Nẵng."),
        ("yoruba-igbo", "Ẹ kú àárọ, ọmọ Ṣàngó"),
        ("hausa-fula", "Ɓarna ƙasa ɗaya Ƴan"),
        ("ewe", "Eʋegbe ɖe Ŋkɔ"),
        ("azerbaijani", "Azərbaycan dili Ğ İ ı"),
        ("ukrainian", "Україна, ґанок і Київ"),
        ("serbian", "Србија, Ђорђе и Љубица"),
        ("macedonian", "Македонија Ѓ Ќ Ѕ"),
        ("georgian", "საქართველო"),
        ("armenian", "Հայաստան, Երևան"),
        ("tifinagh", "ⵜⴰⵎⴰⵣⵉⵖⵜ"),
    ];
    for (name, text) in samples {
        let (_, sets) = page_sets(name).unwrap();
        let characters: Vec<char> = text.chars().collect();
        let (written, _) = encode(&sets, &characters, false, 1).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(shown(name, &written), text, "{name}");
    }
}

/// Vietnamese tones go in X/26, the letter in the row.
#[test]
fn a_tone_is_a_triplet_over_its_letter() {
    let written = cells("vietnamese", "ệ");
    let [Cell::Marked { code, mark }] = written[..] else { panic!("{written:?}") };
    assert_eq!(to_unicode(CharRef::new(page_sets("vietnamese").unwrap().1.base, code)).base, 'ê');
    assert_eq!(mark, 0x9, "dot below");
    let [Cell::Marked { mark, .. }] = cells("vietnamese", "ả")[..] else { panic!() };
    assert_eq!(mark, 0xC, "the hook above is 4/C of the Vietnamese G2 set");
}

/// Latin text on a Cyrillic page is switched with ESC, and each ESC
/// takes a cell.
#[test]
fn latin_text_is_switched_with_esc() {
    let written = cells("ukrainian", "Київ BBC");
    assert_eq!(written.iter().filter(|cell| **cell == Cell::Code(ESC)).count(), 1);
    assert_eq!(written.len(), "Київ BBC".chars().count(), "the ESC takes the space's cell");
    assert_eq!(cells("ukrainian", "BBC").len(), "BBC".len() + 1, "without a space the ESC takes a cell");
}

/// A combining mark after a letter is written as that letter with the
/// mark by X/26: Yoruba ọ̀ is ọ in the row and a grave. A letter with
/// two marks needs two in one cell, which X/26 cannot place: the
/// second is refused.
#[test]
fn a_combining_mark_marks_the_letter_before_it() {
    let written = cells("yoruba-igbo", "ọ\u{0300}");
    let [Cell::Marked { code, mark: 1 }] = written[..] else { panic!("{written:?}") };
    assert_eq!(to_unicode(CharRef::new(page_sets("yoruba-igbo").unwrap().1.base, code)).base, 'ọ');
    let error = write_text("yoruba-igbo", 0x100, "ọ\u{0300}\u{0301}").unwrap_err();
    assert_eq!(error, WriteError::Missing { character: '\u{0301}', line: 1 });
}

#[test]
fn a_missing_character_is_refused_with_its_line() {
    let error = write_text("georgian", 0x100, "საქართველო\nok ☃").unwrap_err();
    assert_eq!(error, WriteError::Missing { character: '☃', line: 2 });
}

#[test]
fn lines_break_at_the_row_width_and_pages_end_at_row_23() {
    let word = "ა".repeat(10);
    let line = [word.as_str(); 8].join(" ");
    let written = write_text("georgian", 0x100, &line).unwrap();
    // Three words of ten cells and two spaces take 32 cells; a fourth
    // with its space would take 43.
    assert_eq!(written.rows, 3);
    let many = "ა\n".repeat(24);
    assert_eq!(write_text("georgian", 0x100, &many).unwrap_err(), WriteError::TooManyRows(24));
}

/// The cells of a right-to-left row as shown, left to right, without
/// the right-aligning indent.
fn shown_rtl(name: &str, text: &str) -> String {
    let (_, sets) = page_sets(name).unwrap();
    let rows = break_line_rtl(&sets, text, 1).unwrap();
    shown(name, &rows[0])
}

/// REQ-CLI-010: لا is the lam-alef code, and a line is reordered into
/// visual order with digits and Latin left to right and brackets
/// mirrored.
#[test]
fn arabic_script_is_written_in_visual_order() {
    assert_eq!(shown_rtl("persian", "سلام"), "م\u{FEFB}س");
    // Logical: salam, space, (1405); visual: (۱۴۰۵) on the left with
    // its digits in order and its brackets mirrored back.
    assert_eq!(shown_rtl("persian", "سلام (۱۴۰۵)"), "(۱۴۰۵) م\u{FEFB}س");
    // A Latin run keeps its order. The row starts in the Urdu set, so
    // an ESC (shown as a space) leads into BBC, and the ESC back takes
    // the place of the space between the words.
    assert_eq!(shown_rtl("urdu", "خبر BBC"), " BBC ربخ");
}

/// A long right-to-left line breaks between words, and each row is
/// right-aligned on the page.
#[test]
fn right_to_left_rows_break_and_align_right() {
    let word = "ب".repeat(12);
    let line = [word.as_str(); 4].join(" ");
    let written = write_text("persian", 0x100, &line).unwrap();
    assert_eq!(written.rows, 2, "three words of 12 and two spaces fit 40 cells");
}

/// The rows of a Thai page as codes, and its X/26 cells.
fn thai_rows(text: &str) -> Vec<Vec<Cell>> {
    let (_, sets) = page_sets("thai").unwrap();
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        lines.extend(marked_lines(&sets, line, index + 1).unwrap());
    }
    marked_rows(lines)
}

fn code(character: char) -> Cell {
    let (_, sets) = page_sets("thai").unwrap();
    Cell::Code(Sets::code_in(sets.default, character).unwrap())
}

/// Section 7.5: a mark goes on the row above its consonant, in its
/// column, and a lower vowel on the row below.
#[test]
fn thai_marks_take_rows_of_their_own() {
    let rows = thai_rows("กิ กุ");
    assert_eq!(rows.len(), 3, "marks, letters, lower vowels");
    assert_eq!(rows[0][0], code('\u{0E34}'), "sara i over ko kai");
    assert_eq!(rows[1][..3], [code('ก'), Cell::Code(SPACE), code('ก')]);
    assert_eq!(rows[2][2], code('\u{0E38}'), "sara u under the second ko kai");
}

/// A vowel with a tone is one G2 glyph over the vowel alone; ำ is ํ
/// over its consonant and า beside it; with a tone, ํ and the tone are
/// one G2 glyph.
#[test]
fn thai_two_marks_are_one_g2_glyph() {
    let rows = thai_rows("นี้ น้ำ");
    let Cell::G2 { stand_in, code: g2 } = rows[0][0] else { panic!("{:?}", rows[0][0]) };
    assert_eq!(Cell::Code(stand_in), code('\u{0E35}'), "the vowel alone at Level 1");
    let unicode = to_unicode(CharRef::new(CharSet::G2(G2Set::Thai), g2));
    assert_eq!((unicode.base, unicode.combining), ('\u{0E35}', Some('\u{0E49}')));
    assert_eq!(rows[1][2..4], [code('น'), code('\u{0E32}')], "sara am's aa beside its consonant");
    let Cell::G2 { code: g2, .. } = rows[0][2] else { panic!("{:?}", rows[0][2]) };
    let unicode = to_unicode(CharRef::new(CharSet::G2(G2Set::Thai), g2));
    assert_eq!((unicode.base, unicode.combining), ('\u{0E4D}', Some('\u{0E49}')));
}

/// A line's lower vowels share the next line's mark row, unless a
/// column needs both; then the pair of lines takes a third row.
#[test]
fn lower_vowels_share_a_row_unless_a_column_needs_both() {
    assert_eq!(thai_rows("กุ\nขก").len(), 4, "u under ko kai shares the row above line 2");
    let rows = thai_rows("กุ\nกิ");
    assert_eq!(rows.len(), 5, "column 0 needs u and i: a row each");
    assert_eq!(rows[2][0], code('\u{0E38}'));
    assert_eq!(rows[3][0], code('\u{0E34}'));
}

/// REQ-CLI-010: a page whose marks exceed the sixteen packets X/26 is
/// refused.
#[test]
fn the_x26_budget_is_enforced() {
    let row = "ả".repeat(COLUMNS);
    let fits = [row.as_str(); 5].join("\n");
    let written = write_text("vietnamese", 0x100, &fits).unwrap();
    assert_eq!(written.triplets, 5 * (COLUMNS + 1));
    let over = [row.as_str(); 6].join("\n");
    assert_eq!(write_text("vietnamese", 0x100, &over).unwrap_err(), WriteError::Author(AuthorError::LocalDataFull));
}
