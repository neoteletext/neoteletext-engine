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

use tellytxt_core::policy::{self, OriginParts};

use crate::bytes::{TextBuf, Writer, hex_value, parse_decimal};
use crate::consts::{
    IPV4_OCTETS, IPV6_GROUPS, MAX_HOST_NAME_LENGTH, MAX_HOST_TEXT_LENGTH, MAX_IPV4_OCTET_DIGITS, MAX_IPV6_GROUP_DIGITS,
    MAX_LABEL_LENGTH, MAX_PORT_DIGITS, MAX_URL_LENGTH, PORT_HTTP, PORT_HTTPS,
};

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

/// Checks the characters every address must keep to.
fn check_characters(text: &[u8]) -> Result<(), UrlError> {
    if text.len() > MAX_URL_LENGTH {
        return Err(UrlError::TooLong);
    }
    for &byte in text {
        match byte {
            b'\\' => return Err(UrlError::Backslash),
            b'#' => return Err(UrlError::Fragment),
            0x21..=0x7E => {}
            _ => return Err(UrlError::BadCharacter),
        }
    }
    Ok(())
}

/// The scheme of an absolute reference: letters, digits, `+`, `-` and `.`
/// before a colon, starting with a letter (RFC 3986 section 3.1).
fn scheme_end(text: &[u8]) -> Option<usize> {
    let first = text.first()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let end = text.iter().position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.')))?;
    (text.get(end) == Some(&b':')).then_some(end)
}

fn split_parts(text: &[u8]) -> Result<Parts<'_>, UrlError> {
    let end = scheme_end(text).ok_or(UrlError::Scheme)?;
    let scheme = Scheme::from_name(text.get(..end).unwrap_or(&[])).ok_or(UrlError::Scheme)?;
    let rest = text.get(end.saturating_add(1)..).unwrap_or(&[]);
    let rest = rest.strip_prefix(b"//").ok_or(UrlError::Scheme)?;
    let authority_end = rest.iter().position(|&b| b == b'/' || b == b'?').unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    if authority.contains(&b'@') {
        return Err(UrlError::UserInfo);
    }
    let (host, bracketed, port_text) = if let Some(inner) = authority.strip_prefix(b"[") {
        let close = inner.iter().position(|&b| b == b']').ok_or(UrlError::Ipv6)?;
        let after = inner.get(close.saturating_add(1)..).unwrap_or(&[]);
        let port_text = match after.split_first() {
            None => None,
            Some((b':', digits)) => Some(digits),
            Some(_) => return Err(UrlError::Host),
        };
        (inner.get(..close).unwrap_or(&[]), true, port_text)
    } else {
        match authority.iter().position(|&b| b == b':') {
            Some(colon) => (
                authority.get(..colon).unwrap_or(&[]),
                false,
                Some(authority.get(colon.saturating_add(1)..).unwrap_or(&[])),
            ),
            None => (authority, false, None),
        }
    };
    let port = match port_text {
        None => None,
        Some(digits) => Some(parse_port(digits)?),
    };
    let (path, query) = match tail.iter().position(|&b| b == b'?') {
        Some(mark) => (tail.get(..mark).unwrap_or(&[]), Some(tail.get(mark.saturating_add(1)..).unwrap_or(&[]))),
        None => (tail, None),
    };
    Ok(Parts { scheme, host, bracketed, port, path, query })
}

fn parse_port(digits: &[u8]) -> Result<u16, UrlError> {
    // A leading zero is not the canonical form of any port; rejecting it
    // keeps every accepted address in one spelling.
    if digits.first() == Some(&b'0') {
        return Err(UrlError::Port);
    }
    let value = parse_decimal(digits, MAX_PORT_DIGITS).ok_or(UrlError::Port)?;
    u16::try_from(value).ok().filter(|&port| port != 0).ok_or(UrlError::Port)
}

/// RFC 3986 section 2: unreserved and sub-delimiter characters, plus `:`
/// and `@` (pchar), `/` in paths, and `?` in queries.
fn is_path_char(byte: u8, query: bool) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
        )
        || matches!(byte, b':' | b'@' | b'/')
        || (query && byte == b'?')
}

