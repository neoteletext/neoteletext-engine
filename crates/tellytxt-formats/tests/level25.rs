//! The authored Level 2.5 and 3.5 stream (REQ-TST-024) through the core's
//! decoder with an enhancement store: every enhancement page lands in the
//! enhancement store as encoded, every display page in the page store with
//! its packets 26 to 28, and the magazine packets in the magazine data
//! (REQ-INP-024, REQ-ACQ-016 to REQ-ACQ-020). The stream is also read back
//! from its TTI form, which must give the same packets (REQ-INP-023).
#![cfg(feature = "std")]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic, missing_docs)]

use tellytxt_core::acq::{Decoder, EnhancementPacket, EnhancementStore, EnhancementSubPage, StoredPage};
use tellytxt_core::config::{Config, PresentationLevel};
use tellytxt_core::consts::{
    ENHANCEMENT_PACKETS_LEVEL_35, MAGAZINES, MOT_PACKETS, OBJECT_POINTER_UNUSED, PACKET_ENHANCEMENT, PACKET_FIRST_ROW,
    PACKET_LINKS, PACKET_MAGAZINE_ENHANCEMENT, PACKET_PAGE_ENHANCEMENT, PACKET_ROW_25,
};
use tellytxt_core::fsm::PageState;
use tellytxt_core::packet::{
    Decoded, MagazinePacket, ObjectPacket, Packet, PacketMeta, PagePacket, TripletFunction, TripletLocation, decode,
    decode_address, triplet_location,
};
use tellytxt_core::types::{Instant, Magazine};
use tellytxt_formats::Item;
use tellytxt_formats::author::{Stream, pointer_place, streams};
use tellytxt_formats::detect::{AutoConfig, AutoReader, Format};
use tellytxt_formats::encode::{TransmissionClass, encode_page, transmission_class};
use tellytxt_formats::item::Reader;
use tellytxt_formats::page::PageFile;

/// One line period between packets, as the command-line program uses.
const LINE_US: u64 = 64;

