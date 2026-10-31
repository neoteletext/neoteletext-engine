//! Round trips of the builders and the REQ-TST-024 stream through the TTI
//! writer and reader: what is written reads back as the same pages.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;

/// Reads TTI text into page files.
fn read_tti(text: &[u8]) -> Vec<PageFile> {
    let mut read = Vec::new();
    let mut reader = crate::tti::TtiReader::new();
    let mut collect = |item: crate::Item<'_>| {
        if let crate::Item::Page(page) = item {
            read.push(page.clone());
        }
    };
    reader.feed(text, &mut collect);
    reader.finish(&mut collect);
    read
}

/// Checks that pages read back from TTI hold what was written.
fn assert_same_pages(read: &[PageFile], written: &[&PageFile]) {
    assert_eq!(read.len(), written.len());
    for (back, original) in read.iter().zip(written) {
        let name = original.address.mpp();
        assert_eq!(back.address, original.address);
        assert_eq!((back.rows, back.rows_present), (original.rows, original.rows_present), "{name:X}");
        assert_eq!(back.enhancements.as_slice(), original.enhancements.as_slice(), "{name:X}");
        assert_eq!(back.raw_packets.as_slice(), original.raw_packets.as_slice(), "{name:X}");
        assert_eq!((back.function, back.subcode, back.status), (original.function, original.subcode, original.status));
    }
}

/// A small stream of every kind of page the builder makes.
fn small_stream() -> Stream {
    let magazine = Magazine::new(3).unwrap();
    let mut stream = Stream::new();
    let mut pop = ObjectPage::new(address(0x3A0).unwrap(), false);
    let object = pop.define(0, ObjectKind::Active, BOTH, {
        let mut body = Triplets::new();
        body.text(0, b"POP");
        body
    });
    let object = object.unwrap();
    let mut mot = Mot::new(magazine);
    mot.object_link(MotLevel::Both, 1, object_link(pop.address(), 0)).unwrap();
    mot.page(PageNumber(0x00), MotPageEntry { global_objects: false, pop_link: 1 });
    let mut packets = MagazinePackets::new(magazine);
    packets.data(neutral_fields(0)).level_35(neutral_fields_level_35(0));
    let mut page = DisplayPage::new(address(0x300).unwrap(), SubCode::new(0));
    page.header(b"HEADER").text(1, 0, b"Display").unwrap();
    let local = page.local_object(ObjectKind::Passive, BOTH, Triplets::new());
    page.local().position(2, 0).unwrap().invoke(object).invoke_local(local);
    page.page_data(neutral_fields(1)).level_35_data(neutral_fields_level_35(1));
    page.link_level_25(LinkFunction::Pop, pop.address(), 1).unwrap();
    // Added in the reverse of Annex B.7.
    stream.push(page.build().unwrap()).extend(pop.build().unwrap());
    stream.push(mot.build().unwrap()).push(packets.build().unwrap());
    stream
}

/// REQ-INP-023 and REQ-INP-025: what the builder makes writes as TTI and
/// reads back unchanged, in the order of Annex B.7, and its T42 ends with
/// a time-filling header in each magazine.
#[test]
fn built_pages_round_trip_through_tti() {
    let stream = small_stream();
    let ordered = stream.in_transmission_order();
    let order: Vec<u16> = ordered.iter().map(|page| page.address.mpp()).collect();
    assert_eq!(order, [0x3FF, 0x3FE, 0x3A0, 0x300]);
    assert_same_pages(&read_tti(&stream.tti().unwrap()), &ordered);
    for (name, text) in stream.tti_files().unwrap() {
        let pages = read_tti(&text);
        assert_eq!(pages.len(), 1, "{name}");
        assert_eq!(name.as_bytes()[..3], pages[0].address.ascii());
    }
    let packets = stream.packets();
    let Ok(Decoded::Header(last)) = decode(packets.last().unwrap()) else { panic!() };
    assert_eq!((last.magazine.number(), last.page.unwrap().0), (3, 0xFF));
    assert_eq!(stream.t42().len(), packets.len() * 42);
}

