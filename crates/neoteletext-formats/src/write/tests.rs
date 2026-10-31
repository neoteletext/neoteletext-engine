//! Writer round trips (REQ-TST-020): TTI write then read, and T42 write then
//! read.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use super::*;
use crate::encode::encode_page;
use crate::page::{FastextLink, PageFile};
use crate::t42::{T42Reader, T42Variant};
use crate::testing::{packets, run};
use crate::tti::tests::{full_page, read};
use neoteletext_core::types::{PageAddress, SubCode};
use std::string::String;
use std::vec::Vec;

/// Line numbers differ between a file and its rewrite; everything else
/// must not.
fn without_lines(mut page: PageFile) -> PageFile {
    page.line = 0;
    for link in page.links.as_mut_slice() {
        link.line = 0;
    }
    for issue in page.issues.as_mut_slice() {
        issue.line = 0;
    }
    page
}

fn round_trip(text: &[u8]) {
    let (pages, _) = read(text);
    assert_ne!(pages.len(), 0);
    let mut written = Vec::new();
    write_tti(&pages.iter().collect::<Vec<_>>(), &mut written).unwrap();
    assert!(written.is_ascii(), "seven-bit text");
    let (again, diagnostics) = read(&written);
    assert_eq!(diagnostics, [], "{}", String::from_utf8_lossy(&written));
    let left: Vec<_> = pages.into_iter().map(without_lines).collect();
    let right: Vec<_> = again.into_iter().map(without_lines).collect();
    assert_eq!(left.len(), right.len());
    let text = String::from_utf8_lossy(&written);
    for (a, b) in left.iter().zip(&right) {
        // Field by field first, so that a failure names the field.
        assert_eq!((a.address, a.subpage, a.subcode, a.status), (b.address, b.subpage, b.subcode, b.status), "{text}");
        assert_eq!((a.region, a.function, a.description, a.source), (b.region, b.function, b.description, b.source));
        assert_eq!((a.rows, a.rows_present), (b.rows, b.rows_present), "{text}");
        assert_eq!(a.enhancements.as_slice(), b.enhancements.as_slice(), "{text}");
        assert_eq!(a.raw_packets.as_slice(), b.raw_packets.as_slice(), "{text}");
        assert_eq!((a.fastext, a.fastext_row_24), (b.fastext, b.fastext_row_24), "{text}");
        assert_eq!((a.links.as_slice(), a.links_valid), (b.links.as_slice(), b.links_valid), "{text}");
        assert_eq!((a.issues.as_slice(), a.considered), (b.issues.as_slice(), b.considered), "{text}");
    }
    assert_eq!(left, right);
}

#[test]
fn tti_round_trips() {
    let mut every = full_page();
    // Unknown commands are not kept, so leave them out of this comparison.
    every = every
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.starts_with(b"SP,") && !line.starts_with(b"XX,"))
        .collect::<Vec<_>>()
        .join(&b'\n');
    round_trip(&every);
    round_trip(
        b"DE,Carousel\nPN,10001\nOL,1,One\nLN,R,101,a\nLN,Y,svc:2A0:0003,b\nCK,considered,no-other-route\n\
PN,10002\nDE,Second\nOL,1,Two\n",
    );
}

#[test]
fn fastext_links_with_sub_codes_use_a_packed_line() {
    let mut page = PageFile::new();
    page.set_row(1, b"Links");
    page.fastext[0] = Some(FastextLink { address: PageAddress::from_mpp(0x234).unwrap(), subcode: SubCode::new(5) });
    page.fastext[5] = Some(FastextLink { address: PageAddress::from_mpp(0x100).unwrap(), subcode: SubCode::ANY });
    page.fastext_row_24 = true;
    let mut written = Vec::new();
    write_tti(&[&page], &mut written).unwrap();
    assert!(written.windows(6).any(|window| window == b"OL,27,"));
    let (again, _) = read(&written);
    assert_eq!(again[0].fastext, page.fastext);
    assert!(again[0].fastext_row_24);
}

#[test]
fn t42_round_trips_and_filters() {
    let (pages, _) = read(&full_page());
    let mut stream = Vec::new();
    let mut writer = T42Writer::new(&mut stream);
    writer.write_page(&pages[0]).unwrap();
    let mut expected = Vec::new();
    encode_page(&pages[0], &mut |packet| expected.push(packet));
    let got = packets(&run(&mut T42Reader::new(T42Variant::Plain, None), &stream, 100));
    assert_eq!(got, expected);

    // Pass-through with a page filter keeps the page's own packets only.
    let mut other = pages[0].clone();
    other.address = PageAddress::from_mpp(0x1B0).unwrap();
    let mut other_packets = Vec::new();
    encode_page(&other, &mut |packet| other_packets.push(packet));
    let filter = PacketFilter { magazine: None, page: Some(pages[0].address.page) };
    let mut filtered = Vec::new();
    let mut writer = T42Writer::with_filter(&mut filtered, filter);
    let mut written = 0;
    for packet in expected.iter().chain(&other_packets) {
        written += usize::from(writer.write_packet(packet).unwrap());
    }
    // The M/29 packet belongs to the magazine, not the page.
    assert_eq!(written, expected.len() - 1);
    assert_eq!(filtered.len(), written * 42);
}

/// REQ-INP-023 and REQ-TST-020: object, pattern and table pages, magazine
/// packets and the page packets of Levels 2.5 and 3.5 read, write and read
/// back unchanged.
#[test]
fn enhancement_pages_round_trip() {
    round_trip(&crate::tti::tests::enhancement_pages());
}

/// REQ-INP-024 and Annex B.7: a set of pages goes out as magazine
/// packets, tables, object pages, other data pages (a DRCS page among
/// them, decision E.95) and then display pages, each class in the order
/// given.
#[test]
fn page_sets_are_written_in_the_order_of_annex_b7() {
    use crate::encode::{TransmissionClass, transmission_class};
    let (pages, _) = read(&crate::tti::tests::enhancement_pages());
    let mut display = PageFile::new();
    display.address = PageAddress::from_mpp(0x150).unwrap();
    display.set_row(1, b"Second display page");
    let mut given: Vec<&PageFile> = std::vec![&display];
    given.extend(pages.iter().rev());
    let ordered = in_transmission_order(&given);
    let classes: Vec<TransmissionClass> = ordered.iter().map(|page| transmission_class(page)).collect();
    assert_eq!(
        classes,
        [
            TransmissionClass::MagazinePackets,
            TransmissionClass::OrganizationTable,
            TransmissionClass::Objects,
            TransmissionClass::OtherData,
            TransmissionClass::Display,
            TransmissionClass::Display,
        ]
    );
    // Within a class the order given is kept.
    assert_eq!(ordered[4].address.mpp(), 0x150);
    let mut stream = Vec::new();
    T42Writer::new(&mut stream).write_pages(&given).unwrap();
    let mut expected = Vec::new();
    for page in &ordered {
        encode_page(page, &mut |packet| expected.extend_from_slice(&packet.bytes));
    }
    assert_eq!(stream, expected);
}

/// Decision E.111: a withdrawn `TF` line is not kept, so not written back.
#[test]
fn a_withdrawn_font_line_is_not_written() {
    let (pages, _) = read(b"PN,10000\nTF,https://fonts.example/f.bdf,5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d\nOL,1,A\n");
    let mut written = Vec::new();
    write_tti(&pages.iter().collect::<Vec<_>>(), &mut written).unwrap();
    assert!(!String::from_utf8_lossy(&written).contains("TF,"), "{}", String::from_utf8_lossy(&written));
}
