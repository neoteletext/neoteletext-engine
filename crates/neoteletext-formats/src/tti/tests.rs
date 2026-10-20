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
use crate::page::{FormMethod, IssueKind, LinkKey, PageFile};
use crate::testing::{diagnostics, pages, same_in_any_chunks};
use neoteletext_core::consts::{
    ALPHA_BLACK, ALPHA_RED, C11_SERIAL, COLOUR_GREEN, MODE_G0_NO_DIACRITIC, MODE_SET_ACTIVE_POSITION, MODE_TERMINATION,
    ROW_ADDRESS_LAST,
};
use neoteletext_core::ecc::check_parity;
use neoteletext_core::packet::{Decoded, PageFormat, PagePacket, Triplet, decode};
use std::vec::Vec;

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

#[test]
fn link_extension_example_of_appendix_e3() {
    let text = b"PN,10000\nOL,1,NEWS INDEX\nOL,3,Headlines        ...\nLN,R,101,Headlines\nLN,G,140,Sport\n\
LN,Y,weather:400,Weather\nLN,C,190,TV guide\nLN,I,100,\nFL,111,112,113,114,8FF,100\n";
    let (pages, diagnostics) = read(text);
    assert_eq!(diagnostics, []);
    let page = &pages[0];
    assert!(page.links_valid);
    assert_eq!(page.links.len(), 5);
    let yellow = page.links.as_slice()[2];
    assert_eq!(yellow.key, LinkKey::Yellow);
    assert_eq!(yellow.service.as_bytes(), b"weather");
    assert_eq!(yellow.address.mpp(), 0x400);
    let row = crate::encode::link_row(page).unwrap();
    assert_eq!(row.codes[0], ALPHA_RED);
    assert_eq!(&row.codes[1..10], b"Headlines");
    assert_eq!(&row.codes[11..16], b"Sport");
    assert_eq!(row.codes[20], neoteletext_core::consts::ALPHA_WHITE);
    assert_eq!(&row.codes[21..29], b">Weather");
    assert_eq!(row.marks.as_slice(), [21]);
    assert_eq!(row.codes[30], 6);
    assert_eq!(&row.codes[31..39], b"TV guide");

    // LN wins over FL (Appendix E.2 rule 3); the other-service link is null.
    let mut packets = Vec::new();
    encode_page(page, &mut |packet| packets.push(packet));
    let links = packets
        .iter()
        .find_map(|packet| if let Ok(Decoded::Links(links)) = decode(packet) { Some(links) } else { None })
        .unwrap();
    let targets: Vec<_> =
        links.links.iter().map(|link| link.map(|link| (link.magazine.number(), link.page.0))).collect();
    assert_eq!(targets, [Some((1, 0x01)), Some((1, 0x40)), None, Some((1, 0x90)), None, Some((1, 0x00))]);
    assert!(links.display_row_24);
    let row_24 = packets
        .iter()
        .find_map(|packet| match decode(packet) {
            Ok(Decoded::Row(row)) if row.row == 24 => Some(row.bytes),
            _ => None,
        })
        .unwrap();
    assert_eq!(row_24.map(|byte| check_parity(byte).0), row.codes);
    // The mark is written as the G2 guillemet by X/26 at row 24 column 21.
    let triplets: Vec<_> = packets
        .iter()
        .filter_map(
            |packet| if let Ok(Decoded::Enhancement(enhancement)) = decode(packet) { Some(enhancement) } else { None },
        )
        .flat_map(|enhancement| enhancement.triplets)
        .map(Option::unwrap)
        .collect();
    assert_eq!(triplets[0], Triplet { address: 40, mode: MODE_SET_ACTIVE_POSITION, data: 0 });
    assert_eq!(triplets[1], Triplet { address: 21, mode: neoteletext_core::consts::MODE_G2, data: 0x3B });
    assert_eq!(triplets[2].mode, MODE_TERMINATION);
}

