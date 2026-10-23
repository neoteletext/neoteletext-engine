//! Character sets (clause 15): the national option positions of Table 36,
//! the right guillemet of Table 37, the G3 set of Table 48 and the undefined
//! positions of the G3 and Latin G2 sets.

use super::spacing_attributes::SPACE;

/// The 13 national option positions of the Latin G0 set (Table 36).
pub const NATIONAL_POSITIONS: [u8; 13] = [0x23, 0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x7B, 0x7C, 0x7D, 0x7E];
/// The right guillemet "»" in the Latin G2 set (Table 37, position 3/B),
/// the receiver's mark for a link leaving the service (REQ-NAV-011).
pub const G2_RIGHT_GUILLEMET: u8 = 0x3B;

/// The G3 set occupies codes 2/0 to 7/F (Table 48).
pub const G3_FIRST_CODE: u8 = SPACE;
/// Positions 2/0 to 7/F: six columns of sixteen (Table 48).
pub const G3_CODES: usize = 96;

/// The undefined positions of the G3 set: 6/E, 6/F, 7/E and 7/F (Table 25,
/// its "7/E and 7/E" read as 7/E and 7/F, decision E.54).
pub const G3_UNDEFINED: [u8; 4] = [0x6E, 0x6F, 0x7E, 0x7F];
/// The Latin G2 positions that should not be transmitted: 5/9, 5/A, 5/B and
/// 6/5 (Table 37 note 5, decision E.50).
pub const LATIN_G2_UNDEFINED: [u8; 4] = [0x59, 0x5A, 0x5B, 0x65];
