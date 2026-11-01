//! A client's connection over the operating system's sockets, for the
//! programs that hold their own network port: `neottx`'s fetcher, the
//! qrkeyboard client and the browser's fetcher.
//!
//! - [`Connection`]: an open connection's reader thread, which blocks on
//!   the socket and wakes its owner when bytes arrive; the bytes it read,
//!   taken plain or through TLS; and the blocking writes;
//! - [`client_config`], [`mozilla_roots`] and [`start_tls`]: Transport
//!   Layer Security as every connection has it;
//! - [`open_tunnel`], [`first_address`] and [`connect_within_limit`]: a
//!   socket opened with one limit on each step, through the proxy or
//!   directly.
//!
//! Which addresses a program may reach is not here: that is each program's
//! rule, and `neottx`, the qrkeyboard client and the browser differ in it.
//! Neither is the browser's opening of a connection, which runs under one
//! deadline on a thread that closing can shut, nor its writer thread, which
//! keeps a socket the network does not drain from holding up its loop
//! (DEC-1906 says why those stay in the fetcher).
//!
//! Everything a peer sends is untrusted: what a connection holds in memory
//! is bounded by the reads it may be ahead, and nothing here panics on
//! input.
// REQ-SEC-003: everything here reads untrusted input.
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

mod connection;
mod reach;
mod tls;

pub use connection::{Connection, SOCKET_READ, write_records};
pub use reach::{CONNECT_TIMEOUT, connect_within_limit, first_address, open_tunnel, web_address};
pub use tls::{TlsFailure, client_config, mozilla_roots, start_tls};

#[cfg(test)]
mod tests;
