//! TTI reader tests, including the Link and Form Extension examples of
//! Appendices C.7 and E.3, and the round trip through the page encoder and
//! the core's packet decoder (REQ-TST-020).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;
use crate::encode::encode_page;
use crate::item::{Diagnostic, DiagnosticKind, Item};
use crate::page::{FieldKind, FormMethod, IssueKind, LinkKey, PageFile, PageFunction, Warning};
use crate::testing::{diagnostics, pages, same_in_any_chunks};
use neoteletext_core::consts::{
    ALPHA_BLACK, ALPHA_RED, C11_SERIAL, COLOUR_GREEN, DOUBLE_HEIGHT, MODE_G0_NO_DIACRITIC, MODE_SET_ACTIVE_POSITION,
    MODE_TERMINATION, ROW_ADDRESS_LAST,
};
use neoteletext_core::ecc::check_parity;
use neoteletext_core::packet::{Decoded, PageFormat, PagePacket, Triplet, decode};
use neoteletext_core::types::{PageAddress, SubCode};
use std::vec::Vec;

mod enhancement;
mod forms;
mod interaction;
mod links;

pub(crate) fn packed_line(designation: u8, triplets: &[Triplet; 13]) -> Vec<u8> {
    let mut out = std::vec![0x40 | designation];
    for triplet in triplets {
        let value = triplet.value();
        for shift in [0, 6, 12] {
            out.push(0x40 | ((value >> shift) & 0x3F) as u8);
        }
    }
    out
}

pub(crate) fn read(text: &[u8]) -> (Vec<PageFile>, Vec<Diagnostic>) {
    let items = same_in_any_chunks(TtiReader::new, text);
    (pages(&items), diagnostics(&items))
}

fn sample_triplets() -> [Triplet; 13] {
    let mut triplets = [Triplet { address: ROW_ADDRESS_LAST, mode: MODE_TERMINATION, data: 0 }; 13];
    triplets[0] = Triplet { address: 41, mode: MODE_SET_ACTIVE_POSITION, data: 0 };
    triplets[1] = Triplet { address: 5, mode: MODE_G0_NO_DIACRITIC, data: b'Z' };
    triplets[2] = Triplet { address: 6, mode: 0x12, data: b'e' };
    triplets
}

/// A page using every command of REQ-INP-013, with CR LF line ends.
pub(crate) fn full_page() -> Vec<u8> {
    let mut text = Vec::new();
    let mut line = |bytes: &[u8]| {
        text.extend_from_slice(bytes);
        text.extend_from_slice(b"\r\n");
    };
    line(b"DE,Test page");
    line(b"DS,inserter");
    line(b"SP,/var/pages/p100.tti");
    line(b"CT,8,T");
    line(b"PN,1A002");
    line(b"SC,0002");
    line(b"PS,80C0");
    line(b"RE,2");
    line(b"PF,0,0");
    line(b"OL,0,XXXXXXXXHEADER ROW");
    // Alpha Red in the 8-bit form, Alpha Green in the escape form, Double
    // Height as 0x10, and a comma in the text.
    line(&[b"OL,1,".as_slice(), &[0x81], b"Red", &[0x1B, 0x42], b"Green", &[0x10], b"Tall, wide"].concat());
    line(b"OL,23,Bottom");
    line(b"OL,25,Row 25");
    line(&[b"OL,26,".as_slice(), &packed_line(0, &sample_triplets())].concat());
    line(&[b"OL,28,".as_slice(), &packed_line(1, &sample_triplets())].concat());
    line(&[b"OL,29,".as_slice(), &packed_line(0, &sample_triplets())].concat());
    line(b"FL,1A1,202,0,8FF,8FF,100");
    line(b"XX,unknown");
    text
}