/// Feeds packets to a decoder at a level and hands it to `check`.
fn with_decoder(packets: &[Packet], level: PresentationLevel, check: impl FnOnce(&Decoder<'_>)) {
    let mut pages = vec![StoredPage::EMPTY; 256];
    let mut sub_pages = vec![EnhancementSubPage::EMPTY; 256];
    let mut pool = vec![EnhancementPacket::EMPTY; ENHANCEMENT_PACKETS_LEVEL_35 + MAGAZINES * MOT_PACKETS];
    let config = Config { level, ..Config::default() };
    let mut decoder =
        Decoder::with_enhancement_store(&mut pages, EnhancementStore::new(&mut sub_pages, &mut pool), config);
    let mut now = 0;
    for packet in packets {
        now += LINE_US;
        decoder.push(packet, &PacketMeta::default(), Instant(now), &mut |_| {});
    }
    decoder.tick(Instant(now + 1_000_000), &mut |_| {});
    check(&decoder);
}

fn encoded(page: &PageFile) -> Vec<Packet> {
    let mut packets = Vec::new();
    encode_page(page, &mut |packet| packets.push(packet));
    packets
}

/// The rows X/1 to X/25 and packets X/26 a page sends, as stored bytes.
fn sent_rows(page: &PageFile) -> Vec<(u8, Option<u8>, [u8; 40])> {
    encoded(page)
        .iter()
        .filter_map(|packet| {
            let address = decode_address(packet).ok()?;
            match address.number {
                PACKET_FIRST_ROW..=PACKET_ROW_25 => Some((address.number, None, packet.data())),
                PACKET_ENHANCEMENT => {
                    let Ok(Decoded::Enhancement(enhancement)) = decode(packet) else { return None };
                    Some((address.number, Some(enhancement.designation), packet.data()))
                }
                _ => None,
            }
        })
        .collect()
}

/// Checks every page of the stream in the stores; returns how many
/// display and enhancement sub-pages were checked.
fn check_stream(stream: &Stream, packets: &[Packet], level: PresentationLevel) -> (usize, usize) {
    let mut counts = (0, 0);
    with_decoder(packets, level, |decoder| {
        let lookup = decoder.enhancement_lookup();
        for page in stream.pages() {
            let class = transmission_class(page);
            let magazine = page.address.magazine;
            match class {
                TransmissionClass::MagazinePackets => {
                    let data = lookup.magazine_data(magazine).unwrap();
                    for packet in encoded(page) {
                        match decode(&packet) {
                            Ok(Decoded::Magazine { packet: MagazinePacket::Data(fields), .. }) => {
                                assert_eq!(data.data, Some(fields));
                            }
                            Ok(Decoded::Magazine { packet: MagazinePacket::DrcsColours(colours), .. }) => {
                                assert_eq!(data.drcs_colours, Some(colours));
                            }
                            Ok(Decoded::Magazine { packet: MagazinePacket::Level35(fields), .. }) => {
                                assert_eq!(data.level35, Some(fields));
                            }
                            _ => {}
                        }
                    }
                }
                TransmissionClass::Display => {
                    let stored = decoder
                        .store()
                        .get(page.address, page.subcode)
                        .unwrap_or_else(|| panic!("display page {:X} missing", page.address.mpp()));
                    assert_eq!(stored.state, PageState::Complete, "{:X}", page.address.mpp());
                    assert!(lookup.sub_page(magazine, page.address.page, 0).is_none(), "{:X}", page.address.mpp());
                    check_display(page, stored);
                    counts.0 += 1;
                }
                _ => {
                    let s1 = (page.subcode.value() & 0xF) as u8;
                    let stored = lookup
                        .sub_page(magazine, page.address.page, s1)
                        .unwrap_or_else(|| panic!("enhancement page {:X}/{s1} missing", page.address.mpp()));
                    assert!(
                        decoder.store().latest(page.address).is_none_or(|copy| copy.state == PageState::Empty),
                        "{:X} also in the page store",
                        page.address.mpp()
                    );
                    let mut expected = 0u64;
                    for (number, designation, bytes) in sent_rows(page) {
                        let held = match designation {
                            None => stored.row(number),
                            Some(designation) => stored.enhancement(designation),
                        };
                        assert_eq!(held, Some(&bytes), "{:X} packet {number}", page.address.mpp());
                        let index = designation.map_or(usize::from(number) - 1, |d| 25 + usize::from(d));
                        expected |= 1 << index;
                    }
                    assert_eq!(stored.received(), expected, "{:X}", page.address.mpp());
                    assert_eq!(stored.subcode(), page.subcode);
                    assert_eq!(
                        stored.format().map(|format| (format.function, format.coding)),
                        page.function.map(|function| (function.function, function.coding))
                    );
                    if class == TransmissionClass::Objects {
                        check_pointers(&stored);
                    }
                    counts.1 += 1;
                }
            }
        }
        for magazine in [1, 2, 4, 5] {
            assert!(lookup.magazine_organization_table(Magazine::new(magazine).unwrap()).is_some());
        }
    });
    counts
}

/// The display page's packets 26 to 28 are stored as sent.
fn check_display(page: &PageFile, stored: &StoredPage) {
    for packet in encoded(page) {
        match decode(&packet) {
            Ok(Decoded::Enhancement(enhancement)) => {
                let code = usize::from(enhancement.designation);
                assert!(stored.enhancements_present & (1 << code) != 0);
                assert_eq!(stored.enhancements[code], enhancement.triplets);
            }
            Ok(Decoded::CompositionalLinks { designation, links, .. }) => {
                assert_eq!(stored.compositional_links[usize::from(designation - 4)], Some(links));
            }
            Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) => assert_eq!(stored.page_data, Some(data)),
            Ok(Decoded::PageData { packet: PagePacket::DrcsColours(data), .. }) => {
                assert_eq!(stored.drcs_colours, Some(data));
            }
            Ok(Decoded::PageData { packet: PagePacket::Level35(data), .. }) => {
                assert_eq!(stored.page_data_level35, Some(data));
            }
            Ok(Decoded::Row(row)) => assert_eq!(stored.cells[usize::from(row.row)], row.bytes),
            _ => {}
        }
    }
}

/// Every used pointer of a stored object sub-page lands on an Object
/// Definition whose number names that pointer (clause 10.5.1.2).
fn check_pointers(page: &tellytxt_core::acq::EnhancementPage<'_>) {
    for packet in 1..=4u8 {
        let Some(bytes) = page.row(packet) else { continue };
        let table = ObjectPacket::new(bytes);
        if !table.carries_pointers() {
            continue;
        }
        for triplet in 1..13 {
            for odd in [false, true] {
                let pointer = table.pointer(triplet, odd);
                if pointer == OBJECT_POINTER_UNUSED {
                    continue;
                }
                let definition = match triplet_location(pointer).unwrap() {
                    TripletLocation::Row { packet, triplet } => {
                        ObjectPacket::new(page.row(packet).unwrap()).triplets[usize::from(triplet)]
                    }
                    TripletLocation::Enhancement { designation, triplet } => {
                        ObjectPacket::new(page.enhancement(designation).unwrap()).triplets[usize::from(triplet)]
                    }
                };
                let TripletFunction::ObjectDefinition { kind, number, .. } = definition.unwrap().function() else {
                    panic!("pointer {pointer} of packet {packet} is not a definition")
                };
                let index = usize::from(number.pointer_packet_index()) * 8
                    + usize::from(number.pointer_group()) * 2
                    + usize::from(number.pointer_odd());
                assert_eq!(pointer_place(kind, index), (packet, triplet, odd));
                assert_eq!(number.subpage(), page.key().s1);
            }
        }
    }
}

