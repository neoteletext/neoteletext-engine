//! Dynamically redefinable characters (area DRC, REQ-TST-016, REQ-TST-018):
//! DRCS pages built with the encoder, linked from a displayed page, and
//! decoded into the patterns and colour tables its grid carries.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

mod common;

use common::{
    DISPLAY, Receiver, build, display_page, drcs_page, end, g0, invoke, link, local_data, local_number, null_link, ptu,
    set_position, x26, x27,
};
use tellytxt_core::config::{Config, PresentationLevel};
use tellytxt_core::consts::{
    DRCS_MODE_6X5X4, DRCS_MODE_12X10X1, DRCS_MODE_12X10X2, DRCS_MODE_12X10X4, DRCS_MODE_NO_DATA, DRCS_MODE_SUBSEQUENT,
    DRCS_PTU_BYTES, DRCS_PTUS, LINK_FUNCTION_DRCS, LINK_FUNCTION_GDRCS, LINK_FUNCTION_GPOP, LINK_FUNCTION_POP,
};
use tellytxt_core::drcs::{DrcsColourTables, DrcsMode};
use tellytxt_core::ecc::with_odd_parity;
use tellytxt_core::enhance;
use tellytxt_core::level1::{DisplayOptions, Glyph, Grid, build_grid};
use tellytxt_core::packet::{DrcsColourData, ObjectKind, ObjectSource, Packet, TripletFunction, encode};
use tellytxt_core::types::{DrcsCharacter, DrcsTable, PageAddress, SubCode};

const L25: PresentationLevel = PresentationLevel::Level25;
const L35: PresentationLevel = PresentationLevel::Level35;
/// The normal DRCS page and the Global DRCS page of the tests.
const DRCS: u16 = 0x1B0;
const GDRCS: u16 = 0x1B1;

/// X/27/4 naming the GDRCS and DRCS pages.
fn drcs_links() -> Packet {
    x27(
        4,
        [
            null_link(LINK_FUNCTION_GPOP),
            null_link(LINK_FUNCTION_POP),
            link(LINK_FUNCTION_GDRCS, 0b11, GDRCS),
            link(LINK_FUNCTION_DRCS, 0b11, DRCS),
            null_link(0),
            null_link(0),
        ],
    )
}

fn invoke_drcs(column: u8, normal: bool, character: u8) -> TripletFunction {
    TripletFunction::DrcsCharacter { column, normal, character }
}

fn drcs_mode(normal: bool, levels: u8, subtable: u8) -> TripletFunction {
    TripletFunction::DrcsMode { normal, levels, subtable }
}

/// Page 100 with DRCS links and local data, then the DRCS pages, which
/// arrive after the links that name them.
fn stream(local: &[TripletFunction], others: Vec<Packet>, pages: Vec<Vec<Packet>>) -> Vec<Packet> {
    let mut all = vec![drcs_links()];
    all.extend(others);
    all.extend(local_data(local));
    let mut packets = display_page(0, &[], all);
    for page in pages {
        packets.extend(page);
    }
    packets
}

fn normal(subtable: u8, character: u8) -> DrcsCharacter {
    DrcsCharacter { table: DrcsTable::Normal, subtable, character }
}

/// Mode codes with the given PTUs set and every other PTU "no data".
fn modes(set: &[(usize, u8)]) -> [u8; DRCS_PTUS] {
    let mut codes = [DRCS_MODE_NO_DATA; DRCS_PTUS];
    for &(index, code) in set {
        codes[index] = code;
    }
    codes
}

/// A recognisable plane: row r has dot r lit, and dot 11 on even rows.
fn diagonal() -> [u16; 10] {
    core::array::from_fn(|row| (0x800 >> row) | u16::from(row % 2 == 0))
}

