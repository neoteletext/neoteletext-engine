//! Tests of object links, bodies and definitions (area OBJ).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::acq::StoredPage;
use crate::consts::{ROW_ADDRESS_BASE, ROW_ADDRESS_ROW_24};
use crate::packet::{ObjectUsage, Triplet, TripletFunction};

fn definition(kind: ObjectKind, usage: u8) -> Triplet {
    TripletFunction::ObjectDefinition { usage: ObjectUsage(usage), kind, number: ObjectNumber(0) }.to_triplet()
}

fn page_with(triplets: &[(usize, usize, Triplet)]) -> StoredPage {
    let mut page = StoredPage::EMPTY;
    for &(designation, index, triplet) in triplets {
        page.enhancements[designation][index] = Some(triplet);
        page.enhancements_present |= 1 << designation;
    }
    page
}

/// REQ-OBJ-003: N7 to N4 and N3 to N0 name the definition; N8 set,
/// triplets 13 to 15 and missing packets are skipped.
#[test]
fn local_objects_are_found_by_designation_and_triplet() {
    let page = page_with(&[(1, 5, definition(ObjectKind::Passive, 0b11))]);
    let number = ObjectNumber(0x15);
    let found = find_local(&page, ObjectKind::Passive, number, PresentationLevel::Level25).unwrap();
    assert_eq!(found.first, 13 + 5 + 1);
    assert!(found.body.is_local());
    assert_eq!(
        find_local(&page, ObjectKind::Active, number, PresentationLevel::Level25).err(),
        Some(Absent::NotDefinition)
    );
    assert_eq!(
        find_local(&page, ObjectKind::Passive, ObjectNumber(0x115), PresentationLevel::Level25).err(),
        Some(Absent::BadNumber)
    );
    assert_eq!(
        find_local(&page, ObjectKind::Passive, ObjectNumber(0x1D), PresentationLevel::Level25).err(),
        Some(Absent::BadNumber)
    );
    assert_eq!(
        find_local(&page, ObjectKind::Passive, ObjectNumber(0x25), PresentationLevel::Level25).err(),
        Some(Absent::NotReceived)
    );
}

/// REQ-OBJ-005: the usage bits must include the level.
#[test]
fn usage_bits_select_the_level() {
    for (usage, at_25, at_35) in [(0b01, true, false), (0b10, false, true), (0b11, true, true), (0b00, false, false)] {
        let page = page_with(&[(0, 0, definition(ObjectKind::Active, usage))]);
        let find = |level| find_local(&page, ObjectKind::Active, ObjectNumber(0), level).is_ok();
        assert_eq!((find(PresentationLevel::Level25), find(PresentationLevel::Level35)), (at_25, at_35), "{usage:02b}");
    }
}

/// REQ-OBJ-006: the body ends at the next definition or termination,
/// crosses packets, and is absent past a missing packet.
#[test]
fn prescan_finds_the_end_and_the_last_position() {
    let column = |column: u8| TripletFunction::G0Character { column, code: b'A' }.to_triplet();
    let row = |row: u8| TripletFunction::SetActivePosition { address: ROW_ADDRESS_BASE + row, column: 3 }.to_triplet();
    let end = TripletFunction::Termination { kind: 0 }.to_triplet();
    let mut entries = std::vec![(0, 0, definition(ObjectKind::Active, 0b11)), (0, 1, column(9)), (0, 2, row(4))];
    for index in 3..13 {
        entries.push((0, index, column(2)));
    }
    entries.push((1, 0, column(5)));
    entries.push((1, 1, end));
    let page = page_with(&entries);
    let object = find_local(&page, ObjectKind::Active, ObjectNumber(0), PresentationLevel::Level25).unwrap();
    let extent = prescan(&object, &page).unwrap();
    assert_eq!(extent, Extent { end: 14, last: Some((4, 5)) });
    // Without packet X/26/1 the body runs into a missing packet.
    let mut short = page;
    short.enhancements_present = 1;
    assert_eq!(prescan(&object, &short), Err(Absent::NotReceived));
}

