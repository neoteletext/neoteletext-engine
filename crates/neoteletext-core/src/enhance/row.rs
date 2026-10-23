//! Row resolution (REQ-FSM-012): each row from its Level 1 bytes and the
//! walk's events, cell by cell, with the Table 26 machine of the underlying
//! plane and the overlays of Adaptive and Passive objects.

use super::{Collector, RANGES, Range, RowCells, Rows, Walker};

use super::attributes::{Flash, LEVEL_1_FLASH, flash_rate};
use super::characters::{enhancement_glyph, is_text};
use super::planes::Plane;
use super::{AttrChange, Attrs};
use crate::acq::EnhancementReport;
use crate::charset::{CharRef, CharSet, G0, G0Set, G2Set, mosaic_pattern};
use crate::colour::{ColourIndex, Remapping};
use crate::consts::{
    ALPHA_BLACK, ALPHA_WHITE, ATTRIBUTE_LAST, BLACK_BACKGROUND, CLUT_ENTRY_MASK, COLOUR_BLACK, COLOUR_WHITE, COLUMNS,
    CONCEAL, CONTIGUOUS_MOSAICS, DOUBLE_HEIGHT, DOUBLE_HEIGHT_FIRST_ROW, DOUBLE_HEIGHT_LAST_ROW, DOUBLE_SIZE,
    DOUBLE_WIDTH, END_BOX, ESC, FLASH, HOLD_MOSAICS, MOSAIC_BIT5, MOSAIC_BLACK, MOSAIC_WHITE, NEW_BACKGROUND,
    NORMAL_SIZE, RELEASE_MOSAICS, SCREEN_COLUMNS, SEPARATED_MOSAICS, SIDE_PANEL_FIRST_ROW, SPACE, START_BOX, STEADY,
};
use crate::level1::{Cell, CellAttributes, CharSource, Glyph, Size, row_codes};
use crate::types::FixedVec;

/// The inputs of a row's resolution that do not change along it.
// Each flag is an independent input: a page setting, a profile setting or
// the row's position.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
struct RowContext {
    remapping: Remapping,
    substitution: bool,
    row_colour: ColourIndex,
    boxed_page: bool,
    double_start_box: bool,
    /// Double height and double size take effect in this row.
    double_allowed: bool,
    default_sets: (G0Set, G2Set),
    level_1_g0: G0,
    second_g0: Option<G0>,
}

/// The record of REQ-FSM-006 for the underlying plane, with the fields
/// REQ-FSM-012 adds.
// Each flag is a field REQ-FSM-006 or REQ-FSM-012 lists.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy)]
struct RowState {
    /// The attributes, with `conceal` the non-spacing conceal.
    attrs: Attrs,
    /// Conceal set by the Level 1 code 1/8, which a colour code ends
    /// (decision E.47).
    level_1_conceal: bool,
    /// The entry number of the foreground colour the Level 1 page selected,
    /// for New Background (REQ-L25-022).
    level_1_foreground: u8,
    mosaic: bool,
    /// Separated mosaics set by the Level 1 code 1/A.
    separated: bool,
    hold: bool,
    held: Option<(u8, bool)>,
    second_g0: bool,
    designation: Option<(G0Set, G2Set)>,
    /// A spacing double height or double size code acted in the row.
    spacing_double: bool,
}

impl RowState {
    /// The start-of-row defaults, re-mapped (REQ-L25-022): white
    /// foreground, and a black background unless substitution replaces it
    /// with the row colour.
    fn start(context: &RowContext) -> Self {
        let background =
            if context.substitution { context.row_colour } else { context.remapping.background(COLOUR_BLACK) };
        Self {
            attrs: Attrs { foreground: context.remapping.foreground(COLOUR_WHITE), background, ..Attrs::PASSIVE },
            level_1_conceal: false,
            level_1_foreground: COLOUR_WHITE,
            mosaic: false,
            separated: false,
            hold: false,
            held: None,
            second_g0: false,
            designation: None,
            spacing_double: false,
        }
    }

    /// The attributes shown.
    fn shown(&self) -> Attrs {
        Attrs { conceal: self.attrs.conceal || self.level_1_conceal, ..self.attrs }
    }

    /// A Level 1 size code (Table 26 notes 2, 5 and 8: any size attribute
    /// ends any other); the held mosaic resets when the size changes.
    fn size(&mut self, height: bool, width: bool) {
        if (self.attrs.double_height, self.attrs.double_width) != (height, width) {
            self.held = None;
        }
        self.attrs.double_height = height;
        self.attrs.double_width = width;
    }

