//! Page and sub-page numbering through the decoder, the store and the
//! viewer (area NUM): the null address and page FF, the reserved page
//! numbers, sub-pages keyed by all thirteen bits, the Magazine Inventory
//! Page, the page function, TOP pages, links that name no page, and a page
//! of 8,190 sub-pages.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use neoteletext_core::acq::{Decoder, EnhancementPacket, EnhancementStore, EnhancementSubPage, StoredPage};
use neoteletext_core::config::Config;
use neoteletext_core::consts::{
    C11_SERIAL, DATA_BYTES, MIP_COUNT_FIRST_PACKET, MIP_NOT_FOLLOWING_RULES, MIP_TOP, PACKET_LINKS, PACKET_SERVICE,
    PAGE_FUNCTION_BASIC, PAGE_FUNCTION_BTT, PAGE_FUNCTION_DATA_BROADCASTING, PAGE_FUNCTION_POP,
};
use neoteletext_core::ecc::{hamming84_encode, with_odd_parity};
use neoteletext_core::limits::{
    DECIMAL_SUBCODES, FIRST_SUBPAGE_CODE, LAST_SUBPAGE_CODE, MAX_SUBPAGES, PAGE_MIP, PAGE_TIME_FILLING, SUBCODE_ANY,
    data_sub_code, enhancement_sub_code,
};
use neoteletext_core::nav::{PageLinks, SubPageEntry, SubPageEntryResult, step_stored};
use neoteletext_core::packet::{
    InventoryFunction, Packet, PacketMeta, PageCode, PageFormat, decode, encode, mip_position,
};
use neoteletext_core::service::{initial_page, interpret};
use neoteletext_core::types::{ControlBits, Instant, Magazine, PageAddress, PageNumber, PresentationFields, SubCode};
use neoteletext_core::viewer::{Action, Key, KeyContext, Viewer, ViewerEvent};

fn address(mpp: u16) -> PageAddress {
    PageAddress::from_mpp(mpp).unwrap()
}

fn header_with(mpp: u16, subcode: u16, control: u16) -> Packet {
    let address = address(mpp);
    let display = [with_odd_parity(b' '); 32];
    encode::header(address.magazine, address.page, SubCode::new(subcode), ControlBits(control), &display).unwrap()
}

fn header(mpp: u16, subcode: u16) -> Packet {
    header_with(mpp, subcode, 0)
}

/// A header the encoder refuses to write: the null address XFF:3F7F, made
/// from XFF:3F7E with S1 set to F (packet byte 8).
fn null_header(magazine: u16) -> Packet {
    let mut packet = header((magazine << 8) | u16::from(PAGE_TIME_FILLING), SUBCODE_ANY - 1);
    packet.bytes[4] = hamming84_encode(0xF);
    packet
}

fn text_row(mpp: u16, row: u8) -> Packet {
    encode::row(address(mpp).magazine, row, &[with_odd_parity(b'A'); DATA_BYTES])
}

/// Packet X/28/0 Format 1 with a page function.
fn page_function(mpp: u16, function: u8) -> Packet {
    let format = PageFormat { function, coding: 0 };
    encode::page_enhancement(address(mpp).magazine, 0, format, &PresentationFields::default())
}

/// A time-filling header in every magazine, which ends every page.
fn filling() -> Vec<Packet> {
    (1..=8u16).map(|magazine| header((magazine << 8) | u16::from(PAGE_TIME_FILLING), 0)).collect()
}

fn feed(decoder: &mut Decoder<'_>, packets: &[Packet]) {
    for (index, packet) in (0u64..).zip(packets) {
        decoder.push(packet, &PacketMeta::default(), Instant(index * 1000), &mut |_| {});
    }
}

