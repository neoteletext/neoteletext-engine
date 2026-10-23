//! The font converter (REQ-FNT-010, decision E.72 item 7 as amended by
//! decision E.77): a source font of another cell size to a form of
//! REQ-FNT-002, only where the conversion is exact and reversible.
//!
//! - 5 by 10 and 6 by 10: every pixel made two pixels wide, a 5-pixel cell
//!   in the right of the 6, leaving the blank column at the left where the
//!   renderer leaves Bedstead's;
//! - 5 by 9 and 6 by 9: the same, below one blank line, where the SAA5050
//!   leaves it;
//! - 12 by 20: every second line kept, to 12 by 10, when each pair of
//!   lines of every glyph, 1 and 2, 3 and 4 and so on, is identical;
//!   otherwise kept whole as a 12 by 20 font.
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
    // A 12 by 20 font halves only when no glyph would lose a pixel.
    let keep_tall = (font.width, font.height) == (CELL_WIDTH, CELL_HEIGHT)
        && !glyphs.iter().all(|glyph| glyph.rows.as_chunks::<2>().0.iter().all(|&[first, second]| first == second));
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
        let converted = if keep_tall {
            FontGlyph::tall(code_point, tall_rows(glyph))
        } else {
            FontGlyph::small(code_point, cell_rows(font.width, font.height, glyph)?)
        };
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
            *slot = converted;
        }
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// The twenty lines of a 12 by 20 glyph, kept whole.
fn tall_rows(glyph: &SourceGlyph) -> [u16; CELL_HEIGHT] {
    let mut rows = [0u16; CELL_HEIGHT];
    for (row, &source) in rows.iter_mut().zip(&glyph.rows) {
        *row = source;
    }
    rows
}

/// The 12 by 10 lines of one glyph. A 12 by 20 glyph reaches here only
/// when every pair of its lines is identical.
fn cell_rows(width: usize, height: usize, glyph: &SourceGlyph) -> Result<[u16; CELL_FIELD_LINES], ConvertError> {
    let mut rows = [0u16; CELL_FIELD_LINES];
    match (width, height) {
        (CELL_WIDTH, CELL_HEIGHT) => {
            for (row, &[first, _]) in rows.iter_mut().zip(glyph.rows.as_chunks::<2>().0) {
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

/// A line of `width` pixels with every pixel made two wide, ending at the
/// right of the 12-pixel line: a 5-pixel line leaves the cell's leftmost
/// dot blank, as the renderer's `widen` does for Bedstead (issue #206).
fn doubled(line: u16, width: usize) -> u16 {
    let mut out = 0u16;
    for source in 0..width {
        let target = source.saturating_mul(2);
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
mod tests;