/// REQ-DRC-001 to REQ-DRC-004, REQ-TST-016, REQ-TST-018: the PTU layout of
/// Figure 27 (characters 0 and 1 in mode 0, character 2 in mode 1 over
/// PTUs 2 and 3, character 4 in mode 2 over PTUs 4 to 7, across packets
/// X/3 and X/4), decoded pixel by pixel.
#[test]
fn ptu_layout_of_figure_27() {
    let full = [0xFFF; 10];
    let empty = [0; 10];
    let ptus = [
        (0, ptu(diagonal())),
        (1, ptu(full)),
        (2, ptu(full)),
        (3, ptu(diagonal())),
        (4, ptu(full)),
        (5, ptu(empty)),
        (6, ptu(diagonal())),
        (7, ptu(full)),
    ];
    let codes = modes(&[
        (0, DRCS_MODE_12X10X1),
        (1, DRCS_MODE_12X10X1),
        (2, DRCS_MODE_12X10X2),
        (3, DRCS_MODE_SUBSEQUENT),
        (4, DRCS_MODE_12X10X4),
        (5, DRCS_MODE_SUBSEQUENT),
        (6, DRCS_MODE_SUBSEQUENT),
        (7, DRCS_MODE_SUBSEQUENT),
    ]);
    let local = [
        set_position(1, 0),
        invoke_drcs(0, true, 0),
        invoke_drcs(1, true, 1),
        invoke_drcs(2, true, 2),
        invoke_drcs(3, true, 4),
    ];
    let packets = stream(&local, vec![], vec![drcs_page(DRCS, 0, &ptus, Some(codes))]);
    let (grid, report) = build(&packets, L35);
    assert_eq!(report.unwrap().faults.drcs_missing, 0);
    let pattern = |character| grid.drcs_pattern(normal(0, character)).copied().unwrap();
    let zero = pattern(0);
    assert_eq!(zero.mode, DrcsMode::Mode0);
    assert_eq!(zero.planes[0], diagonal(), "D-byte i is row i / 2, the most significant bit leftmost");
    assert_eq!((zero.pixel(3, 3), zero.pixel(3, 4), zero.pixel(2, 11), zero.pixel(3, 11)), (1, 0, 1, 0));
    assert_eq!(pattern(1).planes[0], full);
    let two = pattern(2);
    assert_eq!(two.mode, DrcsMode::Mode1);
    assert_eq!((two.pixel(0, 0), two.pixel(0, 5)), (3, 1), "plane 0 from PTU 2, plane 1 from PTU 3");
    let four = pattern(4);
    assert_eq!(four.mode, DrcsMode::Mode2);
    assert_eq!((four.pixel(0, 0), four.pixel(0, 5), four.pixel(5, 5)), (0b1101, 0b1001, 0b1101));
    // The cells name the characters (REQ-DRC-006, REQ-L25-006).
    assert!(
        matches!(grid.cells[1][3].glyph, Glyph::Char(character) if character.drcs_character() == Some(normal(0, 4)))
    );
}

/// REQ-DRC-004, decision E.54: a 6 by 5 by 4 character, D-byte 4r + k as
/// plane k of row r, each pixel 2 by 2 dots.
#[test]
fn mode_3_character() {
    let mut bytes = [with_odd_parity(0x40); DRCS_PTU_BYTES];
    // Row 2: plane 1 the third pixel; row 4: plane 3 every pixel.
    bytes[4 * 2 + 1] = with_odd_parity(0x40 | 0b00_1000);
    bytes[4 * 4 + 3] = with_odd_parity(0x40 | 0b11_1111);
    let packets = stream(
        &[set_position(1, 0), invoke_drcs(0, true, 5)],
        vec![],
        vec![drcs_page(DRCS, 0, &[(5, bytes)], Some(modes(&[(5, DRCS_MODE_6X5X4)])))],
    );
    let grid = build(&packets, L35).0;
    let pattern = grid.drcs_pattern(normal(0, 5)).copied().unwrap();
    assert_eq!(pattern.mode, DrcsMode::Mode3);
    assert_eq!((pattern.pixel(4, 4), pattern.pixel(5, 5), pattern.pixel(4, 6), pattern.pixel(4, 3)), (2, 2, 0, 0));
    assert_eq!((pattern.pixel(8, 0), pattern.pixel(9, 11)), (8, 8));
}

