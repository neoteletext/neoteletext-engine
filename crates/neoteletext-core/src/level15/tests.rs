//! Tests of the Level 1.5 overwrite walk (REQ-FSM-007, decision E.5).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unnecessary_wraps,
    clippy::arithmetic_side_effects
)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::charset::{G0Set, G2Set, NationalOption};
use crate::consts::{
    MODE_ADDRESS_ROW_0, MODE_COLUMN_PDC, MODE_COLUMN_RESERVED_1, MODE_COLUMN_RESERVED_2, MODE_COLUMN_RESERVED_3,
    MODE_DIACRITIC_MASK, MODE_FULL_ROW_COLOUR, MODE_G0_NO_DIACRITIC, MODE_G2, MODE_G3_LEVEL_15, MODE_G3_LEVEL_25,
    MODE_SET_ACTIVE_POSITION, MODE_TERMINATION, MOSAIC_FULL,
};
use crate::level1::Cell;
use crate::trace::{TraceEntry, TraceSink};
use crate::types::Instant;

fn context(strict: bool) -> WalkContext {
    WalkContext {
        designation: Designation {
            g0: G0 { set: G0Set::Latin, national: Some(NationalOption::German) },
            g2: G2Set::Latin,
        },
        strict_row_setting: strict,
    }
}

fn t(address: u8, mode: u8, data: u8) -> Option<Triplet> {
    Some(Triplet { address, mode, data })
}

/// REQ-L15-004.
#[test]
fn row_triplets_follow_annex_a3_by_default() {
    let walking = WalkState::Walking { row: 1, column: 5 };
    // Mode 00000 (reserved row mode) sets the row under Annex A.3 only.
    assert_eq!(walk_transition(walking, t(45, 0x00, 0), context(false)).0, WalkState::Walking { row: 5, column: 0 });
    assert_eq!(walk_transition(walking, t(45, 0x00, 0), context(true)).0, walking);
    assert_eq!(walk_transition(walking, t(40, 0x04, 0), context(true)).0, WalkState::Walking { row: 24, column: 0 });
    assert_eq!(walk_transition(walking, t(63, 0x07, 0), context(true)).0, WalkState::Walking { row: 0, column: 0 });
    assert_eq!(walk_transition(walking, t(63, 0x1F, 0), context(false)).0, WalkState::Terminated);
}

/// REQ-L15-005, REQ-TST-014: the "@" case and data below 2/0.
#[test]
fn character_modes_and_special_cases() {
    let start = WalkState::Walking { row: 2, column: 0 };
    let (state, action) = walk_transition(start, t(7, 0x10, 0x2A), context(false));
    assert_eq!(state, WalkState::Walking { row: 2, column: 7 });
    let WalkAction::Write(c) = action else { panic!() };
    assert_eq!(c.code, LATIN_AT_SIGN);
    assert_eq!(c.set, CharSet::G0(G0 { set: G0Set::Latin, national: None }), "no national option");
    let (_, action) = walk_transition(start, t(3, 0x12, b'e'), context(false));
    let WalkAction::Write(c) = action else { panic!() };
    assert_eq!(c.diacritic, 2, "acute");
    let (state, action) = walk_transition(start, t(9, 0x0F, 0x1F), context(false));
    assert_eq!((state, action), (WalkState::Walking { row: 2, column: 9 }, WalkAction::None), "data below 2/0");
    let (state, _) = walk_transition(start, t(9, 0x06, 0x41), context(false));
    assert_eq!(state, start, "PDC leaves the position");
    let (_, action) = walk_transition(start, t(4, 0x02, 0x51), context(false));
    assert!(matches!(action, WalkAction::Write(CharRef { set: CharSet::G3, code: 0x51, .. })));
    let (_, action) = walk_transition(start, None, context(false));
    assert_eq!(action, WalkAction::None);
}

/// REQ-L15-008, REQ-TST-014: rules (b), (e), (f), (i) and (j).
#[test]
fn overwrites_obey_decision_e5() {
    let mut grid = Grid::EMPTY;
    grid.cells[3][4] = Cell { foreground: 2, background: 4, size: Size::DoubleTop, ..Cell::BLANK };
    grid.cells[4][4] = Cell { size: Size::DoubleBottom, ..grid.cells[3][4] };
    let character = CharRef::new(CharSet::G3, 0x51);
    write(&mut grid, 3, 4, character, true);
    assert_eq!(grid.cells[3][4].glyph, Glyph::Char(character));
    assert_eq!(grid.cells[3][4].foreground, 2, "(b) keeps colours");
    assert_eq!(grid.cells[4][4].glyph, Glyph::Char(character), "(e) bottom half");
    let other = CharRef::new(CharSet::G3, 0x5B);
    write(&mut grid, 4, 4, other, true);
    assert_eq!(grid.cells[4][4].glyph, Glyph::Char(character), "(f) lower row ignored");
    write(&mut grid, 0, 3, other, true);
    assert_eq!(grid.cells[0][3].glyph, Glyph::Space, "(j) receiver columns");
    write(&mut grid, 0, 20, other, false);
    assert_eq!(grid.cells[0][20].glyph, Glyph::Space, "(i) not on the rolling header");
}

