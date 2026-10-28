//! `neottx`, the neoteletext command-line program (area CLI).
//!
//! Exit status (REQ-CLI-005): 0 success, 1 page not found, 2 invalid usage,
//! 3 input unreadable or unrecognised, 4 page checker errors.
//!
//! The program is this library, so that it can be tested; the binary
//! calls [`run`] and nothing else.

// REQ-SEC-003: this crate reads input from outside its process.
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

mod ansi;
mod args;
mod bounded_file;
mod cells;
#[cfg(any(feature = "view", feature = "keyboard"))]
mod clock;
mod commands;
mod frame;
mod image;
#[cfg(any(feature = "view", feature = "keyboard"))]
mod nfc;
mod options;
mod run;
mod terminal;
#[cfg(feature = "view")]
mod view;

pub use run::run;

use neoteletext_input::MAX_MESSAGE;

// The engine's three names for the input system's largest message agree.
const _: () = assert!(
    neoteletext_web::consts::INPUT_MESSAGE_MAX == MAX_MESSAGE
        && neoteletext_app::pipe::MAX_INPUT_MESSAGE == MAX_MESSAGE
);
// The push connection holds a move for every contact, and the longest
// move fits its slot (REQ-RCH-035, REQ-RCH-045).
const _: () = assert!(
    neoteletext_web::consts::PUSH_MOVE_SLOTS == neoteletext_input::MAX_CONTACTS
        && neoteletext_web::consts::PUSH_MOVE_MAX >= neoteletext_input::event::MAX_MOVE_BYTES
);
