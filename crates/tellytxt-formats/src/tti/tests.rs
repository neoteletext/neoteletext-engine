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
use std::vec::Vec;
use tellytxt_core::consts::{
    ALPHA_BLACK, ALPHA_RED, C11_SERIAL, COLOUR_GREEN, MODE_G0_NO_DIACRITIC, MODE_SET_ACTIVE_POSITION, MODE_TERMINATION,
    ROW_ADDRESS_LAST,
};
use tellytxt_core::ecc::check_parity;
use tellytxt_core::packet::{Decoded, PageFormat, PagePacket, Triplet, decode};

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
    assert_eq!(page.control().0, C11_SERIAL | tellytxt_core::consts::C12);
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
    assert_eq!(row.codes[20], tellytxt_core::consts::ALPHA_WHITE);
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
    assert_eq!(triplets[1], Triplet { address: 21, mode: tellytxt_core::consts::MODE_G2, data: 0x3B });
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
    assert!(matches!(decoded[4], Decoded::PageData { packet: PagePacket::DrcsColours(_), .. }));
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
