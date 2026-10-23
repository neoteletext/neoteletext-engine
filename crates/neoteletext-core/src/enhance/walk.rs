//! The walk of REQ-FSM-011: one run over the local enhancement data, the
//! objects it invokes and the default objects, collecting the events that
//! fall on the rows asked for.

use super::{Collector, Frame, Inputs, Range, Slot, Walker};

use super::attributes::Attrs;
use super::machine::{
    Ended, WalkAction, WalkEvent, WalkState, traced_action, walk_event_name, walk_state_name, walk_transition,
};
use super::planes::{Area, Plane};
use crate::acq::{EnhancementFaults, EnhancementReport, StoredPage};
use crate::consts::{
    COLUMNS, LOCAL_TRIPLETS, OBJECT_TYPES, ROW_COLOUR_DOWN_TO_BOTTOM, ROW_COLOUR_THIS_ROW, ROWS, SCREEN_COLUMNS,
    TRIPLETS_PER_PACKET, WALK_DEPTH,
};
use crate::objects::{Body, Fetch};
use crate::packet::{ColourData, TripletFunction};
use crate::trace::{Machine, TraceSink};
use crate::types::FixedVec;

mod invocation;
mod placement;

impl Frame<'_> {
    /// The local enhancement data at row 0, column 0 (REQ-L25-003).
    const fn local() -> Self {
        Self {
            plane: Plane::Local,
            body: Body::Local,
            cursor: 0,
            end: LOCAL_TRIPLETS,
            local_data: true,
            origin: (0, 0),
            row: 0,
            column: 0,
            area: Area::Main,
            slot: 0,
            designation: None,
            passive: Attrs::PASSIVE,
        }
    }

    /// The absolute active position.
    fn position(&self) -> (usize, usize) {
        (self.origin.0.saturating_add(usize::from(self.row)), self.origin.1.saturating_add(usize::from(self.column)))
    }
}

impl Collector {
    pub(super) fn new(row: Option<usize>, last_row: usize) -> Self {
        Self {
            row,
            last_row,
            screen: None,
            row_colours: [None; ROWS],
            event_rows: 0,
            slots: [Slot::default(); SCREEN_COLUMNS],
            ranges: FixedVec::new(Range { slot: 0, first: 0, last: 0 }),
        }
    }

    /// The slot of a cell of the row being resolved.
    fn slot(&mut self, row: usize, column: usize) -> Option<&mut Slot> {
        if self.row != Some(row) {
            self.event_rows |= 1u32.checked_shl(row as u32).unwrap_or(0);
            return None;
        }
        self.slots.get_mut(column)
    }

    /// REQ-L25-033, decision E.45: commands apply in transmission order.
    fn row_colour(&mut self, row: usize, colour: ColourData) {
        if self.row.is_some() {
            return;
        }
        let last = match colour.high {
            ROW_COLOUR_THIS_ROW => row,
            ROW_COLOUR_DOWN_TO_BOTTOM => self.last_row,
            // Decision E.45: the reserved selections set no colour.
            _ => return,
        };
        for slot in self.row_colours.iter_mut().take(last.saturating_add(1)).skip(row) {
            *slot = Some(colour.index);
        }
    }

    /// An Adaptive object addresses a cell, for its range on the row.
    fn touch(&mut self, row: usize, column: usize, slot: u16) {
        if self.row != Some(row) {
            return;
        }
        let column = column as u8;
        if let Some(range) = self.ranges.as_mut_slice().iter_mut().find(|range| range.slot == slot) {
            range.first = range.first.min(column);
            range.last = range.last.max(column);
        } else {
            // There is a range for every Adaptive invocation the limits allow.
            let _ = self.ranges.push(Range { slot, first: column, last: column });
        }
    }
}

impl<'a, 's> Walker<'a, 's> {
    pub(super) fn new(inputs: &'a Inputs<'a, 's>) -> Self {
        Self {
            inputs,
            invoked: [0; OBJECT_TYPES],
            open: FixedVec::new((0, 0)),
            successive: 0,
            next_slot: 1,
            extra_pair: None,
            report: EnhancementReport::new(inputs.build, inputs.level),
        }
    }

