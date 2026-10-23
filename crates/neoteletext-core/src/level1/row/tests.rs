//! Tests of the row attribute machine (REQ-FSM-006): the Table 26 timing
//! of every spacing attribute, and the Annex G examples.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see `row.rs`); the attribute also
// marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::charset;
use crate::consts::{ALPHA_RED, ALPHA_YELLOW, DOUBLE_SIZE, DOUBLE_WIDTH, MOSAIC_FULL, MOSAIC_RED};

fn context(config: &Config) -> RowContext<'_> {
    RowContext {
        config,
        default_g0: G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::English) },
        second_g0: None,
        double_allowed: true,
    }
}

fn row(bytes: &[u8]) -> [u8; COLUMNS] {
    let mut codes = [SPACE; COLUMNS];
    codes[..bytes.len()].copy_from_slice(bytes);
    codes
}

fn decode(bytes: &[u8]) -> [Cell; COLUMNS] {
    let config = Config::default();
    decode_row(&row(bytes), 0, &context(&config)).0
}

/// REQ-L1-003, REQ-TST-013.
#[test]
fn stored_spaces_are_blank_glyphs() {
    let cells = decode(&[b' ', b'A', MOSAIC_RED, 0x20, 0x21]);
    assert!(matches!(cells[0].glyph, Glyph::Char(_)) && cells[0].glyph.is_blank());
    assert!(!cells[1].glyph.is_blank());
    assert!(cells[2].glyph.is_blank() && cells[3].glyph.is_blank());
    assert!(!cells[4].glyph.is_blank());
}

#[test]
fn colour_attributes_are_set_after() {
    let cells = decode(&[ALPHA_RED, b'A']);
    assert_eq!(cells[0].foreground, COLOUR_WHITE, "attribute cell keeps the old colour");
    assert_eq!(cells[1].foreground, 1);
    assert_eq!(cells[0].glyph, Glyph::Space);
}

/// REQ-L1-012; REQ-TST-013's New Background after a colour change.
#[test]
fn new_background_is_set_at() {
    let cells = decode(&[ALPHA_RED, NEW_BACKGROUND, b'A', BLACK_BACKGROUND, b'B']);
    assert_eq!(cells[1].background, 1, "New Background acts in its own cell");
    assert_eq!(cells[2].background, 1);
    assert_eq!(cells[3].background, 0);
}

/// REQ-L1-007.
#[test]
fn hold_mosaics_annex_g33_style() {
    // Mosaic red, a mosaic, hold, a colour change: the attribute cell
    // after the hold shows the held mosaic.
    let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, MOSAIC_RED + 1, 0x21]);
    assert_eq!(cells[1].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false });
    assert!(cells[2].held && cells[3].held);
    assert_eq!(cells[3].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false });
}

/// REQ-L1-007; REQ-TST-013's hold with mode changes.
#[test]
fn held_mosaic_keeps_its_form_and_resets_on_mode_change() {
    let cells = decode(&[MOSAIC_RED, SEPARATED_MOSAICS, 0x7F, HOLD_MOSAICS, CONTIGUOUS_MOSAICS, ALPHA_RED, MOSAIC_RED]);
    // Contiguous (set-at) in column 4: the held mosaic keeps its stored
    // separated form (decision E.16).
    assert_eq!(cells[4].glyph, Glyph::Mosaic { pattern: 0x3F, separated: true });
    // Column 5 switches to alphanumerics (set-after); its own cell still
    // shows the held mosaic, and the reset follows.
    assert!(cells[5].held);
    assert_eq!(cells[6].glyph, Glyph::Space);
}

/// REQ-TST-013's blast-through with national options.
#[test]
fn blast_through_capitals_in_mosaic_mode() {
    let cells = decode(&[MOSAIC_RED, b'A', 0x23]);
    assert!(matches!(cells[1].glyph, Glyph::Char(c) if c.code == b'A'));
    assert!(matches!(cells[2].glyph, Glyph::Mosaic { .. }));
    // Table 47 note 2: 5/B blasts through as the German national
    // character, not as a mosaic.
    let config = Config::default();
    let german = RowContext {
        default_g0: G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::German) },
        ..context(&config)
    };
    let cells = decode_row(&row(&[MOSAIC_RED, 0x5B]), 0, &german).0;
    let Glyph::Char(character) = cells[1].glyph else { panic!("blast-through gave {:?}", cells[1].glyph) };
    assert_eq!(charset::to_unicode(character).base, '\u{C4}');
}

/// REQ-L1-010; REQ-TST-013's conceal ended by a colour attribute.
#[test]
fn conceal_ends_at_colour() {
    let cells = decode(&[CONCEAL, b'A', ALPHA_RED, b'B']);
    assert!(cells[0].conceal && cells[1].conceal && cells[2].conceal);
    assert!(!cells[3].conceal);
}

