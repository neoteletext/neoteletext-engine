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
    /// The Level 2.5 and 3.5 enhancement walk (FSM-011).
    EnhancementWalk,
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

/// Where a machine that has neither a clock nor an event queue of its own,
/// such as the walks that build a grid, reports its transitions: the time
/// to stamp them with, and the caller's receiver.
pub struct TraceSink<'t> {
    /// The time of every entry: when the grid is built.
    pub time: Instant,
    /// Receives each entry.
    pub entries: &'t mut dyn FnMut(TraceEntry),
}

impl TraceSink<'_> {
    /// Reports one transition.
    pub fn transition(
        &mut self,
        machine: Machine,
        from: &'static str,
        event: &'static str,
        action: &'static str,
        to: &'static str,
    ) {
        (self.entries)(TraceEntry { machine, time: self.time, from, event, action, to });
    }
}
