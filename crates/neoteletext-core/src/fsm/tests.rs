//! Tests of the magazine acquisition and page lifecycle machines
//! (REQ-FSM-003, REQ-FSM-004).
#![allow(clippy::unwrap_used)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::types::{PageAddress, SubCode};
use MagazineAction as A;
use MagazineEvent as E;
use MagazineState as S;

fn key(mpp: u16) -> PageKey {
    PageKey { address: PageAddress::from_mpp(mpp).unwrap(), subcode: SubCode::new(0) }
}

/// REQ-FSM-009: one test per table row of REQ-FSM-003.
#[test]
fn magazine_table_rows() {
    let p = key(0x100);
    let q = key(0x101);
    let rows: [(S, E, S, A); 15] = [
        (S::Idle, E::StorableHeader { key: p, erase: true }, S::Receiving(p), A::Start { key: p, erase: true }),
        (S::Idle, E::NonStorableHeader, S::Absorbing, A::None),
        (S::Idle, E::Row, S::Idle, A::DiscardWithoutHeader),
        (S::Idle, E::HeaderOfOtherMagazineSerial, S::Idle, A::None),
        (S::Receiving(p), E::Row, S::Receiving(p), A::Store(p)),
        (S::Receiving(p), E::RowOfOtherMagazineSerial, S::Receiving(p), A::DiscardOtherMagazine),
        (
            S::Receiving(p),
            E::StorableHeader { key: q, erase: false },
            S::Receiving(q),
            A::EndAndStart { ended: p, started: q, erase: false },
        ),
        (S::Receiving(p), E::NonStorableHeader, S::Absorbing, A::End(p)),
        (S::Receiving(p), E::HeaderOfOtherMagazineSerial, S::Idle, A::End(p)),
        (S::Receiving(p), E::TimeFillingHeader, S::Idle, A::End(p)),
        (S::Receiving(p), E::BadHeader, S::Idle, A::End(p)),
        (S::Receiving(p), E::Timeout, S::Idle, A::End(p)),
        (S::Absorbing, E::Row, S::Absorbing, A::DiscardAbsorbed),
        (S::Absorbing, E::TimeFillingHeader, S::Idle, A::None),
        (S::Absorbing, E::Timeout, S::Idle, A::None),
    ];
    for (from, event, to, action) in rows {
        assert_eq!(magazine_transition(from, event), (to, action), "{from:?} {event:?}");
    }
}

/// Every magazine event, with `q` as the page of a storable header.
fn magazine_events(q: PageKey) -> [E; 10] {
    [
        E::StorableHeader { key: q, erase: false },
        E::StorableHeader { key: q, erase: true },
        E::NonStorableHeader,
        E::Row,
        E::RowOfOtherMagazineSerial,
        E::HeaderOfOtherMagazineSerial,
        E::TimeFillingHeader,
        E::BadHeader,
        E::Timeout,
        E::UnrelatedHeader,
    ]
}

/// REQ-FSM-003: Idle ignores every header it does not start a page
/// with, and discards rows of another magazine as out of sequence.
#[test]
fn magazine_idle_ignores_other_headers() {
    for event in [E::HeaderOfOtherMagazineSerial, E::TimeFillingHeader, E::BadHeader, E::Timeout, E::UnrelatedHeader] {
        assert_eq!(magazine_transition(S::Idle, event), (S::Idle, A::None), "{event:?}");
    }
    assert_eq!(magazine_transition(S::Idle, E::RowOfOtherMagazineSerial), (S::Idle, A::DiscardOtherMagazine));
    let p = key(0x100);
    assert_eq!(magazine_transition(S::Receiving(p), E::UnrelatedHeader), (S::Receiving(p), A::None));
}

/// REQ-FSM-003, the Absorbing row: a header that would end a page leads
/// to the same next state as from Receiving P, with the same actions
/// except "page completed".
#[test]
fn magazine_absorbing_follows_receiving_without_page_completed() {
    let p = key(0x100);
    let q = key(0x1FE);
    for event in magazine_events(q) {
        if matches!(event, E::Row | E::RowOfOtherMagazineSerial) {
            continue;
        }
        let (receiving_next, receiving_action) = magazine_transition(S::Receiving(p), event);
        let expected_next = if receiving_next == S::Receiving(p) { S::Absorbing } else { receiving_next };
        let expected_action = match receiving_action {
            A::End(ended) => {
                assert_eq!(ended, p);
                A::None
            }
            A::EndAndStart { ended, started, erase } => {
                assert_eq!(ended, p);
                A::Start { key: started, erase }
            }
            other => other,
        };
        assert_eq!(magazine_transition(S::Absorbing, event), (expected_next, expected_action), "{event:?}");
    }
    assert_eq!(magazine_transition(S::Absorbing, E::RowOfOtherMagazineSerial), (S::Absorbing, A::DiscardOtherMagazine));
}

