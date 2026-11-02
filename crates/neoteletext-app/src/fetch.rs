//! The fetcher process of web and live mode (REQ-SEC-018): it holds the
//! network and runs the web module's state machines over a std transport
//! with TLS, sending packets and page bytes to the decoder process on
//! standard output (decision 26 of IMPLEMENTATION-LOG.md).

use neoteletext_core::types::Instant;
use neoteletext_web::proxy::Proxy;
use neoteletext_web::zxnet::{Channel, ChannelError, KNOWN_CHANNELS};
use std::net::IpAddr;
use std::sync::mpsc;
use std::time::Instant as StdInstant;

mod decoder_link;
mod transport;
mod wakes;
mod web;
mod zxnet;

pub use transport::StdTransport;
pub use wakes::Wake;

/// Directories the fetcher may read to resolve names (decision 28).
const RESOLVER_PATHS: [&str; 6] = ["/etc", "/run/systemd/resolve", "/usr/lib", "/lib", "/usr/lib64", "/lib64"];

/// Runs the fetcher for `address` until the decoder process goes away.
pub fn run(address: &str, no_sandbox: bool, allow_local: &[IpAddr], proxy: Option<&str>) -> Result<(), (u8, String)> {
    let proxy = proxy.map(Proxy::parse).transpose().map_err(|message| (crate::exit::USAGE, message))?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (wake, receiver) = mpsc::channel();
    decoder_link::requests(wake.clone());
    let mut wakes = wakes::Wakes::new(receiver);
    // Name resolution reads the resolver configuration and loads the C
    // library's name-service modules (decision 28 of IMPLEMENTATION-LOG.md).
    let readable: Vec<&std::path::Path> =
        RESOLVER_PATHS.iter().map(std::path::Path::new).filter(|path| path.exists()).collect();
    crate::sandbox::apply(
        crate::sandbox::Policy { writable_directory: None, network: true, readable: &readable },
        no_sandbox,
    )
    .map_err(|message| (crate::exit::INPUT, message))?;
    match feed(address) {
        Some(channel) => {
            let channel = channel.map_err(|error| (crate::exit::USAGE, format!("unknown live feed: {error:?}")))?;
            zxnet::run_zxnet(&channel, &mut wakes, wake, allow_local, proxy);
        }
        None => web::run_web(address, &mut wakes, wake, allow_local, proxy),
    }
    Ok(())
}

/// The live feed an input names, if it names one (REQ-INP-022): a short
/// name or a channel identifier after `zxnet:`, or a `wss` address.
fn feed(address: &str) -> Option<Result<Channel, ChannelError>> {
    if let Some(name) = address.strip_prefix("zxnet:") {
        return Some(
            Channel::from_short_name(name).or_else(|unknown| Channel::from_identifier(name).map_err(|_| unknown)),
        );
    }
    address.starts_with("wss://").then(|| feed_at(address))
}

/// The feed at a `wss` address, which carries no service name: a known
/// feed's own, or the last segment of the path (DEC-2134). A wrong guess
/// draws `wrongname`, which stops the feed with its message.
fn feed_at(address: &str) -> Result<Channel, ChannelError> {
    let known = KNOWN_CHANNELS
        .iter()
        .filter_map(|(short, _)| Channel::from_short_name(short).ok())
        .find(|channel| channel.address().as_str() == address);
    known.map_or_else(|| Channel::from_address(address, address.rsplit('/').next().unwrap_or_default()), Ok)
}

/// `neottx` as it names itself to a channel (REQ-WEB-031): `neottx/0.1
/// (macOS)`, one of the engine's own programs; on a system Rust does not
/// name, the engine's own string.
pub(super) fn neottx() -> Option<neoteletext_web::user_agent::Product> {
    neoteletext_web::user_agent::os_family().map(|family| neoteletext_web::user_agent::Product {
        name: "neottx",
        version: env!("CARGO_PKG_VERSION"),
        family,
        engine_own: true,
    })
}

pub(super) fn clock(start: StdInstant) -> Instant {
    Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests;
