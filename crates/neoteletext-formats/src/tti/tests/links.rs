//! The Link Extension (Appendix E.3) and the address, page-number and text
//! links of the web-native concept, with their writer round trips.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;

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
