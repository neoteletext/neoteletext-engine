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
use crate::trace::{Machine, TraceSink};

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

/// Names for the transition trace (REQ-FSM-010).
#[must_use]
pub const fn walk_state_name(state: WalkState) -> &'static str {
    match state {
        WalkState::Start => "Start",
        WalkState::Walking { .. } => "Walking",
        WalkState::Terminated => "Terminated",
    }
}

/// Names for the transition trace: the triplet's function, or a triplet
/// that failed its Hamming 24/18 check.
#[must_use]
pub const fn walk_event_name(triplet: Option<Triplet>) -> &'static str {
    match triplet {
        Some(triplet) => triplet.function().name(),
        None => "damaged triplet",
    }
}

/// Names for the transition trace.
#[must_use]
pub const fn walk_action_name(action: WalkAction) -> &'static str {
    match action {
        WalkAction::None => "none",
        WalkAction::Write(_) => "write character",
    }
}

/// The trace's action for a write that the strict option of REQ-L15-009
/// ignores.
const IGNORED_OUT_OF_ORDER: &str = "ignore out-of-order write";

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
            character.diacritic = crate::charset::page_mark(context.designation.g2, diacritic);
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
pub fn apply(grid: &mut Grid, packets: &EnhancementPackets, present: u16, config: &Config, include_row_0: bool) {
    walk(grid, packets, present, config, include_row_0, None);
}

/// As [`apply`], and when `Config::trace` is on, reports every transition
/// of the walk to `trace` (REQ-FSM-010).
pub fn apply_traced(
    grid: &mut Grid,
    packets: &EnhancementPackets,
    present: u16,
    config: &Config,
    include_row_0: bool,
    trace: &mut TraceSink<'_>,
) {
    walk(grid, packets, present, config, include_row_0, config.trace.then_some(trace));
}

/// The walk of [`apply`]; `trace` is `None` when tracing is off, so that
/// each transition costs one test of it.
fn walk(
    grid: &mut Grid,
    packets: &EnhancementPackets,
    present: u16,
    config: &Config,
    include_row_0: bool,
    mut trace: Option<&mut TraceSink<'_>>,
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
            let from = state;
            state = next;
            let mut ignored = false;
            if let (WalkAction::Write(character), WalkState::Walking { row, column }) = (action, state) {
                let out_of_order = previous.is_some_and(|before| (row, column) < before);
                ignored = out_of_order && config.ignore_out_of_order_triplets;
                if !ignored {
                    write(grid, row, column, character, include_row_0);
                }
                previous = Some((row, column));
            }
            if let Some(sink) = trace.as_deref_mut() {
                let action = if ignored { IGNORED_OUT_OF_ORDER } else { walk_action_name(action) };
                let event = walk_event_name(triplet);
                sink.transition(Machine::TripletWalk, walk_state_name(from), event, action, walk_state_name(state));
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
mod tests;
