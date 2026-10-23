//! BDF reader tests: the REQ-FNT-002 profile, its errors and its limits.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use super::*;
use core::fmt::Write as _;
use neoteletext_core::consts::{FONT_MAX_CHARACTERS, FONT_MAX_FILE_BYTES, FONT_MAX_LINE_BYTES};
use std::string::String;
use std::vec::Vec;
use std::{format, vec};

/// A glyph table with room for any font, on the heap.
fn table() -> Vec<FontGlyph> {
    vec![FontGlyph::EMPTY; FONT_MAX_CHARACTERS]
}

/// A character: its code point, then ten rows of four digits.
fn character(code: i64, rows: &str) -> String {
    format!(
        "STARTCHAR u{code:04X}\nENCODING {code}\nSWIDTH 1200 0\nDWIDTH 12 0\nBBX 12 10 0 0\nBITMAP\n{rows}ENDCHAR\n"
    )
}

const A_ROWS: &str = "0000\n0F00\n30C0\n6060\n6060\n7FE0\n6060\n6060\n0000\n0000\n";
const BLANK_ROWS: &str = "0000\n0000\n0000\n0000\n0000\n0000\n0000\n0000\n0000\n0000\n";

/// A valid font with `characters`, sent through `edit` first.
fn font(characters: &[String], edit: impl Fn(String) -> String) -> String {
    let mut text = String::from(
        "STARTFONT 2.1\nCOMMENT anywhere\nFONT -test-Test-Medium-R-Normal--10-100-75-75-C-120-ISO10646-1\n\
         SIZE 10 75 75\nFONTBOUNDINGBOX 12 10 0 0\nSTARTPROPERTIES 6\nFAMILY_NAME \"Test \"\"Font\"\"\"\n\
         COPYRIGHT \"(c) nobody\"\nNOTICE \"CC0\"\nCHARSET_REGISTRY \"ISO10646\"\nCHARSET_ENCODING \"1\"\n\
         SPACING \"C\"\nENDPROPERTIES\n",
    );
    let _ = writeln!(text, "CHARS {}", characters.len());
    for character in characters {
        text.push_str(character);
    }
    text.push_str("ENDFONT\n");
    edit(text)
}

fn valid() -> String {
    font(&[character(0x42, A_ROWS), character(0x41, A_ROWS), character(0x20, BLANK_ROWS)], |text| text)
}

fn refused(text: &str) -> FontError {
    let mut glyphs = table();
    read(text.as_bytes(), &mut glyphs).unwrap_err()
}

fn kind(text: &str) -> FontErrorKind {
    refused(text).kind
}

/// REQ-FNT-002: a valid font gives its glyphs sorted, its rows as 12
/// dots with bit 11 the leftmost, and its name and notices.
#[test]
fn a_valid_font_is_read() {
    let text = valid();
    let mut glyphs = table();
    let font = read(text.as_bytes(), &mut glyphs).unwrap();
    assert_eq!(font.characters, 3);
    assert_eq!(font.family, Some(&b"Test \"\"Font\"\""[..]));
    assert_eq!(font.copyright, Some(&b"(c) nobody"[..]));
    assert_eq!(font.notice, Some(&b"CC0"[..]));
    let codes: Vec<u32> = glyphs[..3].iter().map(|glyph| glyph.code_point).collect();
    assert_eq!(codes, [0x20, 0x41, 0x42], "sorted by code point");
    assert_eq!(glyphs[1].lines(), [0x000, 0x0F0, 0x30C, 0x606, 0x606, 0x7FE, 0x606, 0x606, 0x000, 0x000]);
    assert!(!glyphs[1].tall);
    assert_eq!(glyphs[0].lines(), [0; 10]);
}

