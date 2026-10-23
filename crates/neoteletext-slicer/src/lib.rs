//! The neoteletext software data slicer (area SLC, layer L0): sampled
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
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
// REQ-GEN-006: no panicking constructs.
#![deny(
    clippy::indexing_slicing,
    clippy::string_slice,
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

/// REQ-GEN-011: the error types implement `core::error::Error`. Their
/// `Display` text is their `Debug` text, and both need `core::fmt`, which
/// REQ-GEN-016 keeps behind this optional feature so that a small build
/// leaves it out.
#[cfg(feature = "error-trait")]
mod error_trait {
    macro_rules! error_trait {
        ($($error:ty),+ $(,)?) => {$(
            impl core::fmt::Display for $error {
                fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    core::fmt::Debug::fmt(self, formatter)
                }
            }
            impl core::error::Error for $error {}
        )+};
    }
    error_trait!(crate::raw::GeometryError);

    #[cfg(test)]
    mod tests {
        const fn is_error<T: core::error::Error>() {}

        #[test]
        fn every_error_type_is_an_error() {
            is_error::<crate::raw::GeometryError>();
        }
    }
}
