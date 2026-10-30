//! Magazine 1's presentation pages: sizes and display attributes, flash,
//! the G3, G1 and G2 sets, every triplet mode, and sizes written by
//! enhancement data.

use std::vec::Vec;

use neoteletext_core::consts::{
    CONCEAL, DOUBLE_HEIGHT, END_BOX, FLASH, MODE_DRCS_CHARACTER, MODE_FONT_STYLE, MODE_TERMINATION, MOSAIC_WHITE,
    ROW_ADDRESS_LAST, ROW_ADDRESS_RESERVED_DEFAULT, START_BOX,
};
use neoteletext_core::packet::{ColourData, DisplayAttributes, ObjectKind, Triplet, TripletFunction};

use crate::author::{DisplayPage, FlashMode, FlashRate, Result, RowScope, colour, row_address};

use super::{
    BLUE, BOTH, DRCS_INVALID_CHARACTER, DRCS_MODE, DRCS_MODE_DATA, DRCS_NORMAL, FONT_STYLE_BOLD, GERMAN, GREEN, RED,
    page, text_body,
};

/// 109: double height, width and size, underline, invert, conceal and
/// boxing as Display Attributes (Table 29, mode 01100; Annex C.3).
pub(super) fn sizes_and_attributes() -> Result<DisplayPage> {
    let mut page = page(0x109, b"SIZES AND DISPLAY ATTRIBUTES")?;
    let none = DisplayAttributes::default();
    let cases: [(u8, &[u8], DisplayAttributes); 8] = [
        (3, b"DOUBLE HEIGHT", DisplayAttributes { double_height: true, ..none }),
        (6, b"DOUBLE WIDTH", DisplayAttributes { double_width: true, ..none }),
        (9, b"DOUBLE SIZE", DisplayAttributes { double_height: true, double_width: true, ..none }),
        (12, b"UNDERLINE", DisplayAttributes { underline: true, ..none }),
        (14, b"INVERT", DisplayAttributes { invert: true, ..none }),
        (16, b"CONCEAL", DisplayAttributes { conceal: true, ..none }),
        (18, b"BOXING IS A WINDOW", DisplayAttributes { boxing: true, ..none }),
        (20, b"UNDERLINE AND INVERT", DisplayAttributes { underline: true, invert: true, ..none }),
    ];
    for (row, text, attributes) in cases {
        page.text(row, 4, text)?;
        page.text(row, 26, b"PLAIN")?;
        page.local().position(row, 0)?.attributes(4, attributes);
        // Decision E.47: a zero bit cancels the attribute from its cell.
        page.local().attributes(26, none);
    }
    page.text(
        22,
        0,
        &[&[DOUBLE_HEIGHT][..], b"LEVEL 1 DOUBLE HEIGHT, ", &[START_BOX, END_BOX, CONCEAL], b"HIDDEN"].concat(),
    )?;
    Ok(page)
}

/// 110: Additional Flash Functions, every mode at every rate and phase
/// (Table 29, mode 00111; decision E.48), beside Level 1 Flash.
pub(super) fn flash() -> Result<DisplayPage> {
    let mut page = page(0x110, b"FLASH: MODES, RATES AND PHASES")?;
    page.text(2, 0, b"MODE SLOW  F1    F2    F3    INC   DEC")?;
    for (row, mode) in (4u8..).step_by(3).zip(FlashMode::ALL) {
        let label: &[u8] = match mode {
            FlashMode::Steady => b"STD",
            FlashMode::Normal => b"NOR",
            FlashMode::InvertPhase => b"INV",
            FlashMode::AdjacentClut => b"ADJ",
        };
        page.text(row, 0, label)?;
        page.local().position(row, 0)?;
        // An adjacent-CLUT flash shows index XOR 8 in its off phase.
        page.local().foreground(4, colour(2, 1));
        for (block, rate) in (0u8..).zip(FlashRate::ALL) {
            let column = 5u8.saturating_add(block.saturating_mul(6));
            page.text(row, column, b"FLASH")?;
            page.local().flash(column, mode, rate);
        }
    }
    page.text(17, 0, b"RESERVED RATE 110:  FLASH")?;
    page.local().position(17, 0)?.flash(20, FlashMode::Normal, FlashRate::Reserved(0b110));
    page.text(19, 0, &[&[FLASH][..], b"LEVEL 1 FLASH (0/8), SLOW AT LEVEL 2.5"].concat())?;
    page.text(21, 0, b"FAST INCREMENTAL OVER A LONG RUN OF TEXT")?;
    page.local().position(21, 0)?.flash(5, FlashMode::Normal, FlashRate::Incremental);
    page.text(23, 0, b"FAST DECREMENTAL OVER A LONG RUN OF TEXT")?;
    page.local().position(23, 0)?.flash(5, FlashMode::Normal, FlashRate::Decremental);
    Ok(page)
}

