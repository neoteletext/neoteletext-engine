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

fn fonts() -> GlyphSet<'static> {
    GlyphSet::bedstead().with_fonts(Some(LoadedFont::new(&IN_USE).unwrap()), Some(LoadedFont::new(&FALLBACK).unwrap()))
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
    let only_user = bedstead.with_fonts(None, Some(LoadedFont::new(&FALLBACK).unwrap()));
    assert_eq!(lines(&cell(text(b'A')), &only_user, CellMode::Saa5246a), OTHER.to_vec());
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
        .with_fonts(Some(LoadedFont::new(&[FontGlyph { code_point: '\u{A3}' as u32, rows: OTHER }]).unwrap()), None);
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
