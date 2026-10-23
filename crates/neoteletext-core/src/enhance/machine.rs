//! The machine of the enhancement walk (REQ-FSM-011): its states, events
//! and actions, the transition table and the names it is traced with.

use crate::acq::EnhancementFaults;
use crate::consts::ROW_ADDRESS_LAST;
use crate::objects::row_of;
use crate::packet::{ColourData, ObjectKind, ObjectNumber, ObjectSource, TripletFunction};

/// States of the enhancement walk (REQ-FSM-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkState {
    /// Before the walk.
    Start,
    /// Taking the triplets of the current source.
    Walking,
    /// An Origin Modifier waits for the invocation that follows it.
    ModifierPending {
        /// The row offset, the modifier's address minus 40.
        row_offset: u8,
        /// The column offset, its data.
        column_offset: u8,
    },
    /// The current source has ended.
    Returning,
    /// Placing the default objects of REQ-OBJ-040.
    PlacingDefaults,
    /// The walk is over.
    Finished,
}

/// The source that ended, as the Returning state is told it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ended {
    /// The page's local enhancement data.
    Local,
    /// An object invoked by another source.
    Object,
    /// A default object.
    Default,
}

/// The events of the walk (REQ-FSM-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkEvent {
    /// The walk begins; `display` when the page has a packet X/26 with a
    /// display triplet (decision E.53).
    Begin {
        /// The page's local data displays something.
        display: bool,
    },
    /// The next triplet of the current source; `None` when it failed its
    /// Hamming 24/18 check.
    Triplet(Option<TripletFunction>),
    /// The current source has no more data.
    EndOfData,
    /// A source ended.
    Returned(Ended),
    /// The next default object was looked for: `found` when it was found and
    /// accepted.
    DefaultObject {
        /// A default object is ready to run.
        found: bool,
    },
}

/// What a transition of the walk does (REQ-FSM-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkAction {
    /// Nothing.
    None,
    /// The source becomes the local enhancement data at row 0, column 0,
    /// with no Origin Modifier.
    BeginLocal,
    /// The source becomes the default object found, at row 0, column 0.
    StartDefault,
    /// Skip a damaged triplet; the position does not move (decision E.46).
    Skip,
    /// End the current source.
    EndSource,
    /// Pop the walk record: the invoker's position and origin return, and
    /// the Origin Modifier is cancelled (REQ-OBJ-002).
    Pop,
    /// After a default object, the position is row 0, column 0 again.
    ResetPosition,
    /// Full Screen Colour (REQ-L25-032).
    ScreenColour(ColourData),
    /// Full Row Colour or Address Display Row 0: the position becomes `row`,
    /// column 0, and the row colour is set (REQ-L25-033).
    RowColour {
        /// The row, relative to the source's origin.
        row: u8,
        /// The colour and row selection.
        colour: ColourData,
    },
    /// Set Active Position (REQ-L25-003).
    Position {
        /// The row.
        row: u8,
        /// The column, kept when 40 or more.
        column: u8,
    },
    /// An Object Invocation with the offsets of an Origin Modifier that
    /// immediately preceded it, or none (REQ-OBJ-001, REQ-OBJ-002).
    Invoke {
        /// The source.
        source: ObjectSource,
        /// The type.
        kind: ObjectKind,
        /// The number.
        number: ObjectNumber,
        /// The row offset.
        row_offset: u8,
        /// The column offset.
        column_offset: u8,
    },
    /// A column triplet: its column, then its character or attribute
    /// (REQ-L25-004, REQ-L25-010, REQ-L25-020).
    Column(TripletFunction),
}

