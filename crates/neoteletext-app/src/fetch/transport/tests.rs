//! The transport through a proxy played on this computer (REQ-WEB-032).
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "a test")]

use super::*;

/// A proxy played on this computer: it reads the client's handshake,
/// answers with `answer`, then sends `HOST`, as the host would.
fn played_proxy(kind: neoteletext_web::proxy::ProxyKind, answer: Vec<u8>) -> (u16, std::thread::JoinHandle<Vec<u8>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut heard = Vec::new();
        let mut byte = [0_u8; 1];
        let wanted = |heard: &[u8]| match kind {
            neoteletext_web::proxy::ProxyKind::Http => heard.ends_with(b"\r\n\r\n"),
            // The greeting, then the request for a 12-byte name.
            neoteletext_web::proxy::ProxyKind::Socks5 => heard.len() == 3 + 5 + 12 + 2,
        };
        while !wanted(&heard) {
            if kind == neoteletext_web::proxy::ProxyKind::Socks5 && heard.len() == 3 {
                stream.write_all(&[5, 0]).unwrap();
            }
            stream.read_exact(&mut byte).unwrap();
            heard.push(byte[0]);
        }
        // The answer and the host's first bytes in one write, as a proxy
        // relaying a server that speaks at once may send them.
        stream.write_all(&[answer.as_slice(), b"HOST"].concat()).unwrap();
        heard
    });
    (port, handle)
}

fn through(kind: neoteletext_web::proxy::ProxyKind, port: u16) -> StdTransport {
    let (wake, _woken) = mpsc::channel();
    StdTransport::new(&[], wake, Some(Proxy { kind, host: "127.0.0.1".into(), port }))
}

/// REQ-WEB-032: an HTTP proxy by CONNECT and a SOCKS5 proxy with the
/// host's name, on this computer, which the user chose; the tunnel's
/// first bytes are the host's.
#[test]
fn connections_go_through_the_proxy() {
    use neoteletext_web::proxy::ProxyKind;
    let (port, proxy) = played_proxy(ProxyKind::Http, b"HTTP/1.1 200 Connection established\r\n\r\n".to_vec());
    // What the proxy's answer brought, then what the socket gives.
    let first_four = |mut socket: TcpStream, early: Vec<u8>| {
        let mut host = early;
        let mut rest = vec![0_u8; b"HOST".len().saturating_sub(host.len())];
        socket.read_exact(&mut rest).unwrap();
        host.extend(rest);
        host
    };
    let (socket, peer, early) = through(ProxyKind::Http, port).reach("news.example", 443).unwrap();
    assert_eq!(peer, IpAddr::from([127, 0, 0, 1]), "the peer is the proxy");
    assert_eq!(first_four(socket, early), b"HOST");
    assert!(proxy.join().unwrap().starts_with(b"CONNECT news.example:443 HTTP/1.1\r\n"));
    let (port, proxy) = played_proxy(ProxyKind::Socks5, vec![5, 0, 0, 1, 10, 0, 0, 1, 0, 80]);
    let (socket, _, early) = through(ProxyKind::Socks5, port).reach("news.example", 80).unwrap();
    assert!(early.is_empty(), "a SOCKS5 answer is read to its length");
    assert_eq!(first_four(socket, early), b"HOST");
    assert_eq!(&proxy.join().unwrap()[3..8], &[5, 1, 0, 3, 12], "the name for the proxy to resolve");
}

/// REQ-WEB-032: the host's first bytes that came with the proxy's
/// answer reach the connection's reader before the socket's next.
#[test]
fn the_tunnels_first_bytes_reach_the_connection() {
    use neoteletext_web::proxy::ProxyKind;
    let (port, proxy) = played_proxy(ProxyKind::Http, b"HTTP/1.1 200 Connection established\r\n\r\n".to_vec());
    let transport = through(ProxyKind::Http, port);
    let (mut connection, _) = transport.open("news.example", 80, Security::Plain).unwrap();
    proxy.join().unwrap();
    let mut seen = Vec::new();
    let mut buffer = [0_u8; 16];
    let started = std::time::Instant::now();
    while seen.len() < b"HOST".len() && started.elapsed() < Duration::from_secs(5) {
        match connection.receive(&mut buffer) {
            Received::Data(count) => seen.extend_from_slice(&buffer[..count]),
            Received::Nothing => std::thread::yield_now(),
            Received::Closed => break,
        }
    }
    assert_eq!(seen, b"HOST");
}

/// REQ-WEB-032: a refusing proxy fails the connection; nothing goes
/// direct.
#[test]
fn a_refusing_proxy_fails_the_connection() {
    use neoteletext_web::proxy::ProxyKind;
    let (port, proxy) = played_proxy(ProxyKind::Http, b"HTTP/1.1 403 Forbidden\r\n\r\n".to_vec());
    let error = through(ProxyKind::Http, port).reach("news.example", 443).unwrap_err();
    assert!(error.contains("the proxy refused the connection to news.example"), "{error}");
    proxy.join().unwrap();
    assert!(Proxy::parse("ftp://p.example:21").is_err());
}
