//! The application layer of neoteletext (layer L9), shared by the `neottx`
//! command-line program and a future windowed viewer: decoding sessions,
//! reading inputs, the two processes of live mode (REQ-SEC-018) and the
//! operating-system sandbox (REQ-SEC-017).
//!
//! Nothing here knows about the terminal or about parsing arguments; those
//! belong to each program.
//!
//! Features (decision E.61), all on by default: `live` for live feeds and
//! web services, which brings in the web module and TLS, and `slice` for
//! raw sampled video. The types stay whatever the features; a part left
//! out is refused with [`not_in_build`].

pub mod convert;
pub mod extras;
#[cfg(feature = "live")]
pub mod fetch;
pub mod font;
pub mod input;
pub mod interact;
#[cfg(feature = "live")]
pub mod live;
pub mod pipe;
pub mod present;
pub mod sandbox;
pub mod session;
#[cfg(feature = "slice")]
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

/// The failure for a part this build leaves out (decision E.61): invalid
/// usage of this build, with the feature that would bring the part in.
#[must_use]
pub fn not_in_build(part: &str, feature: &str) -> Failure {
    (exit::USAGE, format!("{part}: not in this build (it was built without the `{feature}` feature)"))
}
