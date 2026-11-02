#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
//! REQ-1693's spellings: each listed form, each one normalised, each refused.

use std::format;
use std::string::String;
use std::vec;

use neoteletext_core::types::{PageAddress, SubCode};

use super::*;
use crate::bytes::Writer;
use crate::channel::SETTINGS_FILE;
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

/// REQ-1897: a page is the channel's address and `NNN`, a sub-page's `NNN/SSSS`.
#[test]
fn each_place_reads_as_the_map_says() {
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
fn every_other_place_is_refused() {
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
    for place in refused {
        assert_eq!(Place::read(place), None, "{place}");
    }
}

#[test]
fn digits_are_normalised_to_capitals_and_parameters_are_left_alone() {
    let cases = [("1fe", "1FE"), ("302/3f7e", "302/3F7E"), ("1fe?B=%2f&a=1", "1FE?B=%2f&a=1")];
    for (given, expected) in cases {
        assert_eq!(canonical(&format!("{CHANNEL}{given}")), Ok(format!("{CHANNEL}{expected}")));
    }
}

#[test]
fn a_page_file_name_becomes_its_place() {
    let cases = [
        ("302.tti", "302"),
        ("302.t42", "302"),
        ("302/0002.t42", "302/0002"),
        ("302/0002.tti", "302/0002"),
        ("302.tti?a=1", "302?a=1"),
    ];
    for (given, expected) in cases {
        assert_eq!(canonical(&format!("{CHANNEL}{given}")), Ok(format!("{CHANNEL}{expected}")));
    }
}

/// DEC-1694 rules 1 and 2: where a channel ends is read from the end of the
/// path, and a folder named as a place belongs to the channel above it.
#[test]
fn the_end_of_the_path_says_where_the_channel_ends() {
    let read = MapAddress::read("https://h.example/news/302/0002").unwrap();
    assert_eq!(read.folder.as_str(), "https://h.example/news/");
    assert_eq!(read.place, Some(Place::Page { page: page(0x302), subcode: Some(SubCode::new(2)), parameters: None }));
    let root = MapAddress::read("https://h.example/302").unwrap();
    assert_eq!(root.folder.as_str(), "https://h.example/");
    let numbered = MapAddress::read("https://h.example/123/").unwrap();
    assert_eq!(numbered.folder.as_str(), "https://h.example/", "a numbered folder is a page of the channel above");
    let middle = MapAddress::read("https://h.example/123/news/").unwrap();
    assert_eq!(middle.folder.as_str(), "https://h.example/123/news/", "only the end of the path is read");
    assert_eq!(middle.place, None);
    assert_eq!(MapAddress::read("https://h.example/302/0002").unwrap().folder.as_str(), "https://h.example/");
}

#[test]
fn a_query_goes_with_its_page_and_an_address_with_no_place_keeps_it() {
    let with_place = format!("{CHANNEL}302?p=2");
    let read = MapAddress::read(&with_place).unwrap();
    let place = Place::Page { page: page(0x302), subcode: None, parameters: Some("p=2") };
    assert_eq!((read.query, read.place), (None, Some(place)));
    let with_no_place = format!("{CHANNEL}?c=1");
    let bare = MapAddress::read(&with_no_place).unwrap();
    assert_eq!((bare.query, bare.place), (Some("c=1"), None));
    assert_eq!(canonical(&format!("{CHANNEL}?c=1")), Ok(format!("{CHANNEL}?c=1")));
}

#[test]
fn an_address_with_no_place_arrives_at_the_initial_page() {
    assert_eq!(MapAddress::read(CHANNEL).unwrap().place, None);
}

/// REQ-1898: what fits no case is refused.
#[test]
fn what_is_outside_the_map_is_refused() {
    assert_eq!(canonical(&format!("{CHANNEL}#")), Err(MapError::Place));
    assert_eq!(canonical(&format!("{CHANNEL}#302")), Err(MapError::Place));
    assert_eq!(canonical(&format!("{CHANNEL}302.tti#303")), Err(MapError::Place));
    for name in [SETTINGS_FILE, "index.t42", "contents.txt", "302/0.sub"] {
        assert_eq!(canonical(&format!("{CHANNEL}{name}")), Err(MapError::ReceiverFile), "{name}");
        let address = format!("{CHANNEL}{name}");
        let opened = MapAddress::read_opened(&address).unwrap();
        assert_eq!((opened.folder.as_str(), opened.place), (CHANNEL, None), "DEC-1710: {name}");
        let with_page = format!("{address}#302");
        assert_eq!(MapAddress::read_opened(&with_page), Err(MapError::Place), "{name}");
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
    let longest = MAX_URL_LENGTH - CHANNEL.len() - "302?".len() - name.len();
    let address = format!("{CHANNEL}302?{name}{}", "x".repeat(longest));
    assert_eq!(canonical(&address), Ok(address.clone()));
    let over = format!("{address}x");
    assert!(matches!(canonical(&over), Err(MapError::Channel(_))));
}

/// DEC-1726: the entry file's old name is not a receiver's file any more, as
/// a viewer's address or as one a program was told to open.
#[test]
fn the_entry_files_old_name_is_not_read() {
    let old = format!("{CHANNEL}channel.json");
    let not_a_file = Err(MapError::Channel(ChannelError::NotChannelFile));
    assert_eq!(MapAddress::read(&old), not_a_file);
    assert_eq!(MapAddress::read_opened(&old), not_a_file);
}

/// The table is data, so it names the entry file literally: it must agree
/// with the constant (DEC-1726 item 5).
#[test]
fn the_conformance_table_names_the_entry_file() {
    assert!(CONFORMANCE.contains(&format!("refuse\t{CHANNEL}{SETTINGS_FILE}\treceiver-file\n")));
    assert!(CONFORMANCE.contains(&format!("answer\t/home/{SETTINGS_FILE}\t200\t-\n")));
}

/// REQ-1693's Verification: this parser against the one table. The server
/// runs the table's `answer` rows, and the browser its `open` and `refuse`.
#[test]
fn the_conformance_table_holds() {
    let mut rows = 0;
    for line in CONFORMANCE.lines().filter(|line| !line.is_empty() && !line.starts_with('#')) {
        rows += 1;
        let columns: vec::Vec<&str> = line.split('\t').collect();
        match columns[..] {
            ["open", address, spelled] => assert_eq!(canonical(address).as_deref(), Ok(spelled), "{line}"),
            ["refuse", address, why] => {
                let refused = match MapAddress::read(address) {
                    Err(MapError::Place) => "place",
                    Err(MapError::ReceiverFile) => "receiver-file",
                    Err(MapError::Channel(_)) => "channel",
                    Ok(read) => panic!("{line}: opened as {read:?}"),
                };
                assert_eq!(refused, why, "{line}");
            }
            ["answer", target, status, location] => {
                assert!(target.starts_with('/') && status.len() == 3 && !location.is_empty(), "{line}");
            }
            _ => panic!("not a row of the table: {line}"),
        }
    }
    assert!(rows >= 70, "{rows} rows");
}
