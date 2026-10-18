//! Tests of drawing loaded fonts (REQ-FNT-005, REQ-FNT-008): a 12 by 10
//! glyph drawn as given, each line twice in the 12-by-20 cell, never
//! rounded, enlarged as a mode 0 DRCS character is, with every attribute,
//! and the fallback from the font in use to the user's font to the bundled
//! glyphs. The test fonts are patterns made here, not typefaces.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see `render.rs`); the attribute
// also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::charset::{CharRef, CharSet, G0, G0Set, G2Set, NationalOption};
use crate::glyphs::conformance::{FONT_SET_COUNT, font_set_index};
use crate::glyphs::{FontTableError, LoadedFont};
use crate::level1::CellAttributes;
use crate::types::{DrcsCharacter, FontGlyph};
use std::vec::Vec;

/// A pattern whose ten lines all differ, so a test sees which line is
/// drawn: a diagonal from the top left, with the rightmost dot on even
/// lines.
const PATTERN: [u16; CELL_FIELD_LINES] = {
    let mut rows = [0u16; CELL_FIELD_LINES];
    let mut line = 0;
    while line < CELL_FIELD_LINES {
        rows[line] = (0x800 >> line) | if line % 2 == 0 { 1 } else { 0 };
        line += 1;
    }
    rows
};
/// A second pattern, for the user's font.
const OTHER: [u16; CELL_FIELD_LINES] = [0xFFF, 0, 0xFFF, 0, 0xFFF, 0, 0xFFF, 0, 0xFFF, 0];

static IN_USE: [FontGlyph; 2] =
    [FontGlyph { code_point: 'A' as u32, rows: PATTERN }, FontGlyph { code_point: '\u{E9}' as u32, rows: PATTERN }];
static FALLBACK: [FontGlyph; 2] =
    [FontGlyph { code_point: 'A' as u32, rows: OTHER }, FontGlyph { code_point: 'B' as u32, rows: OTHER }];

fn latin() -> CharSet {
    CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) })
}

fn text(code: u8) -> Glyph {
    Glyph::Char(CharRef::new(latin(), code))
}

/// No user font for any set.
const NONE: [Option<LoadedFont<'static>>; FONT_SET_COUNT] = [None; FONT_SET_COUNT];

/// A user font for one set, by its name in `FONT_SETS`.
fn for_set(name: &str, font: LoadedFont<'static>) -> [Option<LoadedFont<'static>>; FONT_SET_COUNT] {
    let mut by_set = NONE;
    by_set[font_set_index(name).unwrap()] = Some(font);
    by_set
}

/// The font in use, and the user's font for G0 Latin with the English
/// option, the set `text` draws.
fn fonts() -> GlyphSet<'static> {
    GlyphSet::bedstead().with_fonts(
        Some(LoadedFont::new(&IN_USE).unwrap()),
        for_set("G0-Latin-English", LoadedFont::new(&FALLBACK).unwrap()),
    )
}

fn options(mode: CellMode) -> RenderOptions {
    RenderOptions { mode, rounding: true, reveal: false, colour: ColourMode::Resolved(FlashPhase::STEADY) }
}

/// The dots of every line of a cell at text row 1.
fn lines(cell: &Cell, glyphs: &GlyphSet<'_>, mode: CellMode) -> Vec<u16> {
    let grid = Grid::EMPTY;
    (0..options(mode).row_height()).map(|line| cell_line(cell, &grid, 1, 0, line, glyphs, options(mode)).mask).collect()
}

fn cell(glyph: Glyph) -> Cell {
    Cell { glyph, ..Cell::BLANK }
}

/// REQ-FNT-008: in the 12-by-20 cell each glyph line is drawn twice, and
/// in the 12-by-10 cell once; rounding never applies.
#[test]
fn normal_size_draws_each_line_as_given() {
    let a = cell(text(b'A'));
    let expected: Vec<u16> = (0..CELL_HEIGHT).map(|line| PATTERN[line / 2]).collect();
    assert_eq!(lines(&a, &fonts(), CellMode::Saa5050), expected, "12 by 20, rounding on");
    assert_eq!(lines(&a, &fonts(), CellMode::Saa5246a), PATTERN.to_vec(), "12 by 10");
}

