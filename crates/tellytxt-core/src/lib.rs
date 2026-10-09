//! The tellytxt decoder core: World System Teletext (ETSI EN 300 706)
//! Presentation Levels 1.0 and 1.5.
//!
//! `no_std`, no allocation, no input or output and no clock (REQ-GEN-002).
//! Every piece of state lives in memory the caller provides (REQ-GEN-003).
//! The layers are described in `ARCHITECTURE.md`:
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
