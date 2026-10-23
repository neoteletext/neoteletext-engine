//! One character of a loaded font (REQ-FNT-002).

use crate::consts::{CELL_FIELD_LINES, CELL_HEIGHT};

/// One character of a loaded font (REQ-FNT-002, decisions E.72 and E.77):
/// its Unicode code point and the lines of its cell, 12 by 10 or 12 by 20,
/// top line first, bit 11 the leftmost dot.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct FontGlyph {
    /// The Unicode code point.
    pub code_point: u32,
    /// The lines of the cell; bits 12 to 15 are zero. A 12 by 10 glyph uses
    /// the first ten and leaves the rest zero.
    pub rows: [u16; CELL_HEIGHT],
    /// The glyph is 12 by 20 (decision E.77); otherwise it is 12 by 10.
    pub tall: bool,
}

impl FontGlyph {
    /// An empty slot of a glyph table.
    pub const EMPTY: Self = Self { code_point: 0, rows: [0; CELL_HEIGHT], tall: false };

    /// A 12 by 10 glyph.
    #[must_use]
    pub const fn small(code_point: u32, small: [u16; CELL_FIELD_LINES]) -> Self {
        let [l0, l1, l2, l3, l4, l5, l6, l7, l8, l9] = small;
        let rows = [l0, l1, l2, l3, l4, l5, l6, l7, l8, l9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        Self { code_point, rows, tall: false }
    }

    /// A 12 by 20 glyph (decision E.77).
    #[must_use]
    pub const fn tall(code_point: u32, rows: [u16; CELL_HEIGHT]) -> Self {
        Self { code_point, rows, tall: true }
    }

    /// The glyph's lines: ten for a 12 by 10 glyph, twenty for a 12 by 20.
    #[must_use]
    pub fn lines(&self) -> &[u16] {
        let count = if self.tall { CELL_HEIGHT } else { CELL_FIELD_LINES };
        self.rows.get(..count).unwrap_or(&[])
    }
}