/// The page codes of a Magazine Inventory Page, one for each page number,
/// and the 13-bit counts of its groups, as its packets X/1 to X/15.
fn mip_packets(magazine: u16, codes: &[(u8, u8)], counts: &[u16]) -> Vec<Packet> {
    let mut rows = [[0u8; DATA_BYTES]; 15];
    for &(page, code) in codes {
        let position = mip_position(PageNumber(page));
        let row = &mut rows[usize::from(position.packet) - 1];
        row[position.index] = code & 0xF;
        row[position.index + 1] = code >> 4;
    }
    for (group, &count) in counts.iter().enumerate() {
        let row = &mut rows[usize::from(MIP_COUNT_FIRST_PACKET) - 1];
        for nibble in 0..3 {
            row[1 + group * 3 + nibble] = ((count >> (4 * nibble)) & 0xF) as u8;
        }
    }
    let mip = (magazine << 8) | u16::from(PAGE_MIP);
    let mut packets = vec![header(mip, enhancement_sub_code(15, 0, 0))];
    let magazine = address(mip).magazine;
    packets.extend(rows.iter().zip(1u8..).map(|(row, number)| encode::nibble_packet(magazine, number, row)));
    packets
}

/// Runs `test` with a decoder of `slots` page slots and a store for
/// enhancement pages.
fn with_decoder(slots: usize, test: impl FnOnce(&mut Decoder<'_>)) {
    let mut pages = vec![StoredPage::EMPTY; slots];
    let mut sub_pages = vec![EnhancementSubPage::EMPTY; 64];
    let mut packets = vec![EnhancementPacket::EMPTY; 1200];
    let store = EnhancementStore::new(&mut sub_pages, &mut packets);
    let mut decoder = Decoder::with_enhancement_store(&mut pages, store, Config::default());
    test(&mut decoder);
}

fn listed(decoder: &Decoder<'_>, mpp: u16) -> Vec<u16> {
    decoder.store().subcodes(address(mpp)).map(SubCode::value).collect()
}

/// REQ-NUM-007: the encoder refuses to write a header at the null address,
/// and writes every other address, page FF and sub-code 3F7F included.
#[test]
fn the_encoder_refuses_the_null_address() {
    let magazine = Magazine::new(1).unwrap();
    let display = [with_odd_parity(b' '); 32];
    let at = |page: u8, subcode: u16| {
        encode::header(magazine, PageNumber(page), SubCode::new(subcode), ControlBits(0), &display)
    };
    assert!(at(0xFF, SUBCODE_ANY).is_none());
    assert!(at(0xFF, LAST_SUBPAGE_CODE).is_some());
    assert!(at(0xFE, SUBCODE_ANY).is_some());
    let header = decode(&at(0x23, SUBCODE_ANY).unwrap()).unwrap();
    assert!(
        matches!(header, neoteletext_core::packet::Decoded::Header(header) if header.subcode == Some(SubCode::ANY))
    );
}

/// REQ-NUM-007, REQ-NUM-008: a header at the null address, or on page FF
/// with any sub-code, ends the page being received, in parallel and in
/// serial mode, and starts none: nothing is stored on page FF.
#[test]
fn page_ff_and_the_null_address_end_a_page_and_store_nothing() {
    for control in [0, C11_SERIAL] {
        for ender in
            [null_header(1), header_with(0x1FF, 0x0001, control), header_with(0x1FF, LAST_SUBPAGE_CODE, control)]
        {
            with_decoder(8, |decoder| {
                feed(decoder, &[header_with(0x100, 0, control), text_row(0x100, 1), ender, text_row(0x100, 2)]);
                let stored = decoder.store().get(address(0x100), SubCode::ANY).unwrap();
                assert!(stored.has_row(1) && !stored.has_row(2), "the row after page FF's header is not the page's");
                assert!(!decoder.store().contains(address(0x1FF)));
            });
        }
    }
}

/// REQ-NUM-006: sub-pages differing only in S4 or S3 are different
/// sub-pages, stored and listed apart.
#[test]
fn sub_pages_are_keyed_by_all_thirteen_bits() {
    with_decoder(8, |decoder| {
        let mut packets: Vec<Packet> = [0x0001, 0x0101, 0x1001, 0x2001].map(|code| header(0x100, code)).into();
        packets.extend(filling());
        feed(decoder, &packets);
        assert_eq!(listed(decoder, 0x100), [0x0001, 0x0101, 0x1001, 0x2001]);
        for code in [0x0001, 0x0101, 0x1001, 0x2001] {
            assert_eq!(decoder.store().get(address(0x100), SubCode::new(code)).unwrap().key.subcode.value(), code);
        }
    });
}

/// REQ-NUM-010, REQ-NUM-028: a reserved page number is never stored as a
/// page, even with the diagnostic setting that stores hexadecimal pages;
/// the MIP and the MOT go to the enhancement store as the tables they are.
#[test]
fn reserved_page_numbers_are_never_display_pages() {
    let mut pages = vec![StoredPage::EMPTY; 16];
    let mut sub_pages = vec![EnhancementSubPage::EMPTY; 16];
    let mut packets = vec![EnhancementPacket::EMPTY; 64];
    let store = EnhancementStore::new(&mut sub_pages, &mut packets);
    let config = Config { store_hexadecimal_pages: true, ..Config::default() };
    let mut decoder = Decoder::with_enhancement_store(&mut pages, store, config);
    let reserved = [0x1BE, 0x1F0, 0x2FD, 0x3FE, 0x8FD];
    let mut stream: Vec<Packet> = reserved.iter().flat_map(|&mpp| [header(mpp, 0), text_row(mpp, 1)]).collect();
    stream.push(header(0x1BF, 0));
    stream.extend(filling());
    feed(&mut decoder, &stream);
    for mpp in reserved {
        assert!(!decoder.store().contains(address(mpp)), "{mpp:03X}");
    }
    assert!(decoder.store().contains(address(0x1BF)), "an unreserved hexadecimal page is stored");
    let lookup = decoder.enhancement_lookup();
    assert!(lookup.magazine_inventory_page(Magazine::new(2).unwrap()).is_some());
    assert!(lookup.magazine_organization_table(Magazine::new(3).unwrap()).is_some());
}

/// REQ-NUM-018, REQ-NUM-007: in X/27/0, page FF names no page and XFF:3F7F
/// is no link; in 8/30 the initial page FF falls back to page 100.
#[test]
fn links_that_name_page_ff_name_no_page() {
    let magazine = Magazine::new(1).unwrap();
    let links = [
        Some((magazine, PageNumber(0xFF), SubCode::new(0x0001))),
        Some((magazine, PageNumber(0xFF), SubCode::ANY)),
        Some((magazine, PageNumber(0x23), SubCode::ANY)),
        None,
        None,
        None,
    ];
    with_decoder(8, |decoder| {
        let mut stream = vec![header(0x100, 0), encode::links(magazine, &links, true, 0)];
        stream.extend(filling());
        feed(decoder, &stream);
        let page_links = PageLinks::from_page(decoder.store().get(address(0x100), SubCode::ANY).unwrap());
        assert_eq!(page_links.keys[0], None);
        assert_eq!(page_links.keys[1], None);
        assert_eq!(page_links.keys[2].unwrap().address, address(0x123));
    });
    // 8/30 Format 1 shares X/27/0's page reference (Figure 12), with an
    // absolute magazine; its initial page is that of link 0.
    let mut service = encode::links(magazine, &links, true, 0);
    service.bytes[..2].copy_from_slice(&encode::address(Magazine::new(8).unwrap(), PACKET_SERVICE).bytes[..2]);
    with_decoder(8, |decoder| {
        feed(decoder, &[service]);
        let info = interpret(decoder.service().unwrap());
        assert_eq!(info.initial_page, None);
        assert_eq!(initial_page(Some(&info)), address(0x100));
    });
    assert_eq!(PACKET_LINKS, 27);
}

/// REQ-NUM-026: the decoder reads every page code of Table 24 at the
/// position Figure 26 gives each page number, and a thirteen-bit count.
#[test]
fn the_magazine_inventory_page_is_read() {
    // Page p carries code p, so every code is read once, at every
    // position.
    let codes: Vec<(u8, u8)> = (0..=u8::MAX).map(|page| (page, page)).collect();
    // The counted codes in Figure 26's order: 50, 51, D0, D1, E0, E1 and
    // F8 (decimal units first), then 7B and 7E.
    let counts = [4095, 0x0FFE, 80, 0x0FFF, 1, 0x0001, 4, 2, 3];
    with_decoder(8, |decoder| {
        let mut stream = mip_packets(1, &codes, &counts);
        stream.extend(filling());
        feed(decoder, &stream);
        for page in 0..=u8::MAX {
            let at = PageAddress { magazine: Magazine::new(1).unwrap(), page: PageNumber(page) };
            assert_eq!(decoder.inventory_code(at), Some(PageCode(page)), "{page:02X}");
        }
        let count = |page: u16| decoder.inventory_sub_pages(address(0x100 | page));
        assert_eq!(count(0x50), Some(4095));
        assert_eq!(count(0x51).map(usize::from), Some(MAX_SUBPAGES), "D12 from the code's least significant bit");
        assert_eq!(count(0xD0), Some(80));
        assert_eq!(count(0xD1), Some(0x1FFF));
        assert_eq!(count(0xE1), Some(0x1001));
        assert_eq!((count(0x7B), count(0x7E), count(0xF8)), (Some(2), Some(3), Some(4)));
        assert_eq!(
            (count(0x01), count(0x02), count(0x4F), count(0x81), count(0xCF)),
            (Some(1), Some(2), Some(79), Some(1), Some(79))
        );
        assert_eq!(count(0x79), None);
    });
}

/// REQ-NUM-026: Table 24's functions, for every code.
#[test]
fn every_page_code_has_its_function() {
    use InventoryFunction as F;
    let expected = |code: u8| match code {
        0x00 => F::NotInTransmission,
        0x01..=0x51 => F::Normal,
        0x70..=0x77 => F::Subtitle(code - 0x70),
        0x78 => F::SubtitleMenu,
        0x79 => F::NotFollowingRules,
        0x7A => F::TvWarning,
        0x7B | 0x7C => F::Programme,
        0x7D => F::NowAndNext,
        0x7E | 0x7F => F::TvIndex,
        0x80 => F::NotPublic,
        0x81..=0xD1 => F::Schedule,
        0xE0..0xE5 => F::DataBroadcasting,
        0xE5 | 0xE8..=0xEB => F::Drcs,
        0xE6 | 0xEC..=0xEF => F::Object,
        0xE7 => F::SystemWithoutDisplay,
        0xF0..=0xF3 => F::BroadcasterSystem,
        0xF4..=0xF6 => F::Engineering,
        0xF7 => F::SystemWithDisplay,
        0xF8 | 0xF9 => F::KeywordSearch,
        0xFC => F::Trigger,
        0xFD => F::ChannelInstallation,
        0xFE => F::Top,
        _ => F::Reserved,
    };
    for code in 0..=u8::MAX {
        assert_eq!(PageCode(code).function(), expected(code), "{code:02X}");
    }
}

/// REQ-NUM-021: a page its MIP marks with code 79 keeps only its latest
/// version, and its sub-codes are not listed or stepped; without the MIP it
/// is a page like any other.
#[test]
fn a_page_not_following_the_sub_code_rules_keeps_its_latest_version() {
    let times = [0x1200, 0x1201, 0x1202].map(|code| header(0x150, code));
    with_decoder(8, |decoder| {
        let mut stream = mip_packets(1, &[(0x50, MIP_NOT_FOLLOWING_RULES)], &[]);
        stream.extend(times.iter().copied());
        stream.extend(filling());
        feed(decoder, &stream);
        let stored = |page: &&StoredPage| page.key.address == address(0x150) && page.state != StoredPage::EMPTY.state;
        assert_eq!(decoder.store().pages().iter().filter(stored).count(), 1);
        assert_eq!(decoder.store().latest(address(0x150)).unwrap().key.subcode.value(), 0x1202);
        assert_eq!(listed(decoder, 0x150), []);
        let mut viewer = Viewer::new(address(0x150));
        viewer.event(ViewerEvent::Started);
        viewer.event(ViewerEvent::PagePresent);
        let links = PageLinks::default();
        let config = Config::default();
        let no = |_: u8| false;
        let context = KeyContext {
            links: &links,
            store: decoder.store(),
            initial: address(0x100),
            config: &config,
            now: Instant(0),
            confirmed: &no,
            row24_shown: true,
        };
        assert_eq!(viewer.key(Key::NextSubPage, &context), Action::None);
    });
    with_decoder(8, |decoder| {
        let mut stream = times.to_vec();
        stream.extend(filling());
        feed(decoder, &stream);
        assert_eq!(listed(decoder, 0x150), [0x1200, 0x1201, 0x1202]);
    });
}

/// REQ-NUM-024, REQ-NUM-027: a data broadcasting page, by its page
/// function or by its MIP code, has no sub-pages: three data streams and
/// a continuity index that wraps are neither listed nor counted, even on
/// a decimal page number (REQ-NUM-012) and with no store for enhancement
/// pages.
#[test]
fn a_data_broadcasting_page_has_no_sub_pages() {
    let mut stream = Vec::new();
    for continuity in 0..=16u8 {
        for data_stream in 0..3 {
            let code = data_sub_code(data_stream, continuity % 16, 2);
            stream.extend([
                header(0x170, code),
                page_function(0x170, PAGE_FUNCTION_DATA_BROADCASTING),
                text_row(0x170, 1),
            ]);
        }
    }
    stream.extend(filling());
    let mut pages = vec![StoredPage::EMPTY; 64];
    let mut decoder = Decoder::new(&mut pages, Config::default());
    feed(&mut decoder, &stream);
    assert!(decoder.store().contains(address(0x170)));
    assert_eq!(decoder.store().subpage_count(address(0x170)), 0);
    assert!(!decoder.store().has_display_page(address(0x170)));
    // By the MIP's code E3 alone, with no page function sent.
    with_decoder(64, |decoder| {
        let mut by_code = mip_packets(1, &[(0x70, 0xE3)], &[]);
        by_code.extend((0..16u8).map(|continuity| header(0x170, data_sub_code(0, continuity, 1))));
        by_code.extend(filling());
        feed(decoder, &by_code);
        assert_eq!(decoder.store().subpage_count(address(0x170)), 0);
    });
}

/// REQ-NUM-022, REQ-NUM-027: an enhancement data page's sub-code is nnXs:
/// its sub-pages are keyed by S1 alone in the enhancement store, whatever
/// nn and X, and none is listed as a sub-page.
#[test]
fn an_enhancement_data_page_is_keyed_by_s1() {
    with_decoder(8, |decoder| {
        let mut stream = Vec::new();
        for (last, update, sub_page) in [(1, 0, 0), (2, 1, 0), (1, 0, 1)] {
            let code = enhancement_sub_code(last, update, sub_page);
            stream.extend([header(0x1A0, code), page_function(0x1A0, PAGE_FUNCTION_POP), text_row(0x1A0, 1)]);
        }
        stream.extend(filling());
        feed(decoder, &stream);
        let store = decoder.enhancement_store();
        let entries: Vec<u8> =
            store.sub_pages().filter(|page| page.key().address == address(0x1A0)).map(|page| page.key().s1).collect();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&0) && entries.contains(&1));
        assert_eq!(decoder.store().subpage_count(address(0x1A0)), 0);
    });
}

