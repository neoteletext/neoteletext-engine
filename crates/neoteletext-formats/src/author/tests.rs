//! Builder tests (REQ-INP-025): the builder reproduces the worked encodings
//! of `research/notes-level25-objects.md` sections 13.2 to 13.4 and
//! `research/notes-level25-drcs.md` sections 3.5, 4, 5 and 11, and what it
//! builds decodes back to the same fields.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;
use crate::item::Reader;
use crate::page::RawBody;
use neoteletext_core::consts::PACKET_LINKS;
use neoteletext_core::ecc::{hamming84_encode, hamming2418_encode};
use neoteletext_core::packet::{
    Decoded, DefaultObject, FontStyleData, MotPageEntry, ObjectKind, ObjectPacket, ObjectSource, ObjectUsage,
    TripletFunction, decode, mot_object_link,
};
use neoteletext_core::types::Magazine;
use std::vec::Vec;

const BOTH: ObjectUsage = ObjectUsage(0b11);

fn values(triplets: &Triplets) -> Vec<u32> {
    let resolve = |_: LocalObject, _: ObjectKind| -> Result<Triplet> { Err(AuthorError::UnknownLocalObject) };
    triplets.resolved(&resolve).unwrap().iter().map(|triplet| triplet.value()).collect()
}

fn one(build: impl FnOnce(&mut Triplets)) -> u32 {
    let mut triplets = Triplets::new();
    build(&mut triplets);
    values(&triplets)[0]
}

fn raw_triplets(page: &PageFile, number: u8, designation: u8) -> [Triplet; 13] {
    let raw = page.raw_packets.as_slice().iter().find(|raw| raw.number == number && raw.designation == designation);
    let Some(RawPacket { body: RawBody::Triplets(triplets), .. }) = raw else { panic!("no {number}/{designation}") };
    *triplets
}

fn packets_of(page: &PageFile) -> Vec<Packet> {
    let mut packets = Vec::new();
    encode_page(page, &mut |packet| packets.push(packet));
    packets
}

/// REQ-INP-025: the triplet encodings of objects section 13.2.
#[test]
fn object_triplets_match_the_worked_encodings() {
    let mut pop = ObjectPage::new(address(0x2E3).unwrap(), false);
    let c3 = pop.define_at(2, ObjectKind::Active, BOTH, 3, Triplets::new()).unwrap();
    assert_eq!(c3, ObjectRef { source: ObjectSource::Public, kind: ObjectKind::Active, number: public_number(2, 3) });
    let invocation = c3.invocation().to_triplet().value();
    assert_eq!(invocation, 0x19470);
    assert_eq!(hamming2418_encode(invocation), [0x81, 0x47, 0x32]);
    let definition = TripletFunction::ObjectDefinition { usage: BOTH, kind: ObjectKind::Active, number: c3.number };
    assert_eq!(definition.to_triplet().value(), 0x19578);
    assert_eq!(hamming2418_encode(0x19578), [0x43, 0x57, 0x32]);
    assert_eq!(
        one(|t| {
            t.origin(2, 56);
        }),
        0x1C42A
    );
    assert_eq!(hamming2418_encode(0x1C42A), [0xDB, 0x42, 0x38]);
    assert_eq!(
        one(|t| {
            t.terminate(crate::consts::TERMINATION_LOCAL_DATA_END);
        }),
        0x03FFF
    );
    assert_eq!(
        one(|t| {
            t.terminate(crate::consts::TERMINATION_FINAL_MORE);
        }),
        0x017FF
    );
    assert_eq!(hamming2418_encode(0x017FF), [0x76, 0x7F, 0x02]);
}