/// Checks a path or query: allowed characters and well-formed
/// percent-encoding.
fn check_path(text: &[u8], query: bool) -> Result<(), UrlError> {
    let mut index = 0usize;
    while let Some(&byte) = text.get(index) {
        if byte == b'%' {
            let valid = text.get(index.saturating_add(1)).copied().and_then(hex_value).is_some()
                && text.get(index.saturating_add(2)).copied().and_then(hex_value).is_some();
            if !valid {
                return Err(UrlError::Path);
            }
            index = index.saturating_add(3);
        } else if is_path_char(byte, query) {
            index = index.saturating_add(1);
        } else {
            return Err(UrlError::Path);
        }
    }
    Ok(())
}

/// Parses an IPv4 address in strict dotted decimal: four parts of one to
/// three digits, no leading zeros, each at most 255.
fn parse_ipv4(text: &[u8]) -> Result<[u8; IPV4_OCTETS], UrlError> {
    let mut octets = [0u8; IPV4_OCTETS];
    let mut count = 0usize;
    for part in text.split(|&b| b == b'.') {
        let slot = octets.get_mut(count).ok_or(UrlError::Ipv4)?;
        if part.len() > 1 && part.first() == Some(&b'0') {
            return Err(UrlError::Ipv4);
        }
        let value = parse_decimal(part, MAX_IPV4_OCTET_DIGITS).ok_or(UrlError::Ipv4)?;
        *slot = u8::try_from(value).map_err(|_| UrlError::Ipv4)?;
        count = count.saturating_add(1);
    }
    if count == IPV4_OCTETS { Ok(octets) } else { Err(UrlError::Ipv4) }
}

/// Parses an IPv6 address (RFC 4291 section 2.2): eight groups of one to
/// four hexadecimal digits, one `::` standing for one or more zero groups,
/// and an optional dotted IPv4 tail. Zone identifiers are rejected.
fn parse_ipv6(text: &[u8]) -> Result<[u16; IPV6_GROUPS], UrlError> {
    let mut head = [0u16; IPV6_GROUPS];
    let mut tail = [0u16; IPV6_GROUPS];
    let (head_text, tail_text) = match find_double_colon(text) {
        Some(index) => (text.get(..index).unwrap_or(&[]), Some(text.get(index.saturating_add(2)..).unwrap_or(&[]))),
        None => (text, None),
    };
    let head_count = parse_groups(head_text, &mut head, tail_text.is_none())?;
    let tail_count = match tail_text {
        Some(rest) => parse_groups(rest, &mut tail, true)?,
        None => 0,
    };
    let total = head_count.saturating_add(tail_count);
    let valid = match tail_text {
        None => total == IPV6_GROUPS,
        Some(_) => total < IPV6_GROUPS,
    };
    if !valid {
        return Err(UrlError::Ipv6);
    }
    let mut groups = head;
    let offset = IPV6_GROUPS.saturating_sub(tail_count);
    for (index, &value) in tail.iter().take(tail_count).enumerate() {
        if let Some(slot) = groups.get_mut(offset.saturating_add(index)) {
            *slot = value;
        }
    }
    Ok(groups)
}

fn find_double_colon(text: &[u8]) -> Option<usize> {
    text.windows(2).position(|pair| pair == b"::")
}

