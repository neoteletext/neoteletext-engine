//! The tellytxt.com web user agent (layer L7): a deliberately minimal,
//! sans-IO browser for teletext pages served over the web (area WEB,
//! decision E.34), and the reader of the zxnet live feed (REQ-INP-022).
//!
//! `no_std`, no allocation and no input or output (REQ-WEB-001): every
//! protocol is a state machine fed with received bytes, the current time
//! and user requests, producing bytes to send, timers, page bytes (TTI or
//! T42) for the format readers, and events. The port supplies the network
//! transport, including encryption, through [`port::Transport`]. There is
//! no socket and no cryptography here.
//!
//! - Foundation: [`consts`], [`bytes`], [`base64`], [`sha1`], [`json`];
//! - addresses and origins: [`url`];
//! - protocols: [`http`], [`ws`], [`sse`], [`cookie`];
//! - content: [`descriptor`], [`form`];
//! - the machines: [`agent`] (the [`agent::UserAgent`]) and [`zxnet`]
//!   (the [`zxnet::ZxnetFeed`]);
//! - the web half of the platform interface (layer L8): [`port`].
#![no_std]
#![forbid(unsafe_code)]
// REQ-GEN-006: no panicking constructs in library code.
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

pub mod base64;
pub mod bytes;
pub mod json;
pub mod sha1;

pub mod url;

pub mod cookie;
pub mod http;
pub mod sse;
pub mod ws;

pub mod descriptor;
pub mod form;

pub mod agent;
pub mod zxnet;

pub mod port;