/// REQ-FNT-002 as amended by decision E.77: a 12 by 20 font is read
/// with its twenty rows; a 12 by 10 character in it, or a character of
/// the wrong number of rows, refuses the file.
#[test]
fn a_tall_font_is_read() {
    let mut twenty = String::new();
    for line in 1..=20 {
        let _ = writeln!(twenty, "{line:03X}0");
    }
    let tall = |rows: &str| character(0x41, rows).replace("BBX 12 10 0 0", "BBX 12 20 0 0");
    let text = font(&[tall(&twenty)], |text| text.replace("FONTBOUNDINGBOX 12 10 0 0", "FONTBOUNDINGBOX 12 20 0 0"));
    let mut glyphs = table();
    let font_read = read(text.as_bytes(), &mut glyphs).unwrap();
    assert!(font_read.tall);
    assert!(glyphs[0].tall);
    let expected: Vec<u16> = (1..=20).collect();
    assert_eq!(glyphs[0].lines(), expected.as_slice());
    let small_in_tall =
        font(&[character(0x41, A_ROWS)], |text| text.replace("FONTBOUNDINGBOX 12 10 0 0", "FONTBOUNDINGBOX 12 20 0 0"));
    assert_eq!(read(small_in_tall.as_bytes(), &mut glyphs).unwrap_err().kind, FontErrorKind::CharacterBox);
    let short = font(&[tall(A_ROWS)], |text| text.replace("FONTBOUNDINGBOX 12 10 0 0", "FONTBOUNDINGBOX 12 20 0 0"));
    assert_eq!(read(short.as_bytes(), &mut glyphs).unwrap_err().kind, FontErrorKind::Incomplete);
    let long = font(&[character(0x41, &twenty)], |text| text);
    assert_eq!(read(long.as_bytes(), &mut glyphs).unwrap_err().kind, FontErrorKind::Row);
}

/// Line endings may be CRLF, the last line may lack its newline, empty
/// lines may follow ENDFONT, the keywords that BDF 2.1 requires are
/// accepted, and the case of the registry and spacing does not matter.
#[test]
fn tolerated_forms() {
    let mut glyphs = table();
    for text in [
        valid().replace('\n', "\r\n"),
        valid().trim_end().into(),
        format!("{}\n\n", valid()),
        valid().replace("\"ISO10646\"", "\"iso10646\"").replace("SPACING \"C\"", "SPACING \"m\""),
        valid().replace("STARTFONT 2.1", "STARTFONT 2.2"),
        format!("COMMENT before\n{}", valid()),
        valid().replace("BITMAP\n0000", "BITMAP\nCOMMENT inside\n0000"),
        valid().replace("SIZE 10 75 75\n", "").replace("FONT -test", "COMMENT -test"),
        valid().replace("SWIDTH 1200 0\n", ""),
        valid().replace("0F00", "0f00"),
    ] {
        assert!(read(text.as_bytes(), &mut glyphs).is_ok(), "{text}");
    }
    let unnamed = valid().replace("STARTPROPERTIES 6\nFAMILY_NAME \"Test \"\"Font\"\"\"\n", "STARTPROPERTIES 5\n");
    assert_eq!(read(unnamed.as_bytes(), &mut glyphs).unwrap().family, None);
}

