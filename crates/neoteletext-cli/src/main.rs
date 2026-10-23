//! `neottx`, the neoteletext command-line program (area CLI): the binary
//! is the library's [`neoteletext_cli::run`].

// REQ-SEC-003: the program reads input from outside its process.
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

fn main() -> std::process::ExitCode {
    neoteletext_cli::run()
}