    /// Runs the machine of REQ-FSM-011 to the end: the local enhancement
    /// data when it displays anything, otherwise the default objects, with
    /// every object they invoke on an explicit stack of walk records at most
    /// [`WALK_DEPTH`] deep (REQ-GEN-016). Each loop reads one triplet, ends
    /// one source or places one default object, so the loop is bounded by
    /// the data and the limits.
    ///
    /// With `trace`, every transition is reported to it (REQ-FSM-010). The
    /// one test of `trace` is made once for the run, not once for each
    /// transition: the walk without tracing is a separate copy, in which
    /// nothing of the trace remains.
    pub(super) fn walk(&mut self, out: &mut Collector, trace: Option<&mut TraceSink<'_>>) {
        match trace {
            None => self.run::<false>(out, None),
            Some(sink) => self.run::<true>(out, Some(sink)),
        }
    }

    /// The walk of [`Walker::walk`]; `TRACED` says whether `trace` is used.
    fn run<const TRACED: bool>(&mut self, out: &mut Collector, mut trace: Option<&mut TraceSink<'_>>) {
        let mut stack: FixedVec<Frame<'s>, WALK_DEPTH> = FixedVec::new(Frame::local());
        // Without objects compiled in, no default object is placed
        // (decision E.71).
        let mut defaults =
            self.inputs.links.default_objects.into_iter().flatten().filter(|_| cfg!(feature = "objects"));
        let mut found: Option<Frame<'s>> = None;
        let mut state = WalkState::Start;
        loop {
            // The faults before the step, to name one it meets.
            let before = if TRACED { self.report.faults } else { EnhancementFaults::NONE };
            let event = match state {
                WalkState::Start => WalkEvent::Begin { display: has_display_triplets(self.inputs.page) },
                WalkState::Walking | WalkState::ModifierPending { .. } => self.next_triplet(&mut stack),
                WalkState::Returning => {
                    let ended = match stack.as_slice() {
                        [_, _, ..] => Ended::Object,
                        [frame] if frame.local_data => Ended::Local,
                        _ => Ended::Default,
                    };
                    WalkEvent::Returned(ended)
                }
                WalkState::PlacingDefaults => {
                    found = defaults.find_map(|default| self.default_object(default));
                    WalkEvent::DefaultObject { found: found.is_some() }
                }
                WalkState::Finished => return,
            };
            let (next, action) = walk_transition(state, event);
            self.act(action, &mut stack, &mut found, out);
            if TRACED && let Some(sink) = trace.as_deref_mut() {
                let action = traced_action(state, action, &before, &self.report.faults);
                let (from, event, to) = (walk_state_name(state), walk_event_name(event), walk_state_name(next));
                sink.transition(Machine::EnhancementWalk, from, event, action, to);
            }
            state = next;
        }
    }

    /// The next event of the current source: its next triplet, or the end
    /// of its data. The local enhancement data skips missing designation
    /// codes (REQ-L25-002).
    fn next_triplet(&mut self, stack: &mut FixedVec<Frame<'s>, WALK_DEPTH>) -> WalkEvent {
        let Some(frame) = stack.as_mut_slice().last_mut() else { return WalkEvent::EndOfData };
        while frame.cursor < frame.end {
            let index = frame.cursor;
            frame.cursor = frame.cursor.saturating_add(1);
            self.report.steps = self.report.steps.saturating_add(1);
            match frame.body.fetch(self.inputs.page, index) {
                Fetch::Triplet(triplet) => return WalkEvent::Triplet(Some(triplet.function())),
                Fetch::Damaged => return WalkEvent::Triplet(None),
                Fetch::Missing if frame.local_data => {
                    let per_packet = TRIPLETS_PER_PACKET as u16;
                    let next =
                        index.saturating_add(per_packet).saturating_sub(index.checked_rem(per_packet).unwrap_or(0));
                    frame.cursor = frame.cursor.max(next);
                }
                // An object's scan found every packet it needs.
                Fetch::Missing => break,
            }
        }
        WalkEvent::EndOfData
    }

