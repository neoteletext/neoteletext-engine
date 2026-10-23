//! Tests that the constants agree with one another where a value exists in
//! two forms (REQ-GEN-014).

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

#[test]
fn framing_code_forms_agree() {
    assert_eq!(FRAMING_CODE.reverse_bits(), FRAMING_CODE_DVB);
}

#[test]
fn page_geometry_is_consistent() {
    assert_eq!(PAGE_WIDTH, 480);
    assert_eq!(PAGE_HEIGHT, 500);
    assert_eq!(CELL_WIDTH, 2 * CELL_DOTS);
    assert_eq!(CELL_HEIGHT, 2 * CELL_FIELD_LINES);
    assert_eq!(MOSAIC_ROW_TENTHS.iter().sum::<usize>(), 10);
    assert_eq!(PACKET_BITS, PACKET_BYTES * 8);
}

/// Decision E.48: the fast phases fill a half-second exactly, and slow
/// flash keeps the 3 to 1 ratio that decision E.12 measured.
#[test]
fn flash_units_agree_with_decisions_e12_and_e48() {
    assert_eq!(FLASH_FAST_OFF_UNITS * FLASH_PHASES, FLASH_FAST_HALF_UNITS);
    let slow_off = u64::from(FLASH_UNITS - FLASH_SLOW_ON_UNITS);
    assert_eq!(u64::from(FLASH_SLOW_ON_UNITS) * FLASH_FIELDS_HIDDEN, slow_off * FLASH_FIELDS_SHOWN);
}

/// Decision E.43: the Colour Map's transparent entry is the renderer's
/// transparent palette index.
#[test]
fn transparent_indices_agree() {
    assert_eq!(COLOUR_TRANSPARENT, PALETTE_TRANSPARENT);
    assert_eq!(usize::from(COLOUR_TRANSPARENT), CLUT_ENTRIES);
    assert_eq!(COLOUR_MAP_ENTRIES, 1 << (CLUT_SHIFT + 2));
    assert_eq!(usize::from(SIDE_PANEL_MAX_COLUMNS) * 2 + COLUMNS, 72);
}

#[test]
fn palette_ranges_do_not_overlap() {
    let flash_end = usize::from(PALETTE_FLASH_BASE) + COLOURS * COLOURS;
    assert!(flash_end <= usize::from(PALETTE_CONCEAL_BASE));
    assert_eq!(usize::from(PALETTE_CONCEAL_BASE) + COLOURS * COLOURS, PALETTE_SIZE);
}
