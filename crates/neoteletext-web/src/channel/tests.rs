#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
//! The tests of the channel folder and file rules (REQ-WEB-023, REQ-WEB-027).

use super::*;

fn folder(text: &str) -> Result<std::string::String, ChannelError> {
    parse_address(text).map(|address| {
        assert!(address.file.is_none(), "{text}");
        std::string::String::from(address.folder.as_str())
    })
}

// Verifies REQ-WEB-023.
/// REQ-1883: a channel's address is a folder in the canonical form.
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

/// REQ-1884: the fixed names, in their one spelling.
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
    assert_eq!(ChannelFile::from_name("channel.ttx"), Some(ChannelFile::Settings));
    assert_eq!(SETTINGS_FILE, "channel.ttx", "DEC-1726: documents and channels name the file literally");
    assert_eq!(ChannelFile::from_name("channel.json"), None, "DEC-1726: the old name is no channel's file");
    assert_eq!(ChannelFile::from_name("contents.txt"), Some(ChannelFile::Contents));
    let address = parse_address("https://h.example/ceefax/140.tti").unwrap();
    assert_eq!(address.folder.as_str(), "https://h.example/ceefax/");
    assert_eq!(address.file, Some(ChannelFile::Page { page: page(0x140), t42: false }));
    assert_eq!(parse_address("https://h.example/ceefax/page.html"), Err(ChannelError::NotChannelFile));
    let url = file_url(&address.folder, ChannelFile::Page { page: page(0x1FE), t42: true }).unwrap();
    assert_eq!(url.as_str(), "https://h.example/ceefax/1FE.t42");
}

/// REQ-1884, REQ-1885, REQ-1887, REQ-1888 (DEC-2116): a sub-page's file and a
/// part of its page's list are in a folder named for the page, and the flat
/// names are no names of a channel's.
#[test]
fn a_page_folder_holds_its_sub_pages_and_its_list() {
    let page = PageAddress::from_mpp(0x1A0).unwrap();
    let t42 = |value| Some(ChannelFile::SubPage { page, subcode: SubCode::new(value), t42: true });
    let tti = |value| Some(ChannelFile::SubPage { page, subcode: SubCode::new(value), t42: false });
    assert_eq!(ChannelFile::from_name("1A0/0001.t42"), t42(0x0001));
    assert_eq!(ChannelFile::from_name("1A0/3F7E.t42"), t42(0x3F7E));
    assert_eq!(ChannelFile::from_name("1A0/0000.t42"), t42(0x0000));
    assert_eq!(ChannelFile::from_name("1A0/0001.tti"), tti(0x0001));
    assert_eq!(ChannelFile::from_name("1A0/3F7E.tti"), tti(0x3F7E));
    let part = |number| Some(ChannelFile::SubPages { page, part: PartNumber::new(number).unwrap() });
    assert_eq!(ChannelFile::from_name("1A0/0.sub"), part(0));
    assert_eq!(ChannelFile::from_name("1A0/127.sub"), part(127));
    for refused in [
        // The flat names, which are gone (DEC-2116 item 3).
        "1A0-0001.t42",
        "1A0-3F7E.t42",
        "1A0-0001.tti",
        "1A0-0001.sub",
        "1A0.sub",
        // 3F7F names no sub-page, and a sub-code has thirteen bits.
        "1A0/3F7F.t42",
        "1A0/0080.t42",
        "1A0/3f7e.t42",
        "1A0/001.t42",
        "1A0/00001.t42",
        "1a0/0001.t42",
        "1A0/0001.sub",
        "1A0/0001.ep1",
        "1A0/index.html",
        "1a0/0.sub",
        "1A0/128.sub",
        "1A0/00.sub",
        "1A0/.sub",
        "1A0/0.SUB",
        "1A0/0/0.sub",
        "1A0/0001/0002.t42",
        "/0.sub",
        "900/0001.t42",
    ] {
        assert_eq!(ChannelFile::from_name(refused), None, "{refused}");
    }
    let folder = Url::parse("https://h.example/ceefax/").unwrap();
    for (file, name) in [
        (ChannelFile::SubPage { page, subcode: SubCode::new(0x3F7E), t42: true }, "1A0/3F7E.t42"),
        (ChannelFile::SubPage { page, subcode: SubCode::new(0x0012), t42: false }, "1A0/0012.tti"),
        (ChannelFile::SubPages { page, part: PartNumber::FIRST }, "1A0/0.sub"),
        (ChannelFile::SubPages { page, part: PartNumber::new(127).unwrap() }, "1A0/127.sub"),
    ] {
        let url = file_url(&folder, file).unwrap();
        assert_eq!(url.as_str(), std::format!("https://h.example/ceefax/{name}"));
        let read = parse_address(url.as_str()).unwrap();
        assert_eq!((read.folder.as_str(), read.file), ("https://h.example/ceefax/", Some(file)));
    }
}

