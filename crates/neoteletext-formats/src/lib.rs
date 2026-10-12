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
//! - Packet streams: [`t42`] (REQ-INP-002 to 004, always compiled),
//!   `transport` and `pes` (feature `transport`, REQ-INP-006 to 009),
//!   `sliced` (feature `sliced`, REQ-INP-010).
//! - Page files, all producing a [`page::PageFile`]: `tti` (REQ-INP-013),
//!   `ep1` (REQ-INP-015), `edittf` (REQ-INP-016) and `dump` (REQ-INP-017),
//!   each under the feature of its name.
//! - `encode` (feature `pages`): page files to packets (REQ-ARCH-004,
//!   REQ-INP-019).
//! - [`detect`]: format detection and the [`detect::AutoReader`]
//!   (REQ-INP-005), over the formats compiled in.
//! - `check` (feature `check`): the TTI page checker behind `ttx check`
//!   (REQ-CLI-006).
//! - `bdf` (feature `bdf`): the strict reader of the 12 by 10 BDF fonts the
//!   browser loads (REQ-FNT-002). Fonts are not teletext input, but they
//!   are untrusted files read into fixed tables like every format here.
//! - `write` (feature `std`): the T42 writer (REQ-INP-018) and, with `tti`,
//!   the TTI writer (REQ-INP-014).
//! - `author` (features `std` and `tti`): the builder for Level 2.5 and 3.5
//!   test pages and the authored test streams (REQ-INP-025, REQ-TST-024).
//!
//! Decision E.61: every format but T42 is a Cargo feature, all on by
//! default. Detection recognises only the formats compiled in, and a format
//! asked for that is not compiled in is reported as
//! [`DiagnosticKind::FormatNotCompiled`].
//!
//! Raw sampled video (REQ-INP-011) is read by `neoteletext-slicer`.
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

// Decision E.61: the readers, the encoder and the checker are features; the
// data types they report (formats, configurations, items, pages) are
// always compiled, so that enabling a feature breaks no code written
// without it.
#[cfg(feature = "transport")]
pub mod pes;
pub mod sliced;
pub mod t42;
pub mod transport;

#[cfg(feature = "dump")]
pub mod dump;
#[cfg(feature = "edittf")]
pub mod edittf;
#[cfg(feature = "ep1")]
pub mod ep1;
pub mod page;
#[cfg(feature = "tti")]
pub mod tti;

#[cfg(feature = "check")]
pub mod check;

#[cfg(feature = "bdf")]
pub mod bdf;
pub mod detect;
#[cfg(feature = "pages")]
pub mod encode;

#[cfg(feature = "std")]
pub mod write;

#[cfg(all(feature = "std", feature = "tti"))]
pub mod author;

#[cfg(test)]
mod testing;

pub use item::{Diagnostic, DiagnosticKind, Item, Reader};

#[cfg(test)]
mod robustness;
