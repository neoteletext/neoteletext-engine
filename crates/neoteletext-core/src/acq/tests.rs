//! Tests of acquisition and the page store (area ACQ): the magazine
//! machines driven by packets, serial and parallel mode, the parity merge,
//! eviction, enhancement pages, magazine data and the rolling header.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]

extern crate std;
// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::config::PresentationLevel;
use crate::consts::{
    C4_ERASE, C9_INTERRUPTED, C11_SERIAL, CODE_MASK_7BIT, DATA_BYTES, HEADER_DISPLAY_BYTES, TRIPLETS_PER_PACKET,
};
use crate::ecc::{has_odd_parity, with_odd_parity};
use crate::fsm::PageKey;
use crate::packet::{CompositionalLink, Decoded, Packet, PacketMeta, PageFormat, Triplet, decode, encode};
use crate::types::{PresentationFields, SubCode};
use std::vec::Vec;

fn mag(n: u8) -> Magazine {
    Magazine::new(n).unwrap()
}

fn text(s: &[u8]) -> [u8; 40] {
    let mut out = [with_odd_parity(b' '); 40];
    for (slot, &b) in out.iter_mut().zip(s) {
        *slot = with_odd_parity(b);
    }
    out
}

fn header(m: u8, page: u8, sub: u16, control: u16) -> Packet {
    let mut display = [with_odd_parity(b' '); 32];
    display[..4].copy_from_slice(&text(b"HDR!")[..4]);
    encode::header(mag(m), PageNumber(page), SubCode::new(sub), ControlBits(control), &display)
}

fn feed(decoder: &mut Decoder<'_>, packets: &[Packet], events: &mut Vec<Event>) {
    for (i, packet) in packets.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(i as u64 * 1000), &mut |e| events.push(e));
    }
}

fn key(mpp: u16, sub: u16) -> PageKey {
    PageKey { address: PageAddress::from_mpp(mpp).unwrap(), subcode: SubCode::new(sub) }
}

/// REQ-ACQ-002, REQ-ACQ-003: parallel mode.
#[test]
fn parallel_page_is_stored_and_completed() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[
            header(1, 0x00, 0, 0),
            encode::row(mag(1), 1, &text(b"HELLO")),
            encode::row(mag(2), 1, &text(b"OTHER")),
            header(1, 0x01, 0, 0),
        ],
        &mut events,
    );
    let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
    assert_eq!(&page.cells[1][..5], &text(b"HELLO")[..5]);
    assert_eq!(page.state, PageState::Complete);
    assert_eq!(&page.cells[0][8..12], &text(b"HDR!")[..4]);
    assert!(events.contains(&Event::PageCompleted(key(0x100, 0))));
    assert!(events.contains(&Event::Discarded(Discard::RowWithoutHeader)));
}

/// REQ-ACQ-002, REQ-ACQ-003: serial mode.
#[test]
fn serial_mode_header_of_any_magazine_ends_page() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[
            header(1, 0x00, 0, C11_SERIAL),
            encode::row(mag(1), 1, &text(b"ONE")),
            header(2, 0x00, 0, C11_SERIAL),
            encode::row(mag(1), 2, &text(b"LATE")),
        ],
        &mut events,
    );
    assert!(events.contains(&Event::PageCompleted(key(0x100, 0))));
    assert!(events.contains(&Event::Discarded(Discard::OtherMagazine)));
    let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
    assert!(!page.has_row(2));
}

/// REQ-ACQ-008: a corrupted retransmission replaces the row byte by byte,
/// keeping the good stored byte where the new one failed parity.
#[test]
fn erase_bit_clears_rows_and_parity_merge_keeps_good_bytes() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    let mut corrupted = text(b"BNEW");
    corrupted[0] ^= 0x01; // breaks parity of the first byte
    feed(
        &mut decoder,
        &[
            header(1, 0x00, 0, 0),
            encode::row(mag(1), 1, &text(b"GOOD")),
            encode::row(mag(1), 3, &text(b"KEEP")),
            header(1, 0x00, 0, 0),
            encode::row(mag(1), 1, &corrupted),
            header(1, 0x01, 0, 0),
        ],
        &mut events,
    );
    let address = PageAddress::from_mpp(0x100).unwrap();
    let page = decoder.store().get(address, SubCode::ANY).unwrap();
    assert_eq!(page.cells[1][0], with_odd_parity(b'G'), "E.8: good byte kept");
    assert_eq!(&page.cells[1][1..4], &text(b"NEW")[..3], "good new bytes replace the old");
    assert!(page.has_row(3), "rows not received keep their content");
    feed(&mut decoder, &[header(1, 0x00, 0, C4_ERASE), header(1, 0x02, 0, 0)], &mut events);
    let page = decoder.store().get(address, SubCode::ANY).unwrap();
    assert!(!page.has_row(3) && !page.has_row(1), "C4 erased rows");
    assert!(events.contains(&Event::PageErased(key(0x100, 0))));
}

/// REQ-ACQ-003: the decision E.19 timeout. REQ-ACQ-013: at Level 1.5
/// pages with hexadecimal numbers are not stored.
#[test]
fn timeout_completes_and_hex_pages_are_not_stored() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let level_15 = Config { level: PresentationLevel::Level15, ..Config::default() };
    let mut decoder = Decoder::new(&mut slots, level_15);
    let mut events = Vec::new();
    feed(&mut decoder, &[header(3, 0x45, 0, 0), header(4, 0xA5, 0, 0)], &mut events);
    decoder.tick(Instant(1_000_000), &mut |e| events.push(e));
    assert!(events.contains(&Event::PageCompleted(key(0x345, 0))));
    assert!(!decoder.store().contains(PageAddress::from_mpp(0x4A5).unwrap()));
}

