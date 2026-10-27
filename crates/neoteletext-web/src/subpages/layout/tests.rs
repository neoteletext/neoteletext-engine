#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
//! The tests of a page written as a channel's files (REQ-WEB-019,
//! decision E.132), up to the standard's 8,190 sub-pages (REQ-NUM-020).

use super::*;
use crate::subpages::{self, Entry};
use neoteletext_core::consts::{HEADER_DISPLAY_BYTES, PAGE_TIME_FILLING, SPACE};
use neoteletext_core::ecc::with_odd_parity;
use neoteletext_core::packet::encode;
use neoteletext_core::types::{ControlBits, PageNumber};
use std::collections::BTreeMap;
use std::string::ToString;

/// A sub-page as T42: its header, then a time-filling header that ends it
/// (EN 300 706 clause 7.2.1).
pub(crate) fn sub_page_t42(page: PageAddress, subcode: SubCode) -> Vec<u8> {
    let display = [with_odd_parity(SPACE); HEADER_DISPLAY_BYTES];
    let header = encode::header(page.magazine, page.page, subcode, ControlBits(0), &display).unwrap();
    let end = encode::header(page.magazine, PageNumber(PAGE_TIME_FILLING), SubCode::new(0), ControlBits(0), &display)
        .unwrap();
    [header.bytes, end.bytes].concat()
}

/// A sub-page with its rows 1 to 23, about a kilobyte of T42.
fn full_sub_page_t42(page: PageAddress, subcode: SubCode) -> Vec<u8> {
    let display = [with_odd_parity(SPACE); HEADER_DISPLAY_BYTES];
    let text = [with_odd_parity(b'x'); neoteletext_core::consts::DATA_BYTES];
    let mut bytes = encode::header(page.magazine, page.page, subcode, ControlBits(0), &display).unwrap().bytes.to_vec();
    for row in 1..=23 {
        bytes.extend_from_slice(&encode::row(page.magazine, row, &text).bytes);
    }
    bytes.extend_from_slice(&sub_page_t42(page, subcode)[neoteletext_core::consts::PACKET_BYTES..]);
    bytes
}

/// The first `count` sub-codes a page of that many sub-pages may have,
/// 0001 up, 3F7F left out.
fn subcodes(count: usize) -> Vec<SubCode> {
    (1..neoteletext_core::limits::SUBCODE_ANY)
        .filter(|&value| neoteletext_core::limits::is_sub_code(value))
        .map(SubCode::new)
        .take(count)
        .collect()
}

fn page(mpp: u16) -> PageAddress {
    PageAddress::from_mpp(mpp).unwrap()
}

#[test]
fn a_page_of_8190_sub_pages_is_written_as_files_and_read_back() {
    let page = page(0x100);
    let codes = subcodes(MAX_ENTRIES);
    assert_eq!(codes.len(), 8190);
    let bodies: Vec<Vec<u8>> = codes.iter().map(|&code| sub_page_t42(page, code)).collect();
    let titles: Vec<String> = codes.iter().map(|code| std::format!("Story {}", code.value())).collect();
    let sub_pages: Vec<SubPageFile<'_>> = codes
        .iter()
        .zip(&bodies)
        .zip(&titles)
        .map(|((&subcode, t42), title)| SubPageFile { subcode, t42, title })
        .collect();
    let mut channel = ChannelFiles::new();
    channel.add_page(page, &sub_pages).unwrap();
    let files: BTreeMap<String, Vec<u8>> = channel.finish().into_iter().collect();
    // NNN.t42, a file for each sub-page, NNN.sub and index.t42.
    assert_eq!(files.len(), 1 + MAX_ENTRIES + 1 + 1);
    assert_eq!(files["100.t42"], bodies[0], "100.t42 holds the first sub-page");
    assert_eq!(files["index.t42"], bodies[0], "the bundle holds first sub-pages only");
    for (code, body) in codes.iter().zip(&bodies) {
        let name = std::format!("100-{:04X}.t42", code.value());
        assert_eq!(&files[&name], body, "{name}");
        assert_eq!(ChannelFile::from_name(&name), Some(ChannelFile::SubPage { page, subcode: *code }));
    }
    let catalogue = &files["100.sub"];
    assert!(catalogue.len() > DEFAULT_PAGE_LIMIT, "larger than a page response, within its own limit");
    let entries: Vec<Entry<'_>> = subpages::read(catalogue).unwrap().map(Result::unwrap).collect();
    assert_eq!(entries.len(), MAX_ENTRIES);
    for ((entry, (code, body)), title) in entries.iter().zip(codes.iter().zip(&bodies)).zip(&titles) {
        assert_eq!(entry.subcode, *code);
        assert_eq!(entry.validator, Validator::of(body));
        assert_eq!(entry.title, Some(title.as_str()));
    }
}

