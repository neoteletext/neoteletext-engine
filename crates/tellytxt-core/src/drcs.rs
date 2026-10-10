//! Dynamically redefinable characters (area DRC, layer L3; clause 14,
//! decision E.54).
//!
//! This module will decode the stored bytes of DRCS pages, read through
//! the read-only [`crate::acq::EnhancementLookup`], into the patterns and
//! colour tables that a grid carries for its DRCS cells (REQ-DRC-001 to
//! REQ-DRC-010). Until that decoding exists it holds the types the grid
//! carries, so that the grid, the renderer and the decoder agree on them:
//!
//! - [`DrcsPattern`]: one decoded character, as up to four bit planes of 10
//!   rows of 12 dots (REQ-DRC-004);
//! - [`DrcsPatterns`]: the patterns of the characters a page's cells use,
//!   with a capacity the caller sets (REQ-DRC-007, REQ-GEN-003);
//! - [`DrcsColourTables`]: the four DRCS colour lookup tables, resolved
//!   from X/28/1, M/29/1 and the defaults of Annex D.2.2.

use crate::colour::ColourIndex;
use crate::config::PresentationLevel;
use crate::consts::{DRCS_CLUT_4, DRCS_CLUT_16, DRCS_PATTERN_DOTS, DRCS_PATTERN_ROWS, DRCS_PLANES};
use crate::packet::DrcsColourData;
use crate::types::{DrcsCharacter, DrcsTable, FixedVec};

/// The number of DRCS patterns a grid holds unless the caller chooses
/// another capacity: one full sub-table of 48 characters,
/// twice the Level 2.5 budget of 24 characters for a page (clause 14.4).
/// A character beyond the capacity is treated as missing and counted
/// (REQ-DRC-007).
pub const DEFAULT_DRCS_PATTERNS: usize = 48;

/// The mode of a DRCS character (Table 31).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum DrcsMode {
    /// Mode 0: 12 by 10 by 1, one PTU; pixels take the cell's foreground
    /// and background colours.
    Mode0,
    /// Mode 1: 12 by 10 by 2, two PTUs; pixels index a 4-entry table.
    Mode1,
    /// Mode 2: 12 by 10 by 4, four PTUs; pixels index a 16-entry table.
    Mode2,
    /// Mode 3: 6 by 5 by 4, one PTU; pixels index a 16-entry table.
    Mode3,
}

/// One decoded DRCS character, ready to draw (REQ-DRC-004, REQ-RND-038).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DrcsPattern {
    /// The character it is.
    pub character: DrcsCharacter,
    /// Its mode.
    pub mode: DrcsMode,
    /// Bit planes 0 to 3, each 10 rows of 12 dots with bit 11 the leftmost,
    /// as a cell raster line's mask is. A pixel's value is the sum of 2^p
    /// over the planes p whose bit is set. Mode 0 uses plane 0 and mode 1
    /// planes 0 and 1. A mode 3 pixel is stored already doubled, as 2 by 1
    /// dots on each of 2 rows, so that every mode is drawn the same way.
    pub planes: [[u16; DRCS_PATTERN_ROWS]; DRCS_PLANES],
}

impl DrcsPattern {
    /// An empty pattern, for filling unused storage.
    pub const EMPTY: Self = Self {
        character: DrcsCharacter { table: DrcsTable::Normal, subtable: 0, character: 0 },
        mode: DrcsMode::Mode0,
        planes: [[0; DRCS_PATTERN_ROWS]; DRCS_PLANES],
    };

    /// The value of the pixel at `row` (0 to 9) and `dot` (0 to 11, 0 the
    /// leftmost): 0 or 1 in mode 0, 0 to 3 in mode 1, 0 to 15 otherwise.
    #[must_use]
    pub fn pixel(&self, row: usize, dot: usize) -> u8 {
        let shift = DRCS_PATTERN_DOTS.saturating_sub(1).saturating_sub(dot) as u32;
        self.planes.iter().enumerate().fold(0u8, |value, (plane, rows)| {
            let bit = rows.get(row).map_or(0, |line| line.wrapping_shr(shift) & 1) as u8;
            value | bit.wrapping_shl(plane as u32)
        })
    }
}

/// The DRCS patterns a grid carries, `N` at most (REQ-DRC-007).
pub type DrcsPatterns<const N: usize> = FixedVec<DrcsPattern, N>;

