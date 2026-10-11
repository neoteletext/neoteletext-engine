//! Dynamically redefinable characters (area DRC, layer L3; clause 14,
//! decision E.54).
//!
//! This module decodes the stored bytes of DRCS pages, read through the
//! read-only [`crate::acq::EnhancementLookup`], into the patterns and colour
//! tables that a grid carries for its DRCS cells (REQ-DRC-001 to
//! REQ-DRC-010). The grid, the renderer and the decoder share its types:
//!
//! - [`DrcsPattern`]: one decoded character, as up to four bit planes of 10
//!   rows of 12 dots (REQ-DRC-004), made by [`decode_character`];
//! - [`DrcsPatterns`]: the patterns of the characters a page's cells use,
//!   with a capacity the caller sets (REQ-DRC-007, REQ-GEN-003);
//! - [`DrcsColourTables`]: the four DRCS colour lookup tables, resolved
//!   from X/28/1, M/29/1 and the defaults of Annex D.2.2.

use crate::acq::EnhancementPage;
use crate::colour::ColourIndex;
use crate::config::PresentationLevel;
use crate::consts::{
    CODE_MASK_7BIT, DRCS_CHARACTERS, DRCS_CLUT_4, DRCS_CLUT_16, DRCS_D_BYTE_DOTS, DRCS_D_BYTE_FIRST, DRCS_D_BYTE_MASK,
    DRCS_MODE_3_ROWS, DRCS_MODE_3_SCALE, DRCS_MODE_6X5X4, DRCS_MODE_12X10X1, DRCS_MODE_12X10X2, DRCS_MODE_12X10X4,
    DRCS_MODE_SUBSEQUENT, DRCS_PATTERN_DOTS, DRCS_PATTERN_ROWS, DRCS_PLANES, DRCS_PTU_BYTES, DRCS_PTUS,
    DRCS_PTUS_MODE_1, DRCS_PTUS_MODE_2, DRCS_PTUS_PER_PACKET,
};
use crate::ecc::has_odd_parity;
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

/// Why a DRCS character cannot be drawn (REQ-DRC-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum DrcsFault {
    /// The DRCS page, a packet holding one of its PTUs, or the mode of a
    /// PTU has not been received, or the character is not usable at the
    /// presentation level: the cell shows a space in its background colour
    /// until the data arrives.
    Missing,
    /// The data is invalid: a character number from 48 to 63, a first PTU
    /// marked 1110, 1111 or reserved, a further PTU not marked 1110, a
    /// character that runs past PTU 47, or a D-byte outside 4/0 to 7/F or
    /// failing its parity check (decision E.54). The cell shows a space and
    /// a diagnostic is traced.
    Invalid,
}

/// The mode, and the PTUs it takes, of a first-PTU mode code (Table 31).
const fn mode_of(code: u8) -> Option<(DrcsMode, usize)> {
    match code {
        DRCS_MODE_12X10X1 => Some((DrcsMode::Mode0, 1)),
        DRCS_MODE_12X10X2 => Some((DrcsMode::Mode1, DRCS_PTUS_MODE_1)),
        DRCS_MODE_12X10X4 => Some((DrcsMode::Mode2, DRCS_PTUS_MODE_2)),
        DRCS_MODE_6X5X4 => Some((DrcsMode::Mode3, 1)),
        _ => None,
    }
}

/// The twenty D-bytes of PTU `ptu`: PTU 2k is bytes 6 to 25 of packet
/// X/(k + 1), and PTU 2k + 1 bytes 26 to 45 (Figure 27, REQ-DRC-001).
fn ptu<'s>(page: &EnhancementPage<'s>, ptu: usize) -> Result<&'s [u8], DrcsFault> {
    let packet = ptu.checked_div(DRCS_PTUS_PER_PACKET).unwrap_or(0).saturating_add(1);
    let start = ptu.checked_rem(DRCS_PTUS_PER_PACKET).unwrap_or(0).saturating_mul(DRCS_PTU_BYTES);
    let row = page.row(packet as u8).ok_or(DrcsFault::Missing)?;
    row.get(start..start.saturating_add(DRCS_PTU_BYTES)).ok_or(DrcsFault::Missing)
}