/// REQ-INP-025: a Local Object's number is the place of its definition;
/// with 17 triplets of local data and the marker, a Passive object lands
/// at X/26/1 triplet 5 (objects section 13.2: value 0x0ACE8).
#[test]
fn local_object_numbers_come_from_the_layout() {
    let mut page = DisplayPage::new(address(0x100).unwrap(), SubCode::new(0));
    let mut body = Triplets::new();
    body.character(0, b'P');
    let passive = page.local_object(ObjectKind::Passive, BOTH, body);
    page.local().position(1, 0).unwrap();
    page.local().pad_to(16, TripletFunction::SetActivePosition { address: 41, column: 0 });
    page.local().invoke_local(passive);
    let built = page.build().unwrap();
    let triplets: Vec<Triplet> = built.enhancements.as_slice().iter().flat_map(|packet| packet.triplets).collect();
    assert_eq!(triplets[16].value(), 0x0ACE8);
    assert_eq!(hamming2418_encode(0x0ACE8), [0xC9, 0x4E, 0x15]);
    assert_eq!(
        triplets[17],
        TripletFunction::Termination { kind: crate::consts::TERMINATION_LOCAL_DATA_OBJECTS_FOLLOW }.to_triplet()
    );
    let TripletFunction::ObjectDefinition { kind, number, .. } = triplets[18].function() else { panic!() };
    assert_eq!((kind, number.designation(), number.triplet()), (ObjectKind::Passive, 1, 5));
    assert_eq!(triplets[19].function(), TripletFunction::G0Diacritic { column: 0, code: b'P', diacritic: 0 });
    let last = TripletFunction::Termination { kind: crate::consts::TERMINATION_LOCAL_OBJECT_LAST }.to_triplet();
    assert!(triplets[20..].iter().all(|&triplet| triplet == last), "padded with the last marker");
    assert_eq!(triplets.len(), 26);
}

/// REQ-INP-025: a page with no local enhancement data sends no packet
/// X/26. Clause 13.8 makes any X/26 cancel the default objects, so an empty
/// one would change what the page shows.
#[test]
fn no_local_data_sends_no_packet_26() {
    let page = DisplayPage::new(address(0x400).unwrap(), SubCode::new(0));
    assert_eq!(page.build().unwrap().enhancements.as_slice(), []);
}

/// REQ-INP-025: the pointer table of objects section 13.2: C2 unused and
/// C3 at absolute triplet 30, which is packet 5 triplet 4.
#[test]
fn pointer_tables_are_computed() {
    let mut pop = ObjectPage::new(address(0x2E3).unwrap(), false);
    let mut long = Triplets::new();
    long.pad_to(
        28,
        TripletFunction::ForegroundColour {
            column: 0,
            colour: neoteletext_core::packet::ColourData { index: 1, high: 0 },
        },
    );
    pop.define_at(0, ObjectKind::Active, BOTH, 0, long).unwrap();
    let mut body = Triplets::new();
    body.character(3, b'X');
    pop.define_at(0, ObjectKind::Active, BOTH, 3, body).unwrap();
    let pages = pop.build().unwrap();
    assert_eq!(pages.len(), 1);
    let page = &pages[0];
    let packets = packets_of(page);
    let row = |number: u8| {
        packets
            .iter()
            .find_map(|packet| match decode(packet) {
                Ok(Decoded::Row(row)) if row.row == number => Some(row.bytes),
                _ => None,
            })
            .unwrap()
    };
    let pointers = ObjectPacket::new(&row(1));
    assert!(pointers.carries_pointers());
    assert_eq!(pointers.triplets[4].unwrap().value(), 0x03DFF);
    assert_eq!(hamming2418_encode(0x03DFF), [0xFE, 0x5F, 0x87]);
    assert_eq!(pointers.pointer(1, false), 0, "C0 at absolute triplet 0");
    assert_eq!(pointers.pointer(1, true), 0x1FF, "C1 unused");
    let definitions = ObjectPacket::new(&row(5));
    assert!(!definitions.carries_pointers());
    let TripletFunction::ObjectDefinition { number, .. } = definitions.triplets[4].unwrap().function() else {
        panic!()
    };
    assert_eq!(number, public_number(0, 3));
    assert!(
        packets.iter().all(|packet| !matches!(decode(packet), Ok(Decoded::Row(row)) if row.row == 2)),
        "packet 2 unused"
    );
    // Annex A.1: the last packet sent is X/5, S1 = 0.
    assert_eq!(page.subcode.value(), 5 << 8);
    assert_eq!(page.function, Some(crate::page::PageFunction { function: 3, coding: 2 }));
}