/// Parses colon-separated groups into `groups`, returning how many were
/// written. `ipv4_allowed` permits a dotted IPv4 tail as the last part.
fn parse_groups(text: &[u8], groups: &mut [u16; IPV6_GROUPS], ipv4_allowed: bool) -> Result<usize, UrlError> {
    if text.is_empty() {
        return Ok(0);
    }
    let mut count = 0usize;
    let parts = text.split(|&b| b == b':');
    let total_parts = text.split(|&b| b == b':').count();
    for (index, part) in parts.enumerate() {
        let last = index.saturating_add(1) == total_parts;
        if last && ipv4_allowed && part.contains(&b'.') {
            let [a, b, c, d] = parse_ipv4(part).map_err(|_| UrlError::Ipv6)?;
            for value in [u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d])] {
                *groups.get_mut(count).ok_or(UrlError::Ipv6)? = value;
                count = count.saturating_add(1);
            }
            continue;
        }
        if part.is_empty() || part.len() > MAX_IPV6_GROUP_DIGITS {
            return Err(UrlError::Ipv6);
        }
        let value = part.iter().try_fold(0u16, |value, &d| Some((value << 4) | u16::from(hex_value(d)?)));
        *groups.get_mut(count).ok_or(UrlError::Ipv6)? = value.ok_or(UrlError::Ipv6)?;
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// True for a label that a URL parser elsewhere would read as a number:
/// all decimal digits, or `0x` followed by hexadecimal digits.
fn is_numeric_label(label: &[u8]) -> bool {
    if !label.is_empty() && label.iter().all(u8::is_ascii_digit) {
        return true;
    }
    match label {
        [b'0', b'x' | b'X', rest @ ..] => rest.iter().all(u8::is_ascii_hexdigit),
        _ => false,
    }
}

