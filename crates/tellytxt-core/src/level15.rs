//! Level 1.5 presentation (area L15, layer L3): the triplet walk of
//! REQ-FSM-007, applied to a grid already decoded at Level 1 (decision E.1).

use crate::charset::{CharRef, CharSet, Designation, G0};
use crate::config::{Config, PresentationLevel};
use crate::consts::{
    COLUMNS, DESIGNATION_CODES, DIACRITIC_NONE, LATIN_AT_SIGN, RECEIVER_COLUMNS, ROW_ADDRESS_BASE, ROW_ADDRESS_LAST,
    ROW_ADDRESS_ROW_24, TRIPLET_AT_SIGN_CODE, TRIPLET_FIRST_CHARACTER,
};
use crate::level1::{CharSource, Glyph, Grid, Size};
use crate::packet::{Triplet, TripletFunction};

/// The designation code X/26/15, which the "SAA5246A" profile skips
/// (decision E.2).
const LAST_DESIGNATION: u8 = 15;

/// States of the triplet walk (REQ-FSM-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkState {
    /// Before the walk.
    Start,
    /// Walking the triplets.
    Walking {
        /// Active row.
        row: u8,
        /// Active column.
        column: u8,
    },
    /// Finished.
    Terminated,
}

/// What a triplet does to the walk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkAction {
    /// Nothing.
    None,
    /// Write a character at the active position.
    Write(CharRef),
}

/// Inputs of the walk that do not change while it runs.
#[derive(Clone, Copy, Debug)]
pub struct WalkContext {
    /// The default sets (decision E.4); national options and ESC do not
    /// apply (REQ-L15-010).
    pub designation: Designation,
    /// The clause 12.3.2 row rule instead of Annex A.3 (decision E.3).
    pub strict_row_setting: bool,
}

/// One transition of the walk for one triplet (REQ-FSM-007).
#[must_use]
pub fn walk_transition(state: WalkState, triplet: Option<Triplet>, context: WalkContext) -> (WalkState, WalkAction) {
    let (row, column) = match state {
        // REQ-L15-003: clause 12.3.2's starting position.
        WalkState::Start => (0, 0),
        WalkState::Walking { row, column } => (row, column),
        WalkState::Terminated => return (WalkState::Terminated, WalkAction::None),
    };
    // Decision E.9: a failed triplet neither moves nor writes.
    let Some(triplet) = triplet else {
        return (WalkState::Walking { row, column }, WalkAction::None);
    };
    // REQ-PKT-010: the pass reads every triplet through the one decoding of
    // Tables 28 and 29.
    let function = triplet.function();
    // REQ-L15-004, decision E.3.
    match function {
        TripletFunction::Termination { .. } => return (WalkState::Terminated, WalkAction::None),
        TripletFunction::AddressDisplayRow0 { address: ROW_ADDRESS_LAST, .. } => {
            return (WalkState::Walking { row: 0, column: 0 }, WalkAction::None);
        }
        _ if triplet.is_row_address() => {
            // Annex A.3 reads the address field of every row triplet, so the
            // raw address is used whatever the function.
            let sets_row = !context.strict_row_setting
                || matches!(
                    function,
                    TripletFunction::FullRowColour { .. }
                        | TripletFunction::SetActivePosition { .. }
                        | TripletFunction::AddressDisplayRow0 { .. }
                );
            if sets_row {
                let new_row = if triplet.address == ROW_ADDRESS_ROW_24 {
                    24
                } else {
                    triplet.address.saturating_sub(ROW_ADDRESS_BASE)
                };
                return (WalkState::Walking { row: new_row, column: 0 }, WalkAction::None);
            }
            return (WalkState::Walking { row, column }, WalkAction::None);
        }
        _ => {}
    }
    // REQ-L15-005: every column mode sets the column except the reserved
    // ones and Programme Delivery Control (clause 12.3.4).
    let Some(new_column) = function.active_column() else {
        return (WalkState::Walking { row, column }, WalkAction::None);
    };
    let next = WalkState::Walking { row, column: new_column };
    // REQ-L15-007: every other mode, including the Level 2.5 G3 form 01011,
    // only moves the active position.
    let Some(character) = character(function, context) else { return (next, WalkAction::None) };
    // REQ-PKT-009: data values below 2/0 are reserved; the column still
    // moves, but nothing is written.
    if character.code < TRIPLET_FIRST_CHARACTER {
        return (next, WalkAction::None);
    }
    (next, WalkAction::Write(character))
}

/// The character a writing triplet names (REQ-L15-006, REQ-L15-010), with
/// its code as transmitted; `None` for triplets that write nothing at Level
/// 1.5.
fn character(function: TripletFunction, context: WalkContext) -> Option<CharRef> {
    let g0 = CharSet::G0(G0 { set: context.designation.g0.set, national: None });
    match function {
        TripletFunction::G3Character { code, level_15_form: true, .. } => Some(CharRef::new(CharSet::G3, code)),
        TripletFunction::G2Character { code, .. } => Some(CharRef::new(CharSet::G2(context.designation.g2), code)),
        TripletFunction::G0Diacritic { code: TRIPLET_AT_SIGN_CODE, diacritic: DIACRITIC_NONE, .. } => {
            Some(CharRef::new(g0, LATIN_AT_SIGN))
        }
        TripletFunction::G0Diacritic { code, diacritic, .. } => {
            let mut character = CharRef::new(g0, code);
            character.diacritic = diacritic;
            Some(character)
        }
        _ => None,
    }
}