/// REQ-INP-025: objects of index 8 and above need further pointer
/// packets, and definitions then start after them (clause 10.5.1.2).
#[test]
fn pointer_packets_extend_for_high_indices() {
    let mut gpop = ObjectPage::new(address(0x1E0).unwrap(), true);
    let passive = gpop.define_at(1, ObjectKind::Passive, BOTH, 25, Triplets::new()).unwrap();
    assert_eq!(passive.source, ObjectSource::Global);
    assert_eq!(pointer_place(ObjectKind::Passive, 25), (4, 3, true));
    let pages = gpop.build().unwrap();
    let packets = packets_of(&pages[0]);
    let rows: Vec<u8> = packets
        .iter()
        .filter_map(|packet| if let Ok(Decoded::Row(row)) = decode(packet) { Some(row.row) } else { None })
        .collect();
    assert_eq!(rows, [1, 2, 3, 4, 5]);
    let Ok(Decoded::Row(row)) =
        decode(packets.iter().find(|p| matches!(decode(p), Ok(Decoded::Row(r)) if r.row == 4)).unwrap())
    else {
        panic!()
    };
    let table = ObjectPacket::new(&row.bytes);
    // Packet 5 triplet 0 is absolute triplet 26.
    assert_eq!(table.pointer(3, true), 26);
    assert_eq!(pages[0].subcode.value(), (5 << 8) | 1);
}

/// REQ-INP-025: the X/27/4 link of objects section 13.2.
#[test]
fn compositional_link_matches_the_worked_encoding() {
    let target = address(0x2E3).unwrap();
    let pop = link(LinkFunction::Pop, VALID_BOTH, target, 0b11);
    let (first, second) = pop.to_values(Magazine::new(1).unwrap());
    assert_eq!((first, second), (0x39CCD, 0x0000C));
    assert_eq!(hamming2418_encode(first), [0x66, 0x4C, 0xF3]);
    assert_eq!(hamming2418_encode(second), [0xEA, 0x80, 0x80]);
    let mut page = DisplayPage::new(address(0x100).unwrap(), SubCode::new(0));
    page.link_level_25(LinkFunction::Pop, target, 0b11).unwrap();
    let built = page.build().unwrap();
    let triplets = raw_triplets(&built, PACKET_LINKS, 4);
    assert_eq!((triplets[2].value(), triplets[3].value()), (0x39CCD, 0x0000C));
    let Some(Ok(Decoded::CompositionalLinks { links, .. })) =
        packets_of(&built).iter().map(decode).find(|item| matches!(item, Ok(Decoded::CompositionalLinks { .. })))
    else {
        panic!()
    };
    assert_eq!(links[1], Some(pop));
    assert!(links[0].unwrap().is_null() && links[2].unwrap().is_null() && links[5].unwrap().is_null());
}