/// REQ-TST-024 and REQ-INP-023: the authored stream writes as TTI and
/// reads back unchanged, in the order of Annex B.7.
#[test]
fn authored_stream_round_trips_through_tti() {
    let stream = streams::stream().unwrap();
    let ordered = stream.in_transmission_order();
    assert_same_pages(&read_tti(&stream.tti().unwrap()), &ordered);
    let classes: Vec<_> = ordered.iter().map(|page| crate::encode::transmission_class(page)).collect();
    assert!(classes.is_sorted());
    assert_eq!(classes[0], crate::encode::TransmissionClass::MagazinePackets);
}

fn authored_page(mpp: u16) -> PageFile {
    let stream = streams::stream().unwrap();
    stream.pages().into_iter().find(|page| page.address.mpp() == mpp).unwrap().clone()
}

/// REQ-TST-016, Annex C.2: in the third example foreground blue is the
/// last triplet of X/26/0 and foreground yellow the first of X/26/1, both
/// at the cell of the "A" after alpha red.
#[test]
fn annex_c2_examples_are_transmitted_as_the_annex_draws_them() {
    use neoteletext_core::packet::ColourData;
    let page = authored_page(0x120);
    let packets = page.enhancements.as_slice();
    let colour = |index| ColourData { index, high: 0 };
    assert_eq!(packets[0].triplets[1].function(), TripletFunction::ForegroundColour { column: 9, colour: colour(4) });
    assert_eq!(packets[0].triplets[3].function(), TripletFunction::ForegroundColour { column: 10, colour: colour(4) });
    assert_eq!(packets[0].triplets[12].function(), TripletFunction::ForegroundColour { column: 10, colour: colour(4) });
    assert_eq!(packets[1].triplets[0].function(), TripletFunction::ForegroundColour { column: 10, colour: colour(3) });
    for row in [4, 8, 12] {
        assert_eq!(&page.row(row).unwrap()[9..11], &[neoteletext_core::consts::ALPHA_RED, b'A']);
    }
}

/// REQ-TST-016, clause 13.9: the Level 1 row and the object of the
/// example, invoked as each type at the start of its row.
#[test]
fn clause_13_9_example_is_transmitted_as_the_clause_draws_it() {
    use neoteletext_core::consts::{ALPHA_BLACK, ALPHA_RED, COLOUR_BLUE_BIT, COLOUR_GREEN_BIT};
    let page = authored_page(0x117);
    let row = page.row(4).unwrap();
    assert_eq!(&row[1..17], b"THE FAST DOG AND");
    let attributes = (ALPHA_BLACK | COLOUR_BLUE_BIT, ALPHA_RED, ALPHA_BLACK | COLOUR_GREEN_BIT);
    assert_eq!((row[0], row[17], row[21]), attributes);
    assert_eq!(&row[18..21], b"CAT");
    assert_eq!(&row[22..28], b"RAN IN");
    let triplets: Vec<TripletFunction> =
        page.enhancements.as_slice().iter().flat_map(|packet| packet.triplets).map(Triplet::function).collect();
    let definitions: Vec<(ObjectKind, usize)> = triplets
        .iter()
        .enumerate()
        .filter_map(|(index, function)| match function {
            TripletFunction::ObjectDefinition { kind, .. } => Some((*kind, index)),
            _ => None,
        })
        .collect();
    let kinds: Vec<ObjectKind> = definitions.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, [ObjectKind::Active, ObjectKind::Adaptive, ObjectKind::Passive]);
    let pink = neoteletext_core::packet::ColourData { index: 16, high: 0 };
    let yellow = neoteletext_core::packet::ColourData { index: 3, high: 0 };
    for (_, start) in definitions {
        let body: Vec<_> = triplets[start + 1..start + 18].to_vec();
        let text: Vec<(u8, u8)> = body
            .iter()
            .filter_map(|function| match function {
                TripletFunction::G0Diacritic { column, code, diacritic: 0 } => Some((*column, *code)),
                _ => None,
            })
            .collect();
        let expected: Vec<(u8, u8)> = [(5u8, &b"LAZY"[..]), (10, b"COW"), (18, b"FOX"), (22, b"SAT")]
            .iter()
            .flat_map(|(column, word)| (*column..).zip(word.iter().copied()))
            .collect();
        assert_eq!(text, expected);
        assert!(body.contains(&TripletFunction::ForegroundColour { column: 10, colour: pink }));
        assert!(body.contains(&TripletFunction::ForegroundColour { column: 22, colour: yellow }));
    }
}