/// A page's packets X/26 by designation code, and which are present.
pub type EnhancementPackets = [[Option<Triplet>; crate::consts::TRIPLETS_PER_PACKET]; DESIGNATION_CODES];

/// Applies a page's X/26 packets to its grid (REQ-L15-002 to 010). Bit d of
/// `present` marks designation code d as received. `include_row_0` is false
/// while row 0 shows the rolling header (E.5 (i)).
///
/// REQ-L15-001: at Level 1 the packets are stored but not applied, so the
/// pass does nothing. Choosing between this pass (Level 1.5) and the
/// enhancement processor (Levels 2.5 and 3.5) is the caller's dispatch.
pub fn apply<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &mut Grid<DRCS, PANEL_ROWS>,
    packets: &EnhancementPackets,
    present: u16,
    config: &Config,
    include_row_0: bool,
) {
    if config.level == PresentationLevel::Level1 {
        return;
    }
    let context = WalkContext { designation: grid.designation, strict_row_setting: config.strict_row_setting };
    let mut state = WalkState::Start;
    let mut previous: Option<(u8, u8)> = None;
    for (code, triplets) in packets.iter().enumerate().take(DESIGNATION_CODES) {
        let received = present.checked_shr(code as u32).is_some_and(|bits| bits & 1 == 1);
        if !received || (code == usize::from(LAST_DESIGNATION) && !config.process_x26_15) {
            continue;
        }
        for &triplet in triplets {
            let (next, action) = walk_transition(state, triplet, context);
            state = next;
            if let (WalkAction::Write(character), WalkState::Walking { row, column }) = (action, state) {
                let out_of_order = previous.is_some_and(|before| (row, column) < before);
                if !(out_of_order && config.ignore_out_of_order_triplets) {
                    write(grid, row, column, character, include_row_0);
                }
                previous = Some((row, column));
            }
            if state == WalkState::Terminated {
                return;
            }
        }
    }
}

/// Places one overwrite under decision E.5 (REQ-L15-008). Rule (a) holds
/// because the Level 1 pass is complete before [`apply`] runs, and rule (h)
/// because later triplets simply overwrite earlier ones.
fn write<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &mut Grid<DRCS, PANEL_ROWS>,
    row: u8,
    column: u8,
    character: CharRef,
    include_row_0: bool,
) {
    let (row, column) = (usize::from(row), usize::from(column));
    // (j): row 0 columns 0 to 7 belong to the receiver (REQ-SEC-005).
    if row == 0 && (column < RECEIVER_COLUMNS || !include_row_0) {
        return;
    }
    if column >= COLUMNS {
        return;
    }
    let Some(cell) = grid.cell_mut(row, column) else { return };
    // (f): a cell in the lower row of a double-height row is not shown.
    if cell.size == Size::DoubleBottom {
        return;
    }
    // (b), (c), (d), (g): only the glyph changes.
    cell.glyph = Glyph::Char(character);
    cell.attributes = cell.attributes.with_source(CharSource::Local);
    cell.held = false;
    // (e): the bottom half appears in the row below.
    if cell.size == Size::DoubleTop
        && let Some(below) = grid.cell_mut(row.saturating_add(1), column)
        && below.size == Size::DoubleBottom
    {
        below.glyph = Glyph::Char(character);
        below.attributes = below.attributes.with_source(CharSource::Local);
        below.held = false;
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unnecessary_wraps,
    clippy::arithmetic_side_effects
)]
mod tests {
    use super::*;
    use crate::charset::{G0Set, G2Set, NationalOption};
    use crate::consts::{
        MODE_ADDRESS_ROW_0, MODE_COLUMN_PDC, MODE_COLUMN_RESERVED_1, MODE_COLUMN_RESERVED_2, MODE_COLUMN_RESERVED_3,
        MODE_DIACRITIC_MASK, MODE_FULL_ROW_COLOUR, MODE_G0_NO_DIACRITIC, MODE_G2, MODE_G3_LEVEL_15, MODE_G3_LEVEL_25,
        MODE_SET_ACTIVE_POSITION, MODE_TERMINATION, MOSAIC_FULL,
    };
    use crate::level1::Cell;

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
        assert_eq!(
            walk_transition(walking, t(45, 0x00, 0), context(false)).0,
            WalkState::Walking { row: 5, column: 0 }
        );
        assert_eq!(walk_transition(walking, t(45, 0x00, 0), context(true)).0, walking);
        assert_eq!(
            walk_transition(walking, t(40, 0x04, 0), context(true)).0,
            WalkState::Walking { row: 24, column: 0 }
        );
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
            let keeps = matches!(
                mode,
                MODE_COLUMN_RESERVED_1 | MODE_COLUMN_RESERVED_2 | MODE_COLUMN_RESERVED_3 | MODE_COLUMN_PDC
            );
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
}