#[test]
fn reads_every_command() {
    let (pages, diagnostics) = read(&full_page());
    assert_eq!(pages.len(), 1);
    let page = &pages[0];
    assert_eq!(page.address.mpp(), 0x1A0);
    assert_eq!(page.subpage, 2);
    assert_eq!(page.subcode.value(), 2);
    assert_eq!(page.status, 0x80C0);
    // 0x40 is C11, 0x80 is C12 (decision E.21).
    assert_eq!(page.control().0, C11_SERIAL | neoteletext_core::consts::C12);
    assert_eq!(page.region, Some(2));
    assert_eq!(page.function, Some(PageFunction { function: 0, coding: 0 }));
    assert_eq!(page.description.as_bytes(), b"Test page");
    assert_eq!(page.source.as_bytes(), b"inserter");
    assert_eq!(&page.row(0).unwrap()[8..18], b"HEADER ROW");
    let mut row_1 = std::vec![ALPHA_RED];
    row_1.extend_from_slice(b"Red");
    row_1.push(ALPHA_BLACK | COLOUR_GREEN);
    row_1.extend_from_slice(b"Green");
    row_1.push(DOUBLE_HEIGHT);
    row_1.extend_from_slice(b"Tall, wide");
    assert_eq!(&page.row(1).unwrap()[..row_1.len()], row_1.as_slice());
    assert!(page.row(1).unwrap()[row_1.len()..].iter().all(|&code| code == b' '));
    assert!(page.has_row(23) && page.has_row(25) && !page.has_row(2));
    assert_eq!(page.enhancements.len(), 1);
    assert_eq!(page.enhancements.as_slice()[0].triplets, sample_triplets());
    assert_eq!(page.raw_packets.len(), 2);
    let links: Vec<_> = page.fastext.iter().map(|link| link.map(|link| link.address.mpp())).collect();
    assert_eq!(links, [Some(0x1A1), Some(0x202), None, None, None, Some(0x100)]);
    assert!(page.fastext_row_24);
    let kinds: Vec<_> = diagnostics.iter().map(|diagnostic| (diagnostic.line, diagnostic.kind)).collect();
    assert_eq!(
        kinds,
        [
            (Some(3), DiagnosticKind::UnknownCommand { command: *b"SP" }),
            (Some(18), DiagnosticKind::UnknownCommand { command: *b"XX" })
        ]
    );
}