/// REQ-L1-011, REQ-TST-013: the Annex G.3.1 boxing example.
#[test]
fn start_box_needs_two_codes() {
    // Annex G.3.1: x 0B 0B 61 0A 0A — the second 0B, the "a" and the
    // first 0A are boxed.
    let cells = decode(&[b'x', START_BOX, START_BOX, b'a', END_BOX, END_BOX]);
    let boxed: [bool; 6] = core::array::from_fn(|i| cells[i].boxed);
    assert_eq!(boxed, [false, false, true, true, true, false]);
    let single = decode(&[b'x', START_BOX, b'a']);
    assert!(!single[2].boxed);
}

/// REQ-L1-009.
#[test]
fn flash_and_steady() {
    let cells = decode(&[FLASH, b'A', STEADY, b'B']);
    assert!(!cells[0].flash && cells[1].flash && !cells[2].flash);
}

/// REQ-L1-019, decision E.18.
#[test]
fn black_codes_follow_the_profile() {
    // 0/0: alpha black by default and under SAA5246A; no function in the
    // SAA5050 profile.
    let saa5050 = Config::saa5050();
    let cells = decode(&[ALPHA_RED, ALPHA_BLACK, b'A']);
    assert_eq!(cells[2].foreground, COLOUR_BLACK, "0/0 is alpha black by default");
    let cells = decode_row(&row(&[ALPHA_RED, ALPHA_BLACK, b'A']), 0, &context(&saa5050)).0;
    assert_eq!(cells[2].foreground, 1, "0/0 has no function on the SAA5050");
    // The RAI case: alpha black ends mosaic mode, so digits are text.
    let cells = decode(&[MOSAIC_RED, ALPHA_BLACK, b'2']);
    assert!(matches!(cells[2].glyph, Glyph::Char(c) if c.code == b'2'));
    // 1/0: mosaic black by default; no function on the SAA5050.
    let cells = decode(&[ALPHA_RED, MOSAIC_BLACK, 0x7F]);
    assert_eq!(cells[2].foreground, COLOUR_BLACK);
    assert_eq!(cells[2].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false });
    let cells = decode_row(&row(&[MOSAIC_RED, MOSAIC_BLACK, 0x7F]), 0, &context(&saa5050)).0;
    assert_eq!(cells[2].foreground, MOSAIC_RED & COLOUR_ATTRIBUTE_MASK);
    // 0/E and 0/F show a space and change no state in either profile:
    // the row matches one where a repeated colour code fills the cell.
    for config in [Config::default(), Config::saa5246a()] {
        let plain = decode_row(&row(&[ALPHA_RED, ALPHA_RED, b'A']), 0, &context(&config)).0;
        for code in [DOUBLE_WIDTH, DOUBLE_SIZE] {
            let cells = decode_row(&row(&[ALPHA_RED, code, b'A']), 0, &context(&config)).0;
            assert_eq!(cells, plain, "code {code:#04x}");
        }
    }
}

/// REQ-L1-003, REQ-TST-013: every Table 26 attribute with a Level 1
/// function, checked at its own cell and at the next one.
#[test]
fn every_spacing_attribute_has_its_table_26_timing() {
    let colour = |code: u8| code & COLOUR_ATTRIBUTE_MASK;
    // Set-after: 0/1 to 0/7 and 1/1 to 1/7.
    for code in ALPHA_RED..=ALPHA_WHITE {
        let before = if code == ALPHA_RED { ALPHA_YELLOW } else { ALPHA_RED };
        let cells = decode(&[before, code, b'A']);
        assert_eq!((cells[1].foreground, cells[2].foreground), (colour(before), colour(code)), "{code:#04x}");
    }
    for code in MOSAIC_RED..=MOSAIC_WHITE {
        let before = if code == MOSAIC_RED { MOSAIC_RED + 2 } else { MOSAIC_RED };
        let cells = decode(&[before, code, 0x7F]);
        assert_eq!((cells[1].foreground, cells[2].foreground), (colour(before), colour(code)), "{code:#04x}");
        assert_eq!(cells[2].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false });
    }
    // 0/8 set-after, 0/9 set-at.
    let cells = decode(&[FLASH, b'A', STEADY, b'B']);
    assert_eq!(cells.map(|cell| cell.flash)[..4], [false, true, false, false]);
    // 0/A set-after, 0/B set-after (a single Start Box under the option).
    let cells = decode(&[START_BOX, START_BOX, END_BOX, b'a']);
    assert_eq!(cells.map(|cell| cell.boxed)[..4], [false, true, true, false]);
    let single = Config { double_start_box: false, ..Config::default() };
    let cells = decode_row(&row(&[START_BOX, b'a']), 0, &context(&single)).0;
    assert_eq!(cells.map(|cell| cell.boxed)[..2], [false, true]);
    // 0/C set-at, 0/D set-after.
    let cells = decode(&[DOUBLE_HEIGHT, b'A', NORMAL_SIZE, b'B']);
    assert_eq!(cells.map(|cell| cell.size)[..4], [Size::Normal, Size::DoubleTop, Size::Normal, Size::Normal]);
    // 1/8 set-at.
    let cells = decode(&[b'A', CONCEAL, b'B']);
    assert_eq!(cells.map(|cell| cell.conceal)[..3], [false, true, true]);
    // 1/9 and 1/A set-at: the next mosaic takes the new form.
    let cells = decode(&[MOSAIC_RED, SEPARATED_MOSAICS, 0x7F, CONTIGUOUS_MOSAICS, 0x7F]);
    assert_eq!(cells[2].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: true });
    assert_eq!(cells[4].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false });
    // 1/B set-after: the cell after ESC uses the second G0 set.
    let config = Config::default();
    let second = G0 { set: charset::G0Set::Latin, national: Some(charset::NationalOption::German) };
    let with_second = RowContext { second_g0: Some(second), ..context(&config) };
    let cells = decode_row(&row(&[ESC, b'A']), 0, &with_second).0;
    assert_eq!(cells[0].glyph, Glyph::Space);
    assert_eq!(cells[1].glyph, Glyph::Char(CharRef::new(CharSet::G0(second), b'A')));
    // 1/C and 1/D set-at.
    let cells = decode(&[ALPHA_RED, NEW_BACKGROUND, BLACK_BACKGROUND, b'A']);
    assert_eq!(cells.map(|cell| cell.background)[..4], [COLOUR_BLACK, colour(ALPHA_RED), COLOUR_BLACK, COLOUR_BLACK]);
    // 1/E set-at, 1/F set-after: both attribute cells show the held
    // mosaic; the next attribute cell does not.
    let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, RELEASE_MOSAICS, MOSAIC_RED]);
    assert_eq!(cells.map(|cell| cell.held)[..5], [false, false, true, true, false]);
    assert_eq!(cells[4].glyph, Glyph::Space);
}

