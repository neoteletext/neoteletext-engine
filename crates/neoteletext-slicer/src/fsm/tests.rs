//! The REQ-FSM-002 table: every state and event pair has one row.

use super::{SlicerAction as A, SlicerEvent as E, SlicerState as S, slicer_transition};

const ALL_STATES: [S; 9] = [
    S::Start,
    S::SearchingRunIn,
    S::Measuring,
    S::GateCheck,
    S::LookingForFramingCode,
    S::CollectingBytes,
    S::CheckingAddress,
    S::Accepted,
    S::Rejected,
];
const ALL_EVENTS: [E; 12] = [
    E::LineReceived,
    E::RunInFound,
    E::WindowExhausted,
    E::Estimated,
    E::GatePassed,
    E::BelowGate,
    E::FramingExact,
    E::FramingOneBitError,
    E::FramingNotFound,
    E::BitsRead,
    E::AddressValid,
    E::AddressUncorrectable,
];

/// The REQ-FSM-002 table, row by row.
const TABLE: [(S, E, A, S); 12] = [
    (S::Start, E::LineReceived, A::EstimateBlackLevel, S::SearchingRunIn),
    (S::SearchingRunIn, E::RunInFound, A::RecordPosition, S::Measuring),
    (S::SearchingRunIn, E::WindowExhausted, A::CountNoSignal, S::Rejected),
    (S::Measuring, E::Estimated, A::None, S::GateCheck),
    (S::GateCheck, E::GatePassed, A::HoldThreshold, S::LookingForFramingCode),
    (S::GateCheck, E::BelowGate, A::CountBelowGate, S::Rejected),
    (S::LookingForFramingCode, E::FramingExact, A::None, S::CollectingBytes),
    (S::LookingForFramingCode, E::FramingOneBitError, A::MarkFramingCorrected, S::CollectingBytes),
    (S::LookingForFramingCode, E::FramingNotFound, A::CountNoFramingCode, S::Rejected),
    (S::CollectingBytes, E::BitsRead, A::AssembleBytes, S::CheckingAddress),
    (S::CheckingAddress, E::AddressValid, A::OutputPacket, S::Accepted),
    (S::CheckingAddress, E::AddressUncorrectable, A::CountBadAddress, S::Rejected),
];

/// Every table row exists in the code, and every other combination is a
/// "no change" arm (REQ-FSM-001: no transitions beyond the table).
#[test]
fn transition_function_is_exactly_the_table() {
    for state in ALL_STATES {
        for event in ALL_EVENTS {
            let got = slicer_transition(state, event);
            let row = TABLE.iter().find(|(s, e, _, _)| *s == state && *e == event);
            match row {
                Some(&(_, _, action, next)) => assert_eq!(got, (next, action), "{state:?} {event:?}"),
                None => assert_eq!(got, (state, A::None), "{state:?} {event:?} is not in the table"),
            }
        }
    }
}
