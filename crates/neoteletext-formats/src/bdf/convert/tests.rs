//! Font converter tests: the map file, the cell conversions and the round trip through the writer.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use super::super::FontErrorKind;
use super::super::source::read_source;
use super::*;
use core::fmt::Write as _;
use std::string::String;
use std::vec::Vec;
use std::{format, vec};

/// A source font: cell, registry, then each glyph as its encoding, BBX
/// and pixel rows of `#` and `.`.
fn source(cell: (usize, usize), registry: &str, glyphs: &[(i64, [i32; 4], &[&str])]) -> String {
    let (width, height) = cell;
    let mut text = format!(
        "STARTFONT 2.1\nFONT -x-source\nSIZE {height} 75 75\nFONTBOUNDINGBOX {width} {height} 0 0\n\
         STARTPROPERTIES 4\nFAMILY_NAME \"Source\"\nCOPYRIGHT \"(c) nobody\"\nCHARSET_REGISTRY \"{registry}\"\n\
         CHARSET_ENCODING \"1\"\nENDPROPERTIES\nCHARS {}\n",
        glyphs.len()
    );
    for (encoding, bbx, rows) in glyphs {
        let [w, h, x, y] = bbx;
        let _ = write!(
            text,
            "STARTCHAR c{encoding}\nENCODING {encoding}\nSWIDTH 600 0\nDWIDTH {width} 0\nBBX {w} {h} {x} {y}\nBITMAP\n"
        );
        let digits = usize::try_from(*w).unwrap().div_ceil(8) * 2;
        for row in *rows {
            let bits = row.chars().fold(0u32, |acc, c| (acc << 1) | u32::from(c == '#'));
            let padded = bits << (digits * 4 - row.len());
            let _ = writeln!(text, "{padded:0digits$X}");
        }
        text.push_str("ENDCHAR\n");
    }
    text.push_str("ENDFONT\n");
    text
}

fn converted(text: &str, map: Option<&[(u32, u32)]>) -> Result<Vec<FontGlyph>, ConvertError> {
    let mut glyphs = vec![SourceGlyph::EMPTY; 64];
    let font = read_source(text.as_bytes(), &mut glyphs).unwrap();
    let mut out = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let count = convert(&font, &glyphs, map, &mut out)?;
    out.truncate(count);
    Ok(out)
}

fn source_error(text: &str) -> FontErrorKind {
    let mut glyphs = vec![SourceGlyph::EMPTY; 64];
    read_source(text.as_bytes(), &mut glyphs).unwrap_err().kind
}

/// Rows of `#` and `.` as numbers, leftmost pixel highest.
fn bits(rows: &[&str]) -> Vec<u16> {
    rows.iter().map(|row| row.chars().fold(0u16, |acc, c| (acc << 1) | u16::from(c == '#'))).collect()
}

/// The reversal of the width doubling: every second pixel, checking the
/// pixel beside it is the same.
fn halved(row: u16, width: usize) -> u16 {
    (0..width).fold(0u16, |acc, pixel| {
        let left = (row >> (11 - 2 * pixel)) & 1;
        let right = (row >> (10 - 2 * pixel)) & 1;
        assert_eq!(left, right, "pixels are doubled");
        (acc << 1) | left
    })
}

const SIX: [&str; 10] =
    ["......", "..#...", ".#.#..", "#...#.", "#...#.", "#####.", "#...#.", "#...#.", "......", "......"];
const FIVE_NINE: [&str; 9] = ["..#..", ".#.#.", "#...#", "#...#", "#####", "#...#", "#...#", ".....", "....."];

/// REQ-FNT-010: 6 by 10 and 5 by 10 double every pixel's width, the five
/// pixels at the right of the six, so the blank column is at the left as
/// Bedstead's is (issue #206); the reversal gives the source back.
#[test]
fn ten_line_cells_double_the_width() {
    let six = converted(&source((6, 10), "ISO10646", &[(65, [6, 10, 0, 0], &SIX)]), None).unwrap();
    assert_eq!(six.len(), 1);
    assert_eq!(six[0].code_point, 65);
    let back: Vec<u16> = six[0].lines().iter().map(|&row| halved(row, 6)).collect();
    assert_eq!(back, bits(&SIX), "reversal");
    let five_rows: Vec<&str> = SIX.iter().map(|row| &row[..5]).collect();
    let five = converted(&source((5, 10), "ISO10646", &[(65, [5, 10, 0, 0], &five_rows)]), None).unwrap();
    assert!(five[0].lines().iter().all(|row| row >> 10 == 0), "the blank column is at the left");
    let back: Vec<u16> = five[0].lines().iter().map(|&row| halved(row << 2, 5)).collect();
    assert_eq!(back, bits(&five_rows));
    // The same design as 6 pixels with a blank left column is the same glyph.
    let blank_left: Vec<String> = five_rows.iter().map(|row| format!(".{row}")).collect();
    let blank_left: Vec<&str> = blank_left.iter().map(String::as_str).collect();
    let six = converted(&source((6, 10), "ISO10646", &[(65, [6, 10, 0, 0], &blank_left)]), None).unwrap();
    assert_eq!(six[0].rows, five[0].rows);
}