#[test]
fn subpages_are_kept_separately_and_eviction_is_lru() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[header(1, 0x00, 1, 0), header(1, 0x00, 2, 0), header(1, 0x01, 0, 0), header(1, 0x02, 0, 0)],
        &mut events,
    );
    let address = PageAddress::from_mpp(0x100).unwrap();
    let mut subcodes = [SubCode::default(); 4];
    let count = decoder.store().subcodes(address, &mut subcodes);
    // 100/0001 was evicted first, then 100/0002; 101 and 102 remain.
    assert_eq!(count, 0);
    assert!(decoder.store().contains(PageAddress::from_mpp(0x101).unwrap()));
    assert!(decoder.store().contains(PageAddress::from_mpp(0x102).unwrap()));
}

#[test]
fn subcode_listing_is_sorted() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[header(1, 0x00, 3, 0), header(1, 0x00, 1, 0), header(1, 0x00, 2, 0), header(1, 0x01, 0, 0)],
        &mut events,
    );
    let mut subcodes = [SubCode::default(); 8];
    let count = decoder.store().subcodes(PageAddress::from_mpp(0x100).unwrap(), &mut subcodes);
    assert_eq!(&subcodes[..count], &[SubCode::new(1), SubCode::new(2), SubCode::new(3)]);
}

/// REQ-GEN-006: a listing buffer smaller than the stored sub-pages keeps
/// the lowest sub-codes, without panicking when a later sub-code is
/// higher than every one already listed.
#[test]
fn subcode_listing_into_a_full_buffer_keeps_the_lowest() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[header(1, 0x00, 1, 0), header(1, 0x00, 2, 0), header(1, 0x00, 3, 0), header(1, 0x01, 0, 0)],
        &mut events,
    );
    let mut subcodes = [SubCode::default(); 2];
    let count = decoder.store().subcodes(PageAddress::from_mpp(0x100).unwrap(), &mut subcodes);
    assert_eq!(&subcodes[..count], &[SubCode::new(1), SubCode::new(2)]);
}

/// REQ-ACQ-001, REQ-ACQ-002: parallel magazines interleave, the mode
/// follows C11 of the latest header, and in serial mode rows of another
/// magazine are discarded and counted.
#[test]
fn interleaved_magazines_and_serial_mode_follow_c11() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[
            header(1, 0x00, 0, 0),
            header(2, 0x00, 0, 0),
            encode::row(mag(1), 1, &text(b"ONE")),
            encode::row(mag(2), 1, &text(b"TWO")),
            encode::row(mag(1), 2, &text(b"ONE2")),
        ],
        &mut events,
    );
    assert!(!decoder.serial);
    let receiving = |decoder: &Decoder<'_>, magazine: u8| decoder.magazines[mag(magazine).index()];
    assert_eq!(receiving(&decoder, 1), MagazineState::Receiving(key(0x100, 0)));
    assert_eq!(receiving(&decoder, 2), MagazineState::Receiving(key(0x200, 0)));
    let one = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
    assert_eq!((&one.cells[1][..3], &one.cells[2][..4]), (&text(b"ONE")[..3], &text(b"ONE2")[..4]));
    let two = decoder.store().get(PageAddress::from_mpp(0x200).unwrap(), SubCode::ANY).unwrap();
    assert_eq!(&two.cells[1][..3], &text(b"TWO")[..3]);
    assert!(!two.has_row(2));
    let discarded = decoder.statistics().discarded;
    events.clear();
    feed(
        &mut decoder,
        &[header(3, 0x00, 0, C11_SERIAL), encode::row(mag(3), 1, &text(b"S")), encode::row(mag(4), 1, &text(b"X"))],
        &mut events,
    );
    assert!(decoder.serial);
    assert_eq!(decoder.statistics().discarded, discarded + 1, "the magazine 4 row");
    let reasons: Vec<&Event> = events.iter().filter(|event| matches!(event, Event::Discarded(_))).collect();
    assert_eq!(reasons, [&Event::Discarded(Discard::OtherMagazine)], "Annex B.3");
    let three = decoder.store().get(PageAddress::from_mpp(0x300).unwrap(), SubCode::ANY).unwrap();
    assert_eq!(three.cells[1][0], with_odd_parity(b'S'));
    feed(&mut decoder, &[header(3, 0x01, 0, 0)], &mut events);
    assert!(!decoder.serial, "C11 clear: parallel again");
}

/// REQ-ACQ-003: time-filling and uncorrectable headers end the page in
/// progress and start nothing.
#[test]
fn time_filling_and_bad_headers_end_a_page_without_starting_one() {
    let mut bad = header(1, 0x01, 0, 0);
    // Two bit errors in the page units byte are uncorrectable.
    bad.bytes[2] ^= 0b11;
    let Ok(Decoded::Header(bad_header)) = decode(&bad) else { panic!("not a header") };
    assert_eq!(bad_header.page, None);
    for ender in [header(1, 0xFF, 0, 0), bad] {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(
            &mut decoder,
            &[
                header(1, 0x00, 0, 0),
                encode::row(mag(1), 1, &text(b"IN")),
                ender,
                encode::row(mag(1), 2, &text(b"OUT")),
            ],
            &mut events,
        );
        assert!(events.contains(&Event::PageCompleted(key(0x100, 0))));
        assert_eq!(events.iter().filter(|event| matches!(event, Event::PageStarted(_))).count(), 1);
        let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
        assert!(page.has_row(1) && !page.has_row(2));
        assert_eq!(decoder.store().pages().iter().filter(|page| page.state != PageState::Empty).count(), 1);
    }
}