/// REQ-INP-025: the table positions of objects sections 7.2 and 13.3, the
/// Hamming 8/4 values of section 13.2 and the default objects of 13.4.
#[test]
fn magazine_organization_table_places_pairs_and_links() {
    let magazine = Magazine::new(1).unwrap();
    let mut mot = Mot::new(magazine);
    let entry = MotPageEntry { global_objects: true, pop_link: 1 };
    for page in [0x23, 0x4F, 0xB7, 0xFE] {
        mot.page(PageNumber(page), entry);
    }
    let mut pop_link = object_link(address(0x1A0).unwrap(), 2);
    pop_link.default_objects = [
        Some(DefaultObject { kind: ObjectKind::Active, number: 0x32 }),
        Some(DefaultObject { kind: ObjectKind::Adaptive, number: 0x32 }),
    ];
    mot.object_link(MotLevel::Level25, 1, pop_link).unwrap();
    let page = mot.build().unwrap();
    let packets = packets_of(&page);
    let row = |number: u8| {
        packets.iter().find_map(|packet| match decode(packet) {
            Ok(Decoded::Row(row)) if row.row == number => Some(row.bytes),
            _ => None,
        })
    };
    let pair = |number: u8, byte: usize| {
        let bytes = row(number).unwrap();
        (bytes[byte - 6], bytes[byte - 5])
    };
    // The second byte of a pair names no DRCS page (decision E.95).
    let expected = (hamming84_encode(0x8 | 1), hamming84_encode(0));
    assert_eq!(pair(2, 12), expected, "x23");
    assert_eq!(pair(10, 28), expected, "x4F");
    assert_eq!(pair(6, 40), expected, "xB7");
    assert_eq!(pair(14, 14), expected, "xFE");
    let hamming: Vec<u8> = (0..16).map(hamming84_encode).collect();
    assert_eq!(
        hamming,
        [0x15, 0x02, 0x49, 0x5E, 0x64, 0x73, 0x38, 0x2F, 0xD0, 0xC7, 0x8C, 0x9B, 0xA1, 0xB6, 0xFD, 0xEA]
    );
    let links = row(19).unwrap();
    assert_eq!(mot_object_link(&links, 1), Some(pop_link));
    let raw = encode::mot_object_link(&pop_link);
    assert_eq!(&raw[5..8], &[0x9, 0x2, 0x3]);
    assert_eq!(mot_object_link(&links, 0).unwrap().page.0, 0xFF, "the GPOP link is not used");
    assert!(row(20).is_some() && row(21).is_some() && row(22).is_none(), "only the Level 2.5 packets");
    // X/21's links to pattern pages are written unused: magazine 1, page
    // FF, no sub-pages (Table 23, decision E.95).
    let unused = [1, 0xF, 0xF, 0].map(hamming84_encode);
    assert!(row(21).unwrap()[..32].as_chunks::<4>().0.iter().all(|link| *link == unused));
    assert_eq!(page.subcode.value(), 21 << 8);
}

/// REQ-INP-025: the DRCS triplets of DRCS sections 5 and 11, sent as
/// written though not acted on (decision E.95), and other worked
/// encodings, with their TTI characters.
#[test]
fn drcs_and_character_triplets_match_the_worked_encodings() {
    let cases = [
        (
            one(|t| {
                t.function(TripletFunction::ReservedRow { mode: 0b11000, address: 41, data: 0x70 });
            }),
            0x38629,
            [0x69, 0x58, 0x78],
        ),
        (
            one(|t| {
                t.function(TripletFunction::ReservedRow { mode: 0b11000, address: 41, data: 0x13 });
            }),
            0x09E29,
            [0x69, 0x78, 0x49],
        ),
        (
            one(|t| {
                t.function(TripletFunction::UnsupportedColumn { column: 2, mode: 0b01101, data: 0x40 });
            }),
            0x20342,
            [0x42, 0x4D, 0x60],
        ),
        (
            one(|t| {
                t.function(TripletFunction::UnsupportedColumn { column: 10, mode: 0b01101, data: 47 });
            }),
            0x17B4A,
            [0x4A, 0x6D, 0x57],
        ),
        (
            one(|t| {
                t.g3(0, 0x20);
            }),
            0x102C0,
            [0x40, 0x4B, 0x50],
        ),
        (
            one(|t| {
                t.g3_level_15(5, 0x5B);
            }),
            0x2D885,
            [0x45, 0x62, 0x6D],
        ),
        (
            one(|t| {
                t.font(4, FontStyleData { proportional: true, bold: true, italic: true, rows: 2 });
            }),
            0x13B84,
            [0x44, 0x6E, 0x53],
        ),
        (
            one(|t| {
                t.terminate(7);
            }),
            0x03FFF,
            [0x7F, 0x7F, 0x43],
        ),
    ];
    for (value, expected, characters) in cases {
        assert_eq!(value, expected, "{expected:#x}");
        let mut triplets = [Triplet::default(); 13];
        triplets[0] = Triplet::from_value(value);
        assert_eq!(&crate::encode::pack_triplets(&triplets)[..3], &characters, "{expected:#x}");
    }
}

/// Reads TTI text into page files.
fn read_tti(text: &[u8]) -> Vec<PageFile> {
    let mut read = Vec::new();
    let mut reader = crate::tti::TtiReader::new();
    let mut collect = |item: crate::Item<'_>| {
        if let crate::Item::Page(page) = item {
            read.push(*page);
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
    *stream.pages().into_iter().find(|page| page.address.mpp() == mpp).unwrap()
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