/// The six dots of a D-byte, the most significant the leftmost (clause
/// 14.2.1); invalid outside 4/0 to 7/F or with a parity error (REQ-DRC-002).
fn dots(byte: u8) -> Result<u16, DrcsFault> {
    if !has_odd_parity(byte) || byte & CODE_MASK_7BIT < DRCS_D_BYTE_FIRST {
        return Err(DrcsFault::Invalid);
    }
    Ok(u16::from(byte & DRCS_D_BYTE_MASK))
}

/// One 12 by 10 bit plane from a PTU: D-byte i covers row i / 2, dots 0 to
/// 5 when i is even and 6 to 11 when it is odd (clause 14.2.1).
fn plane(bytes: &[u8]) -> Result<[u16; DRCS_PATTERN_ROWS], DrcsFault> {
    let mut rows = [0u16; DRCS_PATTERN_ROWS];
    for (index, &byte) in bytes.iter().enumerate() {
        let left_half = index.is_multiple_of(2);
        let shift = if left_half { DRCS_D_BYTE_DOTS as u32 } else { 0 };
        if let Some(row) = rows.get_mut(index / 2) {
            *row |= dots(byte)?.wrapping_shl(shift);
        }
    }
    Ok(rows)
}

/// Six dots doubled to twelve, each pixel two dots wide (decision E.54).
fn doubled(six: u16) -> u16 {
    (0..DRCS_D_BYTE_DOTS).fold(0u16, |line, bit| {
        let on = six.wrapping_shr(bit as u32) & 1;
        line | on.wrapping_mul(0b11).wrapping_shl(bit.wrapping_mul(DRCS_MODE_3_SCALE) as u32)
    })
}

/// The four planes of a 6 by 5 by 4 character: D-byte 4r + k is plane k of
/// pixel row r, six pixels with the most significant bit leftmost (clause
/// 14.2.4); each pixel covers 2 by 2 dots (decision E.54).
fn mode_3_planes(bytes: &[u8]) -> Result<[[u16; DRCS_PATTERN_ROWS]; DRCS_PLANES], DrcsFault> {
    let mut planes = [[0u16; DRCS_PATTERN_ROWS]; DRCS_PLANES];
    for (index, &byte) in bytes.iter().enumerate().take(DRCS_MODE_3_ROWS * DRCS_PLANES) {
        let (row, plane) = (index / DRCS_PLANES, index % DRCS_PLANES);
        let line = doubled(dots(byte)?);
        for repeat in 0..DRCS_MODE_3_SCALE {
            if let Some(slot) = planes
                .get_mut(plane)
                .and_then(|rows| rows.get_mut(row.wrapping_mul(DRCS_MODE_3_SCALE).wrapping_add(repeat)))
            {
                *slot = line;
            }
        }
    }
    Ok(planes)
}