    /// Step 1: the set-after attributes of the previous cell's byte.
    fn set_after(&mut self, code: u8, next: u8, context: &RowContext) {
        match code {
            // REQ-L25-022: 0/0 and 1/0 are black at these levels, and the
            // colour comes from the re-mapped CLUT.
            ALPHA_BLACK..=ALPHA_WHITE | MOSAIC_BLACK..=MOSAIC_WHITE => {
                let mosaic = code >= MOSAIC_BLACK;
                if mosaic != self.mosaic {
                    self.held = None;
                }
                self.mosaic = mosaic;
                self.level_1_foreground = code & CLUT_ENTRY_MASK;
                self.attrs.foreground = context.remapping.foreground(self.level_1_foreground);
                // Decision E.47: a colour code ends the Level 1 conceal only.
                self.level_1_conceal = false;
            }
            FLASH => self.attrs.flash = LEVEL_1_FLASH,
            // Decision E.47: the Level 1 box codes act only on boxed pages.
            END_BOX if context.boxed_page => self.attrs.boxed = false,
            START_BOX if context.boxed_page && (!context.double_start_box || next == START_BOX) => {
                self.attrs.boxed = true;
            }
            DOUBLE_HEIGHT | DOUBLE_SIZE => {
                self.size(true, code == DOUBLE_SIZE);
                // REQ-L25-026: the row below is suppressed.
                self.spacing_double |= context.double_allowed;
            }
            DOUBLE_WIDTH => self.size(false, true),
            ESC => self.second_g0 = !self.second_g0,
            RELEASE_MOSAICS => self.hold = false,
            _ => {}
        }
    }

    /// Step 2: the set-at attributes of the cell's byte.
    fn set_at(&mut self, code: u8, context: &RowContext) {
        match code {
            STEADY => self.attrs.flash = Flash::Steady,
            NORMAL_SIZE => self.size(false, false),
            CONCEAL => self.level_1_conceal = true,
            CONTIGUOUS_MOSAICS => self.separated = false,
            SEPARATED_MOSAICS => self.separated = true,
            // REQ-L25-022: entry 0 of the background CLUT, or the row colour
            // under substitution.
            BLACK_BACKGROUND => {
                self.attrs.background =
                    if context.substitution { context.row_colour } else { context.remapping.background(COLOUR_BLACK) };
            }
            NEW_BACKGROUND => self.attrs.background = context.remapping.background(self.level_1_foreground),
            HOLD_MOSAICS => self.hold = true,
            _ => {}
        }
    }

    /// Step 3: the non-spacing attributes of the underlying plane, which win
    /// against step 2 (REQ-L25-020, decision E.47).
    fn apply(&mut self, change: &AttrChange) {
        self.attrs.apply(change);
        if let Some(display) = change.display {
            // Decision E.47: a 0 bit also cancels the Level 1 function.
            if !display.conceal {
                self.level_1_conceal = false;
            }
            if !display.underline {
                self.separated = false;
            }
        }
        if let Some(sets) = change.designation {
            self.designation = Some(sets);
        }
    }
}

/// An Adaptive object's overlay on the row (REQ-OBJ-010).
#[derive(Clone, Copy)]
struct Overlay {
    range: Range,
    change: AttrChange,
}

/// What resolving one cell gives, before sizes are applied.
#[derive(Clone, Copy)]
pub(super) struct Resolved {
    pub(super) cell: Cell,
    pub(super) double_height: bool,
    pub(super) double_width: bool,
}

/// Builds a cell from a glyph and the attributes it is drawn with.
pub(super) fn cell_of(glyph: Glyph, attrs: &Attrs, source: CharSource, column: usize) -> Resolved {
    let text = is_text(glyph);
    let mut attributes = CellAttributes::DEFAULT.with_source(source).with_invert(attrs.invert);
    let flash = match attrs.flash {
        Flash::Steady => false,
        Flash::On { mode, rate, start } => {
            attributes = attributes.with_flash_mode(mode).with_flash_rate(flash_rate(rate, start, column));
            true
        }
    };
    if text {
        // Decisions E.47 and E.50: underline is for G0 and G2 characters
        // only.
        attributes = attributes.with_underline(attrs.underline);
    }
    Resolved {
        cell: Cell {
            glyph,
            foreground: attrs.foreground,
            background: attrs.background,
            flash,
            conceal: attrs.conceal,
            boxed: attrs.boxed,
            size: Size::Normal,
            held: false,
            parity_failed: false,
            attributes,
        },
        double_height: attrs.double_height,
        double_width: attrs.double_width,
    }
}

