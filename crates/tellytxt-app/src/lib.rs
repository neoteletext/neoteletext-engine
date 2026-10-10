//! The application layer of tellytxt (layer L9), shared by the `ttx`
//! command-line program and a future windowed viewer: decoding sessions,
//! reading inputs, the two processes of live mode (REQ-SEC-018) and the
//! operating-system sandbox (REQ-SEC-017).
//!
//! Nothing here knows about the terminal or about parsing arguments; those
//! belong to each program.

pub mod convert;
pub mod extras;
pub mod fetch;
pub mod input;
pub mod live;
pub mod pipe;
pub mod sandbox;
pub mod session;
pub mod slice;
pub mod source;

/// Exit statuses of REQ-CLI-005 and REQ-CLI-006.
pub mod exit {
    /// A requested page was not found.
    pub const NOT_FOUND: u8 = 1;
    /// Invalid usage.
    pub const USAGE: u8 = 2;
    /// The input could not be read or recognised.
    pub const INPUT: u8 = 3;
    /// The page checker found errors.
    pub const CHECK_ERRORS: u8 = 4;
}

/// An operation's failure: an exit status and a message.
pub type Failure = (u8, String);