/// 112: the G3 set by both modes, and G1 mosaics written by X/26
/// (Table 29, modes 01011, 00010 and 00001).
pub(super) fn g3_and_g1() -> Result<DisplayPage> {
    let mut page = page(0x112, b"G3 SET, BOTH MODES; G1 MOSAICS")?;
    for (row, codes) in (2u8..).zip((0x20u8..0x80).collect::<Vec<u8>>().chunks(16)) {
        page.text(row, 0, b"G3")?;
        page.local().position(row, 0)?;
        for (column, &code) in (4u8..).zip(codes) {
            page.local().g3(column, code);
        }
    }
    page.text(9, 0, b"G3 L1.5")?;
    page.local().position(9, 0)?;
    for (column, code) in (8u8..).zip(0x20u8..0x30) {
        page.local().g3_level_15(column, code);
    }
    page.text(12, 0, &[MOSAIC_WHITE, b'G', b'1'])?;
    page.local().position(12, 0)?;
    for (column, code) in (4u8..).zip((0x20u8..0x40).chain(0x60..0x68)) {
        page.local().g1(column, code);
    }
    Ok(page)
}

/// 118: the G2 set, every diacritical mark, and a Modified G0 and G2
/// Designation (Table 29, modes 01111, 10000 to 11111, 01000 and 01001).
pub(super) fn g2_and_diacritics() -> Result<DisplayPage> {
    let mut page = page(0x118, b"G2 SET, DIACRITICS, DESIGNATION")?;
    for (row, codes) in (2u8..).zip((0x20u8..0x80).collect::<Vec<u8>>().chunks(16)) {
        page.text(row, 0, b"G2")?;
        page.local().position(row, 0)?;
        for (column, &code) in (4u8..).zip(codes) {
            page.local().g2(column, code);
        }
    }
    for (row, base) in [(10u8, b'a'), (11, b'O')] {
        page.text(row, 0, b"MARK")?;
        page.local().position(row, 0)?;
        for (column, mark) in (5u8..).zip(0u8..16) {
            page.local().diacritic(column, base, mark);
        }
    }
    page.text(14, 0, b"DESIG")?;
    page.local().position(14, 0)?.designation(6, GERMAN);
    for (column, &code) in (6u8..).zip(b"#$@[\\]^_`{|}~") {
        page.local().g0(column, code);
    }
    Ok(page)
}