#[test]
fn a_page_of_one_sub_page_is_one_file() {
    let body = sub_page_t42(page(0x101), SubCode::new(0));
    let files =
        page_files(page(0x101), &[SubPageFile { subcode: SubCode::new(0), t42: &body, title: "Only" }]).unwrap();
    assert_eq!(files, [("101.t42".to_string(), body)]);
}

#[test]
fn the_bundle_takes_first_sub_pages_while_it_has_room() {
    let mut channel = ChannelFiles::new();
    let body = full_sub_page_t42(page(0x1FE), SubCode::new(0));
    channel.add_whole_page(page(0x1FE), &body).unwrap();
    let mut bundled = body.len();
    let mut added = 0usize;
    // Enough pages of two sub-pages to fill the bundle.
    let first_page_count = DEFAULT_PAGE_LIMIT / body.len() + 2;
    for index in 0..first_page_count {
        let address = page(0x200 + u16::try_from(index).unwrap());
        let codes = [SubCode::new(1), SubCode::new(2)];
        let bodies: Vec<Vec<u8>> = codes.iter().map(|&code| full_sub_page_t42(address, code)).collect();
        let sub_pages: Vec<SubPageFile<'_>> =
            codes.iter().zip(&bodies).map(|(&subcode, t42)| SubPageFile { subcode, t42, title: "" }).collect();
        channel.add_page(address, &sub_pages).unwrap();
        if bundled + bodies[0].len() <= DEFAULT_PAGE_LIMIT {
            bundled += bodies[0].len();
            added += 1;
        }
    }
    assert!(channel.unbundled() > 0);
    assert_eq!(channel.unbundled(), first_page_count - added);
    let files = channel.finish();
    let (name, bundle) = files.last().unwrap();
    assert_eq!((name.as_str(), bundle.len()), ("index.t42", bundled));
    assert!(bundle.starts_with(&body), "an enhancement page is bundled whole");
}

#[test]
fn what_cannot_be_laid_out_is_refused() {
    let address = page(0x100);
    let body = sub_page_t42(address, SubCode::new(1));
    let one = SubPageFile { subcode: SubCode::new(1), t42: &body, title: "" };
    assert_eq!(page_files(address, &[]), Err(LayoutError::Empty));
    assert_eq!(page_files(address, &[one, one]), Err(LayoutError::Duplicate(SubCode::new(1))));
    let broken = SubPageFile { t42: b"not packets", ..one };
    assert_eq!(page_files(address, &[broken]), Err(LayoutError::Content(Some(SubCode::new(1)))));
    let too_many = std::vec![one; MAX_ENTRIES + 1];
    assert_eq!(page_files(address, &too_many), Err(LayoutError::TooMany));
    let mut channel = ChannelFiles::new();
    channel.add_page(address, &[one]).unwrap();
    assert_eq!(channel.add_page(address, &[one]), Err(LayoutError::PageTwice(address)));
    assert_eq!(channel.add_whole_page(page(0x1FE), b"x"), Err(LayoutError::Content(None)));
}
