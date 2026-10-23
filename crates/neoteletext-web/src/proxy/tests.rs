#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
//! The tests of the proxy handshakes and the proxy setting's syntax.

use super::*;
use crate::http::HeadCollector;
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
    assert_eq!(read_socks_reply(&[5, 5, 0, 1, 10, 0, 0, 1, 0, 80]), Err(ProxyError::Refused(5)), "connection refused");
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