/// REQ-FNT-010: a 9-line cell gains a blank top line.
#[test]
fn nine_line_cells_gain_the_blank_top_line() {
    let five = converted(&source((5, 9), "ISO10646", &[(65, [5, 9, 0, 0], &FIVE_NINE)]), None).unwrap();
    assert_eq!(five[0].rows[0], 0, "the SAA5050's blank line");
    let back: Vec<u16> = five[0].lines()[1..].iter().map(|&row| halved(row << 2, 5)).collect();
    assert_eq!(back, bits(&FIVE_NINE));
    let six_rows: Vec<String> = FIVE_NINE.iter().map(|row| format!(".{row}")).collect();
    let six_rows: Vec<&str> = six_rows.iter().map(String::as_str).collect();
    let six = converted(&source((6, 9), "ISO10646", &[(65, [6, 9, 0, 0], &six_rows)]), None).unwrap();
    assert_eq!(six[0].rows, five[0].rows, "6 by 9 with a blank left column is the same glyph");
}

/// REQ-FNT-010 as amended by decision E.77: a 12 by 20 font whose line
/// pairs all match keeps every second line; one with any pair that
/// differs is kept whole as a 12 by 20 font.
#[test]
fn twelve_by_twenty() {
    let wide: Vec<String> = SIX.iter().map(|row| row.chars().flat_map(|c| [c, c]).collect()).collect();
    let doubled: Vec<&str> = wide.iter().flat_map(|row| [row.as_str(), row.as_str()]).collect();
    let glyphs = converted(&source((12, 20), "ISO10646", &[(66, [12, 20, 0, 0], &doubled)]), None).unwrap();
    let back: Vec<u16> = glyphs[0].lines().iter().flat_map(|&row| [row, row]).collect();
    assert_eq!(back, bits(&doubled), "reversal: each line twice");
    let mut rounded = doubled.clone();
    rounded[7] = "############";
    let tall = converted(&source((12, 20), "ISO10646", &[(67, [12, 20, 0, 0], &rounded)]), None).unwrap();
    assert!(tall[0].tall);
    assert_eq!(tall[0].rows.to_vec(), bits(&rounded), "every line kept");
    assert!(!glyphs[0].tall);
}

/// A glyph smaller than its cell is placed by its BBX; one reaching
/// outside the cell is refused, and so is any other cell.
#[test]
fn placement_and_cell_refusals() {
    // Three pixels wide, two lines high, one column in, three lines up.
    let small = converted(&source((6, 10), "ISO10646", &[(65, [3, 2, 1, 3], &["###", "#.#"])]), None).unwrap();
    let mut expected = [0u16; CELL_FIELD_LINES];
    expected[5] = 0b0011_1111_0000;
    expected[6] = 0b0011_0011_0000;
    assert_eq!(small[0].lines(), expected);
    assert_eq!(
        source_error(&source((6, 10), "ISO10646", &[(65, [3, 2, 4, 0], &["###", "###"])])),
        FontErrorKind::GlyphTooLarge
    );
    assert_eq!(
        source_error(&source((6, 10), "ISO10646", &[(65, [6, 2, 0, 9], &["######", "######"])])),
        FontErrorKind::GlyphTooLarge
    );
    assert_eq!(source_error(&source((8, 10), "ISO10646", &[])), FontErrorKind::Cell);
    assert_eq!(source_error(&source((12, 10), "ISO10646", &[])), FontErrorKind::Cell, "already the target form");
    let grey = source((6, 10), "ISO10646", &[]).replace("SIZE 10 75 75", "SIZE 10 75 75 4");
    assert_eq!(source_error(&grey), FontErrorKind::Greyscale);
    assert_eq!(source_error(&source((6, 10), "ISO10646", &[]).replace("2.1", "2.3")), FontErrorKind::Version);
    assert_eq!(
        source_error(&source((6, 10), "ISO10646", &[]).replace("FONT -x", "FOO -x")),
        FontErrorKind::UnknownKeyword
    );
    let unencoded = source((6, 10), "ISO10646", &[(-1, [6, 10, 0, 0], &SIX)]);
    assert_eq!(source_error(&unencoded), FontErrorKind::Code);
}