/// REQ-L1-006, REQ-TST-013: the separated mosaics example of Annex
/// G.3.2, 17 39 1A 39.
#[test]
fn annex_g32_separated_mosaics() {
    let cells = decode(&[MOSAIC_WHITE, 0x39, SEPARATED_MOSAICS, 0x39]);
    // 3/9 is bits 0, 3 and 4 of the sixel pattern.
    let pattern = 0b01_1001;
    assert_eq!(cells[1].glyph, Glyph::Mosaic { pattern, separated: false });
    assert_eq!(cells[2].glyph, Glyph::Space);
    assert_eq!(cells[3].glyph, Glyph::Mosaic { pattern, separated: true });
    assert!(cells.iter().all(|cell| !cell.held));
}

/// REQ-L1-007, REQ-TST-013: the hold mosaics example of Annex G.3.3,
/// 17 66 1E 39 1A 1F 66.
#[test]
fn annex_g33_hold_mosaics() {
    let cells = decode(&[MOSAIC_WHITE, 0x66, HOLD_MOSAICS, 0x39, SEPARATED_MOSAICS, RELEASE_MOSAICS, 0x66]);
    // 6/6 sets sixels 1, 2 and 5 (bit 6 maps to sixel 5); 3/9 sets
    // sixels 0, 3 and 4.
    let six = |separated| Glyph::Mosaic { pattern: 0b10_0110, separated };
    let three_nine = Glyph::Mosaic { pattern: 0b01_1001, separated: false };
    let expected = [Glyph::Space, six(false), six(false), three_nine, three_nine, three_nine, six(true)];
    assert_eq!(cells.map(|cell| cell.glyph)[..7], expected);
    assert_eq!(cells.map(|cell| cell.held)[..7], [false, false, true, false, true, true, false]);
}

/// REQ-L1-007; REQ-TST-013's hold with size changes. A change of size
/// resets the held mosaic, a repeated size does not, and neither does
/// turning hold off and on.
#[test]
fn hold_resets_on_size_change_only() {
    let held = Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false };
    // Repeated Normal Size keeps it; Double Height (set-after) shows it
    // in its own cell, then resets it.
    let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, NORMAL_SIZE, DOUBLE_HEIGHT, FLASH]);
    assert_eq!(cells.map(|cell| cell.glyph)[3..6], [held, held, Glyph::Space]);
    // Repeated Double Height keeps it; Normal Size (set-at) resets it
    // before its own cell.
    let cells = decode(&[MOSAIC_RED, DOUBLE_HEIGHT, 0x7F, HOLD_MOSAICS, DOUBLE_HEIGHT, NORMAL_SIZE]);
    assert_eq!(cells.map(|cell| cell.glyph)[3..6], [held, held, Glyph::Space]);
    // Release then Hold again: the held mosaic survives.
    let cells = decode(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, RELEASE_MOSAICS, FLASH, HOLD_MOSAICS]);
    assert_eq!(cells.map(|cell| cell.glyph)[3..6], [held, Glyph::Space, held]);
}