    /// Performs the action of a transition.
    fn act(
        &mut self,
        action: WalkAction,
        stack: &mut FixedVec<Frame<'s>, WALK_DEPTH>,
        found: &mut Option<Frame<'s>>,
        out: &mut Collector,
    ) {
        match action {
            WalkAction::None => {}
            WalkAction::BeginLocal => {
                stack.clear();
                let _ = stack.push(Frame::local());
            }
            WalkAction::StartDefault => {
                stack.clear();
                if let Some(frame) = found.take() {
                    let _ = stack.push(frame);
                }
            }
            // Popping restores the invoker's position and sub-tables, which
            // are in its own record (REQ-OBJ-002); a default object's record
            // goes, leaving row 0, column 0 (clause 13.8).
            WalkAction::Pop | WalkAction::ResetPosition => {
                let _ = stack.pop();
            }
            WalkAction::Skip => {
                self.report.faults.damaged_triplets = self.report.faults.damaged_triplets.saturating_add(1);
            }
            WalkAction::EndSource => {
                if let Some(frame) = stack.as_mut_slice().last_mut() {
                    frame.cursor = frame.end;
                }
            }
            // Without objects compiled in, an invocation is ignored
            // (decision E.71).
            WalkAction::Invoke { .. } if !cfg!(feature = "objects") => {}
            WalkAction::Invoke { source, kind, number, row_offset, column_offset } => {
                let child = stack
                    .as_slice()
                    .last()
                    .and_then(|frame| self.invoke(frame, source, kind, number, (row_offset, column_offset)));
                if let Some(child) = child {
                    // The nesting rules bound the depth (decision E.51).
                    let _ = stack.push(child);
                }
            }
            WalkAction::ScreenColour(_)
            | WalkAction::RowColour { .. }
            | WalkAction::Position { .. }
            | WalkAction::Column(_) => {
                if let Some(frame) = stack.as_mut_slice().last_mut() {
                    self.act_on_frame(action, frame, out);
                }
            }
        }
    }

    /// The actions that change the current source's record or emit events.
    fn act_on_frame(&mut self, action: WalkAction, frame: &mut Frame<'s>, out: &mut Collector) {
        // REQ-OBJ-013: Full Screen Colour, Full Row Colour and Address
        // Display Row 0 act only in local data and Active objects, and the
        // colours only in the main page.
        let restricted = !frame.plane.sets_colours();
        match action {
            WalkAction::ScreenColour(colour) => {
                if !restricted && frame.area == Area::Main && colour.is_defined() && out.row.is_none() {
                    out.screen = Some(colour.index);
                }
            }
            WalkAction::RowColour { row, colour } if !restricted => {
                (frame.row, frame.column) = (row, 0);
                let (row, _) = frame.position();
                if frame.area == Area::Main && row < ROWS {
                    out.row_colour(row, colour);
                }
            }
            WalkAction::Position { row, column } => {
                frame.row = row;
                // Decision E.46: a column of 40 or more leaves the column.
                if usize::from(column) < COLUMNS {
                    frame.column = column;
                }
            }
            WalkAction::Column(function) => self.column_triplet(frame, function, out),
            WalkAction::RowColour { .. }
            | WalkAction::None
            | WalkAction::BeginLocal
            | WalkAction::StartDefault
            | WalkAction::Pop
            | WalkAction::ResetPosition
            | WalkAction::Skip
            | WalkAction::EndSource
            | WalkAction::Invoke { .. } => {}
        }
    }
}

/// True for a triplet that displays something: a character, an attribute,
/// a screen or row colour, or an object (decision E.53).
fn is_display(function: TripletFunction) -> bool {
    matches!(
        function,
        TripletFunction::FullScreenColour(_)
            | TripletFunction::FullRowColour { .. }
            | TripletFunction::AddressDisplayRow0 { .. }
            | TripletFunction::ObjectInvocation { .. }
    ) || function.active_column().is_some()
}

/// True when the page's local enhancement data displays anything, so that
/// default objects are not placed (REQ-OBJ-040, decision E.53).
fn has_display_triplets(page: &StoredPage) -> bool {
    for index in 0..LOCAL_TRIPLETS {
        if let Fetch::Triplet(triplet) = Body::Local.fetch(page, index) {
            let function = triplet.function();
            if matches!(function, TripletFunction::Termination { .. } | TripletFunction::ObjectDefinition { .. }) {
                return false;
            }
            if is_display(function) {
                return true;
            }
        }
    }
    false
}