/// REQ-DRC-003, REQ-DRC-009, REQ-TST-018: missing and invalid data. A
/// missing character keeps its cell and has no pattern; an invalid one
/// shows a space.
#[test]
fn missing_and_invalid_data() {
    let mut bad_byte = ptu([0xFFF; 10]);
    bad_byte[3] = with_odd_parity(0x20);
    let codes = modes(&[
        (0, DRCS_MODE_12X10X1),
        (1, DRCS_MODE_SUBSEQUENT),
        (2, DRCS_MODE_12X10X2),
        (3, DRCS_MODE_12X10X1),
        (4, DRCS_MODE_12X10X1),
        (46, DRCS_MODE_12X10X4),
        (47, DRCS_MODE_SUBSEQUENT),
    ]);
    let ptus = [(0, ptu([1; 10])), (1, ptu([1; 10])), (2, ptu([1; 10])), (3, ptu([1; 10])), (4, bad_byte)];
    let local = [
        set_position(1, 0),
        invoke_drcs(0, true, 0),
        // A first PTU marked "further PTU".
        invoke_drcs(1, true, 1),
        // A mode 1 character whose second PTU is not marked 1110.
        invoke_drcs(2, true, 2),
        // A D-byte below 4/0.
        invoke_drcs(3, true, 4),
        // Runs past PTU 47.
        invoke_drcs(4, true, 46),
        // Numbers 48 to 63 are invalid.
        invoke_drcs(5, true, 50),
        // No data at all: a first PTU marked 1111.
        invoke_drcs(6, true, 10),
        // PTU 20 is in packet X/11, never received.
        invoke_drcs(7, true, 20),
        // Sub-table 3 is not stored.
        drcs_mode(true, 0b11, 3),
        invoke_drcs(8, true, 0),
    ];
    let mut codes_with_missing = codes;
    codes_with_missing[20] = DRCS_MODE_12X10X1;
    let packets = stream(&local, vec![], vec![drcs_page(DRCS, 0, &ptus, Some(codes_with_missing))]);
    let (grid, report) = build(&packets, L35);
    let report = report.unwrap();
    let cell = |column: usize| grid.cells[1][column].glyph;
    assert!(grid.drcs_pattern(normal(0, 0)).is_some());
    for column in [1, 2, 3, 4, 5, 6] {
        assert_eq!(cell(column), Glyph::Space, "column {column}: invalid");
    }
    assert!(matches!(cell(7), Glyph::Char(_)), "missing: the character stays, without a pattern");
    assert!(grid.drcs_pattern(normal(0, 20)).is_none());
    assert!(matches!(cell(8), Glyph::Char(_)) && grid.drcs_pattern(normal(3, 0)).is_none());
    assert_eq!((report.faults.drcs_invalid, report.faults.drcs_missing), (6, 2));
}

/// REQ-DRC-009, decision E.54: a character becomes usable once its PTUs
/// arrive; a later partial reception replaces the PTUs it carries.
#[test]
fn partial_receptions_complete_a_character() {
    let local = [set_position(1, 0), invoke_drcs(0, true, 2), invoke_drcs(1, true, 0)];
    let first = drcs_page(DRCS, 0, &[(0, ptu([0x0F0; 10]))], None);
    // Packet X/2 carries PTUs 2 and 3, sent later as a partial page (C13).
    let mut second = drcs_page(DRCS, 0, &[(2, ptu([0xF00; 10]))], None);
    second[0] = common::header(DRCS, 0, tellytxt_core::consts::C13);
    second.remove(1);
    let only_first = stream(&local, vec![], vec![first.clone()]);
    let grid = build(&only_first, L35).0;
    assert!(grid.drcs_pattern(normal(0, 0)).is_some() && grid.drcs_pattern(normal(0, 2)).is_none());
    let both = stream(&local, vec![], vec![first, second]);
    let grid = build(&both, L35).0;
    assert_eq!(grid.drcs_pattern(normal(0, 2)).unwrap().planes[0], [0xF00; 10]);
    assert_eq!(grid.drcs_pattern(normal(0, 0)).unwrap().planes[0], [0x0F0; 10], "packet X/1 kept");
}

/// REQ-DRC-003, REQ-L25-001, decision E.54: at Level 2.5 only mode 0
/// characters are usable.
#[test]
fn level_25_uses_mode_0_only() {
    let codes = modes(&[(0, DRCS_MODE_12X10X1), (1, DRCS_MODE_12X10X2), (2, DRCS_MODE_SUBSEQUENT)]);
    let ptus = [(0, ptu([1; 10])), (1, ptu([1; 10])), (2, ptu([1; 10]))];
    let local = [set_position(1, 0), invoke_drcs(0, true, 0), invoke_drcs(1, true, 1)];
    let packets = stream(&local, vec![], vec![drcs_page(DRCS, 0, &ptus, Some(codes))]);
    let (grid, report) = build(&packets, L25);
    assert!(grid.drcs_pattern(normal(0, 0)).is_some() && grid.drcs_pattern(normal(0, 1)).is_none());
    assert_eq!(report.unwrap().faults.drcs_missing, 1, "unusable, shown as missing");
    assert!(build(&packets, L35).0.drcs_pattern(normal(0, 1)).is_some());
}

