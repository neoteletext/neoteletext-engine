//! Block mosaics (Table 47, REQ-CHR-005).

use crate::consts::MOSAIC_FULL;

/// BLOCK SEXTANT-1 (Symbols for Legacy Computing): the first contiguous
/// sextant, for pattern 1.
const SEXTANT_FIRST: u32 = 0x1_FB00;
/// SEPARATED BLOCK SEXTANT-1 is one above this (Symbols for Legacy
/// Computing Supplement, Unicode 16), so pattern n maps to this + n.
const SEPARATED_SEXTANT_BASE: u32 = 0x1_CE50;
/// The three left-hand blocks: bits 0, 2 and 4 of the pattern.
pub(super) const MOSAIC_LEFT_COLUMN: u8 = 0b01_0101;
/// The three right-hand blocks: bits 1, 3 and 5 of the pattern.
pub(super) const MOSAIC_RIGHT_COLUMN: u8 = 0b10_1010;
/// LEFT HALF BLOCK, the contiguous form of [`MOSAIC_LEFT_COLUMN`].
const LEFT_HALF_BLOCK: char = '\u{258C}';
/// RIGHT HALF BLOCK, the contiguous form of [`MOSAIC_RIGHT_COLUMN`].
const RIGHT_HALF_BLOCK: char = '\u{2590}';
/// FULL BLOCK, the contiguous form of [`MOSAIC_FULL`].
pub(super) const FULL_BLOCK: char = '\u{2588}';

pub(super) fn mosaic_unicode(pattern: u8, separated: bool) -> char {
    let pattern = pattern & MOSAIC_FULL;
    if pattern == 0 {
        return ' ';
    }
    if separated {
        let scalar = SEPARATED_SEXTANT_BASE.wrapping_add(u32::from(pattern));
        return char::from_u32(scalar).unwrap_or(char::REPLACEMENT_CHARACTER);
    }
    match pattern {
        MOSAIC_LEFT_COLUMN => LEFT_HALF_BLOCK,
        MOSAIC_RIGHT_COLUMN => RIGHT_HALF_BLOCK,
        MOSAIC_FULL => FULL_BLOCK,
        _ => {
            // The sextant block skips the patterns that already exist as
            // half and full blocks; pattern 0 is the space.
            let skipped =
                1_u8.wrapping_add(u8::from(pattern > MOSAIC_LEFT_COLUMN))
                    .wrapping_add(u8::from(pattern > MOSAIC_RIGHT_COLUMN));
            let offset = u32::from(pattern.wrapping_sub(skipped));
            char::from_u32(SEXTANT_FIRST.wrapping_add(offset))
                .unwrap_or(char::REPLACEMENT_CHARACTER)
        }
    }
}
