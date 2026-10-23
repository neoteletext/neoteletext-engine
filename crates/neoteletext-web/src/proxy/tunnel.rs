//! The blocking proxy handshake over any `std::io` stream (REQ-WEB-032):
//! the one place this crate waits on a stream, for a port with the
//! standard library.

use super::connect::{read_connect_reply, write_connect};
use super::socks::{
    SOCKS_GREETING, SOCKS_MAX_NAME, SOCKS_MAX_REPLY, read_socks_choice, read_socks_reply, socks_reply_length,
    write_socks_connect,
};
use super::{ProxyError, ProxyKind, parse_proxy};
use crate::http::HeadCollector;

/// A proxy a port keeps: [`ProxyAddress`] owned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proxy {
    /// HTTP or SOCKS5.
    pub kind: ProxyKind,
    /// Its host.
    pub host: std::string::String,
    /// Its port.
    pub port: u16,
}

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
    /// opens the stream: this crate reaches no network (REQ-SEC-016). The
    /// bytes it returns are the first of the tunnel's, read with the
    /// proxy's answer: they come before anything read from `stream` next.
    ///
    /// # Errors
    /// The proxy refuses, breaks its protocol, or the stream fails.
    pub fn open_tunnel<S: std::io::Read + std::io::Write>(
        &self,
        stream: &mut S,
        host: &str,
        port: u16,
    ) -> Result<std::vec::Vec<u8>, std::string::String> {
        tunnel(stream, self.kind, host, port)
            .map_err(|error| std::format!("the proxy refused the connection to {host}: {error:?}"))
    }
}

/// Why a blocking handshake failed.
#[derive(Debug)]
pub enum TunnelError {
    /// The proxy refused or broke its protocol.
    Proxy(ProxyError),
    /// The connection failed or timed out.
    Io(std::io::Error),
}

/// The longest answer to `CONNECT` read: the engine's default head limit.
const MAX_CONNECT_REPLY: usize = crate::consts::DEFAULT_HEADER_LIMIT;

/// Bytes read from the proxy at a time while its answer to `CONNECT` is
/// awaited: a whole answer of a common proxy in one read.
const CONNECT_READ: usize = 1024;