/// Clause 12.3.2: row address 40 is row 24.
#[test]
fn row_addresses() {
    assert_eq!(row_of(ROW_ADDRESS_ROW_24), 24);
    assert_eq!(row_of(41), 1);
    assert_eq!(row_of(63), 23);
}

// ---------------------------------------------------------------------------
// A receiver for the tests of object pages and the Magazine Organization
// Table: packets built with the encoder, fed through the decoder, read
// back through the enhancement lookup as the presentation layer reads them.

use crate::acq::{Decoder, EnhancementLookup, EnhancementPacket, EnhancementStore, EnhancementSubPage};
use crate::config::Config;
use crate::consts::{
    DATA_BYTES, DATA_START, LINK_FUNCTION_GPOP, LINK_FUNCTION_POP, LINK_NULL_PAGE, OBJECT_POINTER_UNUSED,
    PACKET_ENHANCEMENT, PAGE_TIME_FILLING, ROW_ADDRESS_LAST, TRIPLET_BYTES, TRIPLETS_PER_PACKET,
};
use crate::ecc::with_odd_parity;
use crate::fsm::PageKey;
use crate::packet::{
    ColourData, CompositionalLink, DefaultObject, MotObjectLink, MotPageEntry, Packet, PacketMeta, encode, mot_position,
};
use crate::types::{ControlBits, Instant, PageAddress, SubCode};
use std::vec::Vec;

const L25: PresentationLevel = PresentationLevel::Level25;
const L35: PresentationLevel = PresentationLevel::Level35;

fn mag(number: u8) -> Magazine {
    Magazine::new(number).unwrap()
}

fn address(mpp: u16) -> PageAddress {
    PageAddress::from_mpp(mpp).unwrap()
}

fn page_link(mpp: u16) -> PageLink {
    let address = address(mpp);
    PageLink { magazine: address.magazine, page: address.page }
}

fn header(mpp: u16, s1: u8) -> Packet {
    let address = address(mpp);
    let display = [with_odd_parity(b' '); 32];
    encode::header(address.magazine, address.page, SubCode::new(u16::from(s1)), ControlBits(0), &display).unwrap()
}