#[test]
fn carousel_sub_pages_and_file_level_description() {
    let text = b"DE,Carousel\nPN,10001\nOL,1,One\nPN,10002\nDE,Second\nOL,1,Two\nPN,10003\nOL,1,Three\n";
    let (pages, _) = read(text);
    assert_eq!(pages.len(), 3);
    assert_eq!(pages.iter().map(|page| page.subpage).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(pages[0].description.as_bytes(), b"Carousel");
    assert_eq!(pages[1].description.as_bytes(), b"Second");
    assert_eq!(pages[2].description.as_bytes(), b"Carousel");
    assert_eq!(&pages[2].row(1).unwrap()[..5], b"Three");
}

/// Every carriage return before a line feed belongs to the line ending
/// (REQ-INP-013), so no value ends in one: the writer could not write such
/// a value back unchanged (issue #203, found by fuzzing).
#[test]
fn carriage_returns_before_a_line_feed_end_the_line() {
    let (pages, _) = read(b"PN,10000\nDS,Source\r\r\nDE,Description\r\r\r\nOL,1,Row\r\r\n");
    assert_eq!(pages[0].source.as_bytes(), b"Source");
    assert_eq!(pages[0].description.as_bytes(), b"Description");
    assert_eq!(&pages[0].row(1).unwrap()[..4], b"Row ");
}

#[test]
fn reports_long_rows_and_lines_with_offsets() {
    let mut text = b"PN,10000\nOL,1,".to_vec();
    text.extend(std::iter::repeat_n(b'x', 45));
    text.push(b'\n');
    let long_at = text.len();
    text.extend(std::iter::repeat_n(b'y', LINE_BYTES + 10));
    text.extend_from_slice(b"\nOL,2,ok\n");
    let (pages, diagnostics) = read(&text);
    assert_eq!(diagnostics[0], Diagnostic::at_line(9, 2, DiagnosticKind::RowTooLong { row: 1 }));
    assert_eq!(diagnostics[1], Diagnostic::at_line(long_at as u64, 3, DiagnosticKind::LineTooLong));
    assert_eq!(&pages[0].row(2).unwrap()[..2], b"ok");
}

/// The Appendix C.7 example.
pub(crate) const FORM_EXAMPLE: &[u8] = b"PN,40001
DE,Contact form
PS,8000
FT,5f2c9a17e0
OL,1,CONTACT US
OL,3,Name:  ...............................
FD,TEXT,name,7,31,50,Y,,
OL,4,Email: ...............................
FD,EMAIL,email,7,31,100,Y,
OL,6,Urgency: (*) Low  ( ) Medium  ( ) High
FD,RADIO,urgency,10,1,Y
OPTION,low,Low,Y
OPTION,medium,Medium,,19
OPTION,high,High,,31
OL,8,  < SEND MESSAGE >
FD,SUBMIT,send,2,16,Send message,send_contact
FS,POST,/contact
FP,40101,40102
";

#[test]
fn considered_warnings_are_kept() {
    let (pages, diagnostics) = read(b"PN,10000\nCK,considered,hidden-content\nCK,considered,nonsense\n");
    assert_eq!(pages[0].considered, Warning::HiddenContent.bit());
    assert_eq!(diagnostics[0].kind, DiagnosticKind::BadArgument { command: *b"CK" });
}

#[test]
fn content_before_any_page_number_is_reported() {
    let (pages, diagnostics) = read(b"OL,1,orphan\n");
    assert_eq!(pages[0].address.mpp(), 0x100);
    assert_eq!(diagnostics[0].kind, DiagnosticKind::MissingPageNumber);
}

#[test]
fn encoded_pages_decode_to_the_same_fields() {
    let (pages, _) = read(&full_page());
    let page = &pages[0];
    let mut packets = Vec::new();
    encode_page(page, &mut |packet| packets.push(packet));
    let decoded: Vec<_> = packets.iter().map(|packet| decode(packet).unwrap()).collect();
    // Annex B.2 order: M/29, header, 27, 28, 26, rows.
    assert!(matches!(decoded[0], Decoded::Magazine { .. } | Decoded::Ignored(_)));
    let Decoded::Header(header) = decoded[1] else { panic!("{:?}", decoded[1]) };
    assert_eq!(header.page.unwrap().0, 0xA0);
    assert_eq!(header.magazine.number(), 1);
    assert_eq!(header.subcode.unwrap().value(), 2);
    assert_eq!(header.control.unwrap(), page.control());
    let expected_display: Vec<_> = page.row(0).unwrap()[8..].iter().map(|&code| (code, true)).collect();
    assert_eq!(header.display.map(check_parity).to_vec(), expected_display);
    let Decoded::Links(links) = decoded[2] else { panic!("{:?}", decoded[2]) };
    assert_eq!(links.links[0].unwrap().page.0, 0xA1);
    assert_eq!(links.links[1].unwrap().magazine.number(), 2);
    assert!(matches!(decoded[3], Decoded::PageData { .. }));
    // X/28/1 is sent as read, and not decoded (decision E.95).
    assert!(matches!(decoded[4], Decoded::Ignored(_)));
    let Decoded::Enhancement(enhancement) = decoded[5] else { panic!("{:?}", decoded[5]) };
    assert_eq!(enhancement.triplets.map(Option::unwrap), sample_triplets());
    let rows: Vec<_> = decoded[6..]
        .iter()
        .map(|item| {
            let Decoded::Row(row) = item else { panic!("{item:?}") };
            assert!(row.bytes.iter().all(|&byte| check_parity(byte).1));
            (row.row, row.bytes.map(|byte| byte & 0x7F))
        })
        .collect();
    assert_eq!(rows.len(), 3);
    for (number, codes) in rows {
        assert_eq!(&codes, page.row(usize::from(number)).unwrap());
    }
    // X/28/0 from RE and PF: region 2 above national option index 4.
    let Decoded::PageData { packet: PagePacket::Format1(data), .. } = decoded[3] else { panic!() };
    assert_eq!(data.fields.designations.unwrap().default, (2 << 3) | 4);
    assert_eq!(data.format, Some(PageFormat { function: 0, coding: 0 }));
}

/// The TTI form of the pages of Levels 2.5 and 3.5 (REQ-INP-023): an
/// object page (coding 010), a DRCS page whose X/28/3 line sets its
/// function, a Magazine Organization Table without `PF` (coding 011), the
/// magazine packets on page 1FF, and a display page with X/27/4, X/28/0 and
/// X/28/4 lines.
pub(crate) fn enhancement_pages() -> Vec<u8> {
    use crate::encode::{nibble_row, packed_row};
    use neoteletext_core::packet::CompositionalLink;
    use neoteletext_core::packet::encode as packet_encode;
    use neoteletext_core::types::{Magazine, PageNumber, PresentationFields};
    let mut text = Vec::new();
    let mut line = |parts: &[&[u8]]| {
        for part in parts {
            text.extend_from_slice(part);
        }
        text.push(b'\n');
    };
    let magazine = Magazine::new(1).unwrap();
    let mut pointers = [Triplet::default(); 13];
    pointers[1] = packet_encode::pointer_triplet(0, 0x1FF);
    let mut definitions = [Triplet { address: ROW_ADDRESS_LAST, mode: MODE_TERMINATION, data: 0 }; 13];
    definitions[0] = Triplet { address: 0x38, mode: 0x15, data: 0 };
    definitions[1] = Triplet { address: 2, mode: MODE_G0_NO_DIACRITIC, data: b'O' };
    line(&[b"PN,1A000"]);
    line(&[b"SC,0300"]);
    line(&[b"PS,C000"]);
    line(&[b"PF,3,2"]);
    line(&[b"OL,1,", &packed_row(1, &pointers)]);
    line(&[b"OL,3,", &packed_row(0, &definitions)]);
    line(&[b"OL,26,", &packed_line(0, &definitions)]);
    line(&[b"PN,1B000"]);
    let mut ptus = [0x40u8; 40];
    ptus[..4].copy_from_slice(&[0x7F, 0x7F, 0x60, 0x7E]);
    line(&[b"OL,1,", &ptus]);
    let format = PageFormat { function: 5, coding: 0 };
    line(&[b"OL,28,", &packet_line(3, &packet_encode::page_function(magazine, format))]);
    line(&[b"PN,1FE00"]);
    let mut nibbles = [0u8; 40];
    for (index, value) in nibbles.iter_mut().enumerate() {
        *value = (index % 16) as u8;
    }
    line(&[b"OL,1,", &nibble_row(&nibbles)]);
    line(&[b"PN,1FF00"]);
    let fields = PresentationFields::default();
    line(&[b"OL,29,", &packet_line(0, &packet_encode::magazine_enhancement(magazine, 0, &fields))]);
    let colours = packet_encode::triplet_packet(magazine, 29, 1, &[Triplet::default(); 13]);
    line(&[b"OL,29,", &packet_line(1, &colours)]);
    line(&[b"OL,29,", &packet_line(4, &packet_encode::magazine_enhancement(magazine, 4, &fields))]);
    line(&[b"PN,10000"]);
    line(&[b"OL,1,Display"]);
    let pop = CompositionalLink {
        function: 1,
        validity: 3,
        magazine,
        page: PageNumber(0xA0),
        subpages: 1,
        marker_first: true,
        marker_second: false,
    };
    line(&[b"OL,27,", &packet_line(4, &packet_encode::compositional_links(magazine, 4, &[pop; 6]))]);
    let basic = PageFormat { function: 0, coding: 0 };
    line(&[b"OL,28,", &packet_line(0, &packet_encode::page_enhancement(magazine, 0, basic, &fields))]);
    line(&[b"OL,28,", &packet_line(4, &packet_encode::page_enhancement(magazine, 4, basic, &fields))]);
    text
}

/// The packed characters of a packet's designation code and triplets.
pub(super) fn packet_line(designation: u8, packet: &neoteletext_core::packet::Packet) -> Vec<u8> {
    let data = packet.data();
    let mut triplets = [Triplet::default(); 13];
    for (triplet, chunk) in triplets.iter_mut().zip(data[1..].chunks(3)) {
        *triplet = Triplet::from_value(neoteletext_core::ecc::hamming2418([chunk[0], chunk[1], chunk[2]]).unwrap());
    }
    packed_line(designation, &triplets)
}

pub(super) fn packets_of(page: &PageFile) -> Vec<neoteletext_core::packet::Packet> {
    let mut packets = Vec::new();
    encode_page(page, &mut |packet| packets.push(packet));
    packets
}