/// REQ-1885: `602/0101.t42` is a file of the channel above `602/`, and
/// `other/100.tti` a file of the channel at `other/`. A folder named as a place
/// holds no channel's other files.
#[test]
fn a_file_in_a_page_folder_belongs_to_the_channel_above_it() {
    let root = parse_address("https://h.example/602/0101.t42").unwrap();
    assert_eq!(root.folder.as_str(), "https://h.example/");
    let page = PageAddress::from_mpp(0x602).unwrap();
    assert_eq!(root.file, Some(ChannelFile::SubPage { page, subcode: SubCode::new(0x0101), t42: true }));
    let other = parse_address("https://h.example/other/100.tti").unwrap();
    assert_eq!(other.folder.as_str(), "https://h.example/other/");
    let nested = parse_address("https://h.example/news/2FA/0.sub").unwrap();
    assert_eq!(nested.folder.as_str(), "https://h.example/news/");
    for refused in [
        "https://h.example/602/100.t42",
        "https://h.example/2fa/100.tti",
        "https://h.example/2fa/0101.t42",
        "https://h.example/602/channel.ttx",
        "https://h.example/contents/100.tti",
        "https://h.example/wall/index.t42",
    ] {
        assert_eq!(parse_address(refused), Err(ChannelError::NotChannelFile), "{refused}");
    }
}

/// REQ-1883: origin and folder path are the channel's boundary.
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

/// REQ-1903: a file's content must be what its name says it is.
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

/// Finding F-45 (decision E.114 item 5): a path with an encoded dot,
/// slash or backslash after the folder is outside it, in either case, so
/// a server that decodes them cannot be led out of the folder. The
/// prefix test let them through.
#[test]
fn an_encoded_separator_after_the_folder_is_outside_it() {
    let at = |text: &str| Url::parse(text).unwrap();
    let one = at("http://h.example/one/");
    for outside in [
        "http://h.example/one/%2e%2e/two/100.tti",
        "http://h.example/one/%2E%2E/two/",
        "http://h.example/one/x%2Fy",
        "http://h.example/one/x%2fy",
        "http://h.example/one/..%5Ctwo",
        "http://h.example/one/..%5ctwo",
    ] {
        assert!(!is_inside(&one, &at(outside)), "{outside}");
        assert!(!redirect_keeps_channel(&one, &at(outside)), "{outside}");
        let upgraded = outside.replacen("http:", "https:", 1);
        assert!(!redirect_keeps_channel(&one, &at(&upgraded)), "{upgraded}");
    }
    assert!(is_inside(&one, &at("http://h.example/one/100.tti?x=%2F")), "a query is not the path");
    assert!(is_inside(&one, &at("http://h.example/one/a%20b")), "other encodings are not separators");
}

/// DEC-1710: a folder or a channel's own file opens as a channel, and any
/// other address as a service's descriptor.
// Verifies REQ-WEB-023.
/// REQ-1883: a folder is a channel. REQ-1898: of cases 2 and 3 of the order, a folder and a channel's own file
/// open as a channel.
#[test]
fn an_address_says_whether_it_is_a_channel() {
    let settings = std::format!("http://h.example/a/{SETTINGS_FILE}");
    for channel in ["https://h.example/demosite/", "https://h.example", settings.as_str(), "https://h.example/a/?id=7"]
    {
        assert!(is_channel_address(channel), "{channel}");
    }
    // DEC-1726: the entry file's old name is no longer a channel's.
    for service in [
        "https://h.example/service.json",
        "https://h.example/a/b",
        "wss://h.example/a/",
        "https://h.example/a/channel.json",
    ] {
        assert!(!is_channel_address(service), "{service}");
    }
}
