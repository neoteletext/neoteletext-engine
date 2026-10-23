//! The line slicer state machine (REQ-FSM-002).
//!
//! One run of the machine processes one sampled line. The machine is an
//! enumeration of states and one transition function from state and event
//! to next state and action, with no wildcard arms (REQ-FSM-001). Every row
//! of the REQUIREMENTS.md table is one match arm; combinations the table
//! does not list cannot arise from the slicer and are explicit "no change"
//! arms.

/// States of the line slicer machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlicerState {
    /// A line has not been looked at yet.
    Start,
    /// Looking for the clock run-in (REQ-SLC-004).
    SearchingRunIn,
    /// Estimating levels, threshold and bit phase from the run-in.
    Measuring,
    /// Checking amplitude and match quality (REQ-SLC-005).
    GateCheck,
    /// Looking for the framing code (REQ-SLC-008).
    LookingForFramingCode,
    /// Reading the 336 packet bits (REQ-SLC-007, REQ-SLC-009).
    CollectingBytes,
    /// Checking the packet address bytes (REQ-SLC-010).
    CheckingAddress,
    /// A packet was output.
    Accepted,
    /// The line gave no packet.
    Rejected,
}

/// Events of the line slicer machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlicerEvent {
    /// A sampled line arrived.
    LineReceived,
    /// The run-in pattern was found within the search window.
    RunInFound,
    /// The search window was exhausted without finding the run-in.
    WindowExhausted,
    /// Levels, threshold and bit phase were estimated.
    Estimated,
    /// Amplitude and match quality are at or above the minimums.
    GatePassed,
    /// Amplitude or match quality is below its minimum.
    BelowGate,
    /// The exact framing code was found within ±2 bit periods.
    FramingExact,
    /// A framing code with one wrong bit was found, and single-bit tolerance
    /// is enabled (decision E.24).
    FramingOneBitError,
    /// No acceptable framing code was found.
    FramingNotFound,
    /// The 336 packet bits were read at bit centres.
    BitsRead,
    /// Both packet address bytes decode without an uncorrectable error.
    AddressValid,
    /// A packet address byte is uncorrectable.
    AddressUncorrectable,
}

/// Actions of the line slicer machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlicerAction {
    /// Nothing.
    None,
    /// Estimate the black level (REQ-SLC-003).
    EstimateBlackLevel,
    /// Record the run-in position.
    RecordPosition,
    /// Count "no signal".
    CountNoSignal,
    /// Hold the threshold for the rest of the line.
    HoldThreshold,
    /// Count "below gate".
    CountBelowGate,
    /// Mark "framing corrected".
    MarkFramingCorrected,
    /// Count "no framing code".
    CountNoFramingCode,
    /// Assemble 42 bytes from the bits.
    AssembleBytes,
    /// Output the packet with its quality data (REQ-SLC-012).
    OutputPacket,
    /// Count "bad address".
    CountBadAddress,
}

