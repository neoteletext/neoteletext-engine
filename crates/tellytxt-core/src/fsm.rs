//! The acquisition state machines of area FSM (layer L2).
//!
//! Each machine is an enumeration of states and one transition function from
//! state and event to next state and action, with no wildcard arms
//! (REQ-FSM-001). Every row of the REQUIREMENTS.md tables is a match arm;
//! combinations the tables do not list are explicit "no change" arms.

use crate::types::{PageAddress, SubCode};

/// The page a magazine is receiving: its address and sub-code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PageKey {
    /// The page address.
    pub address: PageAddress,
    /// The sub-code.
    pub subcode: SubCode,
}

/// States of the magazine acquisition machine (REQ-FSM-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MagazineState {
    /// No page is being received in this magazine.
    Idle,
    /// A page is being received and stored.
    Receiving(PageKey),
    /// A page that is not to be stored is being transmitted.
    Absorbing,
}

/// Events of the magazine acquisition machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MagazineEvent {
    /// A valid header for a storable page in this magazine.
    StorableHeader {
        /// The page it starts.
        key: PageKey,
        /// Its erase bit, C4.
        erase: bool,
    },
    /// A valid header for a page not to be stored, in this magazine.
    NonStorableHeader,
    /// A row packet 1 to 28 of this magazine.
    Row,
    /// A row packet of another magazine, in serial mode.
    RowOfOtherMagazineSerial,
    /// A header of another magazine, in serial mode.
    HeaderOfOtherMagazineSerial,
    /// A time-filling header (page FF) in this magazine, or in any magazine
    /// in serial mode.
    TimeFillingHeader,
    /// A header of this magazine with a valid address but uncorrectable page
    /// or control bytes (decision E.7).
    BadHeader,
    /// No packet for the page for the completion timeout (decision E.19).
    Timeout,
    /// Any header of another magazine in parallel mode; it does not concern
    /// this machine.
    UnrelatedHeader,
}

/// Actions of the magazine acquisition machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MagazineAction {
    /// Nothing.
    None,
    /// Start the page, erasing its rows first if `erase` is set.
    Start {
        /// The page.
        key: PageKey,
        /// Erase rows 1 to 28 (REQ-ACQ-004).
        erase: bool,
    },
    /// Store the row in the page being received.
    Store(PageKey),
    /// Discard the row and count it as a row without a header.
    DiscardWithoutHeader,
    /// Discard the row and count it as out of sequence (Annex B.3).
    DiscardOtherMagazine,
    /// Discard silently: the row belongs to an absorbed page.
    DiscardAbsorbed,
    /// End the page and raise "page completed".
    End(PageKey),
    /// End the page, raise "page completed", and start another.
    EndAndStart {
        /// The page that ends.
        ended: PageKey,
        /// The page that starts.
        started: PageKey,
        /// Erase the started page first.
        erase: bool,
    },
}