/// REQ-FNT-008, REQ-RND-038: double height draws glyph lines 0 to 4 in the
/// upper row and 5 to 9 in the lower, each line on four output lines of the
/// 12-by-20 cell and two of the 12-by-10.
#[test]
fn double_height_halves() {
    let top = Cell { size: Size::DoubleTop, ..cell(text(b'A')) };
    let bottom = Cell { size: Size::DoubleBottom, ..cell(text(b'A')) };
    let large_top: Vec<u16> = (0..CELL_HEIGHT).map(|line| PATTERN[line / 4]).collect();
    let large_bottom: Vec<u16> = (0..CELL_HEIGHT).map(|line| PATTERN[5 + line / 4]).collect();
    assert_eq!(lines(&top, &fonts(), CellMode::Saa5050), large_top);
    assert_eq!(lines(&bottom, &fonts(), CellMode::Saa5050), large_bottom);
    let small_top: Vec<u16> = (0..CELL_FIELD_LINES).map(|line| PATTERN[line / 2]).collect();
    let small_bottom: Vec<u16> = (0..CELL_FIELD_LINES).map(|line| PATTERN[5 + line / 2]).collect();
    assert_eq!(lines(&top, &fonts(), CellMode::Saa5246a), small_top);
    assert_eq!(lines(&bottom, &fonts(), CellMode::Saa5246a), small_bottom);
}

/// REQ-RND-034: double width draws the left or right six dots of each line,
/// each twice; double size combines it with double height.
#[test]
fn double_width_and_double_size() {
    let widen =
        |six: u16| (0..6).fold(0u16, |acc, dot| if (six >> dot) & 1 == 1 { acc | (0b11 << (2 * dot)) } else { acc });
    for (width, part) in [(Width::LeftHalf, 6), (Width::RightHalf, 0)] {
        let wide = Cell { attributes: CellAttributes::DEFAULT.with_width(width), ..cell(text(b'A')) };
        let expected: Vec<u16> = PATTERN.iter().map(|row| widen((row >> part) & 0x3F)).collect();
        assert_eq!(lines(&wide, &fonts(), CellMode::Saa5246a), expected, "{width:?}");
        let size = Cell { size: Size::DoubleBottom, ..wide };
        let expected: Vec<u16> =
            (0..CELL_FIELD_LINES).map(|line| widen((PATTERN[5 + line / 2] >> part) & 0x3F)).collect();
        assert_eq!(lines(&size, &fonts(), CellMode::Saa5246a), expected, "double size {width:?}");
    }
}

/// REQ-FNT-005: a character the font in use lacks comes from the user's
/// font, then from the bundled glyphs; a character in both comes from the
/// font in use.
#[test]
fn fallback_order() {
    let bedstead = GlyphSet::bedstead();
    assert_eq!(lines(&cell(text(b'A')), &fonts(), CellMode::Saa5246a), PATTERN.to_vec(), "the font in use wins");
    assert_eq!(lines(&cell(text(b'B')), &fonts(), CellMode::Saa5246a), OTHER.to_vec(), "then the user's font");
    let c = cell(text(b'C'));
    assert_eq!(lines(&c, &fonts(), CellMode::Saa5050), lines(&c, &bedstead, CellMode::Saa5050), "then Bedstead");
    let only_user = bedstead.with_fonts(None, for_set("G0-Latin-English", LoadedFont::new(&FALLBACK).unwrap()));
    assert_eq!(lines(&cell(text(b'A')), &only_user, CellMode::Saa5246a), OTHER.to_vec());
    // The user's font for another set does not draw this set's characters.
    let other_set = bedstead.with_fonts(None, for_set("G0-Latin-German", LoadedFont::new(&FALLBACK).unwrap()));
    let a = cell(text(b'A'));
    assert_eq!(lines(&a, &other_set, CellMode::Saa5050), lines(&a, &bedstead, CellMode::Saa5050));
    assert!(!bedstead.has_fonts() && fonts().has_fonts() && !fonts().without_fonts().has_fonts());
}

/// REQ-FNT-005: row 0 is always drawn in the bundled glyphs.
#[test]
fn row_0_is_always_bedstead() {
    let grid = Grid::EMPTY;
    let a = cell(text(b'A'));
    for line in 0..CELL_HEIGHT {
        let with = cell_line(&a, &grid, 0, 10, line, &fonts(), options(CellMode::Saa5050));
        let without = cell_line(&a, &grid, 0, 10, line, &GlyphSet::bedstead(), options(CellMode::Saa5050));
        assert_eq!(with, without, "line {line}");
    }
}