/// Feeds the packets, ends every magazine's page with a time-filling
/// header, and calls `f` with the lookup over what was stored.
fn received<R>(packets: &[Packet], f: impl FnOnce(&EnhancementLookup<'_>) -> R) -> R {
    let mut pages = std::vec![StoredPage::EMPTY; 8];
    let mut sub_pages = std::vec![EnhancementSubPage::EMPTY; 16];
    let mut slots = std::vec![EnhancementPacket::EMPTY; 256];
    let store = EnhancementStore::new(&mut sub_pages, &mut slots);
    let mut decoder = Decoder::with_enhancement_store(&mut pages, store, Config::default());
    let mut all = packets.to_vec();
    all.extend((1..=8u16).map(|magazine| header(magazine << 8 | u16::from(PAGE_TIME_FILLING), 0)));
    for (index, packet) in all.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    f(&decoder.enhancement_lookup())
}

/// The displayed page 100 of the tests, with its X/27/4 and X/27/5 links.
fn display(x27_4: Option<[Option<CompositionalLink>; 6]>, x27_5: Option<[Option<CompositionalLink>; 6]>) -> StoredPage {
    let mut page = StoredPage::EMPTY;
    page.key = PageKey { address: address(0x100), subcode: SubCode::ANY };
    page.compositional_links = [x27_4, x27_5];
    page
}

/// A Format 1 link of X/27/4 or X/27/5 (Table 15) to every sub-page.
fn link(function: u8, validity: u8, mpp: u16) -> CompositionalLink {
    let address = address(mpp);
    CompositionalLink {
        function,
        validity,
        magazine: address.magazine,
        page: address.page,
        subpages: 0xFFFF,
        marker_first: true,
        marker_second: false,
    }
}

/// An object link of the Magazine Organization Table (Table 21).
fn object_link(mpp: u16, defaults: [Option<DefaultObject>; 2]) -> MotObjectLink {
    let address = address(mpp);
    MotObjectLink {
        magazine: address.magazine,
        x27_flag: false,
        page: address.page,
        subpage_count: 0,
        fallback: FallbackFields::default(),
        default_objects: defaults,
    }
}

/// The packets of the Magazine Organization Table of magazine 1: the
/// byte pair of page 100, and the object links of each packet given
/// (Table 20: four per packet; unused ones link to page FF).
fn table(entry: MotPageEntry, packets: &[(u8, [Option<MotObjectLink>; 4])]) -> Vec<Packet> {
    let mut out = std::vec![header(0x1FE, 0)];
    let position = mot_position(PageNumber(0x00));
    let mut pairs = [0u8; DATA_BYTES];
    pairs[position.index..position.index + 2].copy_from_slice(&encode::mot_page_entry(entry));
    out.push(encode::nibble_packet(mag(1), position.packet, &pairs));
    for (number, links) in packets {
        let mut values = [0u8; DATA_BYTES];
        for (index, link) in links.iter().enumerate() {
            let unused = MotObjectLink { page: PageNumber(LINK_NULL_PAGE), ..object_link(0x1FF, [None; 2]) };
            values[index * 10..index * 10 + 10].copy_from_slice(&encode::mot_object_link(&link.unwrap_or(unused)));
        }
        out.push(encode::nibble_packet(mag(1), *number, &values));
    }
    out
}

const fn pop(link: u8) -> MotPageEntry {
    MotPageEntry { global_objects: false, pop_link: link }
}

/// Table 15 and 16, REQ-OBJ-030: with no Magazine Organization Table and no
/// links of its own, a page uses no enhancement pages and keeps its level;
/// a source names the POP or GPOP link, and local and illegal sources none.
#[test]
fn a_page_without_links_uses_no_enhancement_pages() {
    for level in [L25, L35] {
        let found = received(&[], |lookup| links(&display(None, None), lookup, level));
        assert_eq!(found, EnhancementLinks::none(level));
    }
    let links =
        EnhancementLinks { gpop: Some(page_link(0x1B0)), pop: Some(page_link(0x1A0)), ..EnhancementLinks::none(L25) };
    assert_eq!(links.object_page(ObjectSource::Public), Some(page_link(0x1A0)));
    assert_eq!(links.object_page(ObjectSource::Global), Some(page_link(0x1B0)));
    assert_eq!(links.object_page(ObjectSource::Local), None);
    assert_eq!(links.object_page(ObjectSource::Illegal), None);
}

/// REQ-OBJ-030, Table 16, decision E.52: X/27/4 links 0 and 1 are the GPOP
/// and POP by position, used only with Format 1 markers, a page validity
/// including the level and a link that is not null; at Level 3.5 links 4
/// and 5 of X/27/4 and 0 and 1 of X/27/5 take their role from their
/// function bits, and the first of two links for a role wins.
#[test]
fn compositional_links_by_position_function_and_level() {
    let gpop_25 = link(LINK_FUNCTION_GPOP, 0b01, 0x1B0);
    let pop_both = link(LINK_FUNCTION_POP, 0b11, 0x1A0);
    let gpop_35 = link(LINK_FUNCTION_GPOP, 0b10, 0x2B0);
    let x27_4 = [Some(gpop_25), Some(pop_both), None, None, Some(gpop_35), None];
    let check = |page: &StoredPage, level, gpop: Option<u16>, pop: Option<u16>| {
        let found = received(&[], |lookup| links(page, lookup, level));
        assert_eq!((found.gpop, found.pop), (gpop.map(page_link), pop.map(page_link)), "{level:?}");
        assert_eq!((found.fallback, found.default_objects, found.level), (None, [None; DEFAULT_OBJECTS], level));
    };
    // Level 2.5: links 0 and 1; link 4 has no role at this level.
    check(&display(Some(x27_4), None), L25, Some(0x1B0), Some(0x1A0));
    // Level 3.5: link 0 is valid at Level 2.5 only, so link 4 gives the GPOP.
    check(&display(Some(x27_4), None), L35, Some(0x2B0), Some(0x1A0));
    // A link whose function bits do not match its position is ignored.
    let misplaced = [None, Some(link(LINK_FUNCTION_GPOP, 0b11, 0x1C0)), None, None, None, None];
    check(&display(Some(misplaced), None), L25, None, None);
    // Markers other than Format 1, and null links, are not used.
    let format_2 = CompositionalLink { marker_first: false, ..pop_both };
    let no_subpages = CompositionalLink { subpages: 0, ..gpop_25 };
    let unused = [Some(no_subpages), Some(format_2), None, None, None, None];
    check(&display(Some(unused), None), L25, None, None);
    // X/27/5 at Level 3.5: link 0 duplicates the POP role and is ignored,
    // link 1 gives the GPOP, and link 2 is reserved.
    let x27_5 = [
        Some(link(LINK_FUNCTION_POP, 0b11, 0x3A0)),
        Some(link(LINK_FUNCTION_GPOP, 0b11, 0x3B0)),
        Some(link(LINK_FUNCTION_POP, 0b11, 0x3C0)),
        None,
        None,
        None,
    ];
    let only_pop = [None, Some(pop_both), None, None, None, None];
    check(&display(Some(only_pop), Some(x27_5)), L35, Some(0x3B0), Some(0x1A0));
    // X/27/5 is not read at Level 2.5.
    check(&display(Some(only_pop), Some(x27_5)), L25, None, Some(0x1A0));
    check(&display(None, Some(x27_5)), L35, Some(0x3B0), Some(0x3A0));
}

/// REQ-OBJ-031, REQ-OBJ-040, decision E.53: at Level 2.5 the table's X/19
/// gives the GPOP and the POP link the page's byte pair names; the fallback
/// bits come from that POP link, and the default objects are GPOP 1, GPOP
/// 2, POP 1, POP 2, each with N8 = 0. The global object flag is only a
/// hint: the GPOP is resolved through the magazine's link whatever it says.
/// The page's own X/27/4 takes precedence role by role.
#[test]
fn magazine_organization_table_at_level_25() {
    let default = |kind, number| Some(DefaultObject { kind, number });
    let gpop = object_link(0x1B0, [default(ObjectKind::Active, 0x15), default(ObjectKind::Passive, 0x92)]);
    let fallback = FallbackFields {
        side_panels: crate::types::SidePanelFields { left: true, ..Default::default() },
        black_background_substitution: true,
    };
    let pop_2 = MotObjectLink { fallback, ..object_link(0x1A0, [default(ObjectKind::Adaptive, 0x21), None]) };
    let packets = table(pop(2), &[(19, [Some(gpop), Some(object_link(0x1C0, [None; 2])), Some(pop_2), None])]);
    let found = received(&packets, |lookup| links(&display(None, None), lookup, L25));
    assert_eq!((found.gpop, found.pop, found.level), (Some(page_link(0x1B0)), Some(page_link(0x1A0)), L25));
    let fallback = found.fallback.unwrap();
    assert!(fallback.side_panels.left && !fallback.side_panels.right && fallback.black_background_substitution);
    let invocation = |source, kind, number, mpp| {
        Some(DefaultInvocation { source, kind, number: ObjectNumber(number), page: page_link(mpp) })
    };
    assert_eq!(
        found.default_objects,
        [
            invocation(ObjectSource::Global, ObjectKind::Active, 0x15, 0x1B0),
            invocation(ObjectSource::Global, ObjectKind::Passive, 0x92, 0x1B0),
            invocation(ObjectSource::Public, ObjectKind::Adaptive, 0x21, 0x1A0),
            None,
        ]
    );
    // The page's own GPOP link replaces the table's; the POP, the fallback
    // and the default objects still come from the table.
    let own = [Some(link(LINK_FUNCTION_GPOP, 0b11, 0x2B0)), None, None, None, None, None];
    let with_own = received(&packets, |lookup| links(&display(Some(own), None), lookup, L25));
    assert_eq!((with_own.gpop, with_own.pop), (Some(page_link(0x2B0)), Some(page_link(0x1A0))));
    assert_eq!((with_own.fallback, with_own.default_objects), (found.fallback, found.default_objects));
}

/// REQ-OBJ-031, clause 10.6.1, decision E.52: a Level 3.5 decoder reads
/// X/22 to X/24; with X/22 or X/24 missing it reads X/19 and X/20 and
/// processes the page at Level 2.5, and with only X/23 missing it takes POP
/// links 4 to 7 from X/20.
#[test]
fn magazine_organization_table_at_level_35() {
    let to = |mpp| Some(object_link(mpp, [None; 2]));
    // The GPOP link is the first of the first packet, and POP link 5 the
    // second of the second (Table 20); X/21 and X/24 carry no object links.
    let all = [
        (19, [to(0x1B0), None, None, None]),
        (20, [None, to(0x150), None, None]),
        (21, [None; 4]),
        (22, [to(0x2B0), None, None, None]),
        (23, [None, to(0x250), None, None]),
        (24, [None; 4]),
    ];
    let without = |omit: u8| all.iter().copied().filter(|(number, _)| *number != omit).collect::<Vec<_>>();
    let check = |packets: &[(u8, [Option<MotObjectLink>; 4])], level, expected: (u16, u16, PresentationLevel)| {
        let found = received(&table(pop(5), packets), |lookup| links(&display(None, None), lookup, level));
        assert_eq!(
            (found.gpop, found.pop, found.level),
            (Some(page_link(expected.0)), Some(page_link(expected.1)), expected.2),
            "{level:?} with {:?}",
            packets.iter().map(|(number, _)| number).collect::<Vec<_>>()
        );
    };
    check(&all, L35, (0x2B0, 0x250, L35));
    check(&all, L25, (0x1B0, 0x150, L25));
    check(&without(23), L35, (0x2B0, 0x150, L35));
    check(&without(22), L35, (0x1B0, 0x150, L25));
    check(&without(24), L35, (0x1B0, 0x150, L25));
}

/// REQ-OBJ-031, Tables 21 and 23: a link to page FF is not used, and gives
/// neither a role, nor default objects, nor fallback bits; a POP link
/// number of 0 means no POP; the byte pair of page xFF is ignored, and a
/// page whose byte pair was not received gets nothing from the table.
#[test]
fn table_links_that_are_not_used() {
    let default = Some(DefaultObject { kind: ObjectKind::Active, number: 1 });
    let unused = MotObjectLink { page: PageNumber(LINK_NULL_PAGE), ..object_link(0x1A0, [default, default]) };
    let gpop = object_link(0x1B0, [default, None]);
    // POP link 1 is to page FF.
    let packets = table(pop(1), &[(19, [Some(gpop), Some(unused), None, None]), (20, [None; 4])]);
    let found = received(&packets, |lookup| links(&display(None, None), lookup, L25));
    assert_eq!((found.gpop, found.pop, found.fallback), (Some(page_link(0x1B0)), None, None));
    let global = DefaultInvocation {
        source: ObjectSource::Global,
        kind: ObjectKind::Active,
        number: ObjectNumber(1),
        page: page_link(0x1B0),
    };
    assert_eq!(found.default_objects, [Some(global), None, None, None]);
    // A GPOP link to page FF, and no POP link.
    let packets = table(pop(0), &[(19, [Some(unused), Some(gpop), None, None]), (20, [None; 4])]);
    let found = received(&packets, |lookup| links(&display(None, None), lookup, L25));
    assert_eq!(found, EnhancementLinks::none(L25));
    // Page 1FF's byte pair is ignored, and so is the table for it.
    let packets = table(pop(1), &[(19, [Some(gpop), Some(gpop), None, None]), (20, [None; 4])]);
    let mut page_ff = display(None, None);
    page_ff.key.address = address(0x1FF);
    assert_eq!(received(&packets, |lookup| links(&page_ff, lookup, L25)), EnhancementLinks::none(L25));
    // Page 1A5's byte pair is in another packet, which was not sent.
    let mut elsewhere = display(None, None);
    elsewhere.key.address = address(0x1A5);
    assert_ne!(mot_position(PageNumber(0xA5)).packet, mot_position(PageNumber(0x00)).packet);
    assert_eq!(received(&packets, |lookup| links(&elsewhere, lookup, L25)), EnhancementLinks::none(L25));
}

/// A public or global object's number (Table 28): N8 N7 the pointer
/// packet, N6 N5 the pointer group, N4 the half, N3 to N0 the sub-page.
const fn public_number(s1: u8, pointer_packet: u8, group: u8, odd: bool) -> ObjectNumber {
    ObjectNumber((pointer_packet as u16) << 7 | (group as u16) << 5 | (odd as u16) << 4 | s1 as u16)
}

/// Packet `number` of object page `mpp`, magazine 1: byte 6 and thirteen
/// triplets, the ones not given Termination Markers.
fn object_packet(number: u8, byte_6: u8, triplets: &[Triplet]) -> Packet {
    let mut all = [TripletFunction::Termination { kind: 0b011 }.to_triplet(); TRIPLETS_PER_PACKET];
    all[..triplets.len()].copy_from_slice(triplets);
    encode::triplet_packet(mag(1), number, byte_6, &all)
}

/// A pointer packet (byte 6 bit B1 set, clause 10.5.1.2) whose triplets
/// 1 to 12 carry the given pairs of pointers; the rest are not used.
fn pointer_packet(number: u8, pointers: &[(usize, u16, u16)]) -> Packet {
    let mut all = [encode::pointer_triplet(OBJECT_POINTER_UNUSED, OBJECT_POINTER_UNUSED); TRIPLETS_PER_PACKET];
    for &(triplet, even, odd) in pointers {
        all[triplet] = encode::pointer_triplet(even, odd);
    }
    encode::triplet_packet(mag(1), number, 1, &all)
}

/// Makes triplet `index` of a packet uncorrectable: two bits of each of
/// its bytes flipped.
fn damage(mut packet: Packet, index: usize) -> Packet {
    let start = DATA_START + 1 + TRIPLET_BYTES * index;
    for byte in &mut packet.bytes[start..start + TRIPLET_BYTES] {
        *byte ^= 0b0000_0110;
    }
    packet
}

fn g0(column: u8) -> Triplet {
    TripletFunction::G0Character { column, code: b'A' }.to_triplet()
}

/// The POP 1A0 of page 100, sub-page S1 = 3, named by the Magazine
/// Organization Table so that the decoder keeps it as an enhancement page.
/// Its pointers, packet by packet and triplet by triplet (1 + 3 × group +
/// type), lead to definitions, to other triplets and to nothing.
fn object_page(with_x26: bool) -> Vec<Packet> {
    let pointers = pointer_packet(
        1,
        &[
            // Group 0: Active at 0 and, odd, at 26; Adaptive at 2 and at
            // 297; Passive at 300 (X/26/0) and at 13 (a pointer packet).
            (1, 0, 26),
            (2, 2, 297),
            (3, 300, 13),
            // Group 1 Active: not used, and 508, beyond the 507 triplets.
            (4, OBJECT_POINTER_UNUSED, 508),
            // Group 2 Active: 39, in packet 6, which is not sent; Adaptive
            // at 3, an uncorrectable triplet.
            (7, 39, OBJECT_POINTER_UNUSED),
            (8, 3, OBJECT_POINTER_UNUSED),
            // Group 3 Active: 1, a character.
            (10, 1, OBJECT_POINTER_UNUSED),
        ],
    );
    let mut packets = table(pop(1), &[(19, [None, Some(object_link(0x1A0, [None; 2])), None, None]), (20, [None; 4])]);
    packets.push(header(0x1A0, 3));
    packets.push(pointers);
    packets.push(damage(
        object_packet(
            3,
            0,
            &[definition(ObjectKind::Active, 0b11), g0(4), definition(ObjectKind::Adaptive, 0b10), g0(5)],
        ),
        3,
    ));
    // Packet 4 carries pointers, all unused.
    packets.push(pointer_packet(4, &[]));
    packets.push(object_packet(5, 0, &[definition(ObjectKind::Passive, 0b11)]));
    // The Adaptive object at 297, triplet 11 of packet 25, crosses into
    // X/26/0, whose triplet 1 is the Passive definition that ends it.
    let mut row_25 = [g0(0); TRIPLETS_PER_PACKET];
    row_25[11] = definition(ObjectKind::Adaptive, 0b11);
    row_25[12] = g0(5);
    packets.push(object_packet(25, 0, &row_25));
    if with_x26 {
        packets.push(object_packet(PACKET_ENHANCEMENT, 0, &[g0(7), definition(ObjectKind::Passive, 0b11)]));
    }
    packets
}

/// REQ-OBJ-004, REQ-OBJ-005, REQ-OBJ-032, decision E.51: a public object is
/// found on the sub-page S1 = N3 to N0, through pointer packet 1 + N8 N7,
/// pointer triplet 1 + 3 × N6 N5 + type and the half N4 selects; it is
/// absent when the link, the sub-page or the pointer packet is missing,
/// when the packet carries no pointers, when the pointer is unused, beyond
/// 506 or into a pointer packet, and when the triplet found is not a
/// definition of the type at the level.
#[test]
fn public_objects_are_found_through_the_pointer_tables() {
    let page = display(None, None);
    let link = Some(page_link(0x1A0));
    received(&object_page(true), |lookup| {
        let find = |kind, number, level| {
            find_public(lookup, link, &page, kind, number, level).map(|found| (found.first, found.body.is_local()))
        };
        let number = |pointer_packet, group, odd| public_number(3, pointer_packet, group, odd);
        assert_eq!(find(ObjectKind::Active, number(0, 0, false), L25), Ok((1, false)));
        assert_eq!(find(ObjectKind::Passive, number(0, 0, false), L35), Ok((301, false)), "in X/26/0");
        assert_eq!(find(ObjectKind::Adaptive, number(0, 0, false), L35), Ok((3, false)));
        assert_eq!(find(ObjectKind::Adaptive, number(0, 0, false), L25), Err(Absent::WrongLevel));
        assert_eq!(
            find(ObjectKind::Active, number(0, 0, true), L25),
            Err(Absent::NotDefinition),
            "a Passive definition"
        );
        assert_eq!(find(ObjectKind::Passive, number(0, 0, true), L25), Err(Absent::NoPointer), "into packet 4");
        assert_eq!(find(ObjectKind::Active, number(0, 1, false), L25), Err(Absent::NoPointer), "unused");
        assert_eq!(find(ObjectKind::Active, number(0, 1, true), L25), Err(Absent::NoPointer), "508");
        assert_eq!(find(ObjectKind::Active, number(0, 2, false), L25), Err(Absent::NotReceived), "packet 6");
        assert_eq!(find(ObjectKind::Adaptive, number(0, 2, false), L25), Err(Absent::NotDefinition), "uncorrectable");
        assert_eq!(find(ObjectKind::Active, number(0, 3, false), L25), Err(Absent::NotDefinition), "a character");
        assert_eq!(find(ObjectKind::Active, number(1, 0, false), L25), Err(Absent::NotReceived), "no packet 2");
        assert_eq!(
            find(ObjectKind::Active, number(2, 0, false), L25),
            Err(Absent::NoPointer),
            "packet 3 has no pointers"
        );
        let other_subpage = public_number(2, 0, 0, false);
        assert_eq!(find(ObjectKind::Active, other_subpage, L25), Err(Absent::NotReceived));
        let none = find_public(lookup, None, &page, ObjectKind::Active, number(0, 0, false), L25);
        assert_eq!(none.map(|found| found.first), Err(Absent::NoLink));
    });
}

/// REQ-OBJ-006, clause 10.5.1.2: an object page's body runs from packet 25
/// into X/26 with designation code 0, ends at the next definition, and is
/// absent when a packet before its end was not received. A body can
/// address 507 triplets on an object page and 208 in a page's own X/26.
#[test]
fn object_page_bodies_cross_into_x26_and_stop_at_missing_packets() {
    let page = display(None, None);
    let number = public_number(3, 0, 0, true);
    let scan = |with_x26| {
        received(&object_page(with_x26), |lookup| {
            let object = find_public(lookup, Some(page_link(0x1A0)), &page, ObjectKind::Adaptive, number, L25).unwrap();
            assert_eq!(object.body.capacity(), 507);
            // Absolute triplets are numbered 0 to 506: there is no 507th.
            assert_eq!(object.body.fetch(&page, 507), Fetch::Missing);
            prescan(&object, &page)
        })
    };
    assert_eq!(scan(true), Ok(Extent { end: 300, last: Some((0, 7)) }));
    assert_eq!(scan(false), Err(Absent::NotReceived));
    assert_eq!(Body::Local.capacity(), 208);
}

/// REQ-OBJ-006, decision E.51, Annex A.8: the prescan follows the active
/// position as the walk does. Uncorrectable triplets and triplets that set
/// no column are skipped; in Adaptive and Passive objects Full Row Colour
/// and Address Display Row 0 are ignored entirely; a Set Active Position
/// column above 39 leaves the column.
#[test]
fn prescan_follows_the_active_position_by_object_type() {
    let colour = ColourData { index: 1, high: 0 };
    let body = [
        TripletFunction::FullRowColour { address: ROW_ADDRESS_BASE + 20, colour }.to_triplet(),
        g0(9),
        TripletFunction::AddressDisplayRow0 { address: ROW_ADDRESS_LAST, colour }.to_triplet(),
        TripletFunction::ProgrammeDeliveryColumn { column: 30, data: 0x41 }.to_triplet(),
        TripletFunction::SetActivePosition { address: ROW_ADDRESS_BASE + 3, column: 45 }.to_triplet(),
        g0(2),
        TripletFunction::Termination { kind: 0b010 }.to_triplet(),
    ];
    for (kind, last) in [(ObjectKind::Active, (20, 9)), (ObjectKind::Adaptive, (3, 9)), (ObjectKind::Passive, (3, 9))] {
        let mut entries = std::vec![(0, 0, definition(kind, 0b11))];
        entries.extend(body.iter().enumerate().map(|(index, &triplet)| (0, index + 1, triplet)));
        let mut page = page_with(&entries);
        // An uncorrectable triplet after the last character, before the
        // Termination Marker: it moves nothing and does not end the body.
        page.enhancements[0][8] = page.enhancements[0][7];
        page.enhancements[0][7] = None;
        let object = find_local(&page, kind, ObjectNumber(0), L25).unwrap();
        assert_eq!(object.body.fetch(&page, 7), Fetch::Damaged);
        assert_eq!(prescan(&object, &page), Ok(Extent { end: 8, last: Some(last) }), "{kind:?}");
    }
    // A local object defined in the last triplet of X/26/15 has no body:
    // the data ends there and it addresses no cell.
    let page = page_with(&[(15, 12, definition(ObjectKind::Active, 0b11))]);
    let object = find_local(&page, ObjectKind::Active, ObjectNumber(0xFC), L25).unwrap();
    assert_eq!(prescan(&object, &page), Ok(Extent { end: 208, last: None }));
    assert_eq!(Body::Local.fetch(&page, 208), Fetch::Missing, "beyond the 208 triplets of X/26");
}
