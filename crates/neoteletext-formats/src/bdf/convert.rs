//! The font converter (REQ-FNT-010, decision E.72 item 7): a source font of
//! another cell size to the 12 by 10 form of REQ-FNT-002, only where the
//! conversion is exact and reversible.
//!
//! - 5 by 10 and 6 by 10: every pixel made two pixels wide, a 5-pixel cell
//!   in the left of the 6;
//! - 5 by 9 and 6 by 9: the same, below one blank line, where the SAA5050
//!   leaves it;
//! - 12 by 20: every second line kept, only when each pair of lines, 1 and
//!   2, 3 and 4 and so on, is identical.
//!
//! Anything else is refused, naming the first character that fails.

use super::source::{SourceFont, SourceGlyph};
use crate::consts::FONT_MAP_MAX_FILE_BYTES;
use neoteletext_core::consts::{
    CELL_DOTS, CELL_FIELD_LINES, CELL_HEIGHT, CELL_WIDTH, FONT_MAX_CHARACTERS, GLYPH_LINES,
};
use neoteletext_core::types::FontGlyph;

/// Why a font cannot be converted, and the character, by its `ENCODING`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConvertError {
    /// The font's cell has no exact conversion.
    Cell,
    /// A 12 by 20 glyph whose line pair, counted from 1, differs.
    LinePairs {
        /// The character.
        encoding: u32,
        /// The first line of the pair.
        line: usize,
    },
    /// The font is not in ISO10646 and the map has no line for the
    /// character, or no map was given.
    Unmapped {
        /// The character.
        encoding: u32,
    },
    /// The character's code point is not a Unicode scalar value.
    NotScalar {
        /// The character.
        encoding: u32,
    },
    /// Two characters give the same code point.
    Repeated {
        /// The second character.
        encoding: u32,
    },
    /// More characters than REQ-FNT-002 allows.
    TooMany,
}

/// Why a mapping file was refused, and its line, counted from 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MapError {
    /// The line; 0 for the file as a whole.
    pub line: u32,
}

