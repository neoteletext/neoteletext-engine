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

use crate::bytes::Writer;
use crate::http::{HeadCollector, parse_head};
use crate::url::{IpAddress, ip_literal};

/// The SOCKS protocol's version (RFC 1928 section 3).
const SOCKS_VERSION: u8 = 5;
/// "No authentication required" (RFC 1928 section 3).
const SOCKS_NO_AUTHENTICATION: u8 = 0;
/// The CONNECT command (RFC 1928 section 4).
const SOCKS_CONNECT: u8 = 1;
/// The address types (RFC 1928 section 5).
const SOCKS_IPV4: u8 = 1;
const SOCKS_NAME: u8 = 3;
const SOCKS_IPV6: u8 = 4;
/// The reserved byte (RFC 1928 sections 4 and 6).
const SOCKS_RESERVED: u8 = 0;
/// "Succeeded" (RFC 1928 section 6).
const SOCKS_SUCCEEDED: u8 = 0;
/// The longest name SOCKS5 carries: its length is one byte.
const SOCKS_MAX_NAME: usize = 255;
/// The greeting: version 5, one method, no authentication.
pub const SOCKS_GREETING: [u8; 3] = [SOCKS_VERSION, 1, SOCKS_NO_AUTHENTICATION];
/// The longest SOCKS5 reply: the header, a name of 255 bytes, its length
/// and the port.
pub const SOCKS_MAX_REPLY: usize = 4 + 1 + SOCKS_MAX_NAME + 2;
/// The success status class of an HTTP response.
const HTTP_SUCCESS: core::ops::Range<u16> = 200..300;

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

/// Writes the `CONNECT` request for `host` and `port` (RFC 9110 section
/// 9.3.6), an IPv6 address in brackets.
///
/// # Errors
/// When it does not fit `out`.
pub fn write_connect(host: &str, port: u16, out: &mut [u8]) -> Result<usize, ProxyError> {
    let mut writer = Writer::new(out);
    let authority = |writer: &mut Writer<'_>| -> Result<(), ProxyError> {
        let bracket = host.contains(':');
        if bracket {
            writer.put(b"[").map_err(|_| ProxyError::TooLong)?;
        }
        writer.put(host.as_bytes()).map_err(|_| ProxyError::TooLong)?;
        if bracket {
            writer.put(b"]").map_err(|_| ProxyError::TooLong)?;
        }
        let mut digits = [0_u8; 5];
        writer.put(b":").map_err(|_| ProxyError::TooLong)?;
        writer.put(decimal(port, &mut digits)).map_err(|_| ProxyError::TooLong)
    };
    writer.put(b"CONNECT ").map_err(|_| ProxyError::TooLong)?;
    authority(&mut writer)?;
    writer.put(b" HTTP/1.1\r\nHost: ").map_err(|_| ProxyError::TooLong)?;
    authority(&mut writer)?;
    writer.put(b"\r\n\r\n").map_err(|_| ProxyError::TooLong)?;
    Ok(writer.written().len())
}

/// `value` in decimal digits, in `digits`.
fn decimal(value: u16, digits: &mut [u8; 5]) -> &[u8] {
    let mut start = digits.len();
    let mut rest = value;
    loop {
        start = start.saturating_sub(1);
        if let Some(digit) = digits.get_mut(start) {
            *digit = b'0'.saturating_add(u8::try_from(rest % 10).unwrap_or(0));
        }
        rest /= 10;
        if rest == 0 || start == 0 {
            break;
        }
    }
    digits.get(start..).unwrap_or(&[])
}

/// Reads an HTTP proxy's answer to `CONNECT`, collecting its head in
/// `buffer` across calls: `Ok(Some(used))` once a success's head is whole,
/// `used` being the bytes of `input` it took (what follows is the
/// tunnel's), `Ok(None)` when more is needed.
///
/// # Errors
/// A malformed head, a head longer than `buffer`, or any status but
/// success.
pub fn read_connect_reply(
    collector: &mut HeadCollector,
    buffer: &mut [u8],
    input: &[u8],
) -> Result<Option<usize>, ProxyError> {
    let (used, complete) = collector.feed(buffer, input).map_err(|_| ProxyError::Malformed)?;
    if !complete {
        return Ok(None);
    }
    let head = buffer.get(..collector.len()).ok_or(ProxyError::Malformed)?;
    let parsed = parse_head(head, &mut |_| {}).map_err(|_| ProxyError::Malformed)?;
    if HTTP_SUCCESS.contains(&parsed.status) { Ok(Some(used)) } else { Err(ProxyError::Refused(parsed.status)) }
}

