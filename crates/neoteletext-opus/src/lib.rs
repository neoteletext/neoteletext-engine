//! An integer-only Opus decoder (RFC 6716, RFC 8251) for one profile: mono, one 20 ms frame a
//! packet, 48 kHz out (decision E.123). No floating point, no standard library, no allocation, no
//! dependency: the same on a microcontroller as on a desktop (REQ-AUD-024). Its output is
//! bit-exact with libopus 1.5.2's fixed-point decoder (REQ-AUD-023).

#![no_std]
#![forbid(unsafe_code)]
#![forbid(clippy::float_arithmetic)]
// The engine's no-panic list, the same in every crate root that reads outside input: every
// packet here is untrusted (the engine's REQ-SEC-003, REQ-GEN-008).
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

mod celt;
mod decoder;
mod fixed;
mod math;
mod packet;
mod range;
mod silk;
mod slices;

pub use decoder::{DecodeError, Decoder, FRAME_SAMPLES, SAMPLE_RATE};
pub use packet::{MAX_PACKET, PacketError};