/// REQ-NUM-028, REQ-NUM-027: a TOP page, by its page function or by its
/// MIP code FE, is not offered as a display page: stepping through stored
/// pages passes it, and its sub-codes are not listed.
#[test]
fn top_pages_are_not_offered_as_display_pages() {
    with_decoder(16, |decoder| {
        let mut stream = mip_packets(1, &[(0x02, MIP_TOP)], &[]);
        stream.extend([header(0x101, 0), page_function(0x101, PAGE_FUNCTION_BTT), header(0x102, 1), header(0x103, 0)]);
        stream.push(page_function(0x103, PAGE_FUNCTION_BASIC));
        stream.extend(filling());
        feed(decoder, &stream);
        // The BTT by its function is kept as received, as a data page.
        assert!(decoder.enhancement_store().contains_address(address(0x101)));
        let store = decoder.store();
        assert!(store.contains(address(0x102)), "the MIP's TOP page is stored as the data page it is");
        assert_eq!(step_stored(address(0x100), true, store), address(0x103));
        assert_eq!(store.subpage_count(address(0x102)), 0);
        assert_eq!(store.subpage_count(address(0x103)), 1);
    });
}

/// REQ-NUM-030: the store is a cache of the caller's capacity; the latest
/// sub-page of a rotating page is the last of its sub-pages to go.
#[test]
fn the_latest_sub_page_is_kept() {
    with_decoder(4, |decoder| {
        let mut stream: Vec<Packet> = (1..=10).map(|code| header(0x100, code)).collect();
        stream.push(header(0x101, 0));
        feed(decoder, &stream);
        assert_eq!(decoder.store().latest(address(0x100)).unwrap().key.subcode.value(), 10);
        assert_eq!(listed(decoder, 0x100), [8, 9, 10]);
    });
}