#[test]
fn link_rule_violations_remove_the_links() {
    let five = b"PN,10000\nLN,R,101,a\nLN,G,102,b\nLN,Y,103,c\nLN,C,104,d\nLN,G,105,e\n";
    let (pages, diagnostics) = read(five);
    assert!(!pages[0].links_valid);
    assert!(pages[0].has_issue(IssueKind::RepeatedKey));
    assert_eq!(diagnostics[0], Diagnostic::at_line(53, 6, DiagnosticKind::Page(IssueKind::RepeatedKey)));
    let mut packets = Vec::new();
    encode_page(&pages[0], &mut |packet| packets.push(packet));
    assert!(!packets.iter().any(|packet| matches!(decode(packet), Ok(Decoded::Links(_)))));
    let (pages, _) = read(b"PN,10000\nLN,R,101,a\nLN,G,102,b\nLN,Y,103,c\nLN,C,104,d\nLN,R,105,e\n");
    assert!(pages[0].has_issue(IssueKind::RepeatedKey));
    let (pages, _) = read(b"PN,10000\nLN,R,1,a\nLN,Q,102,b\nLN,G,svc:9AB,x\n");
    assert_eq!(pages[0].issues.len(), 3);
    assert!(pages[0].issues.as_slice().iter().all(|issue| issue.kind == IssueKind::BadLinkTarget));
    let (pages, _) = read(b"PN,10000\nLN,R,101:0003,a\nLN,G,other-svc:2A0:3F7F,b\n");
    let links = pages[0].links.as_slice();
    assert_eq!(links[0].subcode, Some(SubCode::new(3)));
    assert_eq!(links[1].service.as_bytes(), b"other-svc");
    assert_eq!(links[1].address.mpp(), 0x2A0);
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

/// REQ-WEB-010: field definitions with columns, field types, options, the
/// token, the submission target and the result pages.
#[test]
fn form_example_of_appendix_c7() {
    let (pages, diagnostics) = read(FORM_EXAMPLE);
    assert_eq!(diagnostics, []);
    let form = &pages[0].form;
    assert!(form.interactive);
    let fields = form.fields.as_slice();
    assert_eq!(fields.len(), 4);
    assert_eq!((fields[0].kind, fields[0].row, fields[0].column, fields[0].width), (FieldKind::Text, 3, 7, 31));
    assert_eq!((fields[0].max_length, fields[0].required), (Some(50), true));
    assert_eq!(fields[1].kind, FieldKind::Email);
    assert_eq!(fields[1].max_length, Some(100));
    assert_eq!((fields[2].kind, fields[2].row, fields[2].column), (FieldKind::Radio, 6, 10));
    assert_eq!(fields[3].label.as_bytes(), b"Send message");
    assert_eq!(fields[3].action.as_bytes(), b"send_contact");
    let options = form.options.as_slice();
    assert_eq!(options.len(), 3);
    assert!(options[0].selected && !options[1].selected);
    assert_eq!(options.iter().map(|option| option.column).collect::<Vec<_>>(), [None, Some(19), Some(31)]);
    let (method, address) = form.submission.unwrap();
    assert_eq!((method, address.as_bytes()), (FormMethod::Post, b"/contact".as_slice()));
    let (success, error) = form.result_pages.unwrap();
    assert_eq!((success.mpp(), error.mpp()), (0x401, 0x401));
    assert_eq!(form.token.as_bytes(), b"5f2c9a17e0");
}

/// REQ-WEB-010, REQ-INP-013: the form commands never reach the packets, so
/// the page displays identically on a decoder without the web module.
#[test]
fn form_commands_leave_the_transmitted_page_unchanged() {
    let form_command = |line: &[u8]| {
        [b"FT".as_slice(), b"FD", b"FS", b"FP", b"OPTION"].iter().any(|command| line.starts_with(command))
    };
    let stripped: Vec<u8> = FORM_EXAMPLE
        .split_inclusive(|&byte| byte == b'\n')
        .filter(|line| !form_command(line))
        .flatten()
        .copied()
        .collect();
    // FT, four FD, three OPTION, FS and FP.
    assert_eq!(FORM_EXAMPLE.split(|&byte| byte == b'\n').filter(|line| form_command(line)).count(), 10);
    let packets = |text: &[u8]| {
        let (pages, diagnostics) = read(text);
        assert_eq!(diagnostics, []);
        let mut out = Vec::new();
        encode_page(&pages[0], &mut |packet| out.push(packet));
        (pages[0].form.fields.len(), out)
    };
    let (with_fields, with_form) = packets(FORM_EXAMPLE);
    let (without_fields, without_form) = packets(&stripped);
    assert_eq!((with_fields, without_fields), (4, 0));
    assert_eq!(with_form, without_form);
}

#[test]
fn form_problems_are_recorded() {
    // Version 2.0 syntax: no column and width.
    let (pages, _) = read(b"PN,40001\nOL,3,Name: ....\nFD,TEXT,name,50,Y\n");
    assert!(pages[0].has_issue(IssueKind::FormNeedsConversion));
    assert!(!pages[0].form.interactive);
    // Overlapping fields, a field off the row, the withdrawn FH, an option
    // with no field.
    let text = b"PN,40001\nOPTION,a,b,\nOL,3,x\nFD,TEXT,a,5,10,,N,,\nFD,TEXT,b,10,10,,N,,\nFD,TEXT,c,35,10,,N,,\nFH,X-Test,1\n";
    let (pages, diagnostics) = read(text);
    let page = &pages[0];
    for kind in [
        IssueKind::OptionWithoutField,
        IssueKind::FieldOverlap,
        IssueKind::FieldOutsideRow,
        IssueKind::WithdrawnCommand,
    ] {
        assert!(page.has_issue(kind), "{kind:?}");
    }
    assert!(!page.form.interactive);
    let overlap = page.issues.as_slice().iter().find(|issue| issue.kind == IssueKind::FieldOverlap).unwrap();
    assert_eq!(overlap.line, 5);
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.kind == DiagnosticKind::Page(IssueKind::WithdrawnCommand)));
    // Radio markers that collide.
    let text = b"PN,40001\nOL,6,( ) a ( ) b\nFD,RADIO,r,0,3,N\nOPTION,a,a,\nOPTION,b,b,,1\n";
    assert!(read(text).0[0].has_issue(IssueKind::FieldOverlap));
}

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
fn packet_line(designation: u8, packet: &neoteletext_core::packet::Packet) -> Vec<u8> {
    let data = packet.data();
    let mut triplets = [Triplet::default(); 13];
    for (triplet, chunk) in triplets.iter_mut().zip(data[1..].chunks(3)) {
        *triplet = Triplet::from_value(neoteletext_core::ecc::hamming2418([chunk[0], chunk[1], chunk[2]]).unwrap());
    }
    packed_line(designation, &triplets)
}