/// REQ-L15-003: the walk starts at row 0, column 0.
#[test]
fn walk_starts_at_row_0_column_0() {
    let (state, action) = walk_transition(WalkState::Start, t(9, MODE_COLUMN_PDC, 0), context(false));
    assert_eq!((state, action), (WalkState::Walking { row: 0, column: 0 }, WalkAction::None));
    let (state, action) = walk_transition(WalkState::Start, t(20, MODE_G0_NO_DIACRITIC, b'A'), context(false));
    assert_eq!(state, WalkState::Walking { row: 0, column: 20 });
    assert!(matches!(action, WalkAction::Write(CharRef { code: b'A', .. })));
}

/// REQ-L15-004, REQ-TST-014: every row mode in both row-setting
/// settings.
#[test]
fn every_row_mode_in_both_settings() {
    let walking = WalkState::Walking { row: 1, column: 5 };
    let row_5 = ROW_ADDRESS_BASE + 5;
    for mode in 0..=MODE_TERMINATION {
        let (state, action) = walk_transition(walking, t(row_5, mode, 0), context(false));
        assert_eq!((state, action), (WalkState::Walking { row: 5, column: 0 }, WalkAction::None), "{mode:#04x}");
        let (state, _) = walk_transition(walking, t(ROW_ADDRESS_ROW_24, mode, 0), context(false));
        assert_eq!(state, WalkState::Walking { row: 24, column: 0 }, "{mode:#04x}");
        let strict_sets = matches!(mode, MODE_FULL_ROW_COLOUR | MODE_SET_ACTIVE_POSITION | MODE_ADDRESS_ROW_0);
        let expected = if strict_sets { WalkState::Walking { row: 5, column: 0 } } else { walking };
        assert_eq!(walk_transition(walking, t(row_5, mode, 0), context(true)).0, expected, "strict {mode:#04x}");
    }
    for strict in [false, true] {
        let row_0 = walk_transition(walking, t(ROW_ADDRESS_LAST, MODE_ADDRESS_ROW_0, 0), context(strict)).0;
        assert_eq!(row_0, WalkState::Walking { row: 0, column: 0 });
        let end = walk_transition(walking, t(ROW_ADDRESS_LAST, MODE_TERMINATION, 0), context(strict)).0;
        assert_eq!(end, WalkState::Terminated);
    }
}

/// REQ-L15-005, REQ-L15-007, REQ-TST-014: every column mode moves the
/// column except the reserved modes and Programme Delivery Control, and
/// only the G0, G2 and Level 1.5 G3 modes write.
#[test]
fn every_column_mode_sets_the_column_and_only_character_modes_write() {
    let start = WalkState::Walking { row: 2, column: 0 };
    for mode in 0..=MODE_TERMINATION {
        let (state, action) = walk_transition(start, t(9, mode, b'A'), context(false));
        let keeps =
            matches!(mode, MODE_COLUMN_RESERVED_1 | MODE_COLUMN_RESERVED_2 | MODE_COLUMN_RESERVED_3 | MODE_COLUMN_PDC);
        let expected = if keeps { start } else { WalkState::Walking { row: 2, column: 9 } };
        assert_eq!(state, expected, "{mode:#04x}");
        let writes = mode == MODE_G3_LEVEL_15 || mode == MODE_G2 || mode >= MODE_G0_NO_DIACRITIC;
        assert_eq!(matches!(action, WalkAction::Write(_)), writes, "{mode:#04x}");
    }
    assert_eq!(walk_transition(start, t(9, MODE_G3_LEVEL_25, b'A'), context(false)).1, WalkAction::None);
}

