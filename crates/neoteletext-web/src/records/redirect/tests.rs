#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]

use neoteletext_core::limits::SUBCODE_ANY;
use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};

use super::{MAX_ADDRESS, RedirectRecord, encode, read};
use crate::records::{ChannelRecord, Read, RecordFault, RecordReader, fragments};
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

/// INPUT.md section 7.11, REQ-1688: the page form is one fragment, for the
/// page and for one of its sub-pages.
#[test]
fn the_page_form_reaches_a_receiver_in_one_fragment() {
    for subcode in [SubCode::ANY, SubCode::new(0), SubCode::new(0x0002)] {
        let redirect = RedirectRecord::Page { page: page(0x1A0), subcode };
        let written = packets(redirect);
        assert_eq!(written.len(), 1);
        received(&written, |read| assert_eq!(read, redirect));
    }
}

/// INPUT.md section 7.11: the longest address travels in as many fragments
/// as it needs, whole.
#[test]
fn the_longest_address_reaches_a_receiver_whole() {
    let mut address = String::from("https://news.example/");
    address.extend(core::iter::repeat_n('a', MAX_ADDRESS - address.len()));
    let written = packets(RedirectRecord::Address(&address));
    assert_eq!(written.len(), fragments(MAX_ADDRESS + 1));
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

/// INPUT.md section 7.11: a channel's address, with a page after it.
#[test]
fn an_address_may_name_a_page() {
    for address in
        ["http://news.example/teletext/", "https://news.example/teletext/#302", "https://news.example/#302:0002"]
    {
        let mut body = std::vec![1];
        body.extend_from_slice(address.as_bytes());
        assert_eq!(read(&body), Ok(RedirectRecord::Address(address)));
    }
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
        let mut body = std::vec![1];
        body.extend_from_slice(address.as_bytes());
        assert_eq!(read(&body), Err(RecordFault::BadValue("redirect address")), "{address}");
    }
}

/// INPUT.md section 7.11 and 7.10: a field outside its range is refused
/// whole, and so are bytes after the last field.
#[test]
fn a_record_that_breaks_a_rule_is_refused() {
    let any = SUBCODE_ANY.to_be_bytes();
    assert_eq!(read(&[]), Err(RecordFault::Length));
    assert_eq!(read(&[2, 1, 0, any[0], any[1]]), Err(RecordFault::BadValue("redirect form")));
    assert_eq!(read(&[0, 1, 0]), Err(RecordFault::Length));
    assert_eq!(read(&[0, 1, 0, any[0], any[1], 0]), Err(RecordFault::Length));
    assert_eq!(read(&[0, 9, 0, any[0], any[1]]), Err(RecordFault::BadValue("redirect page")));
    assert_eq!(read(&[0, 0, 0xFF, any[0], any[1]]), Err(RecordFault::BadValue("redirect page")));
    assert_eq!(read(&[0, 1, 0xFF, any[0], any[1]]), Err(RecordFault::BadValue("redirect page")), "a reserved number");
    assert_eq!(read(&[0, 1, 0, 0x40, 0]), Err(RecordFault::BadValue("redirect sub-code")));
    assert_eq!(read(&[0, 1, 0, 0, 0x80]), Err(RecordFault::BadValue("redirect sub-code")), "outside the thirteen bits");
}