fn packets_of(page: &PageFile) -> Vec<neoteletext_core::packet::Packet> {
    let mut packets = Vec::new();
    encode_page(page, &mut |packet| packets.push(packet));
    packets
}

/// REQ-INP-023: the rows of enhancement pages are coded by their page
/// coding, X/28/3 and X/28/4 lines set the page function, page mFE is coded
/// Hamming 8/4 without `PF`, and the magazine packets go out before the
/// page mFF that carries them.
#[test]
fn enhancement_pages_are_coded_by_their_page_coding() {
    use neoteletext_core::ecc::hamming84;
    use neoteletext_core::packet::{MagazinePacket, ObjectPacket, TripletFunction};
    let (pages, diagnostics) = read(&enhancement_pages());
    assert_eq!(diagnostics, []);
    assert_eq!(pages.len(), 5);

    let objects = &pages[0];
    assert_eq!(objects.function, Some(PageFunction { function: 3, coding: 2 }));
    let decoded: Vec<Decoded> = packets_of(objects).iter().map(|packet| decode(packet).unwrap()).collect();
    // Header, the X/28/0 made from PF, rows 1 and 3, then X/26 (Annex B.2
    // item 3).
    assert!(matches!(decoded[1], Decoded::PageData { packet: PagePacket::Format1(_), .. }));
    let Decoded::Row(pointer_row) = decoded[2] else { panic!("{:?}", decoded[2]) };
    let pointers = ObjectPacket::new(&pointer_row.bytes);
    assert!(pointers.carries_pointers());
    assert_eq!((pointers.pointer(1, false), pointers.pointer(1, true)), (0, 0x1FF));
    let Decoded::Row(definition_row) = decoded[3] else { panic!() };
    let definitions = ObjectPacket::new(&definition_row.bytes);
    assert_eq!(definitions.byte_6, Some(0));
    assert!(matches!(definitions.triplets[0].unwrap().function(), TripletFunction::ObjectDefinition { .. }));
    assert!(matches!(decoded[4], Decoded::Enhancement(_)));
    assert_eq!(decoded.len(), 5);

    let drcs = &pages[1];
    assert_eq!(drcs.function, Some(PageFunction { function: 5, coding: 0 }));
    let rows: Vec<[u8; 40]> = packets_of(drcs)
        .iter()
        .filter_map(|packet| if let Ok(Decoded::Row(row)) = decode(packet) { Some(row.bytes) } else { None })
        .collect();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].iter().all(|&byte| check_parity(byte).1), "D-bytes carry odd parity");
    assert_eq!(rows[0].map(|byte| byte & 0x7F)[..4], [0x7F, 0x7F, 0x60, 0x7E]);

    let table = &pages[2];
    assert_eq!(table.function, None);
    let row = packets_of(table)
        .iter()
        .find_map(|packet| if let Ok(Decoded::Row(row)) = decode(packet) { Some(row.bytes) } else { None })
        .unwrap();
    let values: Vec<u8> = row.iter().map(|&byte| hamming84(byte).unwrap()).collect();
    assert_eq!(values, (0..40u8).map(|index| index % 16).collect::<Vec<_>>());

    let kinds: Vec<_> = packets_of(&pages[3]).iter().map(|packet| decode(packet).unwrap()).collect();
    assert!(matches!(kinds[0], Decoded::Magazine { packet: MagazinePacket::Data(_), .. }));
    // M/29/1 is sent as read, and not decoded (decision E.95).
    assert!(matches!(kinds[1], Decoded::Ignored(_)));
    assert!(matches!(kinds[2], Decoded::Magazine { packet: MagazinePacket::Level35(_), .. }));
    assert!(matches!(kinds[3], Decoded::Header(_)));

    let display = &pages[4];
    // X/28/0 and X/28/4 lines set the function of a basic page; no X/28/0
    // is made from it, since the page has its own.
    assert_eq!(display.function, Some(PageFunction { function: 0, coding: 0 }));
    let kinds: Vec<_> = packets_of(display).iter().map(|packet| decode(packet).unwrap()).collect();
    assert!(matches!(kinds[1], Decoded::CompositionalLinks { designation: 4, .. }));
    assert!(matches!(kinds[2], Decoded::PageData { packet: PagePacket::Format1(_), .. }));
    assert!(matches!(kinds[3], Decoded::PageData { packet: PagePacket::Level35(_), .. }));
    assert!(matches!(kinds[4], Decoded::Row(_)));
    assert_eq!(kinds.len(), 5);
}

