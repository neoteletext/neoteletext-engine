//! Level 1.5 presentation (area L15, layer L3): the triplet walk of
//! REQ-FSM-007, applied to a grid already decoded at Level 1 (decision E.1).

use crate::charset::{CharRef, CharSet, Designation, G0};
use crate::config::Config;
use crate::consts::{
    COLUMNS, DESIGNATION_CODES, LATIN_AT_SIGN, MODE_ADDRESS_ROW_0, MODE_COLUMN_PDC, MODE_COLUMN_RESERVED_1,
    MODE_COLUMN_RESERVED_2, MODE_COLUMN_RESERVED_3, MODE_DIACRITIC_MASK, MODE_FULL_ROW_COLOUR, MODE_G0_NO_DIACRITIC,
    MODE_G2, MODE_G3_LEVEL_15, MODE_SET_ACTIVE_POSITION, MODE_TERMINATION, RECEIVER_COLUMNS, ROW_ADDRESS_BASE,
    ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24, TRIPLET_AT_SIGN_CODE, TRIPLET_FIRST_CHARACTER,
};
use crate::level1::{Glyph, Grid, Size};
use crate::packet::Triplet;

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
        WalkState::Start => (0, 0),
        WalkState::Walking { row, column } => (row, column),
        WalkState::Terminated => return (WalkState::Terminated, WalkAction::None),
    };
    // Decision E.9: a failed triplet neither moves nor writes.
    let Some(triplet) = triplet else {
        return (WalkState::Walking { row, column }, WalkAction::None);
    };
    if triplet.is_row_address() {
        if triplet.address == ROW_ADDRESS_LAST && triplet.mode == MODE_TERMINATION {
            return (WalkState::Terminated, WalkAction::None);
        }
        if triplet.address == ROW_ADDRESS_LAST && triplet.mode == MODE_ADDRESS_ROW_0 {
            return (WalkState::Walking { row: 0, column: 0 }, WalkAction::None);
        }
        let sets_row = !context.strict_row_setting
            || matches!(triplet.mode, MODE_FULL_ROW_COLOUR | MODE_SET_ACTIVE_POSITION | MODE_ADDRESS_ROW_0);
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
    match triplet.mode {
        MODE_COLUMN_RESERVED_1 | MODE_COLUMN_RESERVED_2 | MODE_COLUMN_RESERVED_3 | MODE_COLUMN_PDC => {
            (WalkState::Walking { row, column }, WalkAction::None)
        }
        MODE_G3_LEVEL_15 | MODE_G2 | MODE_G0_NO_DIACRITIC..=MODE_TERMINATION => {
            let next = WalkState::Walking { row, column: triplet.address };
            if triplet.data < TRIPLET_FIRST_CHARACTER {
                return (next, WalkAction::None);
            }
            (next, WalkAction::Write(character(triplet, context)))
        }
        _ => (WalkState::Walking { row, column: triplet.address }, WalkAction::None),
    }
}

/// The character a writing triplet names (REQ-L15-006, REQ-L15-010).
fn character(triplet: Triplet, context: WalkContext) -> CharRef {
    let g0 = CharSet::G0(G0 { set: context.designation.g0.set, national: None });
    match triplet.mode {
        MODE_G3_LEVEL_15 => CharRef::new(CharSet::G3, triplet.data),
        MODE_G2 => CharRef::new(CharSet::G2(context.designation.g2), triplet.data),
        MODE_G0_NO_DIACRITIC if triplet.data == TRIPLET_AT_SIGN_CODE => CharRef::new(g0, LATIN_AT_SIGN),
        MODE_G0_NO_DIACRITIC => CharRef::new(g0, triplet.data),
        mode => {
            let mut character = CharRef::new(g0, triplet.data);
            character.diacritic = mode & MODE_DIACRITIC_MASK;
            character
        }
    }
}

/// A page's packets X/26 by designation code, and which are present.
pub type EnhancementPackets = [[Option<Triplet>; crate::consts::TRIPLETS_PER_PACKET]; DESIGNATION_CODES];

/// Applies a page's X/26 packets to its grid (REQ-L15-002 to 010). Bit d of
/// `present` marks designation code d as received. `include_row_0` is false
/// while row 0 shows the rolling header (E.5 (i)).
pub fn apply(grid: &mut Grid, packets: &EnhancementPackets, present: u16, config: &Config, include_row_0: bool) {
    if !config.level15 {
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

/// Places one overwrite under decision E.5.
fn write(grid: &mut Grid, row: u8, column: u8, character: CharRef, include_row_0: bool) {
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
    cell.overwritten = true;
    cell.held = false;
    // (e): the bottom half appears in the row below.
    if cell.size == Size::DoubleTop
        && let Some(below) = grid.cell_mut(row.saturating_add(1), column)
        && below.size == Size::DoubleBottom
    {
        below.glyph = Glyph::Char(character);
        below.overwritten = true;
        below.held = false;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::unnecessary_wraps)]
mod tests {
    use super::*;
    use crate::charset::{G0Set, G2Set, NationalOption};
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
}