/// REQ-DRC-005, REQ-DRC-006, REQ-TST-018: the DRCS Mode triplet's table,
/// level bits and scope; the normal and global tables come from their own
/// links; an object starts with sub-tables 0 and passes none back.
#[test]
fn drcs_mode_triplet_and_tables() {
    let page = |mpp, s1, lines| drcs_page(mpp, s1, &[(0, ptu([lines; 10]))], None);
    let object = vec![
        common::define(ObjectKind::Passive, 0b11, local_number(2, 0)),
        invoke_drcs(0, true, 0),
        drcs_mode(true, 0b11, 2),
        invoke_drcs(1, true, 0),
        TripletFunction::Termination { kind: 0b101 },
    ];
    let local = [
        set_position(1, 0),
        invoke_drcs(0, true, 0),
        invoke_drcs(1, false, 0),
        drcs_mode(true, 0b11, 1),
        invoke_drcs(2, true, 0),
        // Level 3.5 only: ignored at 2.5.
        drcs_mode(true, 0b10, 2),
        invoke_drcs(3, true, 0),
        // The reserved value 00 counts as both levels.
        drcs_mode(false, 0b00, 1),
        invoke_drcs(4, false, 0),
        set_position(2, 0),
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(2, 0)),
        // The object's choice of sub-table 2 does not come back.
        invoke_drcs(5, true, 0),
    ];
    let mut others = vec![];
    let mut data = local.to_vec();
    data.push(end());
    others.push(x26(0, &data[..13]));
    others.push(x26(1, &data[13..]));
    others.push(x26(2, &object));
    let mut packets = display_page(0, &[], vec![drcs_links()]);
    packets.extend(others);
    for (mpp, s1, lines) in [(DRCS, 0, 0x001), (DRCS, 1, 0x002), (DRCS, 2, 0x004), (GDRCS, 0, 0x100), (GDRCS, 1, 0x200)]
    {
        packets.extend(page(mpp, s1, lines));
    }
    let subtable_of = |grid: &Grid, row: usize, column: usize| match grid.cells[row][column].glyph {
        Glyph::Char(character) => character.drcs_character().map(|drcs| (drcs.table, drcs.subtable)),
        _ => None,
    };
    for level in [L25, L35] {
        let grid = build(&packets, level).0;
        let expect = |column, table, subtable| {
            assert_eq!(subtable_of(&grid, 1, column), Some((table, subtable)), "{level:?} column {column}");
        };
        expect(0, DrcsTable::Normal, 0);
        expect(1, DrcsTable::Global, 0);
        expect(2, DrcsTable::Normal, 1);
        expect(3, DrcsTable::Normal, if level == L35 { 2 } else { 1 });
        expect(4, DrcsTable::Global, 1);
        assert_eq!(subtable_of(&grid, 2, 0), Some((DrcsTable::Normal, 0)), "an object starts at sub-table 0");
        assert_eq!(subtable_of(&grid, 2, 1), Some((DrcsTable::Normal, 2)));
        let own = if level == L35 { 2 } else { 1 };
        assert_eq!(subtable_of(&grid, 2, 5), Some((DrcsTable::Normal, own)), "the local choice, not the object's");
        // Each pattern from its own page.
        let lines =
            |table, subtable| grid.drcs_pattern(DrcsCharacter { table, subtable, character: 0 }).unwrap().planes[0][0];
        assert_eq!((lines(DrcsTable::Normal, 1), lines(DrcsTable::Global, 1)), (0x002, 0x200));
    }
}