/// The transition of the walk for one triplet in the Walking state, and in
/// Modifier pending for anything but an invocation (REQ-FSM-011).
const fn triplet_transition(function: TripletFunction) -> (WalkState, WalkAction) {
    use TripletFunction as F;
    let walking = WalkState::Walking;
    match function {
        // Decision E.46: any Termination Marker, and an Object Definition,
        // end the source.
        F::Termination { .. } | F::ObjectDefinition { .. } => (WalkState::Returning, WalkAction::EndSource),
        F::ObjectInvocation { source, kind, number } => {
            (walking, WalkAction::Invoke { source, kind, number, row_offset: 0, column_offset: 0 })
        }
        F::OriginModifier { row_offset, column_offset } => {
            (WalkState::ModifierPending { row_offset, column_offset }, WalkAction::None)
        }
        F::FullScreenColour(colour) => (walking, WalkAction::ScreenColour(colour)),
        F::FullRowColour { address, colour } => (walking, WalkAction::RowColour { row: row_of(address), colour }),
        F::AddressDisplayRow0 { address, colour } => {
            if address == ROW_ADDRESS_LAST {
                (walking, WalkAction::RowColour { row: 0, colour })
            } else {
                // Addresses 40 to 62 are reserved: no move (REQ-L25-003).
                (walking, WalkAction::None)
            }
        }
        F::SetActivePosition { address, column } => (walking, WalkAction::Position { row: row_of(address), column }),
        // REQ-L25-052: Programme Delivery Control and reserved triplets do
        // nothing, and do not move the position.
        F::ProgrammeDeliveryRow { .. }
        | F::ReservedRow { .. }
        | F::ProgrammeDeliveryColumn { .. }
        | F::ReservedColumn { .. } => (walking, WalkAction::None),
        F::ForegroundColour { .. }
        | F::G1Character { .. }
        | F::G3Character { .. }
        | F::BackgroundColour { .. }
        | F::AdditionalFlash { .. }
        | F::ModifiedDesignation { .. }
        | F::G0Character { .. }
        | F::DisplayAttributes { .. }
        | F::UnsupportedColumn { .. }
        | F::G2Character { .. }
        | F::G0Diacritic { .. } => (walking, WalkAction::Column(function)),
    }
}

/// The transition function of the enhancement walk (REQ-FSM-011,
/// REQ-FSM-001): the next state and the action for a state and an event.
/// Events that cannot arrive in a state leave it unchanged, so that no
/// input can drive the machine astray (REQ-FSM-008).
#[must_use]
pub const fn walk_transition(state: WalkState, event: WalkEvent) -> (WalkState, WalkAction) {
    use WalkEvent as E;
    use WalkState as S;
    match (state, event) {
        (S::Start, E::Begin { display: true }) => (S::Walking, WalkAction::BeginLocal),
        (S::Start, E::Begin { display: false }) => (S::PlacingDefaults, WalkAction::None),
        // Decision E.46: a damaged triplet is skipped; one after an Origin
        // Modifier is not the invocation it needs, so the offsets go.
        (S::Walking | S::ModifierPending { .. }, E::Triplet(None)) => (S::Walking, WalkAction::Skip),
        (
            S::ModifierPending { row_offset, column_offset },
            E::Triplet(Some(TripletFunction::ObjectInvocation { source, kind, number })),
        ) => (S::Walking, WalkAction::Invoke { source, kind, number, row_offset, column_offset }),
        (S::Walking | S::ModifierPending { .. }, E::Triplet(Some(function))) => triplet_transition(function),
        (S::Walking | S::ModifierPending { .. }, E::EndOfData) => (S::Returning, WalkAction::EndSource),
        (S::Returning, E::Returned(Ended::Object)) => (S::Walking, WalkAction::Pop),
        (S::Returning, E::Returned(Ended::Local)) | (S::PlacingDefaults, E::DefaultObject { found: false }) => {
            (S::Finished, WalkAction::None)
        }
        (S::Returning, E::Returned(Ended::Default)) => (S::PlacingDefaults, WalkAction::ResetPosition),
        (S::PlacingDefaults, E::DefaultObject { found: true }) => (S::Walking, WalkAction::StartDefault),
        (S::Start, E::Triplet(_) | E::EndOfData | E::Returned(_) | E::DefaultObject { .. })
        | (S::Walking | S::ModifierPending { .. }, E::Begin { .. } | E::Returned(_) | E::DefaultObject { .. })
        | (S::Returning, E::Begin { .. } | E::Triplet(_) | E::EndOfData | E::DefaultObject { .. })
        | (S::PlacingDefaults, E::Begin { .. } | E::Triplet(_) | E::EndOfData | E::Returned(_))
        | (S::Finished, E::Begin { .. } | E::Triplet(_) | E::EndOfData | E::Returned(_) | E::DefaultObject { .. }) => {
            (state, WalkAction::None)
        }
    }
}