/// A cell of a row segment: what the walk put there and, in the main page,
/// the Level 1 byte.
struct Segment<'c> {
    first: usize,
    end: usize,
    /// The Level 1 codes and parity failures, `None` in side panels and in
    /// the row below spacing double height (REQ-L25-026).
    codes: Option<(&'c [u8; COLUMNS], u64)>,
    /// The row below spacing double height: cells without enhancement
    /// characters keep the fill of the cell above.
    lower: Option<&'c [Cell]>,
}

/// Resolves the cells of one row segment (REQ-FSM-012 steps 1 to 7).
/// `report` counts undefined characters.
fn resolve_segment(
    segment: &Segment<'_>,
    context: &RowContext,
    start: RowState,
    out: &Collector,
    resolved: &mut [Option<Resolved>; SCREEN_COLUMNS],
    report: &mut EnhancementReport,
) -> RowState {
    let mut state = start;
    let mut overlays: FixedVec<Overlay, RANGES> =
        FixedVec::new(Overlay { range: Range { slot: 0, first: 0, last: 0 }, change: AttrChange::default() });
    for range in out.ranges.as_slice() {
        if usize::from(range.first) >= segment.first && usize::from(range.last) < segment.end {
            let _ = overlays.push(Overlay { range: *range, change: AttrChange::default() });
        }
    }
    let mut pending: Option<u8> = None;
    for column in segment.first..segment.end {
        let slot = out.slots.get(column).copied().unwrap_or_default();
        let code = segment.codes.and_then(|(codes, _)| codes.get(column).copied()).unwrap_or(SPACE);
        // Steps 1 and 2: the Level 1 set-after and set-at attributes.
        if let Some(previous) = pending.take() {
            state.set_after(previous, code, context);
        }
        if segment.codes.is_some() {
            state.set_at(code, context);
        }
        // Step 3: the underlying plane's non-spacing attributes. A flash
        // placed here starts its phases here.
        state.apply(&slot.underlying);
        // Step 4: Adaptive overlays; the later invoked object's applies
        // where ranges overlap.
        let mut top: Option<(u16, AttrChange)> = None;
        for overlay in overlays.as_mut_slice() {
            let range = overlay.range;
            if column < usize::from(range.first) || column > usize::from(range.last) {
                continue;
            }
            if slot.adaptive_owner == Some(range.slot) {
                overlay.change = overlay.change.then(slot.adaptive_change);
            }
            if top.is_none_or(|(slot, _)| range.slot > slot) {
                top = Some((range.slot, overlay.change));
            }
        }
        let underlying = state.shown();
        let plane_attrs = top.map_or(underlying, |(_, change)| {
            let mut attrs = underlying;
            attrs.apply(&change);
            attrs
        });
        // A 0 underline bit of the overlay also clears Level 1 separated.
        let level_1_separated =
            state.separated && top.is_none_or(|(_, change)| change.display.is_none_or(|display| display.underline));
        // Step 6: the character of the highest priority (clause 13.2).
        let row_sets = state.designation.unwrap_or(context.default_sets);
        let enhancement = slot
            .passive
            .map(|(character, sets, attrs)| (character, sets.unwrap_or(context.default_sets), attrs, Plane::Passive))
            .or(slot.adaptive.map(|(character, sets)| {
                (character, sets.unwrap_or(context.default_sets), plane_attrs, Plane::Adaptive)
            }))
            .or(slot.local.map(|character| (character, row_sets, plane_attrs, Plane::Local)))
            .or(slot.active.map(|character| (character, row_sets, plane_attrs, Plane::Active)));
        let cell = match enhancement {
            Some((character, sets, attrs, plane)) => {
                let (glyph, undefined) = enhancement_glyph(character, sets, attrs.underline);
                if undefined {
                    report.faults.undefined_characters = report.faults.undefined_characters.saturating_add(1);
                }
                Some(cell_of(glyph, &attrs, plane.source(), column))
            }
            None if segment.lower.is_some() => None,
            None => {
                let separated = level_1_separated || plane_attrs.underline;
                let (glyph, held) = if segment.codes.is_some() {
                    level_1_glyph(&state, code, separated, context)
                } else {
                    (Glyph::Space, false)
                };
                let mut resolved = cell_of(glyph, &plane_attrs, CharSource::Level1, column);
                resolved.cell.held = held;
                resolved.cell.parity_failed = segment
                    .codes
                    .is_some_and(|(_, failed)| failed.checked_shr(column as u32).is_some_and(|bits| bits & 1 == 1));
                Some(resolved)
            }
        };
        if let Some(slot) = resolved.get_mut(column) {
            *slot = cell;
        }
        // Step 7: the held mosaic, from Level 1 characters only (decision
        // E.50), in the form shown at this cell.
        if segment.codes.is_some() {
            if state.mosaic && code > ATTRIBUTE_LAST && code & MOSAIC_BIT5 != 0 {
                state.held = Some((mosaic_pattern(code), level_1_separated || plane_attrs.underline));
            }
            pending = Some(code);
        }
    }
    state
}