/// The transition function of the magazine acquisition machine
/// (REQ-FSM-003). One arm per row of the table.
// Arms with equal bodies are kept apart so each matches one table row
// (REQ-FSM-001 traceability).
#[allow(clippy::match_same_arms)]
#[must_use]
pub const fn magazine_transition(state: MagazineState, event: MagazineEvent) -> (MagazineState, MagazineAction) {
    use MagazineAction as A;
    use MagazineEvent as E;
    use MagazineState as S;
    match (state, event) {
        // Idle.
        (S::Idle, E::StorableHeader { key, erase }) => (S::Receiving(key), A::Start { key, erase }),
        (S::Idle, E::NonStorableHeader) => (S::Absorbing, A::None),
        (S::Idle, E::Row) => (S::Idle, A::DiscardWithoutHeader),
        (
            S::Idle,
            E::HeaderOfOtherMagazineSerial | E::TimeFillingHeader | E::BadHeader | E::UnrelatedHeader | E::Timeout,
        ) => (S::Idle, A::None),
        (S::Idle, E::RowOfOtherMagazineSerial) => (S::Idle, A::DiscardOtherMagazine),

        // Receiving P.
        (S::Receiving(key), E::Row) => (S::Receiving(key), A::Store(key)),
        (S::Receiving(key), E::RowOfOtherMagazineSerial) => (S::Receiving(key), A::DiscardOtherMagazine),
        (S::Receiving(ended), E::StorableHeader { key, erase }) => {
            (S::Receiving(key), A::EndAndStart { ended, started: key, erase })
        }
        (S::Receiving(key), E::NonStorableHeader) => (S::Absorbing, A::End(key)),
        (S::Receiving(key), E::HeaderOfOtherMagazineSerial) => (S::Idle, A::End(key)),
        (S::Receiving(key), E::TimeFillingHeader) => (S::Idle, A::End(key)),
        (S::Receiving(key), E::BadHeader) => (S::Idle, A::End(key)),
        (S::Receiving(key), E::Timeout) => (S::Idle, A::End(key)),
        (S::Receiving(key), E::UnrelatedHeader) => (S::Receiving(key), A::None),

        // Absorbing: headers that would end a page follow the same rules as
        // Receiving, with no "page completed" event.
        (S::Absorbing, E::Row) => (S::Absorbing, A::DiscardAbsorbed),
        (S::Absorbing, E::RowOfOtherMagazineSerial) => (S::Absorbing, A::DiscardOtherMagazine),
        (S::Absorbing, E::StorableHeader { key, erase }) => (S::Receiving(key), A::Start { key, erase }),
        (S::Absorbing, E::NonStorableHeader) => (S::Absorbing, A::None),
        (S::Absorbing, E::HeaderOfOtherMagazineSerial | E::TimeFillingHeader | E::BadHeader | E::Timeout) => {
            (S::Idle, A::None)
        }
        (S::Absorbing, E::UnrelatedHeader) => (S::Absorbing, A::None),
    }
}

/// Names for the transition trace (REQ-FSM-010).
#[must_use]
pub const fn magazine_state_name(state: MagazineState) -> &'static str {
    match state {
        MagazineState::Idle => "Idle",
        MagazineState::Receiving(_) => "Receiving",
        MagazineState::Absorbing => "Absorbing",
    }
}

/// Names for the transition trace.
#[must_use]
pub const fn magazine_event_name(event: MagazineEvent) -> &'static str {
    match event {
        MagazineEvent::StorableHeader { .. } => "storable header",
        MagazineEvent::NonStorableHeader => "non-storable header",
        MagazineEvent::Row => "row",
        MagazineEvent::RowOfOtherMagazineSerial => "row of other magazine (serial)",
        MagazineEvent::HeaderOfOtherMagazineSerial => "header of other magazine (serial)",
        MagazineEvent::TimeFillingHeader => "time-filling header",
        MagazineEvent::BadHeader => "bad header",
        MagazineEvent::Timeout => "timeout",
        MagazineEvent::UnrelatedHeader => "unrelated header",
    }
}

/// Names for the transition trace.
#[must_use]
pub const fn magazine_action_name(action: MagazineAction) -> &'static str {
    match action {
        MagazineAction::None => "none",
        MagazineAction::Start { .. } => "start page",
        MagazineAction::Store(_) => "store row",
        MagazineAction::DiscardWithoutHeader => "discard: row without header",
        MagazineAction::DiscardOtherMagazine => "discard: other magazine",
        MagazineAction::DiscardAbsorbed => "discard: absorbed",
        MagazineAction::End(_) => "end page",
        MagazineAction::EndAndStart { .. } => "end page and start another",
    }
}

/// States of the page lifecycle machine (REQ-FSM-004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageState {
    /// The slot holds no page.
    Empty,
    /// A reception is in progress.
    Partial,
    /// The page has been received completely.
    Complete,
    /// The page has not been received for the configured period.
    Stale,
}