/// Reads a mapping file for `--map`: one `code<TAB>U+XXXX` line for each
/// character, the code as the font's `ENCODING` writes it, in decimal.
/// Empty lines are allowed. Returns how many pairs `pairs` holds; a code
/// given twice is refused.
pub fn read_map(text: &[u8], pairs: &mut [(u32, u32)]) -> Result<usize, MapError> {
    if text.len() > FONT_MAP_MAX_FILE_BYTES {
        return Err(MapError { line: 0 });
    }
    let mut count = 0;
    for (index, line) in text.split(|&byte| byte == b'\n').enumerate() {
        let number = u32::try_from(index.saturating_add(1)).unwrap_or(u32::MAX);
        let error = MapError { line: number };
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split(|&byte| byte == b'\t');
        let (Some(code), Some(unicode), None) = (fields.next(), fields.next(), fields.next()) else {
            return Err(error);
        };
        let code = decimal(code).ok_or(error)?;
        let hex = unicode.strip_prefix(b"U+").ok_or(error)?;
        if !(4..=6).contains(&hex.len()) {
            return Err(error);
        }
        let scalar = hexadecimal(hex).filter(|&value| char::from_u32(value).is_some()).ok_or(error)?;
        let known = pairs.get(..count).ok_or(error)?;
        if known.iter().any(|&(other, _)| other == code) {
            return Err(error);
        }
        let slot = pairs.get_mut(count).ok_or(error)?;
        *slot = (code, scalar);
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// Converts the glyphs of `font` into `out`, sorted by code point, and
/// returns how many there are. `map` gives the code points of a font not
/// in ISO10646; it is ignored for one that is.
pub fn convert(
    font: &SourceFont<'_>,
    glyphs: &[SourceGlyph],
    map: Option<&[(u32, u32)]>,
    out: &mut [FontGlyph],
) -> Result<usize, ConvertError> {
    let glyphs = glyphs.get(..font.characters).ok_or(ConvertError::TooMany)?;
    let mut count = 0;
    for glyph in glyphs {
        let encoding = glyph.encoding;
        let code_point = if font.unicode {
            encoding
        } else {
            map.and_then(|pairs| pairs.iter().find(|&&(code, _)| code == encoding))
                .map(|&(_, scalar)| scalar)
                .ok_or(ConvertError::Unmapped { encoding })?
        };
        if char::from_u32(code_point).is_none() {
            return Err(ConvertError::NotScalar { encoding });
        }
        let rows = cell_rows(font.width, font.height, glyph)?;
        if count >= FONT_MAX_CHARACTERS || count >= out.len() {
            return Err(ConvertError::TooMany);
        }
        let sorted = out.get(..count).ok_or(ConvertError::TooMany)?;
        let Err(position) = sorted.binary_search_by_key(&code_point, |entry| entry.code_point) else {
            return Err(ConvertError::Repeated { encoding });
        };
        let window = out.get_mut(position..=count).ok_or(ConvertError::TooMany)?;
        window.rotate_right(1);
        if let Some(slot) = window.first_mut() {
            *slot = FontGlyph { code_point, rows };
        }
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// The 12 by 10 lines of one glyph.
fn cell_rows(width: usize, height: usize, glyph: &SourceGlyph) -> Result<[u16; CELL_FIELD_LINES], ConvertError> {
    let mut rows = [0u16; CELL_FIELD_LINES];
    match (width, height) {
        (CELL_WIDTH, CELL_HEIGHT) => {
            for (index, (row, &[first, second])) in rows.iter_mut().zip(glyph.rows.as_chunks::<2>().0).enumerate() {
                if first != second {
                    let line = index.saturating_mul(2).saturating_add(1);
                    return Err(ConvertError::LinePairs { encoding: glyph.encoding, line });
                }
                *row = first;
            }
        }
        (_, GLYPH_LINES | CELL_FIELD_LINES) if width <= CELL_DOTS => {
            // A 9-line cell sits below the blank line of the SAA5050 cell.
            let first = CELL_FIELD_LINES.saturating_sub(height);
            for (row, source) in rows.iter_mut().skip(first).zip(&glyph.rows) {
                *row = doubled(*source, width);
            }
        }
        _ => return Err(ConvertError::Cell),
    }
    Ok(rows)
}

/// A line of `width` pixels with every pixel made two wide, starting at the
/// left of the 12-pixel line.
fn doubled(line: u16, width: usize) -> u16 {
    let mut out = 0u16;
    for pixel in 0..width {
        let source = width.saturating_sub(1).saturating_sub(pixel);
        let target = CELL_WIDTH.saturating_sub(2).saturating_sub(pixel.saturating_mul(2));
        if line.checked_shr(u32::try_from(source).unwrap_or(u32::MAX)).unwrap_or(0) & 1 == 1 {
            out |= 0b11u16.checked_shl(u32::try_from(target).unwrap_or(u32::MAX)).unwrap_or(0);
        }
    }
    out
}

/// The most digits of a decimal code: `u32::MAX` has ten.
const MAX_DECIMAL_DIGITS: usize = 10;

fn decimal(field: &[u8]) -> Option<u32> {
    if field.is_empty() || field.len() > MAX_DECIMAL_DIGITS {
        return None;
    }
    field.iter().try_fold(0u32, |value, &digit| {
        let digit = (digit as char).to_digit(10)?;
        value.checked_mul(10)?.checked_add(digit)
    })
}

fn hexadecimal(field: &[u8]) -> Option<u32> {
    field.iter().try_fold(0u32, |value, &digit| {
        let digit = (digit as char).to_digit(16)?;
        value.checked_mul(16)?.checked_add(digit)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
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
    /// pixels at the left of the six; the reversal gives the source back.
    #[test]
    fn ten_line_cells_double_the_width() {
        let six = converted(&source((6, 10), "ISO10646", &[(65, [6, 10, 0, 0], &SIX)]), None).unwrap();
        assert_eq!(six.len(), 1);
        assert_eq!(six[0].code_point, 65);
        let back: Vec<u16> = six[0].rows.iter().map(|&row| halved(row, 6)).collect();
        assert_eq!(back, bits(&SIX), "reversal");
        let five_rows: Vec<&str> = SIX.iter().map(|row| &row[..5]).collect();
        let five = converted(&source((5, 10), "ISO10646", &[(65, [5, 10, 0, 0], &five_rows)]), None).unwrap();
        assert!(five[0].rows.iter().all(|row| row.trailing_zeros() >= 2), "the 5 pixels sit at the left of the 6");
        let back: Vec<u16> = five[0].rows.iter().map(|&row| halved(row, 5)).collect();
        assert_eq!(back, bits(&five_rows));
    }

    /// REQ-FNT-010: a 9-line cell gains a blank top line.
    #[test]
    fn nine_line_cells_gain_the_blank_top_line() {
        let five = converted(&source((5, 9), "ISO10646", &[(65, [5, 9, 0, 0], &FIVE_NINE)]), None).unwrap();
        assert_eq!(five[0].rows[0], 0, "the SAA5050's blank line");
        let back: Vec<u16> = five[0].rows[1..].iter().map(|&row| halved(row, 5)).collect();
        assert_eq!(back, bits(&FIVE_NINE));
        let six_rows: Vec<String> = FIVE_NINE.iter().map(|row| format!("{row}.")).collect();
        let six_rows: Vec<&str> = six_rows.iter().map(String::as_str).collect();
        let six = converted(&source((6, 9), "ISO10646", &[(65, [6, 9, 0, 0], &six_rows)]), None).unwrap();
        assert_eq!(six[0].rows, five[0].rows, "6 by 9 with a blank right column is the same glyph");
    }

    /// REQ-FNT-010: a 12 by 20 glyph whose line pairs match keeps every
    /// second line; one whose pair differs is refused, naming it.
    #[test]
    fn twelve_by_twenty() {
        let wide: Vec<String> = SIX.iter().map(|row| row.chars().flat_map(|c| [c, c]).collect()).collect();
        let doubled: Vec<&str> = wide.iter().flat_map(|row| [row.as_str(), row.as_str()]).collect();
        let glyphs = converted(&source((12, 20), "ISO10646", &[(66, [12, 20, 0, 0], &doubled)]), None).unwrap();
        let back: Vec<u16> = glyphs[0].rows.iter().flat_map(|&row| [row, row]).collect();
        assert_eq!(back, bits(&doubled), "reversal: each line twice");
        let mut rounded = doubled.clone();
        rounded[7] = "############";
        let error = converted(&source((12, 20), "ISO10646", &[(67, [12, 20, 0, 0], &rounded)]), None).unwrap_err();
        assert_eq!(error, ConvertError::LinePairs { encoding: 67, line: 7 });
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
        assert_eq!(small[0].rows, expected);
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
            comment: "Converted by ttx font convert from a 6 x 10 cell (REQ-FNT-010)",
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
}
