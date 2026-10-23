//! What slicing a line gives: the counters of REQ-FSM-002's actions, the
//! rejections, and the sliced packet.

use neoteletext_core::packet::{Packet, PacketMeta};

/// Counts of lines by outcome (REQ-FSM-002 actions).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Counters {
    /// Lines received.
    pub lines: u32,
    /// Packets output.
    pub accepted: u32,
    /// Lines where no run-in was found.
    pub no_signal: u32,
    /// Lines below the amplitude or match minimum.
    pub below_gate: u32,
    /// Lines without an acceptable framing code.
    pub no_framing_code: u32,
    /// Lines with an uncorrectable address byte.
    pub bad_address: u32,
    /// Packets output whose framing code needed a correction.
    pub framing_corrected: u32,
}

/// Why a line gave no packet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rejection {
    /// No run-in in the search window.
    NoSignal,
    /// Amplitude or match below the minimum.
    BelowGate,
    /// No acceptable framing code.
    NoFramingCode,
    /// An address byte was uncorrectable.
    BadAddress,
}

/// A packet recovered from a line, with its metadata and quality.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sliced {
    /// The packet.
    pub packet: Packet,
    /// Line, field, time and slicer quality (REQ-SLC-012).
    pub meta: PacketMeta,
}