/// Storage for the enhancement store in tests.
struct EnhancementMemory {
    sub_pages: Vec<EnhancementSubPage>,
    packets: Vec<EnhancementPacket>,
}

impl EnhancementMemory {
    fn new(sub_pages: usize, packets: usize) -> Self {
        Self {
            sub_pages: std::vec![EnhancementSubPage::EMPTY; sub_pages],
            packets: std::vec![EnhancementPacket::EMPTY; packets],
        }
    }

    fn store(&mut self) -> EnhancementStore<'_> {
        EnhancementStore::new(&mut self.sub_pages, &mut self.packets)
    }
}

/// Forty data bytes of Hamming 24/18 triplets with an even-parity
/// first byte, which the parity merge of decision E.8 would refuse.
fn object_bytes(seed: u8) -> [u8; DATA_BYTES] {
    let triplets = [Triplet { address: seed % 40, mode: 0x10, data: 0x41 }; TRIPLETS_PER_PACKET];
    let data = encode::triplet_packet(mag(1), 1, 1, &triplets).data();
    assert!(!data[1..].iter().all(|&byte| has_odd_parity(byte)));
    data
}

fn object_row(m: u8, row: u8, seed: u8) -> Packet {
    let mut packet = encode::address(mag(m), row);
    packet.bytes[2..].copy_from_slice(&object_bytes(seed));
    packet
}

fn table_with_pop_link(m: u8, target: u8) -> [u8; DATA_BYTES] {
    let mut nibbles = [0u8; DATA_BYTES];
    // POP link 1 at packet bytes 16 to 25 (Table 20).
    let link = crate::packet::MotObjectLink {
        magazine: mag(m),
        x27_flag: false,
        page: PageNumber(target),
        subpage_count: 0,
        fallback: crate::types::FallbackFields::default(),
        default_objects: [None; 2],
    };
    nibbles[10..20].copy_from_slice(&encode::mot_object_link(&link));
    nibbles
}

/// REQ-ACQ-016, REQ-ACQ-020, REQ-ACQ-021, REQ-ACQ-022: the Magazine
/// Organization Table and a page it names are stored as received, read
/// back through the lookup, replaced by new receptions, erased by C4,
/// and reported when they change.
#[test]
fn enhancement_pages_are_stored_as_received() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut memory = EnhancementMemory::new(8, 100);
    let mut decoder = Decoder::with_enhancement_store(&mut slots, memory.store(), Config::default());
    let mut events = Vec::new();
    let table = table_with_pop_link(1, 0xA0);
    // A sub-code of S4 S3 = 25 (last packet), S2 = 0, S1 = 0.
    let table_subcode = 25 << 8;
    let table_packet = encode::nibble_packet(mag(1), 19, &table);
    feed(&mut decoder, &[header(1, 0xFE, table_subcode, 0), table_packet, header(1, 0x00, 0, 0)], &mut events);
    let lookup = decoder.enhancement_lookup();
    let mot = lookup.magazine_organization_table(mag(1)).unwrap();
    assert_eq!((mot.row(19), mot.last_packet()), (Some(&table_packet.data()), 25));
    assert!(!decoder.store().contains(PageAddress::from_mpp(0x1FE).unwrap()), "kept only as an enhancement page");
    assert!(events.contains(&Event::EnhancementPageChanged(EnhancementKey {
        address: PageAddress::from_mpp(0x1FE).unwrap(),
        s1: 0
    })));
    // Page 1A0, named by the table, sub-page S1 = 2: stored as received,
    // then replaced byte for byte by a reception whose bytes differ.
    let object_key = EnhancementKey { address: PageAddress::from_mpp(0x1A0).unwrap(), s1: 2 };
    for seed in [3, 7] {
        events.clear();
        feed(&mut decoder, &[header(1, 0xA0, 0x0002, 0), object_row(1, 1, seed), header(1, 0x00, 0, 0)], &mut events);
        let page = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap();
        assert_eq!(page.row(1), Some(&object_bytes(seed)), "seed {seed}: no parity merge");
        assert!(events.contains(&Event::EnhancementPageChanged(object_key)));
    }
    let generation = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap().generation();
    events.clear();
    feed(&mut decoder, &[header(1, 0xA0, 0x0002, 0), object_row(1, 1, 7), header(1, 0x00, 0, 0)], &mut events);
    assert!(!events.iter().any(|event| matches!(event, Event::EnhancementPageChanged(_))), "nothing changed");
    // A new update indicator S2 is reported even with the same bytes.
    feed(&mut decoder, &[header(1, 0xA0, 0x0012, 0), object_row(1, 1, 7), header(1, 0x00, 0, 0)], &mut events);
    let page = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap();
    assert_ne!(page.generation(), generation);
    assert!(events.contains(&Event::EnhancementPageChanged(object_key)));
    // C4 erases the stored packets before the new ones.
    feed(&mut decoder, &[header(1, 0xA0, 0x0002, C4_ERASE), object_row(1, 2, 1), header(1, 0x00, 0, 0)], &mut events);
    let page = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap();
    assert_eq!((page.row(1), page.row(2)), (None, Some(&object_bytes(1))));
    assert_eq!(page.received(), 1 << row_index(2).unwrap());
    assert!(decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 0).is_none());
}