/// Reads the SOCKS5 proxy's choice of method after [`SOCKS_GREETING`]:
/// two bytes.
///
/// # Errors
/// Another version, or a method but no authentication.
pub fn read_socks_choice(reply: [u8; 2]) -> Result<(), ProxyError> {
    match reply {
        [SOCKS_VERSION, SOCKS_NO_AUTHENTICATION] => Ok(()),
        [SOCKS_VERSION, _] => Err(ProxyError::NeedsAuthentication),
        _ => Err(ProxyError::Malformed),
    }
}

/// Writes the SOCKS5 request to connect to `host` and `port`: the address
/// when the host is one, otherwise the name for the proxy to resolve.
///
/// # Errors
/// A name longer than 255 bytes, or `out` too small.
pub fn write_socks_connect(host: &str, port: u16, out: &mut [u8]) -> Result<usize, ProxyError> {
    let mut writer = Writer::new(out);
    writer.put(&[SOCKS_VERSION, SOCKS_CONNECT, SOCKS_RESERVED]).map_err(|_| ProxyError::TooLong)?;
    match ip_literal(host) {
        Some(IpAddress::V4(octets)) => {
            writer.put(&[SOCKS_IPV4]).map_err(|_| ProxyError::TooLong)?;
            writer.put(&octets).map_err(|_| ProxyError::TooLong)?;
        }
        Some(IpAddress::V6(groups)) => {
            writer.put(&[SOCKS_IPV6]).map_err(|_| ProxyError::TooLong)?;
            for group in groups {
                writer.put(&group.to_be_bytes()).map_err(|_| ProxyError::TooLong)?;
            }
        }
        None => {
            let length = u8::try_from(host.len()).map_err(|_| ProxyError::TooLong)?;
            if host.is_empty() {
                return Err(ProxyError::TooLong);
            }
            writer.put(&[SOCKS_NAME, length]).map_err(|_| ProxyError::TooLong)?;
            writer.put(host.as_bytes()).map_err(|_| ProxyError::TooLong)?;
        }
    }
    writer.put(&port.to_be_bytes()).map_err(|_| ProxyError::TooLong)?;
    Ok(writer.written().len())
}

/// The length of the SOCKS5 reply that begins with `start`, its first five
/// bytes, so a transport knows how many to read.
///
/// # Errors
/// Another version, a reserved byte not zero, or an unknown address type.
pub fn socks_reply_length(start: [u8; 5]) -> Result<usize, ProxyError> {
    let [version, _, reserved, kind, first] = start;
    if version != SOCKS_VERSION || reserved != SOCKS_RESERVED {
        return Err(ProxyError::Malformed);
    }
    let address = match kind {
        SOCKS_IPV4 => 4,
        SOCKS_IPV6 => 16,
        SOCKS_NAME => 1 + usize::from(first),
        _ => return Err(ProxyError::Malformed),
    };
    // The version, reply, reserved and type bytes, then the address, then
    // the port's two.
    Ok(address.saturating_add(4 + 2))
}

/// Reads a whole SOCKS5 reply, of the length [`socks_reply_length`] gave.
///
/// # Errors
/// As [`socks_reply_length`], a reply of another length, or any reply code
/// but success.
pub fn read_socks_reply(reply: &[u8]) -> Result<(), ProxyError> {
    let start: [u8; 5] = reply.get(..5).and_then(|start| start.try_into().ok()).ok_or(ProxyError::Malformed)?;
    if socks_reply_length(start)? != reply.len() {
        return Err(ProxyError::Malformed);
    }
    match start.get(1).copied() {
        Some(SOCKS_SUCCEEDED) => Ok(()),
        Some(code) => Err(ProxyError::Refused(u16::from(code))),
        None => Err(ProxyError::Malformed),
    }
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

/// A proxy a port keeps: [`ProxyAddress`] owned.
#[cfg(feature = "std")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proxy {
    /// HTTP or SOCKS5.
    pub kind: ProxyKind,
    /// Its host.
    pub host: std::string::String,
    /// Its port.
    pub port: u16,
}