/// REQ-INP-023 and REQ-SEC-001: malformed enhancement lines are bounded,
/// reported and never panic; what can be read is kept.
#[test]
fn malformed_enhancement_lines_are_reported() {
    let mut text = b"PN,1A000\nPF,3,2\n".to_vec();
    // Too short, an escape at the end, too long, and a designation above
    // 15, whose low four bits are used.
    text.extend_from_slice(b"OL,26,@ABC\n");
    text.extend_from_slice(b"OL,28,\x1b\n");
    text.extend_from_slice(b"OL,29,");
    text.extend(std::iter::repeat_n(b'A', 60));
    text.push(b'\n');
    text.extend_from_slice(b"OL,27,");
    text.extend(std::iter::repeat_n(b'Z', 40));
    text.push(b'\n');
    // Row text on an object page is kept as written and coded on output.
    text.extend_from_slice(b"OL,3,short\n");
    let (pages, diagnostics) = read(&text);
    let kinds: Vec<_> = diagnostics.iter().map(|diagnostic| (diagnostic.line, diagnostic.kind)).collect();
    let bad = DiagnosticKind::BadArgument { command: *b"OL" };
    assert_eq!(kinds, [(Some(3), bad), (Some(4), bad), (Some(5), bad)]);
    let page = &pages[0];
    assert_eq!(page.enhancements.len(), 1);
    assert_eq!(page.enhancements.as_slice()[0].designation, 0);
    // "Z" is 5/A: designation code 10 of X/27, thirteen triplets.
    assert!(page.raw_packets.as_slice().iter().any(|raw| raw.number == 27 && raw.designation == 0xA));
    assert!(packets_of(page).iter().all(|packet| decode(packet).is_ok()));
}

