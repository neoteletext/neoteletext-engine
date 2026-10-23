//! The strict address parser (REQ-WEB-016), origins (REQ-WEB-015) and
//! relative resolution (RFC 3986 section 5).
//!
//! One parser is used for every address the web module meets: descriptors,
//! templates, redirects, push addresses, form targets and configuration.
//! It accepts only:
//!
//! - the schemes `http`, `https`, `ws` and `wss`, in any letter case;
//! - a DNS name of ASCII letters, digits, hyphens and dots (no empty
//!   labels, at most 253 characters, already in Punycode), an IPv4 address
//!   in dotted decimal, or an IPv6 address in brackets;
//! - an optional decimal port from 1 to 65535;
//! - a path and query of RFC 3986 characters with percent-encoding.
//!
//! It rejects user information, fragments, white space, control
//! characters, backslashes, IPv4 in octal, hexadecimal or shortened forms,
//! anything over 2048 characters, and anything else it cannot parse
//! exactly. The parsed address is stored normalised: scheme and host in
//! lower case, the path's dot segments removed (RFC 3986 section 5.2.4).

mod host;
mod lexical;
mod path;

pub use host::ip_literal;

use host::{check_host, parse_ipv6, write_ipv6};
use lexical::{check_characters, scheme_end, split_parts};
use neoteletext_core::policy::{self, OriginParts};
use path::{check_path, remove_dot_segments};

use crate::bytes::{TextBuf, Writer};
use crate::consts::{IPV4_OCTETS, IPV6_GROUPS, MAX_HOST_TEXT_LENGTH, MAX_URL_LENGTH, PORT_HTTP, PORT_HTTPS};

/// Why an address was rejected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UrlError {
    /// Longer than 2048 characters.
    TooLong,
    /// White space, a control character or a non-ASCII byte (an
    /// internationalised name not in Punycode).
    BadCharacter,
    /// A backslash.
    Backslash,
    /// A fragment (`#`).
    Fragment,
    /// A scheme other than http, https, ws and wss, or no `//`.
    Scheme,
    /// User name or password (`@` in the authority).
    UserInfo,
    /// An empty or malformed host.
    Host,
    /// An IPv4 address not in strict dotted decimal.
    Ipv4,
    /// A malformed IPv6 address.
    Ipv6,
    /// A port that is empty, not decimal, zero or above 65535.
    Port,
    /// A malformed path or query, including bad percent-encoding.
    Path,
    /// A relative reference used where no base is available.
    Relative,
}

/// The four schemes of REQ-WEB-016.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scheme {
    /// `http`
    Http,
    /// `https`
    Https,
    /// `ws`
    Ws,
    /// `wss`
    Wss,
}

impl Scheme {
    /// The scheme name in lower case.
    #[must_use]
    pub const fn name(self) -> &'static [u8] {
        match self {
            Self::Http => b"http",
            Self::Https => b"https",
            Self::Ws => b"ws",
            Self::Wss => b"wss",
        }
    }

    /// True for the encrypted schemes.
    #[must_use]
    pub const fn is_secure(self) -> bool {
        matches!(self, Self::Https | Self::Wss)
    }

    /// True for WebSocket schemes.
    #[must_use]
    pub const fn is_websocket(self) -> bool {
        matches!(self, Self::Ws | Self::Wss)
    }

    /// The default port (REQ-WEB-015).
    #[must_use]
    pub const fn default_port(self) -> u16 {
        if self.is_secure() { PORT_HTTPS } else { PORT_HTTP }
    }

    fn from_name(name: &[u8]) -> Option<Self> {
        [Self::Http, Self::Https, Self::Ws, Self::Wss]
            .into_iter()
            .find(|scheme| name.eq_ignore_ascii_case(scheme.name()))
    }
}

/// A host as parsed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Host<'a> {
    /// A DNS name in lower case.
    Name(&'a str),
    /// An IPv4 address.
    Ipv4([u8; IPV4_OCTETS]),
    /// An IPv6 address.
    Ipv6([u16; IPV6_GROUPS]),
}

/// An IP address, as the transport reports the peer actually reached
/// (REQ-SEC-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IpAddress {
    /// IPv4.
    V4([u8; IPV4_OCTETS]),
    /// IPv6.
    V6([u16; IPV6_GROUPS]),
}

impl IpAddress {
    /// True when the address is in a blocked range (REQ-SEC-006), decided
    /// by the security policy.
    #[must_use]
    pub const fn is_blocked(self) -> bool {
        match self {
            Self::V4(octets) => policy::ipv4_blocked(octets),
            Self::V6(groups) => policy::ipv6_blocked(groups),
        }
    }
}