/// 113: every row mode of Table 28 and every column mode of Table 29,
/// with reserved values of their data (REQ-TST-015).
pub(super) fn every_mode() -> Result<DisplayPage> {
    let mut page = page(0x113, b"EVERY TRIPLET MODE")?;
    let active = page.local_object(ObjectKind::Active, BOTH, text_body(0, b"ACT"));
    let adaptive = page.local_object(ObjectKind::Adaptive, BOTH, text_body(0, b"ADP"));
    let passive = page.local_object(ObjectKind::Passive, BOTH, text_body(0, b"PAS"));
    let local = page.local();
    local.full_screen_colour(colour(1, 0)).row_0(colour(3, 0), RowScope::ThisRow);
    local.character(30, b'0');
    local.full_row_colour(2, colour(1, 7), RowScope::ThisRow)?;
    // Row 2: column modes 00000 to 01111.
    local.foreground(0, RED).g1(1, 0x7F).g3_level_15(2, 0x30).background(3, BLUE);
    for (column, mode) in [(4u8, 0b00100u8), (5, 0b00101)] {
        local.raw(Triplet { address: column, mode, data: 0 });
    }
    local.pdc_column(6, 0x15);
    local.flash(7, FlashMode::Normal, FlashRate::Slow).designation(8, GERMAN).g0(9, b'@');
    local.raw(Triplet { address: 10, mode: 0b01010, data: 0 });
    local.g3(11, 0x41).attributes(12, DisplayAttributes { underline: true, ..DisplayAttributes::default() });
    // DRCS Mode and DRCS Character Invocation, sent as a service would
    // and not acted on (decision E.95): the second moves the column only.
    local.raw(Triplet { address: ROW_ADDRESS_RESERVED_DEFAULT, mode: DRCS_MODE, data: DRCS_MODE_DATA });
    local.raw(Triplet { address: 13, mode: MODE_DRCS_CHARACTER, data: DRCS_NORMAL });
    // Font Style, bold, sent as a service would and not acted on (decision
    // E.96): it moves the column only.
    local.raw(Triplet { address: 14, mode: MODE_FONT_STYLE, data: FONT_STYLE_BOLD });
    local.g2(15, 0x41);
    // Column modes 10000 to 11111: G0 with each diacritical mark.
    for (column, mark) in (16u8..).zip(0u8..16) {
        local.diacritic(column, b'e', mark);
    }
    // Row 3: reserved data values.
    local.position(3, 0)?;
    local.function(TripletFunction::ForegroundColour { column: 1, colour: ColourData { index: GREEN, high: 0b01 } });
    local.function(TripletFunction::AdditionalFlash { column: 2, mode: 1, rate: 0b111, reserved: 0b11 });
    local.raw(Triplet { address: 3, mode: MODE_DRCS_CHARACTER, data: DRCS_NORMAL | DRCS_INVALID_CHARACTER });
    local.position_address(row_address(4)?, 45);
    // Reserved row modes and Programme Delivery Control, which do not move
    // the Active Position (decision E.46).
    for mode in [0b00010u8, 0b00011, 0b00101, 0b00110, 0b01110, 0b01111, 0b10100] {
        local.raw(Triplet { address: row_address(4)?, mode, data: 0 });
    }
    for mode in 0b01000u8..=0b01101 {
        local.pdc_row(mode, row_address(4)?, 0x11);
    }
    for mode in 0b11001u8..=0b11110 {
        local.raw(Triplet { address: row_address(4)?, mode, data: 0 });
    }
    // Rows 5 to 7: an Origin Modifier and the three invocations, whose
    // definitions use row modes 10101 to 10111.
    local.position(5, 0)?.origin(0, 4).invoke_local(active);
    local.position(6, 0)?.origin(0, 8).invoke_local(adaptive);
    local.position(7, 0)?.origin(0, 12).invoke_local(passive);
    // Mode 11111 with an address other than 63 is reserved (Table 28). It
    // comes last, so that a decoder that ends the data on it loses nothing.
    local.raw(Triplet { address: ROW_ADDRESS_LAST - 1, mode: MODE_TERMINATION, data: 0 });
    page.text(9, 0, b"ROW 0 COLUMN 30: ADDRESS DISPLAY ROW 0")?;
    Ok(page)
}

/// 124: double height, width and size on characters that the enhancement
/// data writes itself, each size ended after its text, with the Level 1
/// rows left empty (Annex C.3).
pub(super) fn sizes_written_by_enhancement_data() -> Result<DisplayPage> {
    let mut page = page(0x124, b"SIZES OF CHARACTERS WRITTEN BY X/26")?;
    let none = DisplayAttributes::default();
    let sizes = [
        (3u8, DisplayAttributes { double_height: true, ..none }, &b"HEIGHT"[..]),
        (6, DisplayAttributes { double_width: true, ..none }, b"WIDTH"),
        (9, DisplayAttributes { double_height: true, double_width: true, ..none }, b"SIZE"),
    ];
    for (row, attributes, text) in sizes {
        // A double-width character covers the next column too, so the
        // characters leave no space between them that the size runs over.
        let pitch = if attributes.double_width { 2 } else { 1 };
        page.local().position(row, 0)?.attributes(4, attributes);
        for (column, &code) in (4u8..).step_by(pitch).zip(text) {
            page.local().character(column, code);
        }
        let end = 4u8.saturating_add(u8::try_from(text.len().saturating_mul(pitch)).unwrap_or(u8::MAX));
        page.local().attributes(end, none);
    }
    Ok(page)
}