/// Names for the transition trace (REQ-FSM-010), as the table of
/// REQ-FSM-011 names the states.
#[must_use]
pub const fn walk_state_name(state: WalkState) -> &'static str {
    match state {
        WalkState::Start => "Start",
        WalkState::Walking => "Walking",
        WalkState::ModifierPending { .. } => "Modifier pending",
        WalkState::Returning => "Returning",
        WalkState::PlacingDefaults => "Placing default objects",
        WalkState::Finished => "Finished",
    }
}

/// Names for the transition trace; a triplet is named by its function.
#[must_use]
pub const fn walk_event_name(event: WalkEvent) -> &'static str {
    match event {
        WalkEvent::Begin { display: true } => "walk begins, with a display triplet",
        WalkEvent::Begin { display: false } => "walk begins, with no display triplet",
        WalkEvent::Triplet(Some(function)) => function.name(),
        WalkEvent::Triplet(None) => "damaged triplet",
        WalkEvent::EndOfData => "end of the source's data",
        WalkEvent::Returned(Ended::Local) => "the local enhancement data ended",
        WalkEvent::Returned(Ended::Object) => "an object ended",
        WalkEvent::Returned(Ended::Default) => "a default object ended",
        WalkEvent::DefaultObject { found: true } => "next default object found",
        WalkEvent::DefaultObject { found: false } => "no more default objects",
    }
}

/// Names for the transition trace.
#[must_use]
pub const fn walk_action_name(action: WalkAction) -> &'static str {
    match action {
        WalkAction::None => "none",
        WalkAction::BeginLocal => "begin the local enhancement data",
        WalkAction::StartDefault => "start the default object",
        WalkAction::Skip => "skip the damaged triplet",
        WalkAction::EndSource => "end the source",
        WalkAction::Pop => "pop the walk record",
        WalkAction::ResetPosition => "active position to row 0, column 0",
        WalkAction::ScreenColour(_) => "screen colour",
        WalkAction::RowColour { .. } => "move to the row; row colour",
        WalkAction::Position { .. } => "set the active position",
        WalkAction::Invoke { .. } => "invoke the object",
        WalkAction::Column(_) => "set the column; place the character or attribute",
    }
}

/// The trace's action for a transition whose step met a fault
/// (REQ-L25-051): the fault, in place of the action that met it. A step
/// meets at most one fault of the walk, except in Placing default objects,
/// where finding the next default object may pass over several.
pub(super) fn traced_action(
    state: WalkState,
    action: WalkAction,
    before: &EnhancementFaults,
    after: &EnhancementFaults,
) -> &'static str {
    let fault = if after.forbidden_invocations != before.forbidden_invocations {
        "skip the invocation: not permitted (REQ-OBJ-008)"
    } else if after.limited_invocations != before.limited_invocations {
        "skip the invocation: a limit of Annex A.8 is reached"
    } else if after.absent_objects != before.absent_objects {
        "skip the invocation: the object is absent"
    } else if after.discarded_writes != before.discarded_writes {
        "discard the write (REQ-OBJ-014, REQ-L25-050)"
    } else if after.ignored_designations != before.ignored_designations {
        "ignore the designation (REQ-L25-012)"
    } else {
        return walk_action_name(action);
    };
    match (state, action) {
        (WalkState::PlacingDefaults, WalkAction::StartDefault) => {
            "start the default object, past default objects absent or over a limit"
        }
        (WalkState::PlacingDefaults, _) => "none: the default objects left are absent or over a limit",
        _ => fault,
    }
}
