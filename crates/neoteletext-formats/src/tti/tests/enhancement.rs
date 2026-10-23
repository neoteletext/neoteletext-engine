//! The TTI form of the pages of Levels 2.5 and 3.5 (REQ-INP-023): row
//! codings, page functions and malformed enhancement lines.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;

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

/// REQ-INP-023 and F-102: an X/28 line on a page that already holds
/// `RAW_PACKETS` packets is dropped, and sets no page function, since the
/// function would be written without the packet that carried it. The
/// `tti_enhancement` fuzz reproducer, hand-reduced: X/28/3 and fifteen X/27
/// lines fill the page, and the X/28/0 that follows used to set function
/// 0, which the written copy lost to the X/28/3 it kept.
#[test]
fn a_dropped_packet_28_sets_no_page_function() {
    use crate::item::Capacity;
    use crate::page::RAW_PACKETS;
    let mut text = b"PN,1FE00\nOL,28,Co\n".to_vec();
    for letter in b'A'..=b'O' {
        text.extend_from_slice(b"OL,27,");
        text.push(letter);
        text.push(b'\n');
    }
    text.extend_from_slice(b"OL,28,\n");
    let (pages, diagnostics) = read(&text);
    let page = &pages[0];
    assert_eq!(page.raw_packets.len(), RAW_PACKETS);
    assert!(!page.raw_packets.as_slice().iter().any(|raw| raw.number == 28 && raw.designation == 0));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.kind == DiagnosticKind::CapacityExceeded { what: Capacity::RawPackets })
    );
    let carried = page.raw_packets.as_slice().iter().find(|raw| raw.number == 28 && raw.designation == 3).unwrap();
    let packet = crate::encode::raw_packet(page.address.magazine, carried);
    let Ok(Decoded::PageData { packet: PagePacket::PageFunction(Some(format)), .. }) = decode(&packet) else {
        panic!("X/28/3 carries a page function");
    };
    assert_eq!(page.function, Some(PageFunction { function: format.function, coding: format.coding }));
    #[cfg(feature = "std")]
    {
        let write = |pages: &[PageFile]| {
            let references: Vec<&PageFile> = pages.iter().collect();
            let mut written = Vec::new();
            crate::write::write_tti(&references, &mut written).unwrap();
            written
        };
        let written = write(&pages);
        assert_eq!(write(&read(&written).0), written, "writing what was read back changes nothing");
    }
}