/// Asks the proxy at the other end of `stream` for a tunnel to `host` and
/// `port`, blocking within the stream's own timeouts. On success the
/// stream reaches the host, and the bytes returned are the first of the
/// host's, read in the same reads as the proxy's answer (REQ-WEB-032):
/// the caller hands them on before anything it reads from `stream`. A
/// SOCKS5 answer is read to its exact length, so it returns none.
///
/// # Errors
/// The proxy's refusal or broken answer, or the stream's failure.
pub fn tunnel<S: std::io::Read + std::io::Write>(
    stream: &mut S,
    kind: ProxyKind,
    host: &str,
    port: u16,
) -> Result<std::vec::Vec<u8>, TunnelError> {
    let mut request = [0_u8; 4 + 1 + SOCKS_MAX_NAME + 2 + 32];
    match kind {
        ProxyKind::Http => {
            let length = write_connect(host, port, &mut request).map_err(TunnelError::Proxy)?;
            stream.write_all(request.get(..length).unwrap_or(&[])).map_err(TunnelError::Io)?;
            stream.flush().map_err(TunnelError::Io)?;
            let mut collector = HeadCollector::default();
            let mut head = std::vec![0_u8; MAX_CONNECT_REPLY];
            let mut chunk = [0_u8; CONNECT_READ];
            loop {
                let count = match stream.read(&mut chunk) {
                    Ok(0) => return Err(TunnelError::Io(std::io::ErrorKind::UnexpectedEof.into())),
                    Ok(count) => count,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(TunnelError::Io(error)),
                };
                let input = chunk.get(..count).unwrap_or(&[]);
                // The head reader bounds the answer to `MAX_CONNECT_REPLY`.
                if let Some(used) = read_connect_reply(&mut collector, &mut head, input).map_err(TunnelError::Proxy)? {
                    return Ok(input.get(used..).unwrap_or(&[]).to_vec());
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
            read_socks_reply(reply.get(..total).unwrap_or(&[])).map_err(TunnelError::Proxy)?;
            Ok(std::vec::Vec::new())
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// A stream whose reads give the answer in the pieces it was cut into,
    /// one piece a read, as a socket gives what has arrived.
    struct Arrivals {
        pieces: std::collections::VecDeque<Vec<u8>>,
        reads: usize,
        sent: Vec<u8>,
    }

    impl Arrivals {
        fn new(pieces: &[&[u8]]) -> Self {
            Self { pieces: pieces.iter().map(|piece| piece.to_vec()).collect(), reads: 0, sent: Vec::new() }
        }
    }

    impl std::io::Read for Arrivals {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.reads = self.reads.saturating_add(1);
            let Some(mut piece) = self.pieces.pop_front() else { return Ok(0) };
            let count = piece.len().min(buffer.len());
            buffer[..count].copy_from_slice(&piece[..count]);
            if count < piece.len() {
                self.pieces.push_front(piece.split_off(count));
            }
            Ok(count)
        }
    }

    impl std::io::Write for Arrivals {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.sent.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// The proxy's answer and the host's first bytes arrive in one read:
    /// they are handed back, not lost, and the answer takes one read, not
    /// one a byte.
    #[test]
    fn the_tunnels_first_bytes_read_with_the_answer_are_handed_back() {
        let mut http = Arrivals::new(&[b"HTTP/1.1 200 Connection established\r\nVia: p\r\n\r\n\x16\x03\x01HOST"]);
        let early = tunnel(&mut http, ProxyKind::Http, "news.example", 443).unwrap();
        assert!(http.sent.starts_with(b"CONNECT news.example:443 "));
        assert_eq!(early, b"\x16\x03\x01HOST", "the host's bytes are the caller's");
        assert_eq!(http.reads, 1, "the answer is read whole, not a byte at a time");

        let mut split = Arrivals::new(&[b"HTTP/1.1 200 OK\r", b"\n\r\n", b"LATER"]);
        assert_eq!(tunnel(&mut split, ProxyKind::Http, "news.example", 443).unwrap(), b"");
        assert_eq!(split.pieces.front().map(Vec::as_slice), Some(&b"LATER"[..]), "nothing past the answer was read");

        let mut socks = Arrivals::new(&[&[5, 0], &[5, 0, 0, 1, 10, 0, 0, 1, 0, 80, b'H']]);
        assert_eq!(tunnel(&mut socks, ProxyKind::Socks5, "news.example", 80).unwrap(), b"");
        assert_eq!(&socks.sent[..3], &SOCKS_GREETING);

        let mut refused = Arrivals::new(&[b"HTTP/1.1 502 Bad Gateway\r\n\r\n"]);
        assert!(matches!(
            tunnel(&mut refused, ProxyKind::Http, "x.example", 80),
            Err(TunnelError::Proxy(ProxyError::Refused(502)))
        ));
        let mut closed = Arrivals::new(&[b"HTTP/1.1 200 OK\r\n"]);
        assert!(matches!(tunnel(&mut closed, ProxyKind::Http, "x.example", 80), Err(TunnelError::Io(_))));
        let mut silent = Arrivals::new(&[]);
        assert!(matches!(tunnel(&mut silent, ProxyKind::Socks5, "x.example", 80), Err(TunnelError::Io(_))));
        let endless: Vec<u8> = [b"HTTP/1.1 200 OK\r\nX: ".as_slice(), &[b'a'; MAX_CONNECT_REPLY]].concat();
        let mut long = Arrivals::new(&[&endless]);
        assert!(
            matches!(
                tunnel(&mut long, ProxyKind::Http, "x.example", 80),
                Err(TunnelError::Proxy(ProxyError::Malformed))
            ),
            "an answer longer than the head limit"
        );
    }
}
