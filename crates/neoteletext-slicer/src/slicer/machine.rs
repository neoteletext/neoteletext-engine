//! The driver of the line slicer machine of REQ-FSM-002: one transition
//! per event, each action performed.

use neoteletext_core::packet::{Packet, PacketMeta, SliceQuality};
use neoteletext_core::trace::{Machine, TraceEntry};
use neoteletext_core::types::Instant;

use crate::fsm::{SlicerAction, SlicerEvent, SlicerState, slicer_transition};

use super::line::Line;
use super::outcome::{Rejection, Sliced};
use super::{MAX_TRANSITIONS, Slicer, Work, check_address};

impl Slicer {
    /// Slices one line.
    pub fn slice(&mut self, line: &Line<'_>) -> Result<Sliced, Rejection> {
        self.slice_traced(line, |_| {})
    }

    /// Slices one line, passing each transition to `trace` when tracing is
    /// on in the settings (REQ-FSM-010).
    pub fn slice_traced<F: FnMut(TraceEntry)>(&mut self, line: &Line<'_>, mut trace: F) -> Result<Sliced, Rejection> {
        let mut state = SlicerState::Start;
        let mut work = Work::new();
        let mut outcome = Err(Rejection::NoSignal);
        for _ in 0..MAX_TRANSITIONS {
            let event = match state {
                SlicerState::Start => SlicerEvent::LineReceived,
                SlicerState::SearchingRunIn => self.search(&mut work),
                SlicerState::Measuring => self.measure(&mut work),
                SlicerState::GateCheck => self.gate(&work),
                SlicerState::LookingForFramingCode => self.find_framing_code(&mut work),
                SlicerState::CollectingBytes => self.collect(&mut work),
                SlicerState::CheckingAddress => check_address(&work),
                SlicerState::Accepted | SlicerState::Rejected => break,
            };
            let (next, action) = slicer_transition(state, event);
            self.perform(action, line, &mut work, &mut outcome);
            if self.config.trace {
                trace(TraceEntry {
                    machine: Machine::Slicer,
                    time: Instant(line.timestamp_us.unwrap_or(0)),
                    from: state.name(),
                    event: event.name(),
                    action: action.name(),
                    to: next.name(),
                });
            }
            state = next;
        }
        outcome
    }

    fn perform(
        &mut self,
        action: SlicerAction,
        line: &Line<'_>,
        work: &mut Work,
        outcome: &mut Result<Sliced, Rejection>,
    ) {
        let counters = &mut self.counters;
        match action {
            SlicerAction::None => {}
            SlicerAction::EstimateBlackLevel => {
                counters.lines = counters.lines.saturating_add(1);
                self.load(line);
            }
            SlicerAction::RecordPosition => {
                work.bit3_centre = self.bit3_centre(work.candidate);
            }
            SlicerAction::CountNoSignal => {
                counters.no_signal = counters.no_signal.saturating_add(1);
                *outcome = Err(Rejection::NoSignal);
            }
            SlicerAction::HoldThreshold => work.held_threshold = work.threshold,
            SlicerAction::CountBelowGate => {
                counters.below_gate = counters.below_gate.saturating_add(1);
                *outcome = Err(Rejection::BelowGate);
            }
            SlicerAction::MarkFramingCorrected => work.framing_corrected = true,
            SlicerAction::CountNoFramingCode => {
                counters.no_framing_code = counters.no_framing_code.saturating_add(1);
                *outcome = Err(Rejection::NoFramingCode);
            }
            SlicerAction::AssembleBytes => work.packet = Some(Packet { bytes: work.bytes }),
            SlicerAction::OutputPacket => {
                counters.accepted = counters.accepted.saturating_add(1);
                if work.framing_corrected {
                    counters.framing_corrected = counters.framing_corrected.saturating_add(1);
                }
                let packet = work.packet.unwrap_or(Packet { bytes: work.bytes });
                let quality = SliceQuality {
                    amplitude_percent: work.amplitude_percent,
                    match_percent: work.match_percent,
                    position: self.run_in_position(work),
                    framing_corrected: work.framing_corrected,
                };
                let meta = PacketMeta {
                    line: line.line,
                    second_field: line.second_field,
                    timestamp_us: line.timestamp_us,
                    source: None,
                    quality: Some(quality),
                };
                *outcome = Ok(Sliced { packet, meta });
            }
            SlicerAction::CountBadAddress => {
                counters.bad_address = counters.bad_address.saturating_add(1);
                *outcome = Err(Rejection::BadAddress);
            }
        }
    }
}