/// REQ-FNT-002: each rule broken refuses the whole file, with the line.
#[test]
fn each_rule_broken_refuses_the_file() {
    let v = valid();
    let cases: Vec<(String, FontErrorKind)> = vec![
        (v.replace("STARTFONT 2.1\n", ""), FontErrorKind::NotBdf),
        (String::new(), FontErrorKind::NotBdf),
        (v.replace("STARTFONT 2.1", "STARTFONT 2.3"), FontErrorKind::Version),
        (v.replace("COMMENT anywhere", "METRICSSET 0"), FontErrorKind::UnknownKeyword),
        (v.replace("COMMENT anywhere", "COMMENT anywhere\n"), FontErrorKind::BadLine),
        (v.replace("COMMENT anywhere", "COMMENT any\u{7}where"), FontErrorKind::BadLine),
        (v.replace("FONTBOUNDINGBOX 12 10 0 0", "FONTBOUNDINGBOX 6 10 0 0"), FontErrorKind::BoundingBox),
        (v.replace("FONTBOUNDINGBOX 12 10 0 0\n", ""), FontErrorKind::BoundingBox),
        (
            v.replace("FONTBOUNDINGBOX 12 10 0 0", "FONTBOUNDINGBOX 12 10 0 0\nFONTBOUNDINGBOX 12 10 0 0"),
            FontErrorKind::Misplaced,
        ),
        (v.replace("SIZE 10 75 75", "SIZE 10 75"), FontErrorKind::Malformed),
        (v.replace("SIZE 10 75 75", "SIZE 10 75 x"), FontErrorKind::Malformed),
        (v.replace("STARTPROPERTIES 6", "STARTPROPERTIES 7"), FontErrorKind::Properties),
        (v.replace("STARTPROPERTIES 6", "STARTPROPERTIES 5"), FontErrorKind::Properties),
        (v.replace("STARTPROPERTIES 6", "STARTPROPERTIES 65"), FontErrorKind::Properties),
        (v.replace("NOTICE \"CC0\"", "NOTICE \"C\"C0\""), FontErrorKind::Properties),
        (v.replace("NOTICE \"CC0\"", "NOTICE \"CC0"), FontErrorKind::Properties),
        (v.replace("NOTICE \"CC0\"", "notice \"CC0\""), FontErrorKind::Properties),
        (v.replace("NOTICE \"CC0\"", "FAMILY_NAME \"Again\""), FontErrorKind::Properties),
        (v.replace("\"ISO10646\"", "\"ISO8859\""), FontErrorKind::Registry),
        (v.replace("CHARSET_REGISTRY \"ISO10646\"", "PIXEL_SIZE 10"), FontErrorKind::Registry),
        (v.replace("CHARSET_ENCODING \"1\"", "CHARSET_ENCODING \"2\""), FontErrorKind::CharsetEncoding),
        (v.replace("SPACING \"C\"", "SPACING \"P\""), FontErrorKind::Spacing),
        (v.replace("CHARS 3", "CHARS 4"), FontErrorKind::Characters),
        (v.replace("CHARS 3", "CHARS 2"), FontErrorKind::Characters),
        (v.replace("CHARS 3", "CHARS 1025"), FontErrorKind::Characters),
        (v.replace("ENCODING 65", "ENCODING -1"), FontErrorKind::Code),
        (v.replace("ENCODING 65", "ENCODING -1 65"), FontErrorKind::Code),
        (v.replace("ENCODING 65", "ENCODING 55296"), FontErrorKind::Code),
        (v.replace("ENCODING 65", "ENCODING 1114112"), FontErrorKind::Code),
        (v.replace("ENCODING 65", "ENCODING 66"), FontErrorKind::RepeatedCode),
        (v.replacen("DWIDTH 12 0", "DWIDTH 6 0", 1), FontErrorKind::Advance),
        (v.replacen("BBX 12 10 0 0", "BBX 12 9 0 1", 1), FontErrorKind::CharacterBox),
        (v.replacen("0F00", "0F01", 1), FontErrorKind::Row),
        (v.replacen("0F00", "0F0", 1), FontErrorKind::Row),
        (v.replacen("0F00", "0G00", 1), FontErrorKind::Row),
        (v.replacen("0F00\n", "", 1), FontErrorKind::Incomplete),
        (v.replacen("0F00\n", "0F00\n0F00\n", 1), FontErrorKind::Row),
        (v.replacen("DWIDTH 12 0\n", "", 1), FontErrorKind::Incomplete),
        (v.replacen("BITMAP\n", "ENDCHAR\nBITMAP\n", 1), FontErrorKind::Incomplete),
        (v.replace("ENDFONT\n", ""), FontErrorKind::NoEnd),
        (format!("{v}COMMENT after\n"), FontErrorKind::AfterEnd),
        (v.replace("CHARS 3", "STARTCHAR early\nCHARS 3"), FontErrorKind::Misplaced),
    ];
    for (text, expected) in cases {
        assert_eq!(kind(&text), expected, "{text}");
    }
}

/// The fault's line is reported, counted from 1.
#[test]
fn the_line_is_reported() {
    let text = valid().replace("SPACING \"C\"", "SPACING \"P\"");
    let line = text.lines().position(|line| line.starts_with("SPACING")).unwrap() + 1;
    assert_eq!(refused(&text), FontError { line: u32::try_from(line).unwrap(), kind: FontErrorKind::Spacing });
}

/// The size limits of REQ-FNT-002, and a table too small for the font.
#[test]
fn limits() {
    let too_large = format!("{}{}", valid(), "\n".repeat(FONT_MAX_FILE_BYTES));
    assert_eq!(refused(&too_large), FontError { line: 0, kind: FontErrorKind::TooLarge });
    let long = valid().replace("COMMENT anywhere", &format!("COMMENT {}", "x".repeat(FONT_MAX_LINE_BYTES)));
    assert_eq!(kind(&long), FontErrorKind::LineTooLong);
    let mut small = [FontGlyph::EMPTY; 2];
    assert_eq!(read(valid().as_bytes(), &mut small).unwrap_err().kind, FontErrorKind::Characters);
    // 1,024 characters fit.
    let all: Vec<String> = (0..1024).map(|code| character(0x100 + code, BLANK_ROWS)).collect();
    let full = font(&all, |text| text);
    let mut glyphs = table();
    assert_eq!(read(full.as_bytes(), &mut glyphs).unwrap().characters, 1024);
}