/// REQ-FNT-010: a font not in ISO10646 needs the map, and a character
/// the map lacks is refused, naming it; two characters cannot share a
/// code point.
#[test]
fn mapping_and_repeats() {
    let text = source((6, 10), "Teletext", &[(0x41, [6, 10, 0, 0], &SIX), (0x23, [6, 10, 0, 0], &SIX)]);
    assert_eq!(converted(&text, None).unwrap_err(), ConvertError::Unmapped { encoding: 0x41 });
    let mut pairs = [(0, 0); 8];
    let count = read_map(b"65\tU+0041\n35\tU+00A3\n", &mut pairs).unwrap();
    let glyphs = converted(&text, Some(&pairs[..count])).unwrap();
    let codes: Vec<u32> = glyphs.iter().map(|glyph| glyph.code_point).collect();
    assert_eq!(codes, [0x41, 0xA3], "sorted by code point");
    let count = read_map(b"65\tU+0041\n", &mut pairs).unwrap();
    assert_eq!(converted(&text, Some(&pairs[..count])).unwrap_err(), ConvertError::Unmapped { encoding: 0x23 });
    let count = read_map(b"65\tU+0041\n35\tU+0041\n", &mut pairs).unwrap();
    assert_eq!(converted(&text, Some(&pairs[..count])).unwrap_err(), ConvertError::Repeated { encoding: 0x23 });
    let iso = source((6, 10), "ISO10646", &[(65, [6, 10, 0, 0], &SIX), (65, [6, 10, 0, 0], &SIX)]);
    assert_eq!(converted(&iso, None).unwrap_err(), ConvertError::Repeated { encoding: 65 });
}

#[test]
fn map_file_faults() {
    let mut pairs = [(0, 0); 4];
    assert_eq!(read_map(b"\n65\tU+0041\r\n\n", &mut pairs), Ok(1), "empty lines and CR LF");
    for (text, line) in [
        (&b"65 U+0041"[..], 1),
        (b"65\tU+41", 1),
        (b"65\t0041", 1),
        (b"x\tU+0041", 1),
        (b"65\tU+D800", 1),
        (b"65\tU+0041\n65\tU+0042", 2),
        (b"65\tU+0041\textra", 1),
        (b"1\tU+0031\n2\tU+0032\n3\tU+0033\n4\tU+0034\n5\tU+0035", 5),
    ] {
        assert_eq!(read_map(text, &mut pairs), Err(MapError { line }), "{text:?}");
    }
}

/// REQ-FNT-010: the converted font passes the reader of loaded fonts,
/// with its glyphs and texts unchanged.
#[cfg(feature = "std")]
#[test]
fn output_passes_the_loaded_font_reader() {
    let text = source((6, 10), "ISO10646", &[(66, [6, 10, 0, 0], &SIX), (65, [6, 10, 0, 0], &SIX)]);
    let mut glyphs = vec![SourceGlyph::EMPTY; 4];
    let font = read_source(text.as_bytes(), &mut glyphs).unwrap();
    let mut table = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let count = convert(&font, &glyphs, None, &mut table).unwrap();
    let carried = crate::write::FontText {
        family: font.family,
        copyright: font.copyright,
        notice: font.notice,
        comment: "Converted by neottx font convert from a 6 x 10 cell (REQ-FNT-010)",
    };
    let mut written = Vec::new();
    crate::write::write_font(&mut written, &carried, &table[..count]).unwrap();
    let mut read_back = vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS];
    let loaded = super::super::read(&written, &mut read_back).unwrap();
    assert_eq!(loaded.characters, 2);
    assert_eq!(read_back[..2], table[..2]);
    assert_eq!(
        (loaded.family, loaded.copyright, loaded.notice),
        (Some(&b"Source"[..]), Some(&b"(c) nobody"[..]), None)
    );
    let long = "x".repeat(600);
    let too_long = crate::write::FontText { family: Some(long.as_bytes()), ..carried };
    assert!(crate::write::write_font(&mut Vec::new(), &too_long, &table[..count]).is_err());
}