/// REQ-ACQ-016: a page that becomes an enhancement page after it was
/// first stored is stored as received from its next reception on, both
/// when an X/27/4 link names it and when its X/28/0 says so.
#[test]
fn pages_become_enhancement_pages_by_link_or_page_function() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut memory = EnhancementMemory::new(8, 100);
    let mut decoder = Decoder::with_enhancement_store(&mut slots, memory.store(), Config::default());
    let mut events = Vec::new();
    let object = [header(2, 0xB0, 0, 0), object_row(2, 3, 5), header(2, 0x00, 0, 0)];
    feed(&mut decoder, &object, &mut events);
    let address = PageAddress::from_mpp(0x2B0).unwrap();
    assert!(decoder.store().contains(address), "REQ-ACQ-017: stored at Level 3.5");
    assert!(decoder.enhancement_lookup().sub_page(mag(2), PageNumber(0xB0), 0).is_none());
    let link = CompositionalLink {
        function: 0b01,
        validity: 0b11,
        magazine: mag(2),
        page: PageNumber(0xB0),
        subpages: 1,
        marker_first: true,
        marker_second: false,
    };
    let links = encode::compositional_links(mag(2), 4, &[link; 6]);
    feed(&mut decoder, &[header(2, 0x00, 0, 0), links, header(2, 0x01, 0, 0)], &mut events);
    assert_eq!(
        decoder.store().get(PageAddress::from_mpp(0x200).unwrap(), SubCode::ANY).unwrap().compositional_links[0],
        Some([Some(link); 6])
    );
    feed(&mut decoder, &object, &mut events);
    let page = decoder.enhancement_lookup().sub_page(mag(2), PageNumber(0xB0), 0).unwrap();
    assert_eq!(page.row(3), Some(&object_bytes(5)));
    // X/28/0 with page function 0011 (POP), before the rows.
    let format = PageFormat { function: 0b0011, coding: 0b010 };
    let page_data = encode::page_enhancement(mag(3), 0, format, &PresentationFields::default());
    feed(&mut decoder, &[header(3, 0xC0, 0, 0), page_data, object_row(3, 1, 9), header(3, 0x00, 0, 0)], &mut events);
    let page = decoder.enhancement_lookup().sub_page(mag(3), PageNumber(0xC0), 0).unwrap();
    assert_eq!((page.format(), page.row(1)), (Some(format), Some(&object_bytes(9))));
    assert!(!decoder.store().contains(PageAddress::from_mpp(0x3C0).unwrap()), "the copy of one reception is released");
}

/// REQ-ACQ-019: the enhancement store keeps to its capacity in packets,
/// replacing the least recently received sub-page, and drops packets
/// only when every slot belongs to a sub-page being received.
#[test]
fn enhancement_store_capacity_and_eviction() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut memory = EnhancementMemory::new(4, 3);
    let mut decoder = Decoder::with_enhancement_store(&mut slots, memory.store(), Config::default());
    let mut events = Vec::new();
    let rows = |page: u8, count: u8| {
        let mut packets = std::vec![header(4, page, 0, 0)];
        packets.extend((1..=count).map(|row| object_row(4, row, row)));
        packets
    };
    feed(&mut decoder, &[header(4, 0xFE, 0, 0), object_row(4, 1, 1), header(4, 0x00, 0, 0)], &mut events);
    // A sub-page of five packets in a pool of three: the table is
    // evicted, and two packets are dropped.
    let mut five = rows(0xFE, 5);
    five[0] = header(4, 0xFE, 1, 0);
    feed(&mut decoder, &five, &mut events);
    assert_eq!(decoder.enhancement_store().packets_used(), 3);
    assert!(decoder.enhancement_lookup().magazine_organization_table(mag(4)).is_none(), "S1 = 0 was evicted");
    let page = decoder.enhancement_lookup().sub_page(mag(4), PageNumber(0xFE), 1).unwrap();
    assert_eq!(page.received(), 0b111);
    assert_eq!(decoder.statistics().enhancement_dropped, 2);
    // A decoder without an enhancement store keeps no enhancement pages.
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    feed(&mut decoder, &rows(0xFE, 2), &mut events);
    assert!(!decoder.enhancement_store().is_enabled());
    assert!(decoder.enhancement_lookup().magazine_organization_table(mag(4)).is_none());
}