#[cfg(feature = "std")]
impl Proxy {
    /// Reads `http://host:port` or `socks5://host:port` ([`parse_proxy`]).
    ///
    /// # Errors
    /// Any other form, said for the user.
    pub fn parse(text: &str) -> Result<Self, std::string::String> {
        use std::borrow::ToOwned;
        parse_proxy(text)
            .map(|proxy| Self { kind: proxy.kind, host: proxy.host.to_owned(), port: proxy.port })
            .ok_or_else(|| std::format!("{text}: a proxy is http://host:port or socks5://host:port"))
    }

    /// Asks the proxy at the other end of `stream` for a tunnel to `host`
    /// and `port` ([`tunnel`]), with the error said for the user. The port
    /// opens the stream: this crate reaches no network (REQ-SEC-016).
    ///
    /// # Errors
    /// The proxy refuses, breaks its protocol, or the stream fails.
    pub fn open_tunnel<S: std::io::Read + std::io::Write>(
        &self,
        stream: &mut S,
        host: &str,
        port: u16,
    ) -> Result<(), std::string::String> {
        tunnel(stream, self.kind, host, port)
            .map_err(|error| std::format!("the proxy refused the connection to {host}: {error:?}"))
    }
}

/// Why a blocking handshake failed.
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum TunnelError {
    /// The proxy refused or broke its protocol.
    Proxy(ProxyError),
    /// The connection failed or timed out.
    Io(std::io::Error),
}

/// The longest answer to `CONNECT` read: the engine's default head limit.
#[cfg(feature = "std")]
const MAX_CONNECT_REPLY: usize = crate::consts::DEFAULT_HEADER_LIMIT;