/// REQ-TST-024, REQ-INP-024: the authored stream decodes into the stores
/// at both enhanced levels.
#[test]
fn authored_stream_lands_in_the_stores() {
    let stream = streams::stream().unwrap();
    let packets = stream.packets();
    for level in [PresentationLevel::Level25, PresentationLevel::Level35] {
        let (display, enhancement) = check_stream(&stream, &packets, level);
        assert_eq!(display, streams::display_pages().unwrap().len());
        // 1A0 (two sub-pages), 1F0, 1B0 (two), 1C0, 1B1, 1B2, 1FE, 2A9,
        // 3A0, 2F0, 2FE, 4F0, 4A0, 4FE, 5A0, 5A1, 5A2, 5FE.
        assert_eq!(enhancement, 20);
    }
    // Every class of page is present.
    let classes: Vec<TransmissionClass> = stream.pages().iter().map(|page| transmission_class(page)).collect();
    for class in [
        TransmissionClass::MagazinePackets,
        TransmissionClass::OrganizationTable,
        TransmissionClass::Drcs,
        TransmissionClass::Objects,
        TransmissionClass::Display,
    ] {
        assert!(classes.contains(&class), "{class:?}");
    }
    assert!(streams::display_pages().unwrap().len() >= 30);
}

/// REQ-INP-024: the stream follows Annex B.7, enhancement pages send their
/// rows before their packets X/26 (Annex B.2 item 3) and display pages the
/// opposite (item 2), and packets 27 and 28 follow each header.
#[test]
fn authored_stream_keeps_the_transmission_order() {
    let stream = streams::stream().unwrap();
    let mut last_class = TransmissionClass::MagazinePackets;
    for page in stream.in_transmission_order() {
        let class = transmission_class(page);
        assert!(class >= last_class);
        last_class = class;
        let numbers: Vec<u8> = encoded(page).iter().map(|packet| decode_address(packet).unwrap().number).collect();
        let header = numbers.iter().position(|&number| number == 0).unwrap();
        assert!(numbers[..header].iter().all(|&number| number == PACKET_MAGAZINE_ENHANCEMENT));
        let body = &numbers[header + 1..];
        let first_other = body.iter().position(|&number| number != PACKET_LINKS && number != PACKET_PAGE_ENHANCEMENT);
        let tail = &body[first_other.unwrap_or(body.len())..];
        assert!(tail.iter().all(|&number| number != PACKET_LINKS && number != PACKET_PAGE_ENHANCEMENT));
        let rows: Vec<usize> = (0..tail.len()).filter(|&i| tail[i] != PACKET_ENHANCEMENT).collect();
        let enhancements: Vec<usize> = (0..tail.len()).filter(|&i| tail[i] == PACKET_ENHANCEMENT).collect();
        if let (Some(&row), Some(&enhancement)) = (rows.first(), enhancements.first()) {
            if class == TransmissionClass::Display {
                assert!(enhancements.last().unwrap() < &row, "{:X}", page.address.mpp());
            } else {
                assert!(rows.last().unwrap() < &enhancement, "{:X}", page.address.mpp());
            }
        }
        assert!(tail.windows(2).all(|pair| pair[0] <= pair[1] || class == TransmissionClass::Display));
    }
}

