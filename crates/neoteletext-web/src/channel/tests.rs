#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
//! The tests of the channel folder and file rules (REQ-WEB-023, REQ-WEB-027).

use super::*;

fn folder(text: &str) -> Result<std::string::String, ChannelError> {
    parse_address(text).map(|address| {
        assert!(address.file.is_none(), "{text}");
        std::string::String::from(address.folder.as_str())
    })
}

#[test]
fn folders_are_checked_as_written() {
    assert_eq!(folder("https://example.org"), Ok("https://example.org/".into()));
    assert_eq!(folder("https://example.org/"), Ok("https://example.org/".into()));
    assert_eq!(folder("https://User.GitHub.io/ceefax/"), Ok("https://user.github.io/ceefax/".into()));
    assert_eq!(folder("http://h.example:8080/~ann/v1.2/"), Ok("http://h.example:8080/~ann/v1.2/".into()));
    assert_eq!(folder("https://h.example/a/../b/"), Err(ChannelError::DotSegment));
    assert_eq!(folder("https://h.example/a/./"), Err(ChannelError::DotSegment));
    assert_eq!(folder("https://h.example/a/.../"), Err(ChannelError::DotSegment));
    assert_eq!(folder("https://h.example/a%2Fb/"), Err(ChannelError::Character));
    assert_eq!(folder("https://h.example/%2E%2E/"), Err(ChannelError::Character));
    assert_eq!(folder("https://h.example/a//b/"), Err(ChannelError::EmptySegment));
    assert_eq!(folder("https://h.example/a/?x=1"), Err(ChannelError::Query));
    assert!(matches!(folder("https://h.example/a/#top"), Err(ChannelError::Address(_))));
    assert!(matches!(folder("wss://h.example/a/"), Err(ChannelError::Scheme)));
    let nine = "https://h.example/a/b/c/d/e/f/g/h/i/";
    assert_eq!(folder(nine), Err(ChannelError::TooDeep));
    let long = std::format!("https://h.example/{}/", "x".repeat(MAX_SEGMENT + 1));
    assert_eq!(folder(&long), Err(ChannelError::LongSegment));
}

#[test]
fn file_names_are_fixed() {
    let page = |mpp| PageAddress::from_mpp(mpp).unwrap();
    assert_eq!(ChannelFile::from_name("140.tti"), Some(ChannelFile::Page { page: page(0x140), t42: false }));
    assert_eq!(ChannelFile::from_name("1FE.t42"), Some(ChannelFile::Page { page: page(0x1FE), t42: true }));
    assert_eq!(ChannelFile::from_name("1fe.t42"), None, "capital digits only");
    assert_eq!(ChannelFile::from_name("940.tti"), None);
    assert_eq!(ChannelFile::from_name("P100.tti"), None);
    assert_eq!(ChannelFile::from_name("index.t42"), Some(ChannelFile::Bundle));
    assert_eq!(ChannelFile::from_name("index.tti"), None);
    assert_eq!(ChannelFile::from_name("channel.json"), Some(ChannelFile::Settings));
    assert_eq!(ChannelFile::from_name("contents.txt"), Some(ChannelFile::Contents));
    let address = parse_address("https://h.example/ceefax/140.tti").unwrap();
    assert_eq!(address.folder.as_str(), "https://h.example/ceefax/");
    assert_eq!(address.file, Some(ChannelFile::Page { page: page(0x140), t42: false }));
    assert_eq!(parse_address("https://h.example/ceefax/page.html"), Err(ChannelError::NotChannelFile));
    let url = file_url(&address.folder, ChannelFile::Page { page: page(0x1FE), t42: true }).unwrap();
    assert_eq!(url.as_str(), "https://h.example/ceefax/1FE.t42");
}

#[test]
fn inside_means_the_same_origin_and_below_the_folder() {
    let at = |text| Url::parse(text).unwrap();
    let one = at("http://h.example/one/");
    assert!(is_inside(&one, &at("http://h.example/one/100.tti")));
    assert!(!is_inside(&one, &at("http://h.example/two/100.tti")));
    assert!(!is_inside(&one, &at("http://h.example/one-more/100.tti")), "/one/ is not a prefix of /one-more/");
    assert!(!is_inside(&one, &at("http://h.example:8080/one/100.tti")));
    assert!(redirect_keeps_channel(&one, &at("https://h.example/one/100.tti")));
    assert!(!redirect_keeps_channel(&one, &at("https://h.example:8443/one/100.tti")), "another port");
    assert!(!redirect_keeps_channel(&one, &at("https://h.example/two/")));
}

#[test]
fn content_must_match_the_name() {
    assert!(is_tti(b"DE,x\r\nPN,10000\r\nOL,1,HELLO\r\n"));
    assert!(!is_tti(b"[% INCLUDE header %]\n"), "a Template Toolkit file");
    let mut packet = [0u8; PACKET_BYTES];
    packet[0] = 0x15;
    packet[1] = 0x15;
    assert!(is_t42(&packet));
    assert!(!is_t42(&packet[..41]));
    assert!(!is_t42(b""));
    // Two bits wrong: Hamming 8/4 detects it but cannot correct it.
    packet[0] = 0x15 ^ 0b11;
    assert!(!is_t42(&packet), "an address that is not Hamming 8/4");
}