/// REQ-INP-023: the page function a packet X/28 carries wins over a `PF`
/// line, and X/28/0 over X/28/3, whatever order the lines come in, so a
/// page read, written and read again keeps its function. The writer puts
/// `PF` first; before this rule, a file with the `OL,28` line first read
/// as function 5 and its written copy as function 4.
#[test]
fn packet_28_function_wins_in_any_line_order() {
    use neoteletext_core::packet::encode as packet_encode;
    use neoteletext_core::types::{Magazine, PresentationFields};
    let magazine = Magazine::new(1).unwrap();
    let drcs = PageFormat { function: 4, coding: 0 };
    let x28_3 = [&b"OL,28,"[..], &packet_line(3, &packet_encode::page_function(magazine, drcs))].concat();
    let basic = PageFormat { function: 0, coding: 0 };
    let fields = PresentationFields::default();
    let x28_0 =
        [&b"OL,28,"[..], &packet_line(0, &packet_encode::page_enhancement(magazine, 0, basic, &fields))].concat();
    let page = |lines: &[&[u8]]| {
        let mut text = b"PN,1B000\n".to_vec();
        for line in lines {
            text.extend_from_slice(line);
            text.push(b'\n');
        }
        let (pages, _) = read(&text);
        pages[0].function.map(|function| PageFormat { function: function.function, coding: function.coding })
    };
    let pf: &[u8] = b"PF,5,1";
    let (x28_0, x28_3) = (&x28_0[..], &x28_3[..]);
    assert_eq!(page(&[pf, x28_3]), Some(drcs));
    assert_eq!(page(&[x28_3, pf]), Some(drcs));
    assert_eq!(page(&[x28_3, x28_0, pf]), Some(basic));
    assert_eq!(page(&[pf, x28_0, x28_3]), Some(basic));
}

/// the web-native concept sections 3 and 5: addresses as link
/// targets, page-number links and text links.
#[test]
fn addresses_number_links_and_text_links() {
    let text = b"PN,10000\nOL,4,Weather 300\nLN,R,https://weather.example/300.t42,Weather\n\
LK,300,https://weather.example/300.t42\nLK,400,sport:100\nLT,4,0,7,http://w.example/\n";
    let (pages, diagnostics) = read(text);
    assert_eq!(diagnostics, []);
    let page = &pages[0];
    let red = page.links.as_slice()[0];
    assert_eq!(red.url.as_bytes(), b"https://weather.example/300.t42");
    assert!(red.is_other_service(), "an address leads to another site");
    let numbers: Vec<_> = page.number_links.as_slice().iter().map(|link| link.number.mpp()).collect();
    assert_eq!(numbers, [0x300, 0x400]);
    assert_eq!(page.number_links.as_slice()[0].destination.url.as_bytes(), b"https://weather.example/300.t42");
    assert_eq!(page.number_links.as_slice()[1].destination.service.as_bytes(), b"sport");
    let text_link = page.text_links.as_slice()[0];
    assert_eq!((text_link.row, text_link.column, text_link.length), (4, 0, 7));
    assert_eq!(text_link.destination.url.as_bytes(), b"http://w.example/");
    // The writer keeps every link.
    #[cfg(feature = "std")]
    writer_keeps_links(page);
}

