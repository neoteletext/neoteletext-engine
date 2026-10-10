//! The tellytxt decoder core: World System Teletext (ETSI EN 300 706)
//! Presentation Levels 1.0 and 1.5.
//!
//! `no_std`, no allocation, no input or output and no clock (REQ-GEN-002).
//! Every piece of state lives in memory the caller provides (REQ-GEN-003),
//! so there are no mutable or lazily initialised statics and several
//! decoders can run side by side (REQ-GEN-004).
//!
//! These are the core modules of REQ-ARCH-001. The core depends on no
//! other crate, so no core module can depend on an optional module. The
//! layers and their allowed dependencies are described in `ARCHITECTURE.md`:
//!
//! - Foundation: [`consts`], [`ecc`], [`types`];
//! - L1 packet decoding: [`packet`];
//! - L2 acquisition: [`fsm`], [`acq`], [`service`];
//! - L3 presentation model: [`charset`], [`level1`], [`level15`];
//! - L4 rendering: [`glyphs`], [`render`];
//! - L6 interaction: [`viewer`], [`nav`], [`forms`];
//! - L8 platform interface: [`port`];
//! - cross-cutting: [`config`], [`policy`], [`trace`].
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
// REQ-GEN-006: no panicking constructs in the core.
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

#[cfg(test)]
extern crate std;

pub mod consts;
pub mod ecc;
pub mod types;

pub mod packet;

pub mod acq;
pub mod fsm;
pub mod service;

pub mod charset;
pub mod level1;
pub mod level15;

pub mod glyphs;
pub mod render;

pub mod forms;
pub mod nav;
pub mod viewer;

pub mod port;

pub mod config;
pub mod policy;
pub mod trace;

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
    error_trait!(crate::render::RenderError, crate::packet::PacketError, crate::forms::FormError);

    #[cfg(test)]
    mod tests {
        const fn is_error<T: core::error::Error>() {}

        #[test]
        fn every_error_type_is_an_error() {
            is_error::<crate::render::RenderError>();
            is_error::<crate::packet::PacketError>();
            is_error::<crate::forms::FormError>();
        }
    }
}
