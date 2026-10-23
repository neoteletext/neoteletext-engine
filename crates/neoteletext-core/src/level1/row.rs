//! The row attribute machine of REQ-FSM-006: the Table 26 spacing
//! attributes, set-at and set-after, applied along one row.

use super::{Cell, Glyph, Size};
use crate::charset::{CharRef, CharSet, G0, mosaic_pattern};
use crate::config::Config;
use crate::consts::{
    ALPHA_BLACK, ALPHA_WHITE, ATTRIBUTE_LAST, BLACK_BACKGROUND, CODE_MASK_7BIT, COLOUR_ATTRIBUTE_MASK, COLOUR_BLACK,
    COLOUR_WHITE, COLUMNS, CONCEAL, CONTIGUOUS_MOSAICS, DOUBLE_HEIGHT, END_BOX, ESC, FLASH, HOLD_MOSAICS, MOSAIC_BIT5,
    MOSAIC_BLACK, MOSAIC_WHITE, NEW_BACKGROUND, NORMAL_SIZE, RELEASE_MOSAICS, SEPARATED_MOSAICS, SPACE, START_BOX,
    STEADY,
};

/// The state record of the row attribute machine (REQ-FSM-006).
// The fields are exactly the record REQ-FSM-006 lists.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RowState {
    foreground: u8,
    background: u8,
    mosaic: bool,
    flash: bool,
    conceal: bool,
    boxed: bool,
    double: bool,
    separated: bool,
    hold: bool,
    held: Option<(u8, bool)>,
    second_g0: bool,
}

impl RowState {
    /// The default state at the start of every row (REQ-L1-002).
    pub const DEFAULT: Self = Self {
        foreground: COLOUR_WHITE,
        background: COLOUR_BLACK,
        mosaic: false,
        flash: false,
        conceal: false,
        boxed: false,
        double: false,
        separated: false,
        hold: false,
        held: None,
        second_g0: false,
    };
}

/// The inputs of one row's decoding.
pub(super) struct RowContext<'a> {
    config: &'a Config,
    default_g0: G0,
    second_g0: Option<G0>,
    /// Double height is honoured only in rows 1 to 22 (decision E.15).
    double_allowed: bool,
}

impl<'a> RowContext<'a> {
    /// The inputs of one row: the configuration, the page's G0 set and second
    /// G0 set, and whether double height takes effect in the row.
    pub(super) const fn new(config: &'a Config, default_g0: G0, second_g0: Option<G0>, double_allowed: bool) -> Self {
        Self { config, default_g0, second_g0, double_allowed }
    }
}

/// Decodes one row of 7-bit codes (REQ-FSM-006). `failed` marks columns
/// whose byte failed parity. Returns the cells and whether the row contains
/// double height.
pub(super) fn decode_row(codes: &[u8; COLUMNS], failed: u64, context: &RowContext<'_>) -> ([Cell; COLUMNS], bool) {
    let mut cells = [Cell::BLANK; COLUMNS];
    let mut state = RowState::DEFAULT;
    let mut has_double = false;
    for (column, out) in cells.iter_mut().enumerate() {
        let code = codes.get(column).copied().unwrap_or(SPACE) & CODE_MASK_7BIT;
        let next = codes.get(column.saturating_add(1)).copied().unwrap_or(SPACE) & CODE_MASK_7BIT;
        // Step 1: set-at attributes.
        set_at(&mut state, code);
        // Step 2: what the cell shows.
        let (glyph, held) = cell_glyph(&state, code, context);
        // Step 3: record the cell.
        *out = Cell {
            glyph,
            foreground: state.foreground,
            background: state.background,
            flash: state.flash,
            conceal: state.conceal,
            boxed: state.boxed,
            size: if state.double { Size::DoubleTop } else { Size::Normal },
            held,
            parity_failed: failed.checked_shr(column as u32).is_some_and(|bits| bits & 1 == 1),
            ..Cell::BLANK
        };
        // Step 4: the held mosaic (decision E.16, REQ-L1-007): only codes
        // with bit 6 set qualify, so blast-through capitals are never held.
        if state.mosaic && code > ATTRIBUTE_LAST && code & MOSAIC_BIT5 != 0 {
            state.held = Some((mosaic_pattern(code), state.separated));
        }
        // Step 5: set-after attributes.
        if set_after(&mut state, code, next, context) {
            has_double = true;
        }
    }
    (cells, has_double)
}