/// REQ-ACQ-018: M/29/0 and M/29/4 are kept per magazine, each
/// replacing the last, and are discarded when the network
/// identification changes or a new input starts.
#[test]
fn magazine_data_is_kept_until_the_service_changes() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    let fields = PresentationFields {
        colours: Some(crate::types::DefaultColourFields { screen: 3, ..Default::default() }),
        ..PresentationFields::default()
    };
    let service = |network_id: u16| {
        let mut packet = encode::address(mag(8), 30);
        packet.bytes[2] = crate::ecc::hamming84_encode(0);
        let [high, low] = network_id.to_be_bytes();
        packet.bytes[9] = high.reverse_bits();
        packet.bytes[10] = low.reverse_bits();
        packet
    };
    feed(
        &mut decoder,
        &[
            service(0x1234),
            encode::magazine_enhancement(mag(5), 0, &fields),
            encode::magazine_enhancement(mag(5), 4, &fields),
            service(0x1234),
        ],
        &mut events,
    );
    let data = decoder.magazine_data(mag(5));
    let colours_of = |fields: Option<PresentationFields>| fields.and_then(|fields| fields.colours);
    assert_eq!((colours_of(data.data), colours_of(data.level35)), (fields.colours, fields.colours));
    assert_eq!(decoder.enhancement_lookup().magazine_data(mag(5)), Some(&data));
    events.clear();
    feed(&mut decoder, &[service(0x4321)], &mut events);
    assert_eq!(decoder.magazine_data(mag(5)), MagazineData::default());
    assert!(events.contains(&Event::MagazineData(mag(5))));
    feed(&mut decoder, &[encode::magazine_enhancement(mag(5), 0, &fields)], &mut events);
    // The same packet again, as a cycle repeats it, changes nothing and
    // is no event (decision E.103); a different one is.
    events.clear();
    feed(&mut decoder, &[encode::magazine_enhancement(mag(5), 0, &fields)], &mut events);
    assert!(!events.contains(&Event::MagazineData(mag(5))), "a repeat is no event: {events:?}");
    let other = PresentationFields {
        colours: Some(crate::types::DefaultColourFields { screen: 5, ..Default::default() }),
        ..PresentationFields::default()
    };
    feed(&mut decoder, &[encode::magazine_enhancement(mag(5), 0, &other)], &mut events);
    assert!(events.contains(&Event::MagazineData(mag(5))), "a change is");
    decoder.start_new_input(&mut |_| {});
    assert_eq!(decoder.magazine_data(mag(5)), MagazineData::default());
}

/// REQ-ACQ-017: pages with hexadecimal numbers are stored at Levels 2.5
/// and 3.5, and not at Levels 1 and 1.5.
#[test]
fn hexadecimal_pages_follow_the_level() {
    use crate::config::PresentationLevel as L;
    for (level, stored) in [(L::Level1, false), (L::Level15, false), (L::Level25, true), (L::Level35, true)] {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config { level, ..Config::default() });
        let mut events = Vec::new();
        feed(&mut decoder, &[header(6, 0x3D, 0, 0), header(6, 0x00, 0, 0)], &mut events);
        assert_eq!(decoder.store().contains(PageAddress::from_mpp(0x63D).unwrap()), stored, "{level:?}");
    }
}

/// REQ-ACQ-011, decision E.11: the rolling header takes every magazine
/// in serial mode and only the requested one in parallel mode, never a
/// header with C9 set; the clock takes every valid header.
#[test]
fn rolling_header_and_clock_follow_decision_e11() {
    let with_text = |m: u8, control: u16, rolling: &[u8], clock: &[u8]| {
        let mut display = [with_odd_parity(b' '); HEADER_DISPLAY_BYTES];
        display[..rolling.len()].copy_from_slice(&text(rolling)[..rolling.len()]);
        display[ROLLING_BYTES..ROLLING_BYTES + clock.len()].copy_from_slice(&text(clock)[..clock.len()]);
        encode::header(mag(m), PageNumber(0x10), SubCode::new(0), ControlBits(control), &display)
    };
    let shown =
        |bytes: &[u8]| -> std::string::String { bytes.iter().map(|&b| char::from(b & CODE_MASK_7BIT)).collect() };
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    decoder.set_requested(Some(PageAddress::from_mpp(0x100).unwrap()));
    let mut events = Vec::new();
    feed(&mut decoder, &[with_text(2, 0, b"OTHER", b"10:00:01")], &mut events);
    assert_eq!(shown(&decoder.rolling_header()[..5]), "     ", "parallel: other magazine");
    assert_eq!(shown(decoder.clock()), "10:00:01");
    feed(&mut decoder, &[with_text(1, 0, b"WANTED", b"10:00:02")], &mut events);
    assert_eq!(shown(&decoder.rolling_header()[..6]), "WANTED");
    feed(&mut decoder, &[with_text(1, C9_INTERRUPTED, b"BROKEN", b"10:00:03")], &mut events);
    assert_eq!(shown(&decoder.rolling_header()[..6]), "WANTED", "C9 set");
    assert_eq!(shown(decoder.clock()), "10:00:03");
    feed(&mut decoder, &[with_text(2, C11_SERIAL, b"SERIAL", b"10:00:04")], &mut events);
    assert_eq!(shown(&decoder.rolling_header()[..6]), "SERIAL", "serial: every magazine");
    assert_eq!(shown(decoder.clock()), "10:00:04");
}