/// Decodes one DRCS character from its DRCS sub-page (REQ-DRC-001 to
/// REQ-DRC-004, REQ-DRC-009; decision E.54). `page` is the sub-page whose
/// S1 is the character's sub-table, or `None` when it is not stored.
///
/// The mode of each PTU comes from the sub-page's X/28/3; without one every
/// PTU is mode 0 (Annex D.1.7). At Level 2.5 a character of modes 1 to 3 is
/// unusable and reported as missing (decision E.54). A character is usable
/// once every PTU it needs has been received.
pub fn decode_character(
    page: Option<EnhancementPage<'_>>,
    character: DrcsCharacter,
    level: PresentationLevel,
) -> Result<DrcsPattern, DrcsFault> {
    if character.character >= DRCS_CHARACTERS {
        return Err(DrcsFault::Invalid);
    }
    let page = page.ok_or(DrcsFault::Missing)?;
    let first = usize::from(character.character);
    let modes = page.drcs_modes();
    let code = |ptu: usize| -> Result<u8, DrcsFault> {
        match modes {
            None => Ok(DRCS_MODE_12X10X1),
            // An uncorrectable mode code is not yet known: missing.
            Some(modes) => modes.mode(ptu).ok_or(DrcsFault::Missing),
        }
    };
    let (mode, ptus) = mode_of(code(first)?).ok_or(DrcsFault::Invalid)?;
    // Multicolour modes are Level 3.5's, and need their capability
    // compiled in (decision E.71).
    if mode != DrcsMode::Mode0 && (!level.is_level35() || !cfg!(feature = "drcs-multicolour")) {
        return Err(DrcsFault::Missing);
    }
    if first.saturating_add(ptus) > DRCS_PTUS {
        return Err(DrcsFault::Invalid);
    }
    for further in first.saturating_add(1)..first.saturating_add(ptus) {
        if code(further)? != DRCS_MODE_SUBSEQUENT {
            return Err(DrcsFault::Invalid);
        }
    }
    let mut planes = [[0u16; DRCS_PATTERN_ROWS]; DRCS_PLANES];
    if mode == DrcsMode::Mode3 {
        planes = mode_3_planes(ptu(&page, first)?)?;
    } else {
        // Clauses 14.2.2 and 14.2.3: the first PTU is the least significant
        // plane.
        for (index, slot) in planes.iter_mut().enumerate().take(ptus) {
            *slot = plane(ptu(&page, first.saturating_add(index))?)?;
        }
    }
    Ok(DrcsPattern { character, mode, planes })
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

    fn d(value: u8) -> u8 {
        crate::ecc::with_odd_parity(DRCS_D_BYTE_FIRST | value)
    }

    /// REQ-DRC-002, REQ-TST-018: the worked PTU of
    /// `research/notes-level25-drcs.md` section 3.5 (row 0 all on, row 1
    /// the leftmost dot), and the D-bytes that make data invalid.
    #[test]
    fn d_bytes_load_left_to_right_and_top_to_bottom() {
        let mut bytes = [d(0); DRCS_PTU_BYTES];
        bytes[..4].copy_from_slice(&[d(0x3F), d(0x3F), d(0x20), d(0)]);
        bytes[5] = d(0x01);
        let rows = plane(&bytes).unwrap();
        assert_eq!(rows[0], 0xFFF);
        assert_eq!(rows[1], 0x800, "0x60 lights dot 0");
        assert_eq!(rows[2], 0x001, "the second D-byte's last bit is dot 11");
        assert!(rows[3..].iter().all(|&row| row == 0));
        // A byte below 4/0 (2/0 padding) or with even parity is invalid.
        bytes[7] = crate::ecc::with_odd_parity(0x20);
        assert_eq!(plane(&bytes), Err(DrcsFault::Invalid));
        bytes[7] = DRCS_D_BYTE_FIRST | crate::consts::PARITY_BIT;
        assert_eq!(plane(&bytes), Err(DrcsFault::Invalid));
    }

    /// REQ-DRC-004, decision E.54: a 6 by 5 by 4 character, D-byte 4r + k
    /// plane k of row r, each pixel 2 by 2 dots.
    #[test]
    fn mode_3_pixels_are_doubled() {
        let mut bytes = [d(0); DRCS_PTU_BYTES];
        // Row 0: plane 0 the leftmost pixel; row 4: plane 3 the rightmost.
        bytes[0] = d(0b10_0000);
        bytes[4 * 4 + 3] = d(0b00_0001);
        let planes = mode_3_planes(&bytes).unwrap();
        assert_eq!((planes[0][0], planes[0][1], planes[0][2]), (0xC00, 0xC00, 0));
        assert_eq!((planes[3][8], planes[3][9]), (0x003, 0x003));
        let mut pattern = DrcsPattern { mode: DrcsMode::Mode3, planes, ..DrcsPattern::EMPTY };
        assert_eq!((pattern.pixel(0, 0), pattern.pixel(1, 1), pattern.pixel(0, 2)), (1, 1, 0));
        assert_eq!(pattern.pixel(9, 11), 8);
        pattern.planes[1][9] = 0x001;
        assert_eq!(pattern.pixel(9, 11), 10);
        assert_eq!(doubled(0b10_1011), 0b1100_1100_1111);
    }

    /// REQ-DRC-003, Table 31: the mode codes of a first PTU.
    #[test]
    fn mode_codes() {
        assert_eq!(mode_of(DRCS_MODE_12X10X1), Some((DrcsMode::Mode0, 1)));
        assert_eq!(mode_of(DRCS_MODE_12X10X2), Some((DrcsMode::Mode1, 2)));
        assert_eq!(mode_of(DRCS_MODE_12X10X4), Some((DrcsMode::Mode2, 4)));
        assert_eq!(mode_of(DRCS_MODE_6X5X4), Some((DrcsMode::Mode3, 1)));
        for code in 4..16 {
            assert_eq!(mode_of(code), None, "{code:04b}");
        }
    }
}