/// The four DRCS colour lookup tables as the grid carries them: Colour Map
/// indices (Figure 28; REQ-DRC-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DrcsColourTables {
    /// For global 12 by 10 by 2 characters.
    pub global_4: [ColourIndex; DRCS_CLUT_4],
    /// For normal 12 by 10 by 2 characters.
    pub normal_4: [ColourIndex; DRCS_CLUT_4],
    /// For global 12 by 10 by 4 and 6 by 5 by 4 characters.
    pub global_16: [ColourIndex; DRCS_CLUT_16],
    /// For normal 12 by 10 by 4 and 6 by 5 by 4 characters.
    pub normal_16: [ColourIndex; DRCS_CLUT_16],
}

/// Colour Map indices 0 to 15 in order: the defaults of Annex D.2.2.
const FIRST_INDICES: [ColourIndex; DRCS_CLUT_16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

impl DrcsColourTables {
    /// The defaults of Annex D.2.2: Colour Map entries 0 to 3 in both
    /// 4-entry tables, and entries 0 to 15, transparent included, in both
    /// 16-entry tables.
    pub const DEFAULT: Self =
        Self { global_4: [0, 1, 2, 3], normal_4: [0, 1, 2, 3], global_16: FIRST_INDICES, normal_16: FIRST_INDICES };

    /// The tables a page uses (decision E.54, REQ-DRC-007): each from the
    /// page's X/28/1 when that table is valid there, else from the
    /// magazine's M/29/1, else the default. The tables serve modes 1 to 3,
    /// which are for Level 3.5 only, so other levels get the defaults.
    #[must_use]
    pub fn resolve(page: Option<&DrcsColourData>, magazine: Option<&DrcsColourData>, level: PresentationLevel) -> Self {
        if !level.is_level35() {
            return Self::DEFAULT;
        }
        let sources = [page, magazine];
        let pick4 = |table: fn(&DrcsColourData) -> Option<[u8; DRCS_CLUT_4]>, default| {
            sources.into_iter().flatten().find_map(table).unwrap_or(default)
        };
        let pick16 = |table: fn(&DrcsColourData) -> Option<[u8; DRCS_CLUT_16]>, default| {
            sources.into_iter().flatten().find_map(table).unwrap_or(default)
        };
        Self {
            global_4: pick4(|data| data.global_4, Self::DEFAULT.global_4),
            normal_4: pick4(|data| data.normal_4, Self::DEFAULT.normal_4),
            global_16: pick16(|data| data.global_16, Self::DEFAULT.global_16),
            normal_16: pick16(|data| data.normal_16, Self::DEFAULT.normal_16),
        }
    }
}

impl Default for DrcsColourTables {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// REQ-DRC-007: each table from X/28/1, then M/29/1, then the
    /// defaults of Annex D.2.2, at Level 3.5 only.
    #[test]
    fn colour_tables_follow_decision_e54() {
        let page = DrcsColourData { global_4: Some([9, 9, 9, 9]), ..DrcsColourData::default() };
        let magazine = DrcsColourData {
            global_4: Some([1, 1, 1, 1]),
            normal_16: Some([31; DRCS_CLUT_16]),
            ..DrcsColourData::default()
        };
        let tables = DrcsColourTables::resolve(Some(&page), Some(&magazine), PresentationLevel::Level35);
        assert_eq!(tables.global_4, [9; 4], "the page's X/28/1 first");
        assert_eq!(tables.normal_16, [31; 16], "then the magazine's M/29/1");
        assert_eq!((tables.normal_4, tables.global_16), (DrcsColourTables::DEFAULT.normal_4, FIRST_INDICES));
        assert_eq!(
            DrcsColourTables::resolve(Some(&page), Some(&magazine), PresentationLevel::Level25),
            DrcsColourTables::DEFAULT
        );
        assert_eq!(DrcsColourTables::DEFAULT.global_16[8], crate::consts::COLOUR_TRANSPARENT);
    }

    /// REQ-DRC-004: a pixel's value is the sum of its plane bits, the
    /// leftmost dot at bit 11.
    #[test]
    fn pixels_combine_their_planes() {
        let mut pattern = DrcsPattern::EMPTY;
        pattern.planes[0][2] = 0b1000_0000_0001;
        pattern.planes[3][2] = 0b1000_0000_0000;
        assert_eq!((pattern.pixel(2, 0), pattern.pixel(2, 11), pattern.pixel(2, 5)), (9, 1, 0));
        assert_eq!(pattern.pixel(10, 0), 0, "outside the pattern");
    }
}