/// What a Level 1 byte shows (REQ-FSM-006 step 2): a space or the held
/// mosaic for an attribute, a mosaic, or a character of the Level 1 G0 set
/// with its national option.
fn level_1_glyph(state: &RowState, code: u8, separated: bool, context: &RowContext) -> (Glyph, bool) {
    if code <= ATTRIBUTE_LAST {
        if state.hold
            && state.mosaic
            && let Some((pattern, separated)) = state.held
        {
            return (Glyph::Mosaic { pattern, separated }, true);
        }
        return (Glyph::Space, false);
    }
    if state.mosaic && code & MOSAIC_BIT5 != 0 {
        return (Glyph::Mosaic { pattern: mosaic_pattern(code), separated }, false);
    }
    let g0 = if state.second_g0 { context.second_g0.unwrap_or(context.level_1_g0) } else { context.level_1_g0 };
    (Glyph::Char(CharRef::new(CharSet::G0(g0), code)), false)
}

impl Rows<'_, '_> {
    /// The inputs of one row's resolution.
    fn context(&self, row: usize) -> RowContext {
        RowContext {
            remapping: self.presentation.remapping,
            substitution: self.presentation.black_background_substitution,
            row_colour: self.row_colours.get(row).copied().unwrap_or(COLOUR_BLACK),
            boxed_page: self.boxed_page,
            double_start_box: self.config.double_start_box,
            double_allowed: (DOUBLE_HEIGHT_FIRST_ROW..=DOUBLE_HEIGHT_LAST_ROW).contains(&row),
            default_sets: self.inputs.default_sets,
            level_1_g0: self.level_1_g0,
            second_g0: self.second_g0,
        }
    }

    /// Resolves and places one row and its side panels (REQ-FSM-012).
    /// `lower` is true for the row below spacing double height, whose
    /// cells without enhancement characters show `fill`. Returns true when
    /// a spacing double height or double size code acted in the row.
    pub(super) fn row(
        &self,
        row: usize,
        lower: bool,
        fill: &[Cell; COLUMNS],
        cells: &mut RowCells,
        report: &mut EnhancementReport,
    ) -> bool {
        let mut events = Collector::new(Some(row), self.last_colour_row);
        if self.with_events.checked_shr(row as u32).is_some_and(|bits| bits & 1 == 1) {
            let mut walker = Walker::new(self.inputs);
            // Only the page-level run is traced: every run makes the same
            // transitions, and each row's run repeats them.
            walker.walk(&mut events, None);
            report.steps = report.steps.saturating_add(walker.report.steps);
        }
        let context = self.context(row);
        let (codes, failed) =
            row_codes(row, Some(self.inputs.page), self.config, self.options, self.config.parity_policy);
        let main = Segment {
            first: 0,
            end: COLUMNS,
            codes: (!lower).then_some((&codes, failed)),
            lower: lower.then_some(&fill[..]),
        };
        let state = resolve_segment(&main, &context, RowState::start(&context), &events, &mut cells.resolved, report);
        cells.place(0, COLUMNS, context.double_allowed);
        // REQ-L25-041: each panel starts from the start-of-row defaults with
        // the row colour as its background, and no attribute crosses into or
        // out of it.
        if row >= SIDE_PANEL_FIRST_ROW {
            let panels = self.inputs.panels;
            let panel_start = RowState::start(&RowContext { substitution: true, ..context });
            let right = (COLUMNS, COLUMNS.saturating_add(usize::from(panels.right)));
            let left = (panels.left_first_column(), SCREEN_COLUMNS);
            for (first, end) in [right, left] {
                if first < end {
                    let segment = Segment { first, end, codes: None, lower: None };
                    let _ = resolve_segment(&segment, &context, panel_start, &events, &mut cells.resolved, report);
                    cells.place(first, end, context.double_allowed);
                }
            }
        }
        state.spacing_double
    }
}