/// REQ-FSM-010: the trace names each state, event and action in the
/// words of the REQ-FSM-003 table, and no two alike.
#[test]
fn magazine_trace_names() {
    let p = key(0x100);
    let states = [S::Idle, S::Receiving(p), S::Absorbing].map(magazine_state_name);
    assert_eq!(states, ["Idle", "Receiving", "Absorbing"]);
    let events = magazine_events(p).map(magazine_event_name);
    assert_eq!(
        events,
        [
            "storable header",
            "storable header",
            "non-storable header",
            "row",
            "row of other magazine (serial)",
            "header of other magazine (serial)",
            "time-filling header",
            "bad header",
            "timeout",
            "unrelated header",
        ]
    );
    let actions = [
        A::None,
        A::Start { key: p, erase: false },
        A::Store(p),
        A::DiscardWithoutHeader,
        A::DiscardOtherMagazine,
        A::DiscardAbsorbed,
        A::End(p),
        A::EndAndStart { ended: p, started: p, erase: true },
    ]
    .map(magazine_action_name);
    assert_eq!(
        actions,
        [
            "none",
            "start page",
            "store row",
            "discard: row without header",
            "discard: other magazine",
            "discard: absorbed",
            "end page",
            "end page and start another",
        ]
    );
    let distinct: std::collections::BTreeSet<&str> = actions.iter().chain(&events).chain(&states).copied().collect();
    // The two storable header events share a name; nothing else does.
    assert_eq!(distinct.len(), actions.len() + events.len() - 1 + states.len());
}

/// REQ-FSM-004: combinations the table does not list change nothing.
#[test]
fn page_unlisted_combinations_change_nothing() {
    use PageAction as PA;
    use PageEvent as PE;
    use PageState as PS;
    let unlisted = [
        (PS::Empty, PE::RowStored(5)),
        (PS::Empty, PE::ReceptionEnded),
        (PS::Empty, PE::UpdateIndicator),
        (PS::Empty, PE::NotReceived),
        (PS::Partial, PE::ReceptionStarted { erase: false }),
        (PS::Partial, PE::ReceptionStarted { erase: true }),
        (PS::Partial, PE::UpdateIndicator),
        (PS::Partial, PE::NotReceived),
        (PS::Complete, PE::RowStored(5)),
        (PS::Complete, PE::ReceptionEnded),
        (PS::Stale, PE::RowStored(5)),
        (PS::Stale, PE::ReceptionEnded),
        (PS::Stale, PE::UpdateIndicator),
        (PS::Stale, PE::NotReceived),
    ];
    for (state, event) in unlisted {
        assert_eq!(page_transition(state, event), (state, PA::None), "{state:?} {event:?}");
    }
    // The "Any" row: eviction from every state.
    for state in [PS::Empty, PS::Partial, PS::Complete, PS::Stale] {
        assert_eq!(page_transition(state, PE::Evicted), (PS::Empty, PA::Release), "{state:?}");
    }
}

/// REQ-FSM-010: trace names for the page lifecycle machine.
#[test]
fn page_trace_names() {
    use PageAction as PA;
    use PageEvent as PE;
    use PageState as PS;
    let states = [PS::Empty, PS::Partial, PS::Complete, PS::Stale].map(page_state_name);
    assert_eq!(states, ["Empty", "Partial", "Complete", "Stale"]);
    let events = [
        PE::ReceptionStarted { erase: false },
        PE::ReceptionStarted { erase: true },
        PE::RowStored(1),
        PE::ReceptionEnded,
        PE::UpdateIndicator,
        PE::NotReceived,
        PE::Evicted,
    ]
    .map(page_event_name);
    assert_eq!(
        events,
        [
            "reception started",
            "reception started with C4",
            "row stored",
            "reception ended",
            "update indicator",
            "not received",
            "evicted",
        ]
    );
    let actions = [
        PA::None,
        PA::Started,
        PA::MergeAndStart,
        PA::Erase,
        PA::Updated(1),
        PA::Completed,
        PA::UpdateIndicator,
        PA::Stale,
        PA::Release,
    ]
    .map(page_action_name);
    assert_eq!(
        actions,
        [
            "none",
            "raise page started",
            "merge and raise page started",
            "erase and raise page erased",
            "raise page updated",
            "raise page completed",
            "raise update indicator",
            "raise page stale",
            "release storage",
        ]
    );
    let distinct: std::collections::BTreeSet<&str> = actions.iter().chain(&events).chain(&states).copied().collect();
    assert_eq!(distinct.len(), actions.len() + events.len() + states.len());
}

/// REQ-FSM-009: one test per table row of REQ-FSM-004.
#[test]
fn page_table_rows() {
    use PageAction as PA;
    use PageEvent as PE;
    use PageState as PS;
    let rows = [
        (PS::Empty, PE::ReceptionStarted { erase: false }, PS::Partial, PA::Started),
        (PS::Empty, PE::ReceptionStarted { erase: true }, PS::Partial, PA::Started),
        (PS::Partial, PE::RowStored(3), PS::Partial, PA::Updated(3)),
        (PS::Partial, PE::ReceptionEnded, PS::Complete, PA::Completed),
        (PS::Complete, PE::ReceptionStarted { erase: false }, PS::Partial, PA::MergeAndStart),
        (PS::Complete, PE::ReceptionStarted { erase: true }, PS::Partial, PA::Erase),
        (PS::Complete, PE::UpdateIndicator, PS::Complete, PA::UpdateIndicator),
        (PS::Complete, PE::NotReceived, PS::Stale, PA::Stale),
        (PS::Stale, PE::ReceptionStarted { erase: false }, PS::Partial, PA::MergeAndStart),
        (PS::Stale, PE::ReceptionStarted { erase: true }, PS::Partial, PA::Erase),
        (PS::Partial, PE::Evicted, PS::Empty, PA::Release),
    ];
    for (from, event, to, action) in rows {
        assert_eq!(page_transition(from, event), (to, action), "{from:?} {event:?}");
    }
}