/// REQ-NUM-017, REQ-NAV-005: sub-page entry accepts exactly the 3,199
/// decimal sub-codes, refusing a digit that cannot stand where it is.
#[test]
fn sub_page_entry_accepts_exactly_the_decimal_sub_codes() {
    let mut accepted = std::collections::BTreeSet::new();
    for value in 0..10_000u16 {
        let digits = [value / 1000, value / 100 % 10, value / 10 % 10, value % 10];
        let mut entry = SubPageEntry::default();
        let mut outcome = SubPageEntryResult::Rejected;
        for digit in digits {
            outcome = entry.push(digit as u8);
            if outcome == SubPageEntryResult::Rejected {
                break;
            }
        }
        if let SubPageEntryResult::Complete(code) = outcome {
            assert!(accepted.insert(code.value()));
        }
    }
    assert_eq!(accepted.len(), DECIMAL_SUBCODES);
    assert_eq!((accepted.first(), accepted.last()), (Some(&0x0001), Some(&0x3979)));
}

/// REQ-NUM-020, REQ-NUM-030, REQ-NAV-005: a page of 8,190 sub-pages,
/// sub-codes 0001 to 3F7E, is stored, listed, counted and stepped through
/// whole by the store and the viewer, which holds no list; the status area
/// shows each place that fits.
#[test]
fn a_page_of_8190_sub_pages_is_held_and_stepped() {
    let codes: Vec<u16> = (FIRST_SUBPAGE_CODE..=LAST_SUBPAGE_CODE).filter(|&code| code & 0x0080 == 0).collect();
    assert_eq!(codes.len(), MAX_SUBPAGES);
    with_decoder(MAX_SUBPAGES + 8, |decoder| {
        let mut stream: Vec<Packet> = codes.iter().map(|&code| header(0x100, code)).collect();
        stream.extend(filling());
        feed(decoder, &stream);
        let store = decoder.store();
        assert_eq!(store.subpage_count(address(0x100)), MAX_SUBPAGES);
        assert_eq!(listed(decoder, 0x100), codes);
        let mut viewer = Viewer::new(address(0x100));
        viewer.event(ViewerEvent::Started);
        viewer.event(ViewerEvent::PagePresent);
        let links = PageLinks::default();
        let config = Config::default();
        let no = |_: u8| false;
        let context = KeyContext {
            links: &links,
            store,
            initial: address(0x100),
            config: &config,
            now: Instant(0),
            confirmed: &no,
            row24_shown: true,
        };
        // From the rotating display, the latest (3F7E) steps on to the first.
        assert_eq!(viewer.key(Key::NextSubPage, &context), Action::Request(address(0x100), SubCode::new(0x0001)));
        assert_eq!(&viewer.status(store), b"P100/01");
        for &expected in &codes[1..] {
            assert_eq!(viewer.key(Key::NextSubPage, &context), Action::Request(address(0x100), SubCode::new(expected)));
        }
        assert_eq!(&viewer.status(store), b"100/+++", "place 8,190 does not fit");
        assert_eq!(
            viewer.key(Key::NextSubPage, &context),
            Action::Request(address(0x100), SubCode::new(0x0001)),
            "wraps"
        );
        assert_eq!(
            viewer.key(Key::PreviousSubPage, &context),
            Action::Request(address(0x100), SubCode::new(LAST_SUBPAGE_CODE)),
            "wraps back"
        );
        viewer.show_subpage(SubCode::new(codes[122]));
        assert_eq!(&viewer.status(store), b"100/123");
    });
}
