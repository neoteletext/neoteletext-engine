//! The software mixer of the audio profile (REQ-AUD-019), for ports with an
//! operating system's audio output: desktops and phones. It behaves as the
//! tellyboy driver does on a microcontroller, so a page sounds alike
//! everywhere: four voices, gains in whole per cent, every change ramped,
//! ducking in EN 300 708 Table 19's steps, and a clamp after the sum.
//!
//! It holds sounds as 16-bit PCM and renders mono 16-bit samples at the
//! output's rate, resampling each voice with a windowed-sinc filter
//! ([`sinc`]). It is pure: no clock, no thread and no device. The port
//! calls [`Mixer::render`] from its audio callback, which never allocates
//! or frees; everything that allocates is a call the port makes elsewhere.
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
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

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod sinc;

mod mixer;
mod ramp;
mod sound;
mod voice;

pub use mixer::{Mixer, Start, VOICES};
pub use sound::{STREAM_RING_MS, STREAM_TARGET_MS, STREAM_TRIM_PPM, SoundKey};
