//! The tellytxt software data slicer (area SLC, layer L0): sampled
//! television lines in, 42-byte teletext packets out.
//!
//! - [`slicer`]: the slicer itself, running the line slicer machine of
//!   REQ-FSM-002 ([`fsm`]) once per line.
//! - [`raw`]: the streaming reader of raw sampled video files (REQ-INP-011),
//!   which splits a file into lines and feeds the slicer.
//! - [`generator`]: the synthetic test signal generator (REQ-TST-060).
//!
//! `no_std` and no allocation. The slicer uses floating point, as
//! REQ-GEN-016 allows; its fixed-point form (REQ-SLC-015) is Phase 2.
#![no_std]
#![forbid(unsafe_code)]
// REQ-GEN-006: no panicking constructs.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

pub mod consts;
pub mod fsm;
pub mod generator;
pub mod math;
pub mod raw;
pub mod slicer;

pub use raw::{GeometryError, Preset, RawGeometry, RawReader};
pub use slicer::{Config, Counters, Line, Rejection, SampleFormat, Sliced, Slicer};

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