/// REQ-TST-014: every diacritical mark, modes 10001 to 11111, on a G0
/// character without national option substitution.
#[test]
fn every_diacritical_mark() {
    let start = WalkState::Walking { row: 2, column: 0 };
    for mode in MODE_G0_NO_DIACRITIC..=MODE_TERMINATION {
        let (_, action) = walk_transition(start, t(3, mode, b'a'), context(false));
        let WalkAction::Write(character) = action else { panic!("mode {mode:#04x} wrote nothing") };
        assert_eq!(character.diacritic, mode & MODE_DIACRITIC_MASK);
        assert_eq!(character.code, b'a');
        assert_eq!(character.set, CharSet::G0(G0 { set: G0Set::Latin, national: None }));
    }
}

/// REQ-TST-014: missing designation codes, the Termination Marker and
/// the triplets after it, and X/26/15 under each profile.
#[test]
fn packets_with_gaps_termination_and_x26_15() {
    let write = |column: u8, data: u8| t(column, MODE_G0_NO_DIACRITIC, data);
    let row_3 = t(ROW_ADDRESS_BASE + 3, MODE_SET_ACTIVE_POSITION, 0);
    let mut packets: EnhancementPackets = [[None; crate::consts::TRIPLETS_PER_PACKET]; DESIGNATION_CODES];
    // Designation codes 0 and 1 are missing; 2 and 15 carry data.
    packets[2][..2].copy_from_slice(&[row_3, write(4, b'A')]);
    packets[15][..4].copy_from_slice(&[
        row_3,
        write(6, b'B'),
        t(ROW_ADDRESS_LAST, MODE_TERMINATION, 0),
        write(8, b'C'),
    ]);
    let present = (1 << 2) | (1 << 15);
    let glyph_code = |grid: &Grid, column: usize| match grid.cells[3][column].glyph {
        Glyph::Char(character) => Some(character.code),
        _ => None,
    };
    let mut grid = Grid::EMPTY;
    apply(&mut grid, &packets, present, &Config::default(), true);
    assert_eq!([4, 6, 8].map(|column| glyph_code(&grid, column)), [Some(b'A'), Some(b'B'), None]);
    let mut grid = Grid::EMPTY;
    apply(&mut grid, &packets, present, &Config::saa5246a(), true);
    assert_eq!([4, 6, 8].map(|column| glyph_code(&grid, column)), [Some(b'A'), None, None]);
}

/// REQ-L15-001, REQ-PRF-006: at Level 1 the stored packets are not
/// applied.
#[test]
fn level_1_applies_nothing() {
    let mut packets: EnhancementPackets = [[None; crate::consts::TRIPLETS_PER_PACKET]; DESIGNATION_CODES];
    packets[0][..2]
        .copy_from_slice(&[t(ROW_ADDRESS_BASE + 3, MODE_SET_ACTIVE_POSITION, 0), t(4, MODE_G0_NO_DIACRITIC, b'A')]);
    let mut grid = Grid::EMPTY;
    apply(&mut grid, &packets, 1, &Config::saa5050(), true);
    assert_eq!(grid, Grid::EMPTY);
    let level15 = Config { level: PresentationLevel::Level15, ..Config::saa5050() };
    apply(&mut grid, &packets, 1, &level15, true);
    assert!(matches!(grid.cells[3][4].glyph, Glyph::Char(character) if character.code == b'A'));
}

/// REQ-L15-008, REQ-TST-014: rules (c), (d), (g) and (h) of decision
/// E.5.
#[test]
fn overwrites_obey_decision_e5_glyph_rules() {
    let mut grid = Grid::EMPTY;
    let held = Glyph::Mosaic { pattern: MOSAIC_FULL, separated: true };
    grid.cells[5][1] = Cell { glyph: held, ..Cell::BLANK };
    grid.cells[5][2] = Cell { glyph: held, held: true, ..Cell::BLANK };
    grid.cells[5][3] = Cell { glyph: held, held: true, ..Cell::BLANK };
    grid.cells[5][4] = Cell { parity_failed: true, ..Cell::BLANK };
    let first = CharRef::new(CharSet::G3, 0x51);
    let second = CharRef::new(CharSet::G3, 0x5B);
    // (c): a character replaces a mosaic, whatever the mosaic mode.
    write(&mut grid, 5, 1, first, true);
    assert_eq!(grid.cells[5][1].glyph, Glyph::Char(first));
    // (d): the overwrite wins over a held mosaic and leaves the next
    // held cell alone.
    write(&mut grid, 5, 2, first, true);
    assert_eq!((grid.cells[5][2].glyph, grid.cells[5][2].held), (Glyph::Char(first), false));
    assert_eq!((grid.cells[5][3].glyph, grid.cells[5][3].held), (held, true));
    // (g): shown over a byte that failed parity.
    write(&mut grid, 5, 4, first, true);
    assert_eq!(grid.cells[5][4].glyph, Glyph::Char(first));
    // (h): the later of two overwrites wins.
    let mut packets: EnhancementPackets = [[None; crate::consts::TRIPLETS_PER_PACKET]; DESIGNATION_CODES];
    packets[0][..3].copy_from_slice(&[
        t(ROW_ADDRESS_BASE + 5, MODE_SET_ACTIVE_POSITION, 0),
        t(4, MODE_G3_LEVEL_15, first.code),
        t(4, MODE_G3_LEVEL_15, second.code),
    ]);
    apply(&mut grid, &packets, 1, &Config::default(), true);
    assert_eq!(grid.cells[5][4].glyph, Glyph::Char(second));
}