/// Decision E.88: `web:` and an address is a web page, allowed in `LN` and
/// `LT` lines and refused in `LK`, kept whole with its mark.
#[test]
fn a_web_target_is_kept_with_its_mark_and_refused_for_a_number() {
    let text = b"PN,10000\nOL,4,Story here\nLN,G,web:https://www.example/story,Story\n\
LT,4,0,5,web:https://www.example/story\nLK,300,web:https://www.example/\nLT,5,0,4,web:ftp://x.example/\n";
    let (pages, diagnostics) = read(text);
    let page = &pages[0];
    let green = page.links.as_slice()[0];
    assert_eq!(green.url.as_bytes(), b"web:https://www.example/story");
    assert!(green.is_web() && green.is_other_service());
    let text_link = page.text_links.as_slice()[0];
    assert!(text_link.destination.is_web());
    assert_eq!(page.text_links.as_slice().len(), 1, "a web target must be http or https");
    assert!(page.number_links.as_slice().is_empty(), "never in LK");
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    #[cfg(feature = "std")]
    {
        let mut written = Vec::new();
        crate::write::write_tti(&[page], &mut written).unwrap();
        let (again, _) = read(&written);
        assert_eq!(again[0].text_links.as_slice()[0].destination.url.as_bytes(), b"web:https://www.example/story");
    }
}

#[cfg(feature = "std")]
fn writer_keeps_links(page: &PageFile) {
    let mut written = Vec::new();
    crate::write::write_tti(&[page], &mut written).unwrap();
    let (again, _) = read(&written);
    // The same links, on other lines of the written file.
    let keys = |page: &PageFile| -> Vec<_> { page.links.as_slice().iter().map(|l| (l.key, l.url, l.label)).collect() };
    let numbers = |page: &PageFile| -> Vec<_> {
        page.number_links.as_slice().iter().map(|l| (l.number, l.destination)).collect()
    };
    let texts = |page: &PageFile| -> Vec<_> {
        page.text_links.as_slice().iter().map(|l| (l.row, l.column, l.length, l.destination)).collect()
    };
    assert_eq!(keys(&again[0]), keys(page));
    assert_eq!(numbers(&again[0]), numbers(page));
    assert_eq!(texts(&again[0]), texts(page));
}

#[test]
fn bad_addresses_and_text_links_are_refused() {
    let long = [b"LK,300,https://a.example/".as_slice(), &[b'x'; 300], b"\n"].concat();
    for line in [
        b"LK,300,javascript://alert(1)\n".as_slice(),
        b"LK,300,ftp://a.example/\n",
        b"LK,300,https://a.example/a b\n",
        b"LK,9A,100\n",
        b"LT,0,0,3,100\n",
        b"LT,24,0,3,100\n",
        b"LT,4,38,3,100\n",
        b"LT,4,0,0,100\n",
        &long,
    ] {
        let text = [b"PN,10000\n".as_slice(), line].concat();
        let (pages, _) = read(&text);
        assert!(pages[0].has_issue(IssueKind::BadLinkTarget), "{line:?}");
        assert!(pages[0].number_links.is_empty() && pages[0].text_links.is_empty());
    }
    let (pages, _) = read(b"PN,10000\nLK,300,100\nLK,300,200\n");
    assert!(pages[0].has_issue(IssueKind::RepeatedKey));
    assert_eq!(pages[0].number_links.len(), 1);
}

const FONT_SHA: &str = "5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d";

