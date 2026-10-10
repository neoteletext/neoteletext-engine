//! Streaming readers, writers and the page encoder for teletext input
//! formats (area INP; layer L0 Ingress of `ARCHITECTURE.md`).
//!
//! Every reader implements [`Reader`]: it is fed bytes in pieces of any size
//! and reports what it finds through a callback as [`Item`]s: packets in the
//! internal form of REQ-ARCH-002 with their metadata, page files, transport
//! stream information and diagnostics carrying the byte offset of the problem
//! (REQ-INP-001 and REQ-INP-020). Memory use is fixed by each reader's type
//! and does not depend on the input.
//!
//! - Packet streams: [`t42`] (REQ-INP-002 to 004), [`transport`] and [`pes`]
//!   (REQ-INP-006 to 009), [`sliced`] (REQ-INP-010).
//! - Page files: [`tti`] (REQ-INP-013), [`ep1`] (REQ-INP-015), [`edittf`]
//!   (REQ-INP-016), [`dump`] (REQ-INP-017), all producing a [`page::PageFile`].
//! - [`encode`]: page files to packets (REQ-ARCH-004, REQ-INP-019).
//! - [`detect`]: format detection and the [`detect::AutoReader`]
//!   (REQ-INP-005).
//! - [`check`]: the TTI page checker behind `ttx check` (REQ-CLI-006).
//! - `write` (feature `std`): the T42 and TTI writers (REQ-INP-014 and 018).
//! - `author` (feature `std`): the builder for Level 2.5 and 3.5 test pages
//!   and the authored test streams (REQ-INP-025, REQ-TST-024).
//!
//! Raw sampled video (REQ-INP-011) is read by `tellytxt-slicer`.
//!
//! Every format is parsed as data into fixed fields; unknown commands are
//! reported and ignored, and nothing read is executed (REQ-SEC-004).
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
// REQ-GEN-006 and REQ-GEN-008: no panicking constructs, as in the core.
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

#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod consts;
pub mod item;
pub mod text;

pub mod pes;
pub mod sliced;
pub mod t42;
pub mod transport;

pub mod dump;
pub mod edittf;
pub mod ep1;
pub mod page;
pub mod tti;

pub mod check;
pub mod detect;
pub mod encode;

#[cfg(feature = "std")]
pub mod write;

#[cfg(feature = "std")]
pub mod author;

#[cfg(test)]
mod testing;

pub use item::{Diagnostic, DiagnosticKind, Item, Reader};

#[cfg(test)]
mod robustness;
