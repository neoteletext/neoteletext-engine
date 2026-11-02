#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
//! The tests of a page written as a channel's files (REQ-WEB-019,
//! decision E.132), up to the standard's 8,190 sub-pages (REQ-NUM-020).

use super::*;
use crate::subpages::{self, Entry, MAX_PARTS, ROWS_PER_PART};
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
    // NNN.t42, a file for each sub-page, 128 parts and index.t42 (REQ-1887).
    assert_eq!(files.len(), 1 + MAX_ENTRIES + MAX_PARTS + 1);
    assert_eq!(files["100.t42"], bodies[0], "100.t42 holds the first sub-page");
    assert_eq!(files["index.t42"], bodies[0], "the bundle holds first sub-pages only");
    for (code, body) in codes.iter().zip(&bodies) {
        let name = std::format!("100-{:04X}.t42", code.value());
        assert_eq!(&files[&name], body, "{name}");
        assert_eq!(ChannelFile::from_name(&name), Some(ChannelFile::SubPage { page, subcode: *code }));
    }
    let version = Validator::of_page(bodies.iter().map(Vec::as_slice));
    let mut read: Vec<Entry<'_>> = Vec::new();
    for number in 0..MAX_PARTS {
        let name = std::format!("100/{number}.sub");
        let bytes = &files[&name];
        assert!(bytes.len() <= MAX_BYTES, "{name}");
        let part = subpages::read(bytes).unwrap();
        assert_eq!((usize::from(part.total), part.version), (MAX_ENTRIES, version), "{name}: one header in every part");
        let rows: Vec<Entry<'_>> = part.entries.map(Result::unwrap).collect();
        let expected = if number + 1 == MAX_PARTS { MAX_ENTRIES % ROWS_PER_PART } else { ROWS_PER_PART };
        assert_eq!(rows.len(), expected, "{name}");
        read.extend(rows);
    }
    assert_eq!(read.len(), MAX_ENTRIES);
    for ((entry, (code, body)), title) in read.iter().zip(codes.iter().zip(&bodies)).zip(&titles) {
        assert_eq!(entry.subcode, *code);
        assert_eq!(entry.validator, Validator::of(body));
        assert_eq!(entry.title, Some(title.as_str()));
    }
}

/// REQ-1896: everything in a list is derived from the page data each time it
/// is written, so a page written twice from the same pages gives the same
/// files, and one sub-page changed changes the version in every part and that
/// sub-page's row.
#[test]
fn a_list_is_derived_from_the_page() {
    let address = page(0x100);
    let bodies: Vec<Vec<u8>> = (1..=65).map(|value| sub_page_t42(address, SubCode::new(value))).collect();
    let mut changed = bodies.clone();
    changed[64] = full_sub_page_t42(address, SubCode::new(65));
    let files = |bodies: &[Vec<u8>]| {
        let sub_pages: Vec<SubPageFile<'_>> = bodies
            .iter()
            .zip(1..)
            .map(|(t42, value)| SubPageFile { subcode: SubCode::new(value), t42, title: "" })
            .collect();
        page_files(address, &sub_pages).unwrap().into_iter().collect::<BTreeMap<String, Vec<u8>>>()
    };
    assert_eq!(files(&bodies), files(&bodies));
    let (before, after) = (files(&bodies), files(&changed));
    let version = |files: &BTreeMap<String, Vec<u8>>, name: &str| subpages::read(&files[name]).unwrap().version;
    for name in ["100/0.sub", "100/1.sub"] {
        assert_ne!(version(&before, name), version(&after, name), "{name}");
    }
    let rows = |files: &BTreeMap<String, Vec<u8>>, name: &str| {
        subpages::read(&files[name]).unwrap().entries.flatten().map(|entry| entry.validator).collect::<Vec<_>>()
    };
    assert_eq!(rows(&before, "100/0.sub"), rows(&after, "100/0.sub"), "the unchanged part's rows");
    assert_ne!(rows(&before, "100/1.sub"), rows(&after, "100/1.sub"), "the changed sub-page's row");
}

#[test]
fn a_page_of_one_sub_page_is_one_file() {
    let body = sub_page_t42(page(0x101), SubCode::new(0));
    let files =
        page_files(page(0x101), &[SubPageFile { subcode: SubCode::new(0), t42: &body, title: "Only" }]).unwrap();
    assert_eq!(files, [("101.t42".to_string(), body)]);
}

/// REQ-1893: first sub-pages go into `index.t42` until the next would pass the bound.
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

/// REQ-1893: the bundle takes pages in ascending page order whatever order
/// they were added in, and stops at the first page that does not fit, so that
/// a smaller page after it stays out.
#[test]
fn the_bundle_follows_the_page_order_and_stops_at_the_first_page_that_does_not_fit() {
    let full = |address| full_sub_page_t42(address, SubCode::new(0));
    let small = sub_page_t42(page(0x700), SubCode::new(0));
    let whole = full(page(0x100));
    let fitting = DEFAULT_PAGE_LIMIT / whole.len();
    let mut channel = ChannelFiles::new();
    // Added in descending order: the last pages added are the first bundled.
    channel.add_whole_page(page(0x700), &small).unwrap();
    channel.add_whole_page(page(0x600), &whole).unwrap();
    for index in (0..fitting).rev() {
        let address = page(0x100 + u16::try_from(index).unwrap());
        channel.add_whole_page(address, &full(address)).unwrap();
    }
    assert_eq!(channel.unbundled(), 2, "the page that does not fit and the smaller page after it");
    let files = channel.finish();
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names[..2], ["100.t42", "101.t42"], "the files are written in page order");
    let (name, bundle) = files.last().unwrap();
    assert_eq!(name, "index.t42");
    assert!(bundle.starts_with(&full(page(0x100))), "the lowest page comes first");
    assert_eq!(bundle.len(), fitting * whole.len(), "page 700 would fit, and stays out");
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

/// REQ-1890: sub-pages given out of order are laid out in ascending
/// sub-code order: the page's file holds the lowest, its list is ascending,
/// and the bundle takes the lowest.
#[test]
fn sub_pages_given_out_of_order_are_laid_out_ascending() {
    let address = page(0x100);
    let codes = [SubCode::new(3), SubCode::new(1), SubCode::new(2)];
    let bodies: Vec<Vec<u8>> = codes.iter().map(|&code| sub_page_t42(address, code)).collect();
    let sub_pages: Vec<SubPageFile<'_>> =
        codes.iter().zip(&bodies).map(|(&subcode, t42)| SubPageFile { subcode, t42, title: "" }).collect();
    let files = page_files(address, &sub_pages).unwrap();
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["100.t42", "100-0001.t42", "100-0002.t42", "100-0003.t42", "100/0.sub"]);
    assert_eq!(files[0].1, bodies[1], "the page's file holds the lowest sub-code");
    let part = subpages::read(&files[4].1).unwrap();
    let listed: Vec<u16> = part.entries.flatten().map(|entry| entry.subcode.value()).collect();
    assert_eq!(listed, [1, 2, 3]);
    let mut channel = ChannelFiles::new();
    channel.add_page(address, &sub_pages).unwrap();
    let all = channel.finish();
    assert_eq!(all.last().unwrap().1, bodies[1], "the bundle holds the lowest sub-code");
}