/// The transition function of the line slicer machine (REQ-FSM-002). One
/// arm per row of the table, then one "no change" arm per state for the
/// events the table does not list in it.
// Arms with equal bodies are kept apart so each matches one table row
// (REQ-FSM-001 traceability).
#[allow(clippy::match_same_arms, clippy::too_many_lines)]
#[must_use]
pub const fn slicer_transition(state: SlicerState, event: SlicerEvent) -> (SlicerState, SlicerAction) {
    use SlicerAction as A;
    use SlicerEvent as E;
    use SlicerState as S;
    match (state, event) {
        // Row 1.
        (S::Start, E::LineReceived) => (S::SearchingRunIn, A::EstimateBlackLevel),
        (
            S::Start,
            E::RunInFound
            | E::WindowExhausted
            | E::Estimated
            | E::GatePassed
            | E::BelowGate
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::BitsRead
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::Start, A::None),

        // Rows 2 and 3.
        (S::SearchingRunIn, E::RunInFound) => (S::Measuring, A::RecordPosition),
        (S::SearchingRunIn, E::WindowExhausted) => (S::Rejected, A::CountNoSignal),
        (
            S::SearchingRunIn,
            E::LineReceived
            | E::Estimated
            | E::GatePassed
            | E::BelowGate
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::BitsRead
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::SearchingRunIn, A::None),

        // Row 4.
        (S::Measuring, E::Estimated) => (S::GateCheck, A::None),
        (
            S::Measuring,
            E::LineReceived
            | E::RunInFound
            | E::WindowExhausted
            | E::GatePassed
            | E::BelowGate
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::BitsRead
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::Measuring, A::None),

        // Rows 5 and 6.
        (S::GateCheck, E::GatePassed) => (S::LookingForFramingCode, A::HoldThreshold),
        (S::GateCheck, E::BelowGate) => (S::Rejected, A::CountBelowGate),
        (
            S::GateCheck,
            E::LineReceived
            | E::RunInFound
            | E::WindowExhausted
            | E::Estimated
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::BitsRead
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::GateCheck, A::None),

        // Rows 7, 8 and 9.
        (S::LookingForFramingCode, E::FramingExact) => (S::CollectingBytes, A::None),
        (S::LookingForFramingCode, E::FramingOneBitError) => (S::CollectingBytes, A::MarkFramingCorrected),
        (S::LookingForFramingCode, E::FramingNotFound) => (S::Rejected, A::CountNoFramingCode),
        (
            S::LookingForFramingCode,
            E::LineReceived
            | E::RunInFound
            | E::WindowExhausted
            | E::Estimated
            | E::GatePassed
            | E::BelowGate
            | E::BitsRead
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::LookingForFramingCode, A::None),

        // Row 10.
        (S::CollectingBytes, E::BitsRead) => (S::CheckingAddress, A::AssembleBytes),
        (
            S::CollectingBytes,
            E::LineReceived
            | E::RunInFound
            | E::WindowExhausted
            | E::Estimated
            | E::GatePassed
            | E::BelowGate
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::CollectingBytes, A::None),

        // Rows 11 and 12.
        (S::CheckingAddress, E::AddressValid) => (S::Accepted, A::OutputPacket),
        (S::CheckingAddress, E::AddressUncorrectable) => (S::Rejected, A::CountBadAddress),
        (
            S::CheckingAddress,
            E::LineReceived
            | E::RunInFound
            | E::WindowExhausted
            | E::Estimated
            | E::GatePassed
            | E::BelowGate
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::BitsRead,
        ) => (S::CheckingAddress, A::None),

        // The final states: one run of the machine ends here.
        (
            S::Accepted,
            E::LineReceived
            | E::RunInFound
            | E::WindowExhausted
            | E::Estimated
            | E::GatePassed
            | E::BelowGate
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::BitsRead
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::Accepted, A::None),
        (
            S::Rejected,
            E::LineReceived
            | E::RunInFound
            | E::WindowExhausted
            | E::Estimated
            | E::GatePassed
            | E::BelowGate
            | E::FramingExact
            | E::FramingOneBitError
            | E::FramingNotFound
            | E::BitsRead
            | E::AddressValid
            | E::AddressUncorrectable,
        ) => (S::Rejected, A::None),
    }
}

impl SlicerState {
    /// The state's name for the transition trace (REQ-FSM-010).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::SearchingRunIn => "Searching for run-in",
            Self::Measuring => "Measuring",
            Self::GateCheck => "Gate check",
            Self::LookingForFramingCode => "Looking for framing code",
            Self::CollectingBytes => "Collecting bytes",
            Self::CheckingAddress => "Checking address",
            Self::Accepted => "Accepted",
            Self::Rejected => "Rejected",
        }
    }
}

impl SlicerEvent {
    /// The event's name for the transition trace (REQ-FSM-010).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LineReceived => "Line received",
            Self::RunInFound => "Run-in pattern found",
            Self::WindowExhausted => "Window exhausted",
            Self::Estimated => "Levels, threshold and bit phase estimated",
            Self::GatePassed => "At or above the minimums",
            Self::BelowGate => "Below either minimum",
            Self::FramingExact => "Exact framing code found",
            Self::FramingOneBitError => "Framing code with one wrong bit found",
            Self::FramingNotFound => "Framing code not found",
            Self::BitsRead => "336 bits read",
            Self::AddressValid => "Both address bytes decode",
            Self::AddressUncorrectable => "Address byte uncorrectable",
        }
    }
}

impl SlicerAction {
    /// The action's name for the transition trace (REQ-FSM-010).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::EstimateBlackLevel => "Estimate black level",
            Self::RecordPosition => "Record position",
            Self::CountNoSignal => "Count no signal",
            Self::HoldThreshold => "Hold threshold",
            Self::CountBelowGate => "Count below gate",
            Self::MarkFramingCorrected => "Mark framing corrected",
            Self::CountNoFramingCode => "Count no framing code",
            Self::AssembleBytes => "Assemble 42 bytes",
            Self::OutputPacket => "Output packet",
            Self::CountBadAddress => "Count bad address",
        }
    }
}

#[cfg(test)]
mod tests;