/// Decisions E.7 and E.9, REQ-ARCH-007: a packet whose address byte
/// fails Hamming 8/4 is discarded and counted as an address failure, one
/// whose designation code fails as a designation failure; packets that
/// carry nothing the decoder keeps (X/27/6 and above, packet 30 outside
/// magazine 8, packet 31) are counted per magazine and packet number and
/// as ignored (REQ-PKT-005, REQ-PKT-008). The caller resets the counts.
#[test]
fn failures_and_ignored_packets_are_counted() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    let mut bad_address = encode::row(mag(1), 1, &text(b"ROW"));
    // Two bit errors are detected and not corrected by Hamming 8/4.
    bad_address.bytes[crate::consts::ADDRESS_BYTE_0] ^= 0b11;
    let mut bad_designation = encode::triplet_packet(mag(2), 26, 0, &[Triplet::default(); TRIPLETS_PER_PACKET]);
    bad_designation.bytes[crate::consts::DATA_START] ^= 0b11;
    feed(&mut decoder, &[bad_address, bad_designation], &mut events);
    assert_eq!(
        events,
        [
            Event::Discarded(Discard::Packet(crate::packet::PacketError::BadAddress)),
            Event::Discarded(Discard::Packet(crate::packet::PacketError::BadDesignation)),
        ]
    );
    let statistics = *decoder.statistics();
    assert_eq!((statistics.address_failures, statistics.designation_failures), (1, 1));
    assert_eq!(statistics.per_magazine, [0; MAGAZINES], "a failed packet has no trustworthy address to count");
    events.clear();
    let ignored =
        [encode::raw(mag(3), 27, 6, &[0; DATA_BYTES - 1]), encode::address(mag(4), 30), encode::address(mag(8), 31)];
    feed(&mut decoder, &ignored, &mut events);
    assert!(events.is_empty(), "{events:?}");
    let statistics = *decoder.statistics();
    assert_eq!(statistics.ignored, 3);
    assert_eq!((statistics.per_magazine[mag(3).index()], statistics.per_magazine[mag(4).index()]), (1, 1));
    assert_eq!(statistics.per_magazine[0], 1, "index 0 is magazine 8");
    assert_eq!(
        (statistics.per_packet_number[27], statistics.per_packet_number[30], statistics.per_packet_number[31]),
        (1, 1, 1)
    );
    decoder.reset_statistics();
    assert_eq!(*decoder.statistics(), Statistics::default());
}

/// REQ-L25-051, REQ-ARCH-007: the faults that building a page met are
/// added to the statistics once for each build, kind by kind, however
/// often the same build is reported; a new reception is a new build.
#[test]
fn enhancement_faults_are_counted_once_per_build() {
    let mut slots = std::vec![StoredPage::EMPTY; 1];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let faults = EnhancementFaults {
        damaged_triplets: 1,
        forbidden_invocations: 2,
        absent_objects: 3,
        limited_invocations: 4,
        discarded_writes: 5,
        ignored_designations: 6,
        undefined_characters: 7,
    };
    let mut page = StoredPage::EMPTY;
    page.key = key(0x100, 0);
    page.receptions = 1;
    let build = PageBuild::of(&page, PresentationLevel::Level25);
    assert_eq!((build.key, build.receptions, build.level), (key(0x100, 0), 1, PresentationLevel::Level25));
    assert!(decoder.record_enhancement(build, &faults));
    assert!(!decoder.record_enhancement(build, &faults), "a redraw of the same build");
    assert_eq!(decoder.statistics().enhancement, faults);
    page.receptions = 2;
    assert!(decoder.record_enhancement(PageBuild::of(&page, PresentationLevel::Level25), &faults));
    let twice = EnhancementFaults {
        damaged_triplets: 2,
        forbidden_invocations: 4,
        absent_objects: 6,
        limited_invocations: 8,
        discarded_writes: 10,
        ignored_designations: 12,
        undefined_characters: 14,
    };
    assert_eq!(decoder.statistics().enhancement, twice);
    // The counts saturate rather than wrap.
    let mut total = EnhancementFaults { damaged_triplets: u32::MAX, ..EnhancementFaults::NONE };
    total.add(&faults);
    assert_eq!((total.damaged_triplets, total.undefined_characters), (u32::MAX, 7));
}

/// REQ-FSM-010: with tracing on, a known input gives the expected trace
/// of both machines, each entry naming the machine and instance, the
/// time, the state before, the event, the action and the state after.
/// A transition is a change of state or an action performed: a row
/// stored on a page being received keeps it Partial and raises "page
/// updated", and a row without a header keeps magazine 2 Idle and is
/// discarded; both are traced. With tracing off nothing is.
#[test]
fn a_known_input_gives_the_expected_trace() {
    use crate::trace::{Machine, TraceEntry};
    let packets = [
        header(1, 0x00, 0, 0),
        encode::row(mag(1), 1, &text(b"ROW")),
        header(1, 0x01, 0, 0),
        encode::row(mag(2), 1, &text(b"X")),
    ];
    let entry =
        |machine, time, from, event, action, to| TraceEntry { machine, time: Instant(time), from, event, action, to };
    let expected = [
        entry(Machine::Magazine(1), 0, "Idle", "storable header", "start page", "Receiving"),
        entry(Machine::Page(0x100), 0, "Empty", "reception started", "raise page started", "Partial"),
        entry(Machine::Magazine(1), 1000, "Receiving", "row", "store row", "Receiving"),
        entry(Machine::Page(0x100), 1000, "Partial", "row stored", "raise page updated", "Partial"),
        entry(Machine::Magazine(1), 2000, "Receiving", "storable header", "end page and start another", "Receiving"),
        entry(Machine::Page(0x100), 2000, "Partial", "reception ended", "raise page completed", "Complete"),
        entry(Machine::Page(0x101), 2000, "Empty", "reception started", "raise page started", "Partial"),
        entry(Machine::Magazine(2), 3000, "Idle", "row", "discard: row without header", "Idle"),
    ];
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config { trace: true, ..Config::default() });
    let mut events = Vec::new();
    feed(&mut decoder, &packets, &mut events);
    let trace: Vec<TraceEntry> =
        events.iter().filter_map(|event| if let Event::Trace(entry) = event { Some(*entry) } else { None }).collect();
    assert_eq!(trace, expected);
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(&mut decoder, &packets, &mut events);
    assert!(!events.iter().any(|event| matches!(event, Event::Trace(_))));
}