/// REQ-FNT-008: colours, underline and the font styles apply as to any
/// character.
#[test]
fn attributes_apply() {
    let grid = Grid::EMPTY;
    let coloured = Cell { foreground: 1, background: 4, ..cell(text(b'A')) };
    let raster = cell_line(&coloured, &grid, 1, 0, 0, &fonts(), options(CellMode::Saa5246a));
    assert_eq!((raster.foreground, raster.background, raster.mask), (1, 4, PATTERN[0]));
    let underlined = Cell { attributes: CellAttributes::DEFAULT.with_underline(true), ..cell(text(b'A')) };
    let drawn = lines(&underlined, &fonts(), CellMode::Saa5246a);
    assert_eq!(drawn[CELL_FIELD_LINES - 1], 0xFFF, "the last line underlined");
    assert_eq!(drawn[..CELL_FIELD_LINES - 1], PATTERN[..CELL_FIELD_LINES - 1]);
}

/// Decision E.72 item 8: mosaics, G3 and DRCS characters are never drawn
/// from a font.
#[test]
fn mosaics_g3_and_drcs_are_unchanged() {
    for glyph in [
        Glyph::Mosaic { pattern: 0x2A, separated: false },
        Glyph::Mosaic { pattern: 0x15, separated: true },
        Glyph::Char(CharRef::new(CharSet::G3, 0x2F)),
        Glyph::Char(CharRef::drcs(DrcsCharacter { table: DrcsTable::Normal, subtable: 0, character: 3 })),
        Glyph::Space,
    ] {
        let c = cell(glyph);
        for mode in [CellMode::Saa5050, CellMode::Saa5246a] {
            assert_eq!(lines(&c, &fonts(), mode), lines(&c, &GlyphSet::bedstead(), mode), "{glyph:?}");
        }
    }
}

/// A character with a diacritical mark comes from the font's precomposed
/// glyph; without one, from the bundled glyphs, which overlay the mark
/// (decision E.26).
#[test]
fn diacritics() {
    // Diacritic 2 is the acute accent (Table 28, mode 10010).
    let e_acute = cell(Glyph::Char(CharRef { diacritic: 2, ..CharRef::new(latin(), b'e') }));
    assert_eq!(lines(&e_acute, &fonts(), CellMode::Saa5246a), PATTERN.to_vec(), "precomposed in the font");
    let a_acute = cell(Glyph::Char(CharRef { diacritic: 2, ..CharRef::new(latin(), b'a') }));
    assert_eq!(
        lines(&a_acute, &fonts(), CellMode::Saa5050),
        lines(&a_acute, &GlyphSet::bedstead(), CellMode::Saa5050),
        "not in the font: Bedstead"
    );
    // A G2 character is looked up like a G0 one.
    let g2 = GlyphSet::bedstead()
        .with_fonts(Some(LoadedFont::new(&[FontGlyph { code_point: '\u{A3}' as u32, rows: OTHER }]).unwrap()), NONE);
    let pound = cell(Glyph::Char(CharRef::new(CharSet::G2(G2Set::Latin), 0x23)));
    assert_eq!(lines(&pound, &g2, CellMode::Saa5246a), OTHER.to_vec());
}

/// REQ-FNT-003, REQ-FNT-008: a table of the wrong shape is refused.
#[test]
fn table_shape_is_checked() {
    let glyph = |code_point: u32| FontGlyph { code_point, rows: [0; CELL_FIELD_LINES] };
    assert_eq!(LoadedFont::new(&[glyph(66), glyph(65)]).unwrap_err(), FontTableError::Order);
    assert_eq!(LoadedFont::new(&[glyph(65), glyph(65)]).unwrap_err(), FontTableError::Order);
    assert_eq!(LoadedFont::new(&[glyph(0xD800)]).unwrap_err(), FontTableError::Code);
    assert_eq!(LoadedFont::new(&[glyph(0x11_0000)]).unwrap_err(), FontTableError::Code);
    let wide = FontGlyph { code_point: 65, rows: [0x1000, 0, 0, 0, 0, 0, 0, 0, 0, 0] };
    assert_eq!(LoadedFont::new(&[wide]).unwrap_err(), FontTableError::Width);
    let too_many: Vec<FontGlyph> = (0..1025).map(|code| glyph(0x100 + code)).collect();
    assert_eq!(LoadedFont::new(&too_many).unwrap_err(), FontTableError::TooMany);
    let font = LoadedFont::new(&too_many[..1024]).unwrap();
    assert_eq!((font.len(), font.is_empty()), (1024, false));
    assert!(LoadedFont::new(&[]).unwrap().is_empty());
}