/// REQ-FSM-010, REQ-FSM-007, REQ-L15-009: a known walk gives the expected
/// trace, one entry per triplet up to and including the Termination
/// Marker, naming the states, the triplet's function or a damaged triplet,
/// and the action, including a write the strict option ignores. With
/// tracing off nothing is reported; after the Termination Marker the walk
/// stays terminated.
#[test]
fn the_walk_reports_every_transition_when_tracing() {
    let write = |column: u8, data: u8| t(column, MODE_G0_NO_DIACRITIC, data);
    let mut packets: EnhancementPackets = [[None; crate::consts::TRIPLETS_PER_PACKET]; DESIGNATION_CODES];
    packets[0][..6].copy_from_slice(&[
        t(ROW_ADDRESS_BASE + 3, MODE_SET_ACTIVE_POSITION, 0),
        write(6, b'B'),
        None,
        write(4, b'A'),
        t(ROW_ADDRESS_LAST, MODE_TERMINATION, 0),
        write(8, b'C'),
    ]);
    let strict = Config { trace: true, ignore_out_of_order_triplets: true, ..Config::default() };
    let mut entries = std::vec::Vec::new();
    let mut grid = Grid::EMPTY;
    let mut record = |entry: TraceEntry| entries.push(entry);
    let mut sink = TraceSink { time: Instant(7), entries: &mut record };
    apply_traced(&mut grid, &packets, 1, &strict, true, &mut sink);
    let character = walk_event_name(write(6, b'B'));
    let expected = [
        ("Start", "Set Active Position", "none", "Walking"),
        ("Walking", character, "write character", "Walking"),
        ("Walking", "damaged triplet", "none", "Walking"),
        ("Walking", character, "ignore out-of-order write", "Walking"),
        ("Walking", "Termination Marker", "none", "Terminated"),
    ];
    let got: std::vec::Vec<_> = entries.iter().map(|entry| (entry.from, entry.event, entry.action, entry.to)).collect();
    assert_eq!(got, expected);
    assert!(entries.iter().all(|entry| entry.machine == Machine::TripletWalk && entry.time == Instant(7)));
    let code = |column: usize| match grid.cells[3][column].glyph {
        Glyph::Char(character) => Some(character.code),
        _ => None,
    };
    assert_eq!([code(4), code(6), code(8)], [None, Some(b'B'), None]);
    // Tracing off: the same walk reports nothing.
    let mut count = 0;
    let mut counting = |_: TraceEntry| count += 1;
    let mut sink = TraceSink { time: Instant(0), entries: &mut counting };
    let mut untraced = Grid::EMPTY;
    apply_traced(&mut untraced, &packets, 1, &Config::default(), true, &mut sink);
    assert_eq!(count, 0);
    assert_eq!(walk_state_name(WalkState::Terminated), "Terminated");
    for triplet in [write(8, b'C'), t(ROW_ADDRESS_BASE + 2, MODE_SET_ACTIVE_POSITION, 0), None] {
        assert_eq!(
            walk_transition(WalkState::Terminated, triplet, context(false)),
            (WalkState::Terminated, WalkAction::None)
        );
    }
}

/// REQ-L15-008, clause 12.3.4: a Level 1.5 overwrite has the 40 columns of
/// the page; a column beyond them writes nothing, not even into a grid's
/// side-panel cells, which belong to Level 2.5.
#[test]
fn overwrites_beyond_column_39_write_nothing() {
    let mut grid = Grid::EMPTY;
    write(&mut grid, 5, COLUMNS as u8, CharRef::new(CharSet::G3, 0x51), true);
    write(&mut grid, 5, 60, CharRef::new(CharSet::G3, 0x51), true);
    assert_eq!(grid, Grid::EMPTY);
}
