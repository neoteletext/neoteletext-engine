//! State machine transition trace (REQ-FSM-010).
//!
//! Each machine reports a [`TraceEntry`] per transition when tracing is
//! switched on in the configuration. When it is off the cost is one test.

use crate::types::Instant;

/// Which machine made a transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Machine {
    /// The magazine acquisition machine of the given magazine (FSM-003).
    Magazine(u8),
    /// The page lifecycle machine of a page, as `0xMPP` (FSM-004).
    Page(u16),
    /// The viewer machine (FSM-005).
    Viewer,
    /// The line slicer machine (FSM-002).
    Slicer,
    /// The Level 1.5 triplet walk (FSM-007).
    TripletWalk,
}

/// One transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TraceEntry {
    /// The machine and instance.
    pub machine: Machine,
    /// When the transition happened.
    pub time: Instant,
    /// The state before.
    pub from: &'static str,
    /// The event.
    pub event: &'static str,
    /// The actions performed.
    pub action: &'static str,
    /// The state after.
    pub to: &'static str,
}