/// REQ-FNT-007: `TF,address,sha256` names the page's font; the address may
/// be relative, and applies to the page of the most recent `PN` only.
#[test]
fn a_font_line_names_the_page_font() {
    let text = std::format!("PN,10000\nTF,fonts/teletext.bdf,{FONT_SHA}\nOL,1,A\nPN,10100\nOL,1,B\n");
    let (pages, diagnostics) = read(text.as_bytes());
    assert_eq!(diagnostics, []);
    match pages[0].font {
        PageFont::Named { address, sha256 } => {
            assert_eq!(address.as_bytes(), b"fonts/teletext.bdf");
            assert_eq!(&sha256[..], FONT_SHA.as_bytes());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(pages[1].font, PageFont::None, "the next page names none");
    let absolute = std::format!("PN,10000\nTF,https://fonts.example/a,b/f.bdf,{}\n", FONT_SHA.to_uppercase());
    let (pages, diagnostics) = read(absolute.as_bytes());
    assert_eq!(diagnostics, []);
    assert!(
        matches!(pages[0].font, PageFont::Named { address, .. } if address.as_bytes() == b"https://fonts.example/a,b/f.bdf")
    );
}

/// REQ-FNT-007: a second line, a malformed line or a bad checksum makes the
/// page's font null, with a diagnostic, and the page is still read.
#[test]
fn a_bad_or_second_font_line_makes_the_font_null() {
    let short = &FONT_SHA[..63];
    let bad_digit = std::format!("{}g", &FONT_SHA[..63]);
    for line in [
        std::format!("TF,a.bdf,{FONT_SHA}\nTF,b.bdf,{FONT_SHA}"),
        std::format!("TF,a.bdf,{short}"),
        std::format!("TF,a.bdf,{bad_digit}"),
        std::format!("TF,{FONT_SHA}"),
        std::format!("TF,,{FONT_SHA}"),
        std::format!("TF,a b.bdf,{FONT_SHA}"),
        std::format!("TF,{},{FONT_SHA}", "a".repeat(257)),
    ] {
        let text = std::format!("PN,10000\n{line}\nOL,1,STILL SHOWN\n");
        let (pages, diagnostics) = read(text.as_bytes());
        assert_eq!(pages[0].font, PageFont::Null, "{line}");
        assert!(pages[0].issues.as_slice().iter().any(|issue| issue.kind == IssueKind::BadFontLine), "{line}");
        assert!(!diagnostics.is_empty(), "{line}");
        assert_eq!(&pages[0].rows[1][..11], b"STILL SHOWN", "{line}");
    }
}

/// Each page starts with no link lines of its own: those of the page
/// before it in the file are not carried over.
#[test]
fn a_pages_link_lines_are_its_own() {
    let (pages, _) = read(b"PN,10000\nLK,300,https://a.example/\nLT,4,0,3,301\nPN,10100\nOL,1,Second\n");
    assert_eq!(pages.len(), 2);
    assert_eq!((pages[0].number_links.len(), pages[0].text_links.len()), (1, 1));
    assert!(pages[1].number_links.is_empty(), "{:?}", pages[1].number_links.as_slice());
    assert!(pages[1].text_links.is_empty());
}

/// REQ-INT-006: `CU,row,column,width,flags[,acked]` is the page's cursor
/// record; the last line is kept, the writer writes it back, and a line
/// that cannot be read is reported and dropped.
#[test]
fn a_cursor_line_is_read_kept_and_written_back() {
    let (pages, _) = read(b"PN,10000\nOL,5,Name: \nCU,1,0,0,0\nCU,5,6,20,1,42\nPN,10100\nCU,255,0,0,0\n");
    let cursor = pages[0].cursor.unwrap();
    assert_eq!((cursor.row, cursor.column, cursor.width, cursor.flags, cursor.acked), (5, 6, 20, 1, 42));
    assert_eq!(pages[1].cursor.map(|cursor| (cursor.row, cursor.acked)), Some((255, 0)), "acked defaults to 0");
    #[cfg(feature = "std")]
    {
        let mut written = Vec::new();
        crate::write::write_tti(&[&pages[0]], &mut written).unwrap();
        let (again, _) = read(&written);
        assert_eq!(
            again[0].cursor.map(|cursor| (cursor.row, cursor.column, cursor.width, cursor.flags, cursor.acked)),
            Some((5, 6, 20, 1, 42))
        );
    }
    for line in [b"CU,5,6\n".as_slice(), b"CU,256,0,0,0\n", b"CU,5,6,20,1,70000\n", b"CU,a,0,0,0\n"] {
        let text = [b"PN,10000\n".as_slice(), line].concat();
        let (pages, _) = read(&text);
        assert!(pages[0].cursor.is_none() && pages[0].has_issue(IssueKind::BadCursorLine), "{line:?}");
    }
}