/// Asks the proxy at the other end of `stream` for a tunnel to `host` and
/// `port`, blocking within the stream's own timeouts. On success the
/// stream reaches the host, and nothing of the host's has been read: the
/// answer is read a byte at a time to its end.
///
/// # Errors
/// The proxy's refusal or broken answer, or the stream's failure.
#[cfg(feature = "std")]
pub fn tunnel<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    kind: ProxyKind,
    host: &str,
    port: u16,
) -> Result<(), TunnelError> {
    let mut request = [0_u8; 4 + 1 + SOCKS_MAX_NAME + 2 + 32];
    match kind {
        ProxyKind::Http => {
            let length = write_connect(host, port, &mut request).map_err(TunnelError::Proxy)?;
            stream.write_all(request.get(..length).unwrap_or(&[])).map_err(TunnelError::Io)?;
            stream.flush().map_err(TunnelError::Io)?;
            let mut collector = HeadCollector::default();
            let mut head = std::vec![0_u8; MAX_CONNECT_REPLY];
            let mut byte = [0_u8; 1];
            loop {
                stream.read_exact(&mut byte).map_err(TunnelError::Io)?;
                if read_connect_reply(&mut collector, &mut head, &byte).map_err(TunnelError::Proxy)?.is_some() {
                    return Ok(());
                }
            }
        }
        ProxyKind::Socks5 => {
            stream.write_all(&SOCKS_GREETING).map_err(TunnelError::Io)?;
            stream.flush().map_err(TunnelError::Io)?;
            let mut choice = [0_u8; 2];
            stream.read_exact(&mut choice).map_err(TunnelError::Io)?;
            read_socks_choice(choice).map_err(TunnelError::Proxy)?;
            let length = write_socks_connect(host, port, &mut request).map_err(TunnelError::Proxy)?;
            stream.write_all(request.get(..length).unwrap_or(&[])).map_err(TunnelError::Io)?;
            stream.flush().map_err(TunnelError::Io)?;
            let mut reply = [0_u8; SOCKS_MAX_REPLY];
            let (start, rest) = reply.split_at_mut(5);
            stream.read_exact(start).map_err(TunnelError::Io)?;
            let start_bytes: [u8; 5] = start.try_into().map_err(|_| TunnelError::Proxy(ProxyError::Malformed))?;
            let total = socks_reply_length(start_bytes).map_err(TunnelError::Proxy)?;
            let rest = rest.get_mut(..total.saturating_sub(5)).ok_or(TunnelError::Proxy(ProxyError::Malformed))?;
            stream.read_exact(rest).map_err(TunnelError::Io)?;
            read_socks_reply(reply.get(..total).unwrap_or(&[])).map_err(TunnelError::Proxy)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn connect(host: &str, port: u16) -> std::string::String {
        let mut out = [0_u8; 256];
        let length = write_connect(host, port, &mut out).unwrap();
        std::string::String::from_utf8(out[..length].to_vec()).unwrap()
    }

    #[test]
    fn connect_names_the_authority() {
        assert_eq!(connect("news.example", 443), "CONNECT news.example:443 HTTP/1.1\r\nHost: news.example:443\r\n\r\n");
        assert_eq!(connect("2001:db8::1", 80), "CONNECT [2001:db8::1]:80 HTTP/1.1\r\nHost: [2001:db8::1]:80\r\n\r\n");
        assert_eq!(connect("x", 0), "CONNECT x:0 HTTP/1.1\r\nHost: x:0\r\n\r\n");
        assert_eq!(write_connect("news.example", 443, &mut [0; 20]), Err(ProxyError::TooLong));
    }

    fn reply(chunks: &[&[u8]]) -> Result<Option<usize>, ProxyError> {
        let mut collector = HeadCollector::default();
        let mut buffer = [0_u8; 512];
        let mut last = Ok(None);
        for chunk in chunks {
            last = read_connect_reply(&mut collector, &mut buffer, chunk);
            if !matches!(last, Ok(None)) {
                return last;
            }
        }
        last
    }

    #[test]
    fn connect_replies_succeed_only_with_success() {
        assert_eq!(reply(&[b"HTTP/1.1 200 Connection established\r\n\r\n"]), Ok(Some(39)));
        assert_eq!(
            reply(&[b"HTTP/1.1 200 OK\r\n", b"Via: proxy\r\n\r\n\x16\x03"]),
            Ok(Some(14)),
            "what follows is the tunnel's"
        );
        assert_eq!(reply(&[b"HTTP/1.1 403 Forbidden\r\n\r\n"]), Err(ProxyError::Refused(403)));
        assert_eq!(reply(&[b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n"]), Err(ProxyError::Refused(407)));
        assert_eq!(reply(&[b"not http\r\n\r\n"]), Err(ProxyError::Malformed));
        assert_eq!(reply(&[b"HTTP/1.1 200 OK\r\n"]), Ok(None), "more is needed");
        let long: Vec<u8> = [b"HTTP/1.1 200 OK\r\nX: ".as_slice(), &[b'a'; 600], b"\r\n\r\n"].concat();
        assert_eq!(reply(&[&long]), Err(ProxyError::Malformed), "a head longer than its buffer");
    }

    #[test]
    fn socks_requests_carry_a_name_or_an_address() {
        let mut out = [0_u8; 300];
        let length = write_socks_connect("news.example", 443, &mut out).unwrap();
        assert_eq!(&out[..length], [&[5, 1, 0, 3, 12][..], b"news.example", &[1, 187]].concat());
        let length = write_socks_connect("192.0.2.7", 80, &mut out).unwrap();
        assert_eq!(&out[..length], [5, 1, 0, 1, 192, 0, 2, 7, 0, 80]);
        let length = write_socks_connect("2001:db8::1", 80, &mut out).unwrap();
        assert_eq!(&out[..length], [&[5, 1, 0, 4, 0x20, 0x01, 0x0d, 0xb8][..], &[0; 11], &[1, 0, 80]].concat());
        assert_eq!(write_socks_connect(&"a".repeat(256), 80, &mut out), Err(ProxyError::TooLong));
        assert_eq!(write_socks_connect("", 80, &mut out), Err(ProxyError::TooLong));
    }

    #[test]
    fn socks_replies_are_checked() {
        assert_eq!(read_socks_choice([5, 0]), Ok(()));
        assert_eq!(read_socks_choice([5, 2]), Err(ProxyError::NeedsAuthentication));
        assert_eq!(read_socks_choice([4, 0]), Err(ProxyError::Malformed));
        assert_eq!(socks_reply_length([5, 0, 0, 1, 10]), Ok(10));
        assert_eq!(socks_reply_length([5, 0, 0, 4, 0]), Ok(22));
        assert_eq!(socks_reply_length([5, 0, 0, 3, 7]), Ok(4 + 8 + 2));
        assert_eq!(socks_reply_length([5, 0, 0, 9, 0]), Err(ProxyError::Malformed));
        assert_eq!(read_socks_reply(&[5, 0, 0, 1, 10, 0, 0, 1, 0, 80]), Ok(()));
        assert_eq!(
            read_socks_reply(&[5, 5, 0, 1, 10, 0, 0, 1, 0, 80]),
            Err(ProxyError::Refused(5)),
            "connection refused"
        );
        assert_eq!(read_socks_reply(&[5, 0, 0, 1, 10, 0]), Err(ProxyError::Malformed), "short");
    }

    #[test]
    fn proxy_addresses_are_read_strictly() {
        assert_eq!(
            parse_proxy("http://proxy.example:3128"),
            Some(ProxyAddress { kind: ProxyKind::Http, host: "proxy.example", port: 3128 })
        );
        assert_eq!(
            parse_proxy("socks5://127.0.0.1:1080/"),
            Some(ProxyAddress { kind: ProxyKind::Socks5, host: "127.0.0.1", port: 1080 })
        );
        assert_eq!(
            parse_proxy("socks5://[::1]:1080"),
            Some(ProxyAddress { kind: ProxyKind::Socks5, host: "::1", port: 1080 })
        );
        for refused in [
            "https://p.example:443",
            "http://p.example",
            "http://p.example:0",
            "http://user:pw@p.example:80",
            "http://p.example:80/path",
            "http://:80",
            "socks4://p.example:1080",
            "http://p example:80",
        ] {
            assert_eq!(parse_proxy(refused), None, "{refused}");
        }
    }

    /// The blocking handshake against an answer played from memory.
    #[cfg(feature = "std")]
    struct Played {
        answer: std::io::Cursor<Vec<u8>>,
        sent: Vec<u8>,
    }

    #[cfg(feature = "std")]
    impl std::io::Read for Played {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.answer.read(buffer)
        }
    }

    #[cfg(feature = "std")]
    impl std::io::Write for Played {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.sent.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[cfg(feature = "std")]
    #[test]
    fn the_handshake_reads_no_byte_of_the_tunnel() {
        use std::io::Read as _;
        let mut http =
            Played { answer: std::io::Cursor::new(b"HTTP/1.1 200 OK\r\n\r\nHOST".to_vec()), sent: Vec::new() };
        tunnel(&mut http, ProxyKind::Http, "news.example", 443).unwrap();
        assert!(http.sent.starts_with(b"CONNECT news.example:443 "));
        let mut after = std::string::String::new();
        http.read_to_string(&mut after).unwrap();
        assert_eq!(after, "HOST", "the host's bytes are left for the tunnel");
        let mut socks = Played {
            answer: std::io::Cursor::new(std::vec![5, 0, 5, 0, 0, 1, 10, 0, 0, 1, 0, 80, b'H']),
            sent: Vec::new(),
        };
        tunnel(&mut socks, ProxyKind::Socks5, "news.example", 80).unwrap();
        assert_eq!(&socks.sent[..3], &SOCKS_GREETING);
        let mut refused =
            Played { answer: std::io::Cursor::new(b"HTTP/1.1 502 Bad Gateway\r\n\r\n".to_vec()), sent: Vec::new() };
        assert!(matches!(
            tunnel(&mut refused, ProxyKind::Http, "x.example", 80),
            Err(TunnelError::Proxy(ProxyError::Refused(502)))
        ));
        let mut silent = Played { answer: std::io::Cursor::new(Vec::new()), sent: Vec::new() };
        assert!(matches!(tunnel(&mut silent, ProxyKind::Socks5, "x.example", 80), Err(TunnelError::Io(_))));
    }

    /// REQ-WEB-032 with REQ-SEC-006: what the client can see as local.
    #[test]
    fn local_hosts_are_seen_before_the_proxy() {
        for local in [
            "localhost",
            "LOCALHOST.",
            "printer.local",
            "db.internal",
            "app.localhost",
            "127.0.0.1",
            "192.168.1.20",
            "10.0.0.1",
            "::1",
            "fe80::1",
        ] {
            assert!(hidden_local(local), "{local}");
        }
        for public in
            ["news.example", "local.example", "internal.example", "93.184.216.34", "2001:db8::1", "localhost.example"]
        {
            assert!(!hidden_local(public), "{public}");
        }
    }
}