/// REQ-ACQ-012, REQ-FSM-004: the requested page is "searching" until a
/// header of it arrives, "found" while it is received and once complete,
/// "found but not in the most recent cycle" when it has not been
/// received for the configured time, and "found" again with its next
/// reception, which keeps the stored rows.
#[test]
fn search_status_follows_the_page_lifecycle() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let stale_after = decoder.config().stale_after_us;
    let mut events = Vec::new();
    let address = PageAddress::from_mpp(0x100).unwrap();
    assert_eq!(decoder.search_status(address), SearchStatus::Searching);
    feed(&mut decoder, &[header(1, 0x00, 0, 0), encode::row(mag(1), 1, &text(b"KEPT"))], &mut events);
    assert_eq!(decoder.store().latest(address).unwrap().state, PageState::Partial);
    assert_eq!(decoder.search_status(address), SearchStatus::Found);
    feed(&mut decoder, &[header(1, 0x01, 0, 0)], &mut events);
    assert_eq!(decoder.search_status(address), SearchStatus::Found);
    events.clear();
    decoder.tick(Instant(stale_after), &mut |event| events.push(event));
    assert_eq!(decoder.search_status(address), SearchStatus::Found, "not yet past the configured time");
    decoder.tick(Instant(stale_after + 1001), &mut |event| events.push(event));
    assert_eq!(decoder.search_status(address), SearchStatus::FoundNotRecent);
    assert!(events.contains(&Event::PageStale(key(0x100, 0))));
    let now = Instant(stale_after + 2000);
    decoder.push(&header(1, 0x00, 0, 0), &PacketMeta::default(), now, &mut |event| events.push(event));
    assert_eq!(decoder.search_status(address), SearchStatus::Found);
    assert_eq!(decoder.store().latest(address).unwrap().cells[1][..4], text(b"KEPT")[..4], "merged, not erased");
}

/// REQ-ACQ-010: a header with C8 (update indicator) set raises the
/// update indicator event for its page; one without C8 does not.
#[test]
fn c8_raises_the_update_indicator() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(&mut decoder, &[header(1, 0x00, 0, 0), header(1, 0x01, 0, crate::consts::C8_UPDATE)], &mut events);
    let raised: Vec<&Event> = events.iter().filter(|event| matches!(event, Event::UpdateIndicator(_))).collect();
    assert_eq!(raised, [&Event::UpdateIndicator(key(0x101, 0))]);
    assert!(
        decoder
            .store()
            .get(PageAddress::from_mpp(0x101).unwrap(), SubCode::ANY)
            .unwrap()
            .control
            .has(crate::consts::C8_UPDATE)
    );
}

/// REQ-ARCH-008: replacing the configuration keeps the stored pages.
#[test]
fn a_new_configuration_keeps_the_stored_pages() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[header(1, 0x00, 0, 0), encode::row(mag(1), 1, &text(b"A")), header(1, 0x01, 0, 0)],
        &mut events,
    );
    let saa5050 = Config::saa5050();
    decoder.set_config(saa5050);
    assert_eq!(*decoder.config(), saa5050);
    let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
    assert_eq!((page.state, page.cells[1][0]), (PageState::Complete, with_odd_parity(b'A')));
}

/// Clause 9.8.1, REQ-ACQ-018: the latest packet 8/30 is kept for the
/// caller and its arrival is an event.
#[test]
fn the_latest_service_packet_is_kept() {
    let mut slots = std::vec![StoredPage::EMPTY; 1];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    assert!(decoder.service().is_none());
    let mut events = Vec::new();
    let mut packet = encode::address(mag(8), 30);
    packet.bytes[crate::consts::DATA_START] = crate::ecc::hamming84_encode(0);
    // Table 18: the network identification in bytes 13 and 14, sent
    // most significant bit first.
    packet.bytes[9] = 0xFD_u8.reverse_bits();
    packet.bytes[10] = 0x01_u8.reverse_bits();
    feed(&mut decoder, &[packet], &mut events);
    assert_eq!(events, [Event::ServiceData]);
    assert_eq!(decoder.service().map(|service| (service.designation, service.network_id)), Some((0, 0xFD01)));
}