/// A parsed, normalised address held in a fixed buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Url {
    text: TextBuf<MAX_URL_LENGTH>,
    scheme: Scheme,
    /// The host text, without brackets for IPv6.
    host_start: u16,
    host_end: u16,
    host_ip: Option<IpAddress>,
    port: Option<u16>,
    path_start: u16,
    /// Start of the query, after `?`, when there is one.
    query_start: Option<u16>,
}

/// The positions found by the first pass over an absolute address.
struct Parts<'a> {
    scheme: Scheme,
    host: &'a [u8],
    bracketed: bool,
    port: Option<u16>,
    path: &'a [u8],
    query: Option<&'a [u8]>,
}

impl Url {
    /// Parses an absolute address.
    pub fn parse(text: &str) -> Result<Self, UrlError> {
        let bytes = text.as_bytes();
        check_characters(bytes)?;
        let parts = split_parts(bytes)?;
        Self::build(&parts)
    }

    fn build(parts: &Parts<'_>) -> Result<Self, UrlError> {
        let host_ip =
            if parts.bracketed { Some(IpAddress::V6(parse_ipv6(parts.host)?)) } else { check_host(parts.host)? };
        check_path(parts.path, false)?;
        if let Some(query) = parts.query {
            check_path(query, true)?;
        }
        if parts.host.len() > MAX_HOST_TEXT_LENGTH {
            return Err(UrlError::Host);
        }

        let mut buffer = [0u8; MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut buffer);
        let too_long = |_| UrlError::TooLong;
        writer.put(parts.scheme.name()).map_err(too_long)?;
        writer.put(b"://").map_err(too_long)?;
        if parts.bracketed {
            writer.byte(b'[').map_err(too_long)?;
        }
        let host_start = writer.len();
        match host_ip {
            Some(IpAddress::V6(groups)) => write_ipv6(&groups, &mut writer).map_err(too_long)?,
            _ => {
                for &byte in parts.host {
                    writer.byte(byte.to_ascii_lowercase()).map_err(too_long)?;
                }
            }
        }
        let host_end = writer.len();
        if parts.bracketed {
            writer.byte(b']').map_err(too_long)?;
        }
        if let Some(port) = parts.port {
            writer.byte(b':').map_err(too_long)?;
            writer.decimal(u64::from(port)).map_err(too_long)?;
        }
        let path_start = writer.len();
        if parts.path.is_empty() {
            writer.byte(b'/').map_err(too_long)?;
        } else {
            remove_dot_segments(parts.path, &mut writer).map_err(too_long)?;
        }
        let query_start = match parts.query {
            Some(query) => {
                writer.byte(b'?').map_err(too_long)?;
                let start = writer.len();
                writer.put(query).map_err(too_long)?;
                Some(start)
            }
            None => None,
        };
        let text = TextBuf::from_bytes(writer.written()).ok_or(UrlError::TooLong)?;
        let narrow = |value: usize| u16::try_from(value).map_err(|_| UrlError::TooLong);
        Ok(Self {
            text,
            scheme: parts.scheme,
            host_start: narrow(host_start)?,
            host_end: narrow(host_end)?,
            host_ip,
            port: parts.port,
            path_start: narrow(path_start)?,
            query_start: query_start.map(narrow).transpose()?,
        })
    }

    /// Checks a host on its own, as stored without brackets and in lower
    /// case: a DNS name, a dotted IPv4 address or an IPv6 address.
    pub fn parse_host_only(host: &str) -> Result<Option<IpAddress>, UrlError> {
        let bytes = host.as_bytes();
        if bytes.iter().any(u8::is_ascii_uppercase) {
            return Err(UrlError::Host);
        }
        check_characters(bytes)?;
        if bytes.contains(&b':') {
            parse_ipv6(bytes).map(|groups| Some(IpAddress::V6(groups)))
        } else {
            check_host(bytes)
        }
    }

