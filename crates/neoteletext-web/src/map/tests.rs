#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
//! REQ-1693's spellings: each listed form, each one normalised, each refused.

use std::format;
use std::string::String;
use std::vec;

use neoteletext_core::types::{PageAddress, SubCode};

use super::*;
use crate::bytes::Writer;
use crate::consts::MAX_URL_LENGTH;

const CHANNEL: &str = "https://h.example/home/";

fn canonical(address: &str) -> Result<String, MapError> {
    let read = MapAddress::read(address)?;
    let mut buffer = vec![0u8; MAX_URL_LENGTH];
    let mut writer = Writer::new(&mut buffer);
    read.write(&mut writer).unwrap();
    Ok(String::from_utf8(writer.written().to_vec()).unwrap())
}

fn page(mpp: u16) -> PageAddress {
    PageAddress::from_mpp(mpp).unwrap()
}

#[test]
fn each_fragment_reads_as_the_map_says() {
    let plain = |mpp, code: Option<u16>, parameters| Place::Page {
        page: page(mpp),
        subcode: code.map(SubCode::new),
        parameters,
    };
    assert_eq!(Place::read("302"), Some(plain(0x302, None, None)));
    assert_eq!(Place::read("302/0002"), Some(plain(0x302, Some(2), None)));
    assert_eq!(Place::read("302?name=value"), Some(plain(0x302, None, Some("name=value"))));
    assert_eq!(Place::read("302/0002?a=1&b="), Some(plain(0x302, Some(2), Some("a=1&b="))));
    assert_eq!(Place::read("contents"), Some(Place::Contents));
    assert_eq!(Place::read("wall"), Some(Place::Wall));
}

#[test]
fn every_page_the_standard_defines_is_accepted() {
    for mpp in 0x100..=0x8FFu16 {
        let digits = format!("{mpp:03X}");
        assert_eq!(Place::read(&digits), Some(Place::Page { page: page(mpp), subcode: None, parameters: None }));
    }
    assert!(Place::read("100/3F7E").is_some());
    assert!(Place::read("100/0000").is_some());
}

#[test]
fn every_other_fragment_is_refused() {
    let refused = [
        "",
        "30",
        "3021",
        "G02",
        "902",
        "002",
        "302/2",
        "302/3F7F",
        "302/3f7f",
        "302/0080",
        "302:0002",
        "WALL",
        "Wall",
        "CONTENTS",
        "contents/1",
        "wall/1",
        "wall?name=value",
        "contents?name=value",
        "?name=value",
        "302?",
        "302?name",
        "302?=value",
        "302?a=1&",
        "302?&a=1",
        "302/",
        "/0002",
        "302/0002/0003",
        "302#303",
        "302 ",
        "%33%30%32",
        "302?a=%G0",
        "302?a=\u{e9}",
    ];
    for fragment in refused {
        assert_eq!(Place::read(fragment), None, "#{fragment}");
    }
}

#[test]
fn digits_are_normalised_to_capitals_and_parameters_are_left_alone() {
    let cases = [("#1fe", "#1FE"), ("#302/3f7e", "#302/3F7E"), ("#1fe?B=%2f&a=1", "#1FE?B=%2f&a=1")];
    for (given, expected) in cases {
        assert_eq!(canonical(&format!("{CHANNEL}{given}")), Ok(format!("{CHANNEL}{expected}")));
    }
}

#[test]
fn a_page_file_name_becomes_its_fragment() {
    let cases = [("302.tti", "#302"), ("302.t42", "#302"), ("302-0002.t42", "#302/0002")];
    for (given, expected) in cases {
        assert_eq!(canonical(&format!("{CHANNEL}{given}")), Ok(format!("{CHANNEL}{expected}")));
    }
    assert_eq!(canonical(&format!("{CHANNEL}302.tti?a=1")), Ok(format!("{CHANNEL}?a=1#302")));
}

#[test]
fn channel_parameters_stay_in_the_channel_and_page_parameters_in_the_fragment() {
    let address = format!("{CHANNEL}?c=1#302?p=2");
    let read = MapAddress::read(&address).unwrap();
    assert_eq!(read.folder.as_str(), CHANNEL);
    assert_eq!(read.query, Some("c=1"));
    assert_eq!(read.place, Some(Place::Page { page: page(0x302), subcode: None, parameters: Some("p=2") }));
    assert_eq!(canonical(&address), Ok(address.clone()));
}

#[test]
fn an_address_with_no_fragment_arrives_at_the_initial_page() {
    assert_eq!(MapAddress::read(CHANNEL).unwrap().place, None);
}

#[test]
fn what_is_outside_the_map_is_refused() {
    assert_eq!(canonical(&format!("{CHANNEL}#")), Err(MapError::Place));
    assert_eq!(canonical(&format!("{CHANNEL}#302:0002")), Err(MapError::Place));
    assert_eq!(canonical(&format!("{CHANNEL}302.tti#303")), Err(MapError::TwoPlaces));
    for name in ["channel.json", "index.t42", "contents.txt", "302.sub"] {
        assert_eq!(canonical(&format!("{CHANNEL}{name}")), Err(MapError::ReceiverFile), "{name}");
        let address = format!("{CHANNEL}{name}");
        let opened = MapAddress::read_opened(&address).unwrap();
        assert_eq!((opened.folder.as_str(), opened.place), (CHANNEL, None), "DEC-1710: {name}");
        let with_page = format!("{address}#302");
        assert_eq!(MapAddress::read_opened(&with_page), Err(MapError::ReceiverFile), "{name}");
    }
    assert!(matches!(canonical("https://h.example/home"), Err(MapError::Channel(_))));
    assert!(matches!(canonical("ftp://h.example/home/"), Err(MapError::Channel(_))));
}

#[test]
fn a_hash_cannot_hide_the_real_host() {
    let read = MapAddress::read("https://evil.example/#@good.example");
    assert_eq!(read, Err(MapError::Place));
}

#[test]
fn the_whole_address_obeys_the_general_limit() {
    let name = "a=";
    let longest = MAX_URL_LENGTH - CHANNEL.len() - "#302?".len() - name.len();
    let address = format!("{CHANNEL}#302?{name}{}", "x".repeat(longest));
    assert_eq!(canonical(&address), Ok(address.clone()));
    let over = format!("{address}x");
    assert!(matches!(canonical(&over), Err(MapError::Channel(_))));
}