/// Step 1 of REQ-FSM-006: the Table 26 set-at attributes, which act in
/// their own cell (REQ-L1-003). Steady ends flash (REQ-L1-009), Conceal
/// starts concealment (REQ-L1-010), and the background codes act at once so
/// the attribute cell itself is painted (REQ-L1-012).
fn set_at(state: &mut RowState, code: u8) {
    match code {
        STEADY => state.flash = false,
        NORMAL_SIZE => {
            // Decision E.16: a set-at size change resets the held mosaic
            // before its own cell is shown.
            if state.double {
                state.held = None;
            }
            state.double = false;
        }
        CONCEAL => state.conceal = true,
        CONTIGUOUS_MOSAICS => state.separated = false,
        SEPARATED_MOSAICS => state.separated = true,
        BLACK_BACKGROUND => state.background = COLOUR_BLACK,
        NEW_BACKGROUND => state.background = state.foreground,
        HOLD_MOSAICS => state.hold = true,
        _ => {}
    }
}

/// Step 2 of REQ-FSM-006: the glyph and whether it is a held mosaic.
fn cell_glyph(state: &RowState, code: u8, context: &RowContext<'_>) -> (Glyph, bool) {
    if code <= ATTRIBUTE_LAST {
        // REQ-L1-004: attribute cells show a space, or the held mosaic.
        if state.hold
            && state.mosaic
            && let Some((pattern, separated)) = state.held
        {
            return (Glyph::Mosaic { pattern, separated }, true);
        }
        return (Glyph::Space, false);
    }
    let g0 = if state.second_g0 { context.second_g0.unwrap_or(context.default_g0) } else { context.default_g0 };
    // REQ-L1-006: the G1 pattern keeps the form in force at this cell.
    if state.mosaic && code & MOSAIC_BIT5 != 0 {
        return (Glyph::Mosaic { pattern: mosaic_pattern(code), separated: state.separated }, false);
    }
    // Alphanumerics, and blast-through capitals in mosaic mode, with the
    // national option applied (Table 47 note 2).
    (Glyph::Char(CharRef::new(CharSet::G0(g0), code)), false)
}

/// Step 5 of REQ-FSM-006: the Table 26 set-after attributes, which act
/// from the next cell (REQ-L1-003). Returns true when the row gains double
/// height.
fn set_after(state: &mut RowState, code: u8, next: u8, context: &RowContext<'_>) -> bool {
    // REQ-L1-019, decision E.18: 0/0 and 1/0 act only under the SAA5246A
    // profile; 0/E and 0/F fall through to the no-op arm in every profile.
    let black_codes = context.config.black_colour_codes;
    match code {
        ALPHA_BLACK..=ALPHA_WHITE if code != ALPHA_BLACK || black_codes => {
            if state.mosaic {
                state.held = None;
            }
            state.mosaic = false;
            state.foreground = code & COLOUR_ATTRIBUTE_MASK;
            // REQ-L1-010: a colour attribute ends concealment.
            state.conceal = false;
        }
        MOSAIC_BLACK..=MOSAIC_WHITE if code != MOSAIC_BLACK || black_codes => {
            if !state.mosaic {
                state.held = None;
            }
            state.mosaic = true;
            state.foreground = code & COLOUR_ATTRIBUTE_MASK;
            state.conceal = false;
        }
        // REQ-L1-009: flashing until Steady or the end of the row.
        FLASH => state.flash = true,
        // REQ-L1-011: box membership is set-after at both ends.
        END_BOX => state.boxed = false,
        START_BOX => {
            // Decision E.17: a box starts only at the second of two
            // consecutive Start Box codes.
            if !context.config.double_start_box || next == START_BOX {
                state.boxed = true;
            }
        }
        DOUBLE_HEIGHT if context.double_allowed => {
            if !state.double {
                state.held = None;
            }
            state.double = true;
            return true;
        }
        ESC => state.second_g0 = !state.second_g0,
        RELEASE_MOSAICS => state.hold = false,
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests;
