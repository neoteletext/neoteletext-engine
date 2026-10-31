#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The tests of the strict address parser, relative resolution and
//! origins.

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
        let mut text = [b'a'; MAX_URL_LENGTH + 1];
        text[..11].copy_from_slice(b"http://a.b/");
        text
    };
    let long = core::str::from_utf8(&long).unwrap();
    let cases: [(&str, UrlError); 34] = [
        ("ftp://a.b/", UrlError::Scheme),
        ("javascript:alert(1)", UrlError::Scheme),
        ("file:///etc/passwd", UrlError::Scheme),
        ("data:text/plain,x", UrlError::Scheme),
        ("http:a.b", UrlError::Scheme),
        ("http//a.b", UrlError::Scheme),
        ("http://user:pass@a.b/", UrlError::UserInfo),
        ("http://good.example@evil.example/", UrlError::UserInfo),
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
    let joined = base.join("g#f").unwrap();
    assert_eq!((joined.as_str(), joined.fragment()), ("http://a.b/b/c/g", Some("f")));
    let joined = base.join("#f").unwrap();
    assert_eq!((joined.as_str(), joined.fragment()), (base.as_str(), Some("f")));
    let inherited = base.join("#f").unwrap().join("g").unwrap();
    assert_eq!(inherited.fragment(), None, "a reference never takes the base's fragment");
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
        // Finding F-96: one address, however written, is one origin.
        ("http://[::1]/", "http://[0:0:0:0:0:0:0:1]/", true),
        ("http://[2001:db8::1]/", "http://[2001:0DB8:0:0:0:0:0:0001]/", true),
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

/// Finding F-96 (decision E.114 item 9): an IPv6 host is kept in RFC
/// 5952's text form, so the memory of secure hosts, the cookie host and
/// the table of confirmed origins do not split between two spellings.
#[test]
fn ipv6_hosts_are_kept_in_rfc_5952_form() {
    for (input, canonical) in [
        ("http://[0:0:0:0:0:0:0:1]/", "::1"),
        ("http://[2001:DB8:0:0:1:0:0:1]/", "2001:db8::1:0:0:1"),
        ("http://[2001:db8:0:1:1:1:1:1]/", "2001:db8:0:1:1:1:1:1"),
        ("http://[0001:0db8::]/", "1:db8::"),
        ("http://[1:0:0:2:0:0:0:3]/", "1:0:0:2::3"),
        ("http://[1:0:0:2:3:0:0:4]/", "1::2:3:0:0:4"),
        ("http://[::]/", "::"),
        ("http://[0:0:0:0:0:ffff:808:808]/", "::ffff:8.8.8.8"),
        ("http://[::1.2.3.4]/", "::102:304"),
        ("http://[fe80:0:0:0:0:0:0:0]/", "fe80::"),
    ] {
        let url = Url::parse(input).unwrap();
        assert_eq!(url.host_text(), canonical, "{input}");
        assert_eq!(url.as_str(), std::format!("http://[{canonical}]/"), "{input}");
    }
}

/// REQ-WEB-016, REQ-1661: 8,000 octets is accepted, and one more is not.
#[test]
fn the_longest_address_is_8000_octets() {
    let start = "https://a.example/";
    let exact = std::format!("{start}{}", "a".repeat(8000 - start.len()));
    assert_eq!(Url::parse(&exact).map(|url| url.as_str().len()), Ok(8000));
    let over = std::format!("{exact}a");
    assert_eq!(Url::parse(&over).map(|_| ()), Err(UrlError::TooLong));
}

/// REQ-WEB-016: a fragment is kept apart and reaches nothing that is sent.
#[test]
fn a_fragment_is_kept_apart() {
    let url = Url::parse("https://a.example/b?c=d#302").unwrap();
    assert_eq!(url.as_str(), "https://a.example/b?c=d");
    assert_eq!(url.target(), "/b?c=d");
    assert_eq!(url.path(), "/b");
    assert_eq!(url.query(), Some("c=d"));
    assert_eq!(url.fragment(), Some("302"));
    let bare = Url::parse("https://a.example#302").unwrap();
    assert_eq!((bare.as_str(), bare.target(), bare.fragment()), ("https://a.example/", "/", Some("302")));
    assert_eq!(Url::parse("https://a.example/").unwrap().fragment(), None);
    assert_eq!(Url::parse("https://a.example/#").unwrap().fragment(), Some(""));
}

/// REQ-WEB-016: a `#` can never hide the real host.
#[test]
fn a_fragment_cannot_pose_as_the_host() {
    let url = Url::parse("https://evil.example#@good.example").unwrap();
    assert_eq!(url.host_text(), "evil.example");
    assert_eq!(url.fragment(), Some("@good.example"));
    assert_eq!(Url::parse("https://a.example/#b%zz").map(|_| ()), Err(UrlError::Path));
    assert_eq!(Url::parse("https://a.example/#b#c").map(|_| ()), Err(UrlError::Path));
}
