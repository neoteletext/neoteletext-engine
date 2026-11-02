#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]

use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};

use super::{MAX_ADDRESS, RedirectRecord, encode, read};
use crate::records::{ChannelRecord, Read, RecordFault, RecordReader, fragments};
use std::format;
use std::string::String;
use std::vec::Vec;

const SERVICE: u8 = 3;

fn packets(redirect: RedirectRecord<'_>) -> Vec<Packet> {
    let mut written = Vec::new();
    let count = encode(redirect, SERVICE, 7, &mut |packet| written.push(packet)).unwrap();
    assert_eq!(count, written.len());
    written
}

/// What a receiver reads from `written`, fragment by fragment.
fn received(written: &[Packet], check: impl Fn(RedirectRecord<'_>)) {
    let mut reader = RecordReader::new();
    let (last, before) = written.split_last().unwrap();
    for packet in before {
        assert_eq!(reader.push(packet), Read::Nothing);
    }
    match reader.push(last) {
        Read::Record(ChannelRecord::Redirect(redirect)) => check(redirect),
        other => panic!("{other:?}"),
    }
}

fn page(mpp: u16) -> PageAddress {
    PageAddress::from_mpp(mpp).unwrap()
}

/// DOC-1960, REQ-1688: a place is one fragment, for the page
/// and for one of its sub-pages.
#[test]
fn a_place_reaches_a_receiver_in_one_fragment() {
    for subcode in [SubCode::ANY, SubCode::new(0), SubCode::new(0x0002)] {
        let redirect = RedirectRecord::Page { page: page(0x1A0), subcode, parameters: None };
        let written = packets(redirect);
        assert_eq!(written.len(), 1);
        received(&written, |read| assert_eq!(read, redirect));
    }
}

/// DOC-1960: the longest address travels in as many fragments
/// as it needs, whole.
#[test]
fn the_longest_address_reaches_a_receiver_whole() {
    let mut address = String::from("https://news.example/");
    address.extend(core::iter::repeat_n('a', MAX_ADDRESS - address.len()));
    let written = packets(RedirectRecord::Address(&address));
    assert_eq!(written.len(), fragments(MAX_ADDRESS));
    received(&written, |read| assert_eq!(read, RedirectRecord::Address(&address)));
}

/// REQ-1688 item 1: an address too long for the record is not written.
#[test]
fn an_address_too_long_for_the_record_is_refused() {
    let mut address = String::from("https://news.example/");
    address.extend(core::iter::repeat_n('a', MAX_ADDRESS + 1 - address.len()));
    let refused = encode(RedirectRecord::Address(&address), SERVICE, 0, &mut |_| panic!("nothing is written"));
    assert_eq!(refused, Err(RecordFault::Length));
}

/// DEC-1730 item 6: the body is the address itself, as a `Location` is, and
/// a page is a place.
#[test]
fn the_body_is_the_address_itself() {
    for address in
        ["http://news.example/teletext/", "https://news.example/teletext/302", "https://news.example/302/0002"]
    {
        assert_eq!(read(address.as_bytes()), Ok(RedirectRecord::Address(address)));
    }
    let place = |subcode| Ok(RedirectRecord::Page { page: page(0x302), subcode, parameters: None });
    assert_eq!(read(b"302"), place(SubCode::ANY));
    assert_eq!(read(b"302/0002"), place(SubCode::new(2)));
    assert!(read(b"contents").is_err(), "only a page");
    assert!(read(b"https://news.example/#302").is_err(), "no fragment");
}

/// TODO-1731 item 5: a place keeps the query of the selection it moves, up
/// to the longest a selection may have, and a whole address keeps its own
/// bound.
#[test]
fn a_place_keeps_its_query() {
    let kept = RedirectRecord::Page { page: page(0x302), subcode: SubCode::ANY, parameters: Some("a=1") };
    assert_eq!(read(b"302?a=1"), Ok(kept));
    received(&packets(kept), |read| assert_eq!(read, kept));
    let longest = format!("a={}", "x".repeat(MAX_ADDRESS - 2));
    let kept = RedirectRecord::Page { page: page(0x302), subcode: SubCode::new(2), parameters: Some(&longest) };
    received(&packets(kept), |read| assert_eq!(read, kept));
    assert_eq!(read(format!("302/0002?{longest}&b=c").as_bytes()), Err(RecordFault::Length));
    assert!(read(b"302?x").is_err(), "the map's pairs");
    let long = format!("https://news.example/{}", "a".repeat(MAX_ADDRESS - 20));
    assert_eq!(
        read(long.as_bytes()),
        Err(RecordFault::BadValue("redirect address")),
        "only a place may pass the longest address"
    );
}

/// REQ-1688 item 3: only an `http` or `https` address, as the engine's
/// strict parser reads one; the record never carries anything else.
#[test]
fn only_http_and_https_addresses_are_read() {
    for address in [
        "",
        "#302",
        "file:///etc/passwd",
        "web:https://news.example/",
        "neoteletext:home",
        "wss://news.example/",
        "ws://news.example/",
        "https://",
        "https://user@news.example/",
        "https://news.example/a b",
        "https://news.example/\u{e9}",
        "https://news.example/#",
    ] {
        if !address.is_empty() {
            assert_eq!(read(address.as_bytes()), Err(RecordFault::BadValue("redirect address")), "{address}");
        }
    }
}

/// DOC-1960 and DOC-1952: what is neither is refused whole.
#[test]
fn a_record_that_breaks_a_rule_is_refused() {
    assert_eq!(read(&[]), Err(RecordFault::Length));
    assert_eq!(read(b"900"), Err(RecordFault::BadValue("redirect address")));
    assert_eq!(read(b"1FF"), Err(RecordFault::BadValue("redirect page")), "a reserved number");
    assert_eq!(read(b"302/3F7F"), Err(RecordFault::BadValue("redirect address")));
    assert_eq!(read(b"302/0080"), Err(RecordFault::BadValue("redirect address")), "outside the thirteen bits");
    assert_eq!(read(&[0xFF, 0xFE]), Err(RecordFault::BadValue("redirect address")), "not text");
}