/// REQ-DRC-007, REQ-TST-016, REQ-TST-018: the DRCS colour tables from the
/// page's X/28/1, then the magazine's M/29/1, then the defaults of Annex
/// D.2.2, at Level 3.5 only; Figure 28's example values.
#[test]
fn drcs_colour_tables_follow_decision_e54() {
    let figure_28 = DrcsColourData {
        triplet_1: Some(0),
        global_4: Some([21, 3, 14, 31]),
        normal_4: Some([19, 16, 17, 1]),
        global_16: Some([12, 26, 25, 21, 28, 19, 3, 15, 1, 0, 31, 10, 23, 2, 14, 17]),
        normal_16: Some([15, 5, 25, 10, 21, 8, 26, 11, 12, 4, 26, 28, 30, 3, 7, 24]),
    };
    let magazine = DrcsColourData { normal_4: Some([5, 6, 7, 8]), ..DrcsColourData::default() };
    let page_packet = encode::drcs_colours(common::mag(1), 28, &figure_28);
    let magazine_packet = encode::drcs_colours(common::mag(1), 29, &magazine);
    let tables = |others: Vec<Packet>, level| {
        let mut packets = others;
        packets.extend(stream(&[], vec![], vec![]));
        build(&packets, level).0.drcs_colours
    };
    let page = tables(vec![], L35);
    assert_eq!(page, DrcsColourTables::DEFAULT, "Annex D.2.2");
    let from_page = {
        let mut packets = vec![magazine_packet];
        packets.extend(stream(&[], vec![page_packet], vec![]));
        build(&packets, L35).0.drcs_colours
    };
    assert_eq!(from_page.global_4, [21, 3, 14, 31]);
    assert_eq!(from_page.normal_16, figure_28.normal_16.unwrap());
    let from_magazine = tables(vec![magazine_packet], L35);
    assert_eq!(from_magazine.normal_4, [5, 6, 7, 8]);
    assert_eq!(from_magazine.global_4, [0; 4], "M/29/1 carries all four tables");
    assert_eq!(tables(vec![magazine_packet], L25), DrcsColourTables::DEFAULT, "Level 3.5 only");
}

/// REQ-DRC-007: a character beyond the grid's pattern capacity is missing
/// and counted.
#[test]
fn pattern_capacity_is_the_callers() {
    let ptus = [(0, ptu([1; 10])), (1, ptu([2; 10])), (2, ptu([4; 10]))];
    let local = [set_position(1, 0), invoke_drcs(0, true, 0), invoke_drcs(1, true, 1), invoke_drcs(2, true, 2)];
    let packets = stream(&local, vec![], vec![drcs_page(DRCS, 0, &ptus, None)]);
    let config = Config::default();
    Receiver::new().run(&packets, config, |decoder| {
        let address = PageAddress::from_mpp(DISPLAY).unwrap();
        let page = decoder.store().get(address, SubCode::ANY).unwrap();
        let magazine = decoder.magazine_data(address.magazine);
        let mut grid: Box<Grid<2>> = Box::new(Grid::empty());
        let options = DisplayOptions::default();
        build_grid(&mut grid, Some(page), &magazine, &config, &options);
        let report = enhance::apply(&mut grid, page, &magazine, &decoder.enhancement_lookup(), &config, &options);
        assert_eq!(grid.drcs.as_slice().len(), 2);
        assert!(grid.drcs_pattern(normal(0, 2)).is_none());
        assert_eq!(report.faults.drcs_missing, 1);
    });
}

/// REQ-DRC-010: the Level 2.5 limits of two sub-tables and 24 characters
/// bind the broadcaster, not the receiver.
#[test]
fn level_25_limits_are_not_enforced() {
    let ptus: Vec<(usize, [u8; DRCS_PTU_BYTES])> = (0..30).map(|index| (index, ptu([index as u16 + 1; 10]))).collect();
    let mut local = vec![set_position(1, 0)];
    for index in 0..30u8 {
        if index == 15 {
            local.push(set_position(2, 0));
        }
        local.push(invoke_drcs(index % 15, true, index));
    }
    local.push(drcs_mode(true, 0b01, 1));
    local.push(set_position(3, 0));
    local.push(invoke_drcs(0, true, 0));
    let mut pages = vec![drcs_page(DRCS, 0, &ptus, None)];
    pages.push(drcs_page(DRCS, 1, &[(0, ptu([7; 10]))], None));
    let mut others = vec![drcs_links()];
    others.extend(local_data(&local));
    let mut packets = display_page(0, &[], others);
    for page in pages {
        packets.extend(page);
    }
    let (grid, report) = build(&packets, L25);
    assert_eq!(grid.drcs.as_slice().len(), 31, "30 characters of sub-table 0 and one of sub-table 1");
    assert_eq!(report.unwrap().faults.drcs_missing, 0);
}

/// REQ-DRC-008: a mode 0 character takes the colours of its cell.
#[test]
fn mode_0_characters_take_the_cell_colours() {
    let local = [set_position(1, 0), common::foreground(0, 20), common::background(0, 9), invoke_drcs(1, true, 0)];
    let packets = stream(&local, vec![], vec![drcs_page(DRCS, 0, &[(0, ptu([1; 10]))], None)]);
    let grid = build(&packets, L25).0;
    let cell = grid.cells[1][1];
    assert_eq!((cell.foreground, cell.background), (20, 9));
    assert!(!cell.attributes.underline());
    let _ = g0(0, 0);
}