    /// Resolves `reference` against this address (RFC 3986 section 5.2.2)
    /// and parses the result again (REQ-WEB-016).
    pub fn join(&self, reference: &str) -> Result<Self, UrlError> {
        let bytes = reference.as_bytes();
        check_characters(bytes)?;
        if scheme_end(bytes).is_some() {
            return Self::parse(reference);
        }
        let mut buffer = [0u8; MAX_URL_LENGTH];
        let mut writer = Writer::new(&mut buffer);
        let too_long = |_| UrlError::TooLong;
        writer.put(self.scheme.name()).map_err(too_long)?;
        writer.put(b":").map_err(too_long)?;
        if bytes.starts_with(b"//") {
            writer.put(bytes).map_err(too_long)?;
        } else {
            writer.put(b"//").map_err(too_long)?;
            writer.put(self.authority()).map_err(too_long)?;
            let (path, query) = match bytes.iter().position(|&b| b == b'?') {
                Some(mark) => (bytes.get(..mark).unwrap_or(&[]), Some(bytes.get(mark..).unwrap_or(&[]))),
                None => (bytes, None),
            };
            if path.is_empty() {
                writer.put(self.path().as_bytes()).map_err(too_long)?;
                match query {
                    Some(query) => writer.put(query).map_err(too_long)?,
                    None => {
                        if let Some(own) = self.query() {
                            writer.byte(b'?').map_err(too_long)?;
                            writer.put(own.as_bytes()).map_err(too_long)?;
                        }
                    }
                }
            } else {
                if path.first() == Some(&b'/') {
                    writer.put(path).map_err(too_long)?;
                } else {
                    // RFC 3986 section 5.2.3: merge with the base path's
                    // directory.
                    let base = self.path().as_bytes();
                    let directory = base.iter().rposition(|&b| b == b'/').map_or(0, |i| i.saturating_add(1));
                    writer.put(base.get(..directory).unwrap_or(b"/")).map_err(too_long)?;
                    writer.put(path).map_err(too_long)?;
                }
                if let Some(query) = query {
                    writer.put(query).map_err(too_long)?;
                }
            }
        }
        let joined = core::str::from_utf8(writer.written()).map_err(|_| UrlError::BadCharacter)?;
        Self::parse(joined)
    }

    /// The whole normalised address.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn slice(&self, start: u16, end: u16) -> &str {
        self.as_str().get(usize::from(start)..usize::from(end)).unwrap_or("")
    }

    /// The scheme.
    #[must_use]
    pub const fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// True for https and wss.
    #[must_use]
    pub const fn is_secure(&self) -> bool {
        self.scheme.is_secure()
    }

    /// The host as text in lower case, without brackets: what the
    /// transport is asked to connect to.
    #[must_use]
    pub fn host_text(&self) -> &str {
        self.slice(self.host_start, self.host_end)
    }

    /// The host classified.
    #[must_use]
    pub fn host(&self) -> Host<'_> {
        match self.host_ip {
            Some(IpAddress::V4(octets)) => Host::Ipv4(octets),
            Some(IpAddress::V6(groups)) => Host::Ipv6(groups),
            None => Host::Name(self.host_text()),
        }
    }

    /// The host when it is an address literal.
    #[must_use]
    pub const fn ip(&self) -> Option<IpAddress> {
        self.host_ip
    }

    /// The port as written, if any.
    #[must_use]
    pub const fn explicit_port(&self) -> Option<u16> {
        self.port
    }

    /// The port, with the scheme's default filled in (REQ-WEB-015).
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(self.scheme.default_port())
    }

    /// The authority as written in the address: host, brackets and port.
    #[must_use]
    pub fn authority(&self) -> &[u8] {
        let start = self.scheme.name().len().saturating_add(3);
        self.as_str().as_bytes().get(start..usize::from(self.path_start)).unwrap_or(&[])
    }

    /// The path, at least `/`.
    #[must_use]
    pub fn path(&self) -> &str {
        let end = self.query_start.map_or(self.text.len(), |start| usize::from(start).saturating_sub(1));
        self.as_str().get(usize::from(self.path_start)..end).unwrap_or("/")
    }

    /// The query without `?`, if any.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query_start.map(|start| self.as_str().get(usize::from(start)..).unwrap_or(""))
    }

    /// The request target: path and query (RFC 9112 section 3.2.1).
    #[must_use]
    pub fn target(&self) -> &str {
        self.as_str().get(usize::from(self.path_start)..).unwrap_or("/")
    }

    /// The origin (REQ-WEB-015).
    #[must_use]
    pub fn origin(&self) -> Origin {
        let mut host = TextBuf::new();
        // The host always fits: it was checked against the same bound.
        let _ = host.set(self.host_text().as_bytes());
        Origin { secure: self.is_secure(), host, port: self.port() }
    }
}

/// An origin: whether the scheme is encrypted, the host and the port
/// (REQ-WEB-015). `ws` counts as `http` and `wss` as `https`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Origin {
    /// The scheme is https or wss.
    pub secure: bool,
    /// The host in lower case, without brackets.
    pub host: TextBuf<MAX_HOST_TEXT_LENGTH>,
    /// The port with the scheme's default filled in.
    pub port: u16,
}

impl Origin {
    /// The parts compared by the security policy.
    #[must_use]
    pub fn parts(&self) -> OriginParts<'_> {
        OriginParts { secure: self.secure, host: self.host.as_str(), port: self.port }
    }

    /// True when both are the same origin, as the policy decides.
    #[must_use]
    pub fn same(&self, other: &Self) -> bool {
        policy::same_origin(self.parts(), other.parts())
    }
}

#[cfg(test)]
mod tests;