/// REQ-TST-024: between them the pages use every row mode of Table 28 and
/// every column mode of Table 29, every object source and type, DRCS
/// characters of every mode, and every variant of X/28 and M/29.
#[test]
fn authored_stream_uses_every_mode() {
    use std::collections::BTreeSet;
    use tellytxt_core::packet::{ObjectKind, ObjectSource};
    let stream = streams::stream().unwrap();
    let mut row_modes = BTreeSet::new();
    let mut column_modes = BTreeSet::new();
    let mut invocations = BTreeSet::new();
    let mut packets_seen = BTreeSet::new();
    let mut drcs_modes = BTreeSet::new();
    let kind_name = |kind: ObjectKind| kind as u8;
    let pages_and_packets = stream.pages().into_iter().flat_map(|page| {
        let objects = transmission_class(page) == TransmissionClass::Objects;
        encoded(page).into_iter().map(move |packet| (objects, packet))
    });
    for (objects, packet) in pages_and_packets {
        let Ok(address) = decode_address(&packet) else { continue };
        let triplets: Vec<_> = match decode(&packet) {
            Ok(Decoded::Enhancement(enhancement)) => enhancement.triplets.iter().flatten().copied().collect(),
            // The definition rows of object pages hold triplets too.
            Ok(Decoded::Row(row)) if objects && row.row >= 3 => {
                let object = ObjectPacket::new(&row.bytes);
                assert!(object.triplets.iter().all(Option::is_some));
                object.triplets.iter().flatten().copied().collect()
            }
            Ok(Decoded::PageData { packet: page_packet, .. }) => {
                packets_seen.insert((address.number, packet_designation(&page_packet)));
                if let PagePacket::DrcsModes(modes) = page_packet {
                    drcs_modes.extend((0..48).filter_map(|ptu| modes.mode(ptu)));
                }
                Vec::new()
            }
            Ok(Decoded::Magazine { packet, .. }) => {
                let designation = match packet {
                    MagazinePacket::Data(_) => 0,
                    MagazinePacket::DrcsColours(_) => 1,
                    MagazinePacket::Level35(_) => 4,
                };
                packets_seen.insert((PACKET_MAGAZINE_ENHANCEMENT, designation));
                Vec::new()
            }
            Ok(Decoded::CompositionalLinks { designation, .. }) => {
                packets_seen.insert((PACKET_LINKS, designation));
                Vec::new()
            }
            _ => Vec::new(),
        };
        for triplet in triplets {
            if triplet.is_row_address() {
                row_modes.insert(triplet.mode);
            } else {
                column_modes.insert(triplet.mode);
            }
            if let TripletFunction::ObjectInvocation { source, kind, .. } = triplet.function() {
                invocations.insert((source as u8, kind_name(kind)));
            }
        }
    }
    assert_eq!(row_modes, (0..32).collect(), "row modes");
    assert_eq!(column_modes, (0..32).collect(), "column modes");
    let sources = [ObjectSource::Local, ObjectSource::Public, ObjectSource::Global];
    let kinds = [ObjectKind::Active, ObjectKind::Adaptive, ObjectKind::Passive];
    for source in sources {
        for kind in kinds {
            assert!(invocations.contains(&(source as u8, kind_name(kind))), "{source:?} {kind:?}");
        }
    }
    for expected in [(27, 4), (27, 5), (28, 0), (28, 1), (28, 3), (28, 4), (29, 0), (29, 1), (29, 4)] {
        assert!(packets_seen.contains(&expected), "{expected:?}");
    }
    for mode in [0, 1, 2, 3, 0xE, 0xF] {
        assert!(drcs_modes.contains(&mode), "DRCS mode code {mode}");
    }
}

fn packet_designation(packet: &PagePacket) -> u8 {
    match packet {
        PagePacket::Format1(_) => 0,
        PagePacket::DrcsColours(_) => 1,
        PagePacket::DrcsModes(_) => 3,
        PagePacket::Level35(_) => 4,
    }
}

/// REQ-INP-023 and REQ-TST-020: the stream written as TTI and read by the
/// TTI reader, which encodes each page as it reads it, gives the same
/// packets, and the per-page files hold the same pages.
#[test]
fn authored_stream_survives_tti() {
    let stream = streams::stream().unwrap();
    let text = stream.tti().unwrap();
    let mut reader =
        AutoReader::new(AutoConfig { format: Some(Format::Tti), encode_pages: true, ..AutoConfig::default() });
    let mut packets = Vec::new();
    let mut collect = |item: Item<'_>| {
        if let Item::Packet { packet, .. } = item {
            packets.push(packet);
        }
    };
    reader.feed(&text, &mut collect);
    reader.finish(&mut collect);
    let direct: Vec<Packet> = stream.in_transmission_order().into_iter().flat_map(encoded).collect();
    assert_eq!(packets, direct);
    let files = stream.tti_files().unwrap();
    let pages: usize = files.iter().map(|(_, text)| text.windows(3).filter(|window| window == b"PN,").count()).sum();
    assert_eq!(pages, stream.pages().len());
    assert!(files.iter().any(|(name, _)| name == "1FE.tti"));
}
