#![allow(clippy::unwrap_used, reason = "a test")]
use super::pages::CODING_TRIPLETS;
use super::*;
use crate::session::Session;
use neoteletext_core::config::Config;
use neoteletext_core::consts::PACKET_PAGE_ENHANCEMENT;
use neoteletext_core::consts::PAGE_FUNCTION_POP;
use neoteletext_core::packet::Packet;
use neoteletext_core::packet::PacketMeta;
use neoteletext_core::types::Magazine;
use neoteletext_formats::Item;
use neoteletext_formats::author::streams;
use neoteletext_formats::detect::{AutoConfig, AutoReader, Format};
use neoteletext_formats::item::Reader;
use neoteletext_formats::page::PageFunction;

fn session_of(packets: &[Packet]) -> Session {
    let mut session = Session::new(Config::default(), 512);
    for packet in packets {
        session.push(packet, &PacketMeta::default());
    }
    session.finish();
    session
}

/// REQ-INP-023 and REQ-TST-020: the authored stream (REQ-TST-024),
/// converted from T42 to TTI and read back, gives the same stores:
/// display pages with their packets 26 to 28, enhancement pages as
/// received, and the magazine packets.
#[test]
fn authored_stream_converts_to_tti_and_back() {
    let stream = streams::stream().unwrap();
    let first = session_of(&stream.packets());
    let files = session_page_files(&first, None);
    let references: Vec<&PageFile> = files.iter().collect();
    let mut text = Vec::new();
    neoteletext_formats::write::write_tti(&references, &mut text).unwrap();
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
    let second = session_of(&packets);

    let (left, right) = (first.pages(), second.pages());
    assert_eq!(left.len(), right.len());
    assert!(left.len() >= 27);
    for (a, b) in left.iter().zip(&right) {
        let name = a.key.address.mpp();
        assert_eq!(a.key, b.key);
        assert_eq!(a.cells, b.cells, "{name:X}");
        assert_eq!((a.enhancements, a.enhancements_present), (b.enhancements, b.enhancements_present), "{name:X}");
        assert_eq!(a.compositional_links, b.compositional_links, "{name:X}");
        assert_eq!((a.page_data, a.page_data_level35), (b.page_data, b.page_data_level35));
        assert_eq!(a.control, b.control, "{name:X}");
    }
    let stored = |session: &Session| {
        let store = session.decoder().enhancement_store();
        let mut pages: Vec<_> = store
            .sub_pages()
            .map(|page| {
                let rows: Vec<_> = (1..=25).map(|packet| page.row(packet).copied()).collect();
                let enhancements: Vec<_> = (0..16).map(|code| page.enhancement(code).copied()).collect();
                (page.key().address, page.key().s1, page.subcode(), page.format(), rows, enhancements, page.received())
            })
            .collect();
        pages.sort_by_key(|page| (page.0, page.1));
        pages
    };
    let (left, right) = (stored(&first), stored(&second));
    assert_eq!(left.len(), 15);
    assert_eq!(left, right);
    for bits in 0..8 {
        let magazine = Magazine::from_bits(bits);
        assert_eq!(first.decoder().magazine_data(magazine), second.decoder().magazine_data(magazine));
    }
    assert!(first.decoder().magazine_data(Magazine::new(7).unwrap()).data.is_some());
}

/// REQ-INP-023: an enhancement page without X/28 is written in the
/// coding its bytes show, and page mFE in Hamming 8/4.
#[test]
fn enhancement_pages_without_a_function_keep_their_bytes() {
    let stream = streams::stream().unwrap();
    // Leave out every X/28 so that only the bytes tell the coding.
    let packets: Vec<Packet> = stream
        .packets()
        .into_iter()
        .filter(|packet| {
            neoteletext_core::packet::decode_address(packet)
                .map_or(true, |address| address.number != PACKET_PAGE_ENHANCEMENT)
        })
        .collect();
    let session = session_of(&packets);
    let files = session_page_files(&session, None);
    let table = files.iter().find(|file| file.address.mpp() == 0x1FE).unwrap();
    assert_eq!(table.function, None);
    let objects = files.iter().find(|file| file.address.mpp() == 0x1A0).unwrap();
    assert_eq!(objects.function, Some(PageFunction { function: PAGE_FUNCTION_POP, coding: CODING_TRIPLETS }));
}