/// Events of the page lifecycle machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageEvent {
    /// Reception started; `erase` is the header's C4.
    ReceptionStarted {
        /// C4.
        erase: bool,
    },
    /// A row was stored.
    RowStored(u8),
    /// Reception ended.
    ReceptionEnded,
    /// A header with C8 was received, or the content changed.
    UpdateIndicator,
    /// The page was not received for the configured period.
    NotReceived,
    /// The page was evicted from the cache.
    Evicted,
}

/// Actions of the page lifecycle machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageAction {
    /// Nothing.
    None,
    /// Raise "page started".
    Started,
    /// Merge the new rows under decision E.8 and raise "page started".
    MergeAndStart,
    /// Erase rows 1 to 28 and raise "page erased".
    Erase,
    /// Raise "page updated" for a row.
    Updated(u8),
    /// Raise "page completed".
    Completed,
    /// Raise "update indicator".
    UpdateIndicator,
    /// Raise "page stale".
    Stale,
    /// Release the storage.
    Release,
}

/// The transition function of the page lifecycle machine (REQ-FSM-004).
// One arm per table row (REQ-FSM-001 traceability).
#[allow(clippy::match_same_arms)]
#[must_use]
pub const fn page_transition(state: PageState, event: PageEvent) -> (PageState, PageAction) {
    use PageAction as A;
    use PageEvent as E;
    use PageState as S;
    match (state, event) {
        (S::Empty, E::ReceptionStarted { .. }) => (S::Partial, A::Started),
        (S::Partial, E::RowStored(row)) => (S::Partial, A::Updated(row)),
        (S::Partial, E::ReceptionEnded) => (S::Complete, A::Completed),
        (S::Complete | S::Stale, E::ReceptionStarted { erase: false }) => (S::Partial, A::MergeAndStart),
        (S::Complete | S::Stale, E::ReceptionStarted { erase: true }) => (S::Partial, A::Erase),
        (S::Complete, E::UpdateIndicator) => (S::Complete, A::UpdateIndicator),
        (S::Complete, E::NotReceived) => (S::Stale, A::Stale),
        (S::Empty | S::Partial | S::Complete | S::Stale, E::Evicted) => (S::Empty, A::Release),
        // Combinations the table does not list change nothing.
        (S::Empty, E::RowStored(_) | E::ReceptionEnded | E::UpdateIndicator | E::NotReceived) => (S::Empty, A::None),
        (S::Partial, E::ReceptionStarted { .. } | E::UpdateIndicator | E::NotReceived) => (S::Partial, A::None),
        (S::Complete, E::RowStored(_) | E::ReceptionEnded) => (S::Complete, A::None),
        (S::Stale, E::RowStored(_) | E::ReceptionEnded | E::UpdateIndicator | E::NotReceived) => (S::Stale, A::None),
    }
}

/// Names for the transition trace.
#[must_use]
pub const fn page_state_name(state: PageState) -> &'static str {
    match state {
        PageState::Empty => "Empty",
        PageState::Partial => "Partial",
        PageState::Complete => "Complete",
        PageState::Stale => "Stale",
    }
}

/// Names for the transition trace.
#[must_use]
pub const fn page_event_name(event: PageEvent) -> &'static str {
    match event {
        PageEvent::ReceptionStarted { erase: false } => "reception started",
        PageEvent::ReceptionStarted { erase: true } => "reception started with C4",
        PageEvent::RowStored(_) => "row stored",
        PageEvent::ReceptionEnded => "reception ended",
        PageEvent::UpdateIndicator => "update indicator",
        PageEvent::NotReceived => "not received",
        PageEvent::Evicted => "evicted",
    }
}

/// Names for the transition trace.
#[must_use]
pub const fn page_action_name(action: PageAction) -> &'static str {
    match action {
        PageAction::None => "none",
        PageAction::Started => "raise page started",
        PageAction::MergeAndStart => "merge and raise page started",
        PageAction::Erase => "erase and raise page erased",
        PageAction::Updated(_) => "raise page updated",
        PageAction::Completed => "raise page completed",
        PageAction::UpdateIndicator => "raise update indicator",
        PageAction::Stale => "raise page stale",
        PageAction::Release => "release storage",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
}