/// Bedstead's 5 by 9 glyphs as a loaded font: below the blank top line,
/// each dot two pixels wide, as `neottx font convert` makes them.
fn bedstead_as_loaded(entry: &crate::glyphs::GlyphEntry) -> FontGlyph {
    let mut rows = [0u16; CELL_FIELD_LINES];
    for (row, &dots) in rows.iter_mut().skip(1).zip(&entry.rows) {
        *row = widen(dots);
    }
    FontGlyph { code_point: entry.code_point, rows }
}

/// Issue #209: a doubled dot matrix is rounded as the bundled glyphs are,
/// so Bedstead converted from its matrices draws exactly as Bedstead.
#[test]
fn a_doubled_dot_font_draws_as_bedstead() {
    let entries = GlyphSet::bedstead().entries();
    for entry in entries {
        let loaded = bedstead_as_loaded(entry);
        for line in 0..CELL_HEIGHT {
            for rounding in [true, false] {
                assert_eq!(
                    font_line_20(&loaded.rows, line, rounding),
                    glyph_line_20(&entry.rows, line, rounding),
                    "U+{:04X} line {line}, rounding {rounding}",
                    entry.code_point
                );
            }
        }
    }
    // Through the renderer: the letter A from the loaded font and from the
    // bundled glyphs.
    let a = entries.iter().find(|entry| entry.code_point == u32::from('A')).unwrap();
    let table = [bedstead_as_loaded(a)];
    let font = LoadedFont::new(&table).unwrap();
    assert!(font.is_dot_matrix());
    let glyphs = GlyphSet::bedstead().with_fonts(Some(font), NONE);
    let a_cell = cell(text(b'A'));
    assert_eq!(lines(&a_cell, &glyphs, CellMode::Saa5050), lines(&a_cell, &GlyphSet::bedstead(), CellMode::Saa5050));
}

/// Issue #209: only a font of doubled pixels is rounded, and only with
/// rounding on.
#[test]
fn only_doubled_dot_fonts_are_rounded() {
    assert!(!LoadedFont::new(&IN_USE).unwrap().is_dot_matrix(), "the half-dot pattern");
    assert!(LoadedFont::new(&FALLBACK).unwrap().is_dot_matrix(), "whole rows are pairs");
    assert!(!LoadedFont::new(&[]).unwrap().is_dot_matrix());
    // A doubled diagonal gains half dots where the rows meet.
    let mut rows = [0u16; CELL_FIELD_LINES];
    rows[1] = 0b11_0000_0000;
    rows[2] = 0b00_1100_0000;
    let table = [FontGlyph { code_point: u32::from('A'), rows }];
    let glyphs = GlyphSet::bedstead().with_fonts(Some(LoadedFont::new(&table).unwrap()), NONE);
    let a_cell = cell(text(b'A'));
    let drawn = lines(&a_cell, &glyphs, CellMode::Saa5050);
    assert_eq!(drawn[3], 0b11_1000_0000, "the lower half of line 1 reaches toward line 2");
    assert_eq!(drawn[4], 0b01_1100_0000, "the upper half of line 2 reaches toward line 1");
    // The half-dot pattern is drawn as given.
    let plain = lines(&cell(text(b'A')), &fonts(), CellMode::Saa5050);
    assert!(plain.iter().enumerate().all(|(line, &dots)| dots == PATTERN[line / 2]));
}

/// REQ-FNT-005, decision E.75 item 6: characters of two sets on one page
/// are drawn from the user's fonts for their own sets.
#[test]
fn each_set_draws_from_its_own_font() {
    static LATIN: [FontGlyph; 1] = [FontGlyph { code_point: 'A' as u32, rows: PATTERN }];
    static G2: [FontGlyph; 1] = [FontGlyph { code_point: '\u{A1}' as u32, rows: OTHER }];
    let mut by_set = for_set("G0-Latin-English", LoadedFont::new(&LATIN).unwrap());
    by_set[font_set_index("G2-Latin").unwrap()] = Some(LoadedFont::new(&G2).unwrap());
    let glyphs = GlyphSet::bedstead().with_fonts(None, by_set);
    assert_eq!(lines(&cell(text(b'A')), &glyphs, CellMode::Saa5246a), PATTERN.to_vec(), "G0 from its font");
    // G2 Latin 2/1 is the inverted exclamation mark.
    let inverted = Glyph::Char(CharRef::new(CharSet::G2(G2Set::Latin), 0x21));
    assert_eq!(lines(&cell(inverted), &glyphs, CellMode::Saa5246a), OTHER.to_vec(), "G2 from its own");
}