/// Classifies and checks a host that is not in brackets.
fn check_host(host: &[u8]) -> Result<Option<IpAddress>, UrlError> {
    if host.is_empty() {
        return Err(UrlError::Host);
    }
    let last_label = host.rsplit(|&b| b == b'.').next().unwrap_or(&[]);
    // A host ending in a numeric label is an IPv4 address to other
    // parsers (the WHATWG URL Standard's "ends in a number"), so it must be
    // strict dotted decimal here or be rejected: this closes the octal,
    // hexadecimal and shortened forms.
    if is_numeric_label(last_label) {
        return parse_ipv4(host).map(|octets| Some(IpAddress::V4(octets)));
    }
    if host.len() > MAX_HOST_NAME_LENGTH {
        return Err(UrlError::Host);
    }
    for label in host.split(|&b| b == b'.') {
        let valid = !label.is_empty()
            && label.len() <= MAX_LABEL_LENGTH
            && label.iter().all(|&b| b.is_ascii_alphanumeric() || b == b'-')
            && label.first() != Some(&b'-')
            && label.last() != Some(&b'-');
        if !valid {
            return Err(UrlError::Host);
        }
    }
    Ok(None)
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
        for &byte in parts.host {
            writer.byte(byte.to_ascii_lowercase()).map_err(too_long)?;
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

/// RFC 3986 section 5.2.4, writing the result. Percent-encoded dots are
/// not decoded, as the RFC specifies.
fn remove_dot_segments(path: &[u8], writer: &mut Writer<'_>) -> Result<(), crate::bytes::Overflow> {
    let start = writer.len();
    let mut rest = path;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix(b"../").or_else(|| rest.strip_prefix(b"./")) {
            rest = after;
        } else if rest.starts_with(b"/./") {
            rest = rest.get(2..).unwrap_or(&[]);
        } else if rest == b"/." {
            rest = b"/";
        } else if rest.starts_with(b"/../") || rest == b"/.." {
            rest = if rest.len() == 3 { b"/" } else { rest.get(3..).unwrap_or(&[]) };
            // Remove the last segment written, and its preceding slash.
            let written = writer.written().get(start..).unwrap_or(&[]);
            let cut = written.iter().rposition(|&b| b == b'/').unwrap_or(0);
            writer.truncate(start.saturating_add(cut));
        } else if rest == b"." || rest == b".." {
            rest = &[];
        } else {
            let skip = usize::from(rest.first() == Some(&b'/'));
            let end = rest.iter().skip(skip).position(|&b| b == b'/').map_or(rest.len(), |i| i.saturating_add(skip));
            writer.put(rest.get(..end).unwrap_or(&[]))?;
            rest = rest.get(end..).unwrap_or(&[]);
        }
    }
    if writer.len() == start {
        writer.byte(b'/')?;
    }
    Ok(())
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn valid_addresses_normalise() {
        let cases = [
            ("http://example.org", "http://example.org/"),
            ("HTTPS://Example.ORG/Pages/100?x=1", "https://example.org/Pages/100?x=1"),
            ("ws://a.b:8080/ws", "ws://a.b:8080/ws"),
            ("wss://zxnet.co.uk/ws/teefax", "wss://zxnet.co.uk/ws/teefax"),
            ("http://8.8.8.8/", "http://8.8.8.8/"),
            ("http://[2001:db8::1]:81/x", "http://[2001:db8::1]:81/x"),
            ("http://[::FFFF:8.8.8.8]/", "http://[::ffff:8.8.8.8]/"),
            ("http://xn--bcher-kva.example/", "http://xn--bcher-kva.example/"),
            ("http://a.b/x/./y/../z", "http://a.b/x/z"),
            ("http://a.b/%7Euser?q=%20", "http://a.b/%7Euser?q=%20"),
            ("http://a.b?", "http://a.b/?"),
        ];
        for (input, normal) in cases {
            assert_eq!(Url::parse(input).map(|u| u.as_str() == normal), Ok(true), "{input}");
        }
    }

    #[test]
    fn components() {
        let url = Url::parse("https://Example.org:8443/a/b?c=d").unwrap();
        assert_eq!(url.scheme(), Scheme::Https);
        assert_eq!(url.host_text(), "example.org");
        assert_eq!(url.port(), 8443);
        assert_eq!(url.path(), "/a/b");
        assert_eq!(url.query(), Some("c=d"));
        assert_eq!(url.target(), "/a/b?c=d");
        assert_eq!(url.authority(), b"example.org:8443");
        let v6 = Url::parse("http://[::1]/").unwrap();
        assert_eq!(v6.host(), Host::Ipv6([0, 0, 0, 0, 0, 0, 0, 1]));
        assert_eq!(v6.host_text(), "::1");
        assert_eq!(v6.authority(), b"[::1]");
        assert_eq!(Url::parse("http://a.b").unwrap().port(), 80);
        assert_eq!(Url::parse("wss://a.b").unwrap().port(), 443);
    }

    #[test]
    fn invalid_and_deceptive_addresses() {
        let long = {
            let mut text = [b'a'; 2100];
            text[..11].copy_from_slice(b"http://a.b/");
            text
        };
        let long = core::str::from_utf8(&long).unwrap();
        let cases: [(&str, UrlError); 35] = [
            ("ftp://a.b/", UrlError::Scheme),
            ("javascript:alert(1)", UrlError::Scheme),
            ("file:///etc/passwd", UrlError::Scheme),
            ("data:text/plain,x", UrlError::Scheme),
            ("http:a.b", UrlError::Scheme),
            ("http//a.b", UrlError::Scheme),
            ("http://user:pass@a.b/", UrlError::UserInfo),
            ("http://good.example@evil.example/", UrlError::UserInfo),
            ("http://a.b/#frag", UrlError::Fragment),
            ("http://a.b/x y", UrlError::BadCharacter),
            ("http://a.b/\t", UrlError::BadCharacter),
            ("http://a.b/\u{0}", UrlError::BadCharacter),
            ("http://a.b\\@evil.c/", UrlError::Backslash),
            ("http:\\\\a.b/", UrlError::Backslash),
            ("http://0177.0.0.1/", UrlError::Ipv4),
            ("http://0x7f.0.0.1/", UrlError::Ipv4),
            ("http://0x7f000001/", UrlError::Ipv4),
            ("http://2130706433/", UrlError::Ipv4),
            ("http://127.1/", UrlError::Ipv4),
            ("http://256.0.0.1/", UrlError::Ipv4),
            ("http://1.2.3.4.5/", UrlError::Ipv4),
            ("http://b\u{fc}cher.example/", UrlError::BadCharacter),
            ("http://a..b/", UrlError::Host),
            ("http://.a.b/", UrlError::Host),
            ("http://a.b./", UrlError::Host),
            ("http://-a.b/", UrlError::Host),
            ("http://a_b.c/", UrlError::Host),
            ("http:///x", UrlError::Host),
            ("http://a.b:/", UrlError::Port),
            ("http://a.b:0/", UrlError::Port),
            ("http://a.b:65536/", UrlError::Port),
            ("http://a.b:080/", UrlError::Port),
            ("http://[::1%25eth0]/", UrlError::Ipv6),
            ("http://a.b/%zz", UrlError::Path),
            ("http://a.b/<script>", UrlError::Path),
        ];
        for (input, error) in cases {
            assert_eq!(Url::parse(input).map(|u| u.as_str().len()), Err(error), "{input}");
        }
        assert_eq!(Url::parse(long).map(|_| ()), Err(UrlError::TooLong));
        let label = "http://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.b/";
        assert_eq!(Url::parse(label).map(|_| ()), Err(UrlError::Host));
        for bad in ["http://[1:2:3:4:5:6:7:8:9]/", "http://[1::2::3]/", "http://[12345::]/", "http://[::1/"] {
            assert_eq!(Url::parse(bad).map(|_| ()), Err(UrlError::Ipv6), "{bad}");
        }
    }

    /// RFC 3986 section 5.4.1 normal examples, adapted to the schemes and
    /// syntax the strict parser accepts.
    #[test]
    fn relative_resolution() {
        let base = Url::parse("http://a.b/b/c/d;p?q").unwrap();
        let cases = [
            ("g", "http://a.b/b/c/g"),
            ("./g", "http://a.b/b/c/g"),
            ("g/", "http://a.b/b/c/g/"),
            ("/g", "http://a.b/g"),
            ("//g.h", "http://g.h/"),
            ("?y", "http://a.b/b/c/d;p?y"),
            ("g?y", "http://a.b/b/c/g?y"),
            (";x", "http://a.b/b/c/;x"),
            ("g;x", "http://a.b/b/c/g;x"),
            ("", "http://a.b/b/c/d;p?q"),
            (".", "http://a.b/b/c/"),
            ("./", "http://a.b/b/c/"),
            ("..", "http://a.b/b/"),
            ("../", "http://a.b/b/"),
            ("../g", "http://a.b/b/g"),
            ("../..", "http://a.b/"),
            ("../../", "http://a.b/"),
            ("../../g", "http://a.b/g"),
            ("../../../g", "http://a.b/g"),
            ("/./g", "http://a.b/g"),
            ("/../g", "http://a.b/g"),
            ("g.", "http://a.b/b/c/g."),
            ("..g", "http://a.b/b/c/..g"),
            ("https://other.example/x", "https://other.example/x"),
        ];
        for (reference, expected) in cases {
            assert_eq!(base.join(reference).map(|u| u.as_str() == expected), Ok(true), "{reference}");
        }
        assert_eq!(base.join("#f").map(|_| ()), Err(UrlError::Fragment));
        assert_eq!(base.join("\\\\evil").map(|_| ()), Err(UrlError::Backslash));
        assert_eq!(base.join("//user@evil.example/").map(|_| ()), Err(UrlError::UserInfo));
    }

    #[test]
    fn origins() {
        let origin = |text: &str| Url::parse(text).unwrap().origin();
        let pairs = [
            ("http://a.b/", "http://A.B/x", true),
            ("http://a.b/", "http://a.b:80/", true),
            ("https://a.b/", "https://a.b:443/", true),
            ("http://a.b/", "https://a.b/", false),
            ("http://a.b/", "http://a.b:8080/", false),
            ("http://a.b/", "ws://a.b/", true),
            ("https://a.b/", "wss://a.b:443/", true),
            ("https://a.b/", "ws://a.b/", false),
            ("http://a.b/", "http://c.a.b/", false),
            ("http://xn--bcher-kva.example/", "http://XN--BCHER-KVA.example/", true),
            ("http://[::1]/", "http://[0:0:0:0:0:0:0:1]/", false),
        ];
        for (a, b, same) in pairs {
            assert_eq!(origin(a).same(&origin(b)), same, "{a} {b}");
        }
    }

    #[test]
    fn blocked_literals() {
        for blocked in ["http://127.0.0.1/", "http://10.0.0.1/", "http://[::1]/", "http://[::ffff:192.168.0.1]/"] {
            assert!(Url::parse(blocked).unwrap().ip().unwrap().is_blocked(), "{blocked}");
        }
        assert!(!Url::parse("http://8.8.8.8/").unwrap().ip().unwrap().is_blocked());
    }
}
