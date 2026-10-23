//! A font loaded at run time (REQ-FNT-002, REQ-FNT-003, REQ-FNT-008): a
//! table of 12 by 10 or 12 by 20 glyphs the program supplies, checked for
//! its shape.

use crate::consts::{CELL_HEIGHT, CELL_WIDTH, FONT_MAX_CHARACTERS};
use crate::types::FontGlyph;

/// Why a glyph table cannot be a loaded font (REQ-FNT-003, REQ-FNT-008).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontTableError {
    /// More than 1,024 glyphs.
    TooMany,
    /// The code points are not in strictly increasing order: unsorted or
    /// repeated.
    Order,
    /// A code point is not a Unicode scalar value.
    Code,
    /// A line has a dot beyond the 12 of the cell.
    Width,
    /// The glyphs are not all of one height, or a 12 by 10 glyph has lines
    /// beyond its tenth (decision E.77).
    Height,
}

/// A font loaded at run time (REQ-FNT-008): a table of 12 by 10 or 12 by 20
/// glyphs, all of one height (decision E.77), sorted by code point. The table comes from outside the library, so it is
/// checked for shape when the font is made
/// (`docs/spec/design/font-security.md` section 3.1).
#[derive(Clone, Copy, Debug)]
pub struct LoadedFont<'a> {
    glyphs: &'a [FontGlyph],
    dot_matrix: bool,
    tall: bool,
}

/// The right-hand pixel of each of a 12-pixel row's six pairs.
const PAIR_RIGHT_PIXELS: u16 = 0b0101_0101_0101;

/// A glyph of a loaded font, and whether the font is a doubled dot matrix,
/// which is rounded as Bedstead is (REQ-FNT-008, issue #209).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LoadedGlyph {
    /// The lines of 12 pixels, bit 11 the leftmost: the first ten of a 12
    /// by 10 glyph, or all twenty of a 12 by 20 glyph.
    pub rows: [u16; CELL_HEIGHT],
    /// The glyph is 12 by 20 and is drawn line for line (decision E.77).
    pub tall: bool,
    /// Every row of every glyph of its font is made of identical pixel
    /// pairs. Never true of a 12 by 20 font.
    pub dot_matrix: bool,
}

impl<'a> LoadedFont<'a> {
    /// The font of a table, after checking its shape.
    pub fn new(glyphs: &'a [FontGlyph]) -> Result<Self, FontTableError> {
        if glyphs.len() > FONT_MAX_CHARACTERS {
            return Err(FontTableError::TooMany);
        }
        if !glyphs.windows(2).all(|pair| matches!(pair, [a, b] if a.code_point < b.code_point)) {
            return Err(FontTableError::Order);
        }
        let tall = glyphs.first().is_some_and(|glyph| glyph.tall);
        for glyph in glyphs {
            if char::from_u32(glyph.code_point).is_none() {
                return Err(FontTableError::Code);
            }
            if glyph.rows.iter().any(|row| row >> CELL_WIDTH != 0) {
                return Err(FontTableError::Width);
            }
            let beyond = glyph.rows.get(glyph.lines().len()..).unwrap_or(&[]);
            if glyph.tall != tall || beyond.iter().any(|&row| row != 0) {
                return Err(FontTableError::Height);
            }
        }
        // A font converted from a 5- or 6-dot matrix (REQ-FNT-010) has
        // every pixel doubled; one drawn at half-dot resolution does not. A
        // 12 by 20 font is never rounded (decision E.77).
        let dot_matrix = !tall
            && !glyphs.is_empty()
            && glyphs.iter().all(|glyph| glyph.rows.iter().all(|row| (row ^ (row >> 1)) & PAIR_RIGHT_PIXELS == 0));
        Ok(Self { glyphs, dot_matrix, tall })
    }

    /// True when every row of every glyph is made of identical pixel
    /// pairs: a 5- or 6-dot matrix with each dot drawn two pixels wide,
    /// which the renderer rounds as the SAA5050 rounded its matrix
    /// (REQ-FNT-008, decision E.13).
    #[must_use]
    pub const fn is_dot_matrix(&self) -> bool {
        self.dot_matrix
    }

    /// Number of glyphs.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// True when the font has no glyphs.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// True when the font's glyphs are 12 by 20 (decision E.77).
    #[must_use]
    pub const fn is_tall(&self) -> bool {
        self.tall
    }

    /// The glyph of a code point.
    #[must_use]
    pub fn lookup(&self, character: char) -> Option<FontGlyph> {
        let code_point = u32::from(character);
        self.glyphs
            .binary_search_by_key(&code_point, |glyph| glyph.code_point)
            .ok()
            .and_then(|index| self.glyphs.get(index))
            .copied()
    }
}
