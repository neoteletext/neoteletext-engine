//! The neoteletext web user agent (layer L7): a deliberately minimal,
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
//! Everything received is data (REQ-SEC-004): pages go to the format
//! readers, and descriptors and pushed messages are parsed into fixed
//! fields. Nothing received is executed or loaded as code.
//!
//! - Foundation: [`consts`], [`bytes`], [`base64`], [`sha1`], [`json`];
//! - addresses and origins: [`url`];
//! - protocols: [`http`], [`ws`], [`sse`], [`cookie`], and `dbsc` (feature
//!   `dbsc`): Device Bound Session Credentials (REQ-WEB-017);
//! - content: [`channel`], [`contents`], [`subpages`] (a page's catalogue
//!   of sub-pages, and with `std` a page written as a channel's files),
//!   [`descriptor`];
//! - the machines: [`agent`] (the [`agent::UserAgent`]) and [`zxnet`]
//!   (the [`zxnet::ZxnetFeed`]);
//! - the web half of the platform interface (layer L8): [`port`].
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
// REQ-GEN-006: no panicking constructs in library code.
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

#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod consts;

pub mod base64;
pub mod bytes;
pub mod json;
pub mod sha1;
pub mod sha256;

pub mod proxy;
pub mod url;
pub mod user_agent;

pub mod cookie;
#[cfg(feature = "dbsc")]
pub mod dbsc;
pub mod http;
pub mod sse;
pub mod ws;

pub mod channel;
pub mod contents;
pub mod descriptor;
pub mod map;
pub mod records;
pub mod subpages;

pub mod agent;
pub mod zxnet;

pub mod port;

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
    error_trait!(
        crate::sse::SseError,
        crate::bytes::Overflow,
        crate::descriptor::DescriptorError,
        crate::http::HttpError,
        crate::ws::WsError,
        crate::agent::Failure,
        crate::url::UrlError,
        crate::zxnet::ChannelError,
        crate::cookie::CookieRejected
    );

    #[cfg(test)]
    mod tests {
        const fn is_error<T: core::error::Error>() {}

        #[test]
        fn every_error_type_is_an_error() {
            is_error::<crate::sse::SseError>();
            is_error::<crate::bytes::Overflow>();
            is_error::<crate::descriptor::DescriptorError>();
            is_error::<crate::http::HttpError>();
            is_error::<crate::ws::WsError>();
            is_error::<crate::agent::Failure>();
            is_error::<crate::url::UrlError>();
            is_error::<crate::zxnet::ChannelError>();
            is_error::<crate::cookie::CookieRejected>();
        }
    }
}
