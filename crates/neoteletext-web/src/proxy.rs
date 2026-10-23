//! Connections through a proxy (REQ-WEB-032, decision E.102), sans-IO: a
//! transport that is told to connect to a host and port connects to the
//! proxy instead, writes the request this module gives it, reads the
//! proxy's answer with it, and after a success uses the connection as if
//! it reached the host, TLS included, so encryption stays end to end.
//!
//! - **HTTP proxies** by `CONNECT` (RFC 9110 section 9.3.6), for every
//!   connection, plain or TLS.
//! - **SOCKS5 proxies** (RFC 1928) with no authentication, sending the
//!   host's name for the proxy to resolve (address type 3), or the address
//!   when the host is one.
//!
//! The proxy resolves names, so the local-network rule (REQ-SEC-006) is
//! kept on what the client can see: [`hidden_local`].

mod connect;
mod socks;
#[cfg(feature = "std")]
mod tunnel;

pub use connect::{read_connect_reply, write_connect};
pub use socks::{
    SOCKS_GREETING, SOCKS_MAX_REPLY, read_socks_choice, read_socks_reply, socks_reply_length, write_socks_connect,
};
#[cfg(feature = "std")]
pub use tunnel::{Proxy, TunnelError, tunnel};

use socks::SOCKS_MAX_NAME;

use crate::url::ip_literal;

/// The kinds of proxy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProxyKind {
    /// An HTTP proxy, by `CONNECT`.
    Http,
    /// A SOCKS5 proxy, with no authentication.
    Socks5,
}

/// Why a proxy did not connect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProxyError {
    /// The request does not fit its buffer, or a name is too long.
    TooLong,
    /// The proxy's answer broke its protocol.
    Malformed,
    /// The proxy refused, with an HTTP status or a SOCKS5 reply code.
    Refused(u16),
    /// The SOCKS5 proxy wants authentication, which is not supported.
    NeedsAuthentication,
}

/// Whether `host`, as a client sees it before asking a proxy, is on this
/// computer or the local network (REQ-SEC-006 through a proxy): an address
/// in a blocked range, `localhost`, or a name under `.localhost`, `.local`
/// or `.internal`.
#[must_use]
pub fn hidden_local(host: &str) -> bool {
    if let Some(address) = ip_literal(host) {
        return address.is_blocked();
    }
    let name = host.strip_suffix('.').unwrap_or(host);
    let lower = |suffix: &str| {
        name.len() >= suffix.len()
            && name.get(name.len().saturating_sub(suffix.len())..).is_some_and(|end| end.eq_ignore_ascii_case(suffix))
    };
    name.eq_ignore_ascii_case("localhost") || lower(".localhost") || lower(".local") || lower(".internal")
}

/// A proxy's kind and address, as a port's setting writes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProxyAddress<'a> {
    /// HTTP or SOCKS5.
    pub kind: ProxyKind,
    /// Its host: a name, or an address, an IPv6 one without brackets.
    pub host: &'a str,
    /// Its port.
    pub port: u16,
}

/// Reads a proxy's address written as `http://host:port` or
/// `socks5://host:port`, an IPv6 address in brackets; `None` for anything
/// else, a path or a user name included.
#[must_use]
pub fn parse_proxy(text: &str) -> Option<ProxyAddress<'_>> {
    let (kind, rest) = if let Some(rest) = text.strip_prefix("http://") {
        (ProxyKind::Http, rest)
    } else {
        (ProxyKind::Socks5, text.strip_prefix("socks5://")?)
    };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let (host, port) = if let Some(bracketed) = rest.strip_prefix('[') {
        let (host, after) = bracketed.split_once(']')?;
        (host, after.strip_prefix(':')?)
    } else {
        rest.rsplit_once(':')?
    };
    let valid_host = !host.is_empty()
        && host.len() <= SOCKS_MAX_NAME
        && host.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':'));
    let port = port.parse::<u16>().ok().filter(|&port| port != 0)?;
    valid_host.then_some(ProxyAddress { kind, host, port })
}

#[cfg(test)]
mod tests;