/// Clause 9.6.1, clause 9.4, REQ-ACQ-004, REQ-ACQ-016: X/27/1 to X/27/3
/// are kept with their page as received, X/28/4 as its Level 3.5 page
/// data, and C4 clears both. X/28/4's page function does not make a page
/// an enhancement page (REQ-ACQ-016 names X/28/0 and X/28/3), X/28/3's
/// does, and an X/28/3 whose triplet 1 is uncorrectable names no
/// function and leaves the page where it is.
#[test]
fn page_related_packets_are_kept_with_their_page() {
    use crate::consts::{PAGE_FUNCTION_GDRCS, PAGE_FUNCTION_POP};
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut memory = EnhancementMemory::new(8, 100);
    let mut decoder = Decoder::with_enhancement_store(&mut slots, memory.store(), Config::default());
    let mut events = Vec::new();
    let mut body = [0u8; DATA_BYTES - 1];
    for (index, byte) in body.iter_mut().enumerate() {
        *byte = index as u8;
    }
    let raw_links = encode::raw(mag(1), 27, 2, &body);
    let pop = PageFormat { function: PAGE_FUNCTION_POP, coding: 0 };
    let level35 = encode::page_enhancement(mag(1), 4, pop, &PresentationFields::default());
    feed(&mut decoder, &[header(1, 0x50, 0, 0), raw_links, level35, header(1, 0x51, 0, 0)], &mut events);
    let address = PageAddress::from_mpp(0x150).unwrap();
    let page = decoder.store().get(address, SubCode::ANY).unwrap();
    assert_eq!((page.links_raw[1], page.links_raw_present), (raw_links.data(), 0b010));
    assert_eq!((page.page_data, page.page_data_level35.map(|data| data.format)), (None, Some(Some(pop))));
    assert!(!decoder.enhancement_store().contains_address(address), "X/28/4 names no enhancement page");
    feed(&mut decoder, &[header(1, 0x50, 0, C4_ERASE), header(1, 0x51, 0, 0)], &mut events);
    let page = decoder.store().get(address, SubCode::ANY).unwrap();
    assert_eq!((page.links_raw_present, page.page_data_level35), (0, None), "REQ-ACQ-004");
    // X/28/3 with a page function: an enhancement page from here on.
    let drcs = PageFormat { function: PAGE_FUNCTION_GDRCS, coding: 0 };
    feed(
        &mut decoder,
        &[header(2, 0x60, 0, 0), encode::page_function(mag(2), drcs), header(2, 0x61, 0, 0)],
        &mut events,
    );
    let stored = decoder.enhancement_lookup().sub_page(mag(2), PageNumber(0x60), 0).unwrap();
    assert_eq!(stored.format(), Some(drcs));
    assert!(!decoder.store().contains(PageAddress::from_mpp(0x260).unwrap()), "REQ-OBJ-041: never a text page");
    // The same packet with two bit errors in triplet 1.
    let mut damaged = encode::page_function(mag(2), drcs);
    damaged.bytes[crate::consts::DATA_START + 1] ^= 0b11;
    feed(&mut decoder, &[header(2, 0x70, 0, 0), damaged, header(2, 0x71, 0, 0)], &mut events);
    assert!(decoder.store().contains(PageAddress::from_mpp(0x270).unwrap()));
    assert!(!decoder.enhancement_store().contains_address(PageAddress::from_mpp(0x270).unwrap()));
}

/// Decision E.20: a request for one sub-code gets that sub-page, or
/// nothing when it is not stored; a request for any gets the latest.
#[test]
fn a_sub_code_request_gets_that_sub_page() {
    let mut slots = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(&mut decoder, &[header(1, 0x00, 1, 0), header(1, 0x00, 2, 0), header(1, 0x01, 0, 0)], &mut events);
    let address = PageAddress::from_mpp(0x100).unwrap();
    assert_eq!(decoder.store().get(address, SubCode::new(1)).unwrap().key, key(0x100, 1));
    assert_eq!(decoder.store().get(address, SubCode::ANY).unwrap().key, key(0x100, 2), "received last");
    assert!(decoder.store().get(address, SubCode::new(3)).is_none());
}

/// REQ-GEN-006: a full listing buffer keeps the lowest sub-codes when a
/// lower one than those listed comes later in the store.
#[test]
fn subcode_listing_into_a_full_buffer_takes_a_later_lower_sub_code() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[header(1, 0x00, 2, 0), header(1, 0x00, 3, 0), header(1, 0x00, 1, 0), header(1, 0x01, 0, 0)],
        &mut events,
    );
    let mut subcodes = [SubCode::default(); 2];
    let count = decoder.store().subcodes(PageAddress::from_mpp(0x100).unwrap(), &mut subcodes);
    assert_eq!(&subcodes[..count], &[SubCode::new(1), SubCode::new(2)]);
}

/// REQ-ACQ-005: the store uses only the slots the caller gives. When
/// every slot holds a page being received, a new page is not stored and
/// the pages in progress keep their slots; with no slots nothing is
/// stored.
#[test]
fn pages_being_received_are_never_evicted() {
    let mut slots = std::vec![StoredPage::EMPTY; 1];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut events = Vec::new();
    feed(
        &mut decoder,
        &[
            header(1, 0x00, 0, 0),
            header(2, 0x00, 0, 0),
            encode::row(mag(2), 1, &text(b"TWO")),
            encode::row(mag(1), 1, &text(b"ONE")),
        ],
        &mut events,
    );
    assert_eq!(events.iter().filter(|event| matches!(event, Event::PageStarted(_))).count(), 1);
    assert!(!decoder.store().contains(PageAddress::from_mpp(0x200).unwrap()));
    let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
    assert_eq!((page.state, page.cells[1][0]), (PageState::Partial, with_odd_parity(b'O')));
    let mut decoder = Decoder::new(&mut [], Config::default());
    events.clear();
    feed(&mut decoder, &[header(1, 0x00, 0, 0), encode::row(mag(1), 1, &text(b"ONE"))], &mut events);
    assert_eq!(decoder.store().pages(), []);
    assert!(!events.iter().any(|event| matches!(event, Event::PageStarted(_) | Event::PageUpdated { .. })));
}
