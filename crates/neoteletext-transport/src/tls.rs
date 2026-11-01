//! Transport Layer Security as every connection has it: Mozilla's roots,
//! resumption, no early data, and the handshake over any blocking stream.

use std::fmt;
use std::io::{self, Read, Write};
use std::sync::Arc;

/// Mozilla's root certificates, compiled in.
#[must_use]
pub fn mozilla_roots() -> rustls::RootCertStore {
    webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect()
}

/// The TLS configuration of every connection a client opens (REQ-WEB-035,
/// REQ-RCH-014): one, shared, so a reconnection to a host resumes the
/// session of the last connection where the server allows, from rustls's
/// own in-memory store of sessions, saving a round trip; and no early data
/// (TLS 0-RTT), which could replay an input message. TCP Fast Open is never
/// used: the standard library does not offer it. ring is the one provider
/// built in, so the builder installs it.
#[must_use]
pub fn client_config(roots: rustls::RootCertStore) -> rustls::ClientConfig {
    let mut config = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
    config.resumption = rustls::client::Resumption::default();
    config.enable_early_data = false;
    config
}

/// Why a TLS connection did not start. Each caller says it its own way, and
/// the browser tells a refused certificate from the rest.
#[derive(Debug)]
pub enum TlsFailure {
    /// The host is not a name a certificate can be checked against.
    Name(rustls::pki_types::InvalidDnsNameError),
    /// rustls would not start a connection.
    Start(rustls::Error),
    /// The bytes that came with the proxy's answer were not taken.
    Read(io::Error),
    /// The bytes that came with the proxy's answer were not good records.
    Records(rustls::Error),
    /// The handshake failed: the socket, or a certificate refused, which
    /// is a hard stop.
    Handshake(io::Error),
}

impl fmt::Display for TlsFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(error) => error.fmt(formatter),
            Self::Start(error) | Self::Records(error) => error.fmt(formatter),
            Self::Read(error) | Self::Handshake(error) => error.fmt(formatter),
        }
    }
}

/// Starts TLS with `host` over `stream`, and completes the handshake,
/// blocking for as long as `stream` lets it. `first` is what came through
/// the tunnel with the proxy's answer: the server's, read before the
/// stream's next bytes.
///
/// # Errors
/// [`TlsFailure`] says at which step.
pub fn start_tls<S: Read + Write>(
    config: Arc<rustls::ClientConfig>,
    host: &str,
    first: &[u8],
    stream: &mut S,
) -> Result<rustls::ClientConnection, TlsFailure> {
    let name = rustls::pki_types::ServerName::try_from(host.to_owned()).map_err(TlsFailure::Name)?;
    let mut connection = rustls::ClientConnection::new(config, name).map_err(TlsFailure::Start)?;
    let mut records = first;
    while !records.is_empty() {
        // rustls takes nothing more once it has seen the server's
        // close_notify, and a read that takes nothing would never finish.
        if connection.read_tls(&mut records).map_err(TlsFailure::Read)? == 0 {
            break;
        }
        connection.process_new_packets().map_err(TlsFailure::Records)?;
    }
    while connection.is_handshaking() {
        connection.complete_io(stream).map_err(TlsFailure::Handshake)?;
    }
    Ok(connection)
}
