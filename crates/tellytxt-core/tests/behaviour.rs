//! Behaviour across modules: the reordering tests of REQ-FSM-008 and
//! REQ-FSM-009, non-text pages (REQ-ACQ-014), inhibit display
//! (REQ-L1-016), the second G0 set (REQ-CHR-009, REQ-L1-013), Level 1.5
//! overwrites over Level 1 attributes (REQ-L15-008), subtitle pages
//! (REQ-ACQ-015) and independent decoders (REQ-GEN-004).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use tellytxt_core::acq::{Decoder, MagazineData, StoredPage};
use tellytxt_core::charset::{CharSet, G0Set};
use tellytxt_core::config::Config;
use tellytxt_core::consts::{
    ALPHA_RED, ALPHA_WHITE, C4_ERASE, C6_SUBTITLE, C10_INHIBIT_DISPLAY, COLOUR_ATTRIBUTE_MASK, DESIGNATION_CODES, ESC,
    HOLD_MOSAICS, MODE_G0_NO_DIACRITIC, MODE_SET_ACTIVE_POSITION, MOSAIC_FULL, MOSAIC_RED, ROW_ADDRESS_BASE, START_BOX,
    TRIPLETS_PER_PACKET,
};
use tellytxt_core::ecc::with_odd_parity;
use tellytxt_core::level1::{DisplayOptions, Glyph, Grid, build_grid};
use tellytxt_core::level15;
use tellytxt_core::packet::{Packet, PacketMeta, Triplet, encode};
use tellytxt_core::types::{ControlBits, Instant, Magazine, PageAddress, PageNumber, SubCode};

fn mag() -> Magazine {
    Magazine::new(2).unwrap()
}

fn text(s: &[u8]) -> [u8; 40] {
    let mut out = [with_odd_parity(b' '); 40];
    for (slot, &b) in out.iter_mut().zip(s) {
        *slot = with_odd_parity(b);
    }
    out
}

fn header(page: u8, control: u16) -> Packet {
    encode::header(mag(), PageNumber(page), SubCode::new(0), ControlBits(control), &[with_odd_parity(b' '); 32])
}

/// The packets of one page: rows, two X/26 packets, X/27/0 and X/28/0.
fn body() -> Vec<Packet> {
    let mut packets = Vec::new();
    for row in 1..=5u8 {
        packets.push(encode::row(mag(), row, &text(format!("ROW {row} TEXT").as_bytes())));
    }
    let mut first = [Triplet { address: 41, mode: 0x04, data: 0 }; 13];
    first[1] = Triplet { address: 3, mode: 0x12, data: b'e' };
    first[2] = Triplet { address: 63, mode: 0x1F, data: 0 };
    packets.push(encode::triplet_packet(mag(), 26, 0, &first));
    let second = [Triplet { address: 42, mode: 0x04, data: 0 }; 13];
    packets.push(encode::triplet_packet(mag(), 26, 1, &second));
    let target = (Magazine::new(3).unwrap(), PageNumber(0x45), SubCode::ANY);
    packets.push(encode::links(mag(), &[Some(target), None, None, None, None, None], true, 0));
    let [a, b] = encode::designation_triplets(0x00, 0x7F);
    let mut data = [Triplet::default(); 13];
    data[0] = a;
    data[1] = b;
    packets.push(encode::triplet_packet(mag(), 28, 0, &data));
    packets
}

fn decode(order: &[Packet]) -> (StoredPage, Grid) {
    let mut slots = vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let mut all = vec![header(0x00, 0)];
    all.extend_from_slice(order);
    all.push(header(0x01, 0));
    for (index, packet) in all.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    let address = PageAddress::from_mpp(0x200).unwrap();
    let mut page = *decoder.store().get(address, SubCode::ANY).unwrap();
    // Reception times differ by construction and are not page content.
    page.first_received = Instant(0);
    page.last_received = Instant(0);
    let mut grid = Grid::EMPTY;
    build_grid(
        &mut grid,
        Some(&page),
        &decoder.magazine_data(address.magazine),
        &Config::default(),
        &DisplayOptions::default(),
    );
    level15::apply(&mut grid, &page.enhancements, page.enhancements_present, &Config::default(), true);
    (page, grid)
}

#[test]
fn any_packet_order_gives_the_same_page_and_grid() {
    let standard = body();
    let (expected_page, expected_grid) = decode(&standard);
    // Rows reversed.
    let mut reversed = standard.clone();
    reversed[..5].reverse();
    // Enhancement, link and page packets after the rows, designation codes
    // descending.
    let mut late = standard[..5].to_vec();
    late.extend(standard[5..].iter().rev().copied());
    // Everything interleaved.
    let (rows, others) = standard.split_at(5);
    let mut mixed = Vec::new();
    for index in 0..rows.len().max(others.len()) {
        mixed.extend(others.iter().rev().nth(index).copied());
        mixed.extend(rows.get(index).copied());
    }
    assert_eq!(mixed.len(), standard.len());
    for order in [reversed, late, mixed] {
        let (page, grid) = decode(&order);
        assert_eq!(page, expected_page);
        assert_eq!(grid, expected_grid);
    }
}

#[test]
fn non_text_pages_and_inhibit_display_show_no_rows() {
    let mut page = decode(&body()).0;
    let mut grid = Grid::EMPTY;
    let config = Config::default();
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &config, &DisplayOptions::default());
    assert!(grid.cells[1].iter().any(|cell| cell.glyph != Glyph::Space));
    page.control = ControlBits(C10_INHIBIT_DISPLAY);
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &config, &DisplayOptions::default());
    assert!(grid.cells[1..24].iter().all(|row| row.iter().all(|cell| cell.glyph == Glyph::Space)), "REQ-L1-016");
    page.control = ControlBits(0);
    if let Some(data) = page.page_data.as_mut() {
        data.page_function = 1;
    }
    assert!(!page.is_text_page());
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &config, &DisplayOptions::default());
    assert!(grid.cells[1..24].iter().all(|row| row.iter().all(|cell| cell.glyph == Glyph::Space)), "REQ-ACQ-014");
}

#[test]
fn esc_switches_to_the_second_g0_set() {
    let mut page = decode(&body()).0;
    // Table 33 value 0x24: Cyrillic option 2 in the group 0100 encoding.
    if let Some(data) = page.page_data.as_mut() {
        data.second_designation = 0x24;
    }
    page.cells[2][..3].copy_from_slice(&[with_odd_parity(b'A'), with_odd_parity(ESC), with_odd_parity(b'A')]);
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &Config::default(), &DisplayOptions::default());
    let set = |cell: &tellytxt_core::level1::Cell| match cell.glyph {
        Glyph::Char(c) => Some(c.set),
        _ => None,
    };
    assert!(matches!(set(&grid.cells[2][0]), Some(CharSet::G0(g0)) if g0.set == G0Set::Latin));
    assert!(matches!(set(&grid.cells[2][2]), Some(CharSet::G0(g0)) if g0.set != G0Set::Latin), "REQ-CHR-009");
}

/// REQ-L15-008, decision E.5 (a) and (d): Level 1 attributes act from the
/// raw bytes even under an overwrite, and the held mosaic comes from the
/// Level 1 bytes only.
#[test]
fn overwrites_keep_level1_attributes_and_the_held_mosaic() {
    let mut page = decode(&body()).0;
    page.cells[3][..2].copy_from_slice(&[with_odd_parity(ALPHA_RED), with_odd_parity(b'A')]);
    page.cells[4][..4].copy_from_slice(&[MOSAIC_RED, 0x7F, HOLD_MOSAICS, HOLD_MOSAICS].map(with_odd_parity));
    let position = |row: u8| Some(Triplet { address: ROW_ADDRESS_BASE + row, mode: MODE_SET_ACTIVE_POSITION, data: 0 });
    let write = |column: u8| Some(Triplet { address: column, mode: MODE_G0_NO_DIACRITIC, data: b'X' });
    page.enhancements = [[None; TRIPLETS_PER_PACKET]; DESIGNATION_CODES];
    page.enhancements[0][..4].copy_from_slice(&[position(3), write(0), position(4), write(1)]);
    page.enhancements_present = 1;
    let config = Config::default();
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(&page), &MagazineData::default(), &config, &DisplayOptions::default());
    level15::apply(&mut grid, &page.enhancements, page.enhancements_present, &config, true);
    let is_x = |cell: &tellytxt_core::level1::Cell| matches!(cell.glyph, Glyph::Char(c) if c.code == b'X');
    // (a): the overwritten red attribute still colours the next cell.
    assert!(is_x(&grid.cells[3][0]));
    assert_eq!(grid.cells[3][0].foreground, ALPHA_WHITE & COLOUR_ATTRIBUTE_MASK);
    assert_eq!(grid.cells[3][1].foreground, ALPHA_RED & COLOUR_ATTRIBUTE_MASK);
    // (d): the overwritten mosaic is still the one held.
    assert!(is_x(&grid.cells[4][1]));
    let held = Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false };
    assert_eq!((grid.cells[4][3].glyph, grid.cells[4][3].held), (held, true));
}

/// A subtitle page reception: header with C6 and C4, then one boxed row.
fn subtitle(row: u8, words: &[u8]) -> Vec<Packet> {
    let mut line = vec![START_BOX, START_BOX];
    line.extend_from_slice(words);
    vec![header(0x88, C4_ERASE | C6_SUBTITLE), encode::row(mag(), row, &text(&line))]
}

/// REQ-ACQ-015: each subtitle reception replaces the last, and the page is
/// shown in boxed mode (decision E.22).
#[test]
fn subtitle_receptions_replace_each_other_and_show_boxed() {
    let mut stream = subtitle(20, b"FIRST");
    stream.extend(subtitle(21, b"SECOND"));
    stream.push(header(0x89, 0));
    let mut slots = vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    for (index, packet) in stream.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    let page = decoder.store().get(PageAddress::from_mpp(0x288).unwrap(), SubCode::ANY).unwrap();
    assert!(!page.has_row(20) && page.has_row(21), "the second reception replaced the first");
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(page), &MagazineData::default(), &Config::default(), &DisplayOptions::default());
    assert!(grid.boxed_mode);
    let boxed: Vec<bool> = grid.cells[21][..4].iter().map(|cell| cell.boxed).collect();
    assert_eq!(boxed, [false, true, true, true]);
}

/// The pages a decoder stores from a stream, run on its own.
fn pages_alone(stream: &[Packet]) -> Vec<StoredPage> {
    let mut slots = vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    for (index, packet) in stream.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    decoder.store().pages().to_vec()
}

const fn assert_send<T: Send>() {}

/// REQ-GEN-004: two decoders fed different streams, interleaved, store
/// exactly what each stores when run alone; the state types are `Send`.
#[test]
fn two_decoders_run_independently() {
    const _: () = {
        assert_send::<Decoder<'static>>();
        assert_send::<StoredPage>();
        assert_send::<Grid>();
        assert_send::<tellytxt_core::viewer::Viewer>();
        assert_send::<tellytxt_core::render::ChangeTracker>();
        assert_send::<tellytxt_core::forms::FormModel>();
    };
    let mut first = vec![header(0x00, 0)];
    first.extend(body());
    first.push(header(0x01, 0));
    let mut second = subtitle(20, b"OTHER");
    second.push(header(0x89, 0));
    let (expected_first, expected_second) = (pages_alone(&first), pages_alone(&second));
    let mut slots_first = vec![StoredPage::EMPTY; 4];
    let mut slots_second = vec![StoredPage::EMPTY; 4];
    let mut decoder_first = Decoder::new(&mut slots_first, Config::default());
    let mut decoder_second = Decoder::new(&mut slots_second, Config::default());
    for index in 0..first.len().max(second.len()) {
        let now = Instant(index as u64 * 64);
        if let Some(packet) = first.get(index) {
            decoder_first.push(packet, &PacketMeta::default(), now, &mut |_| {});
        }
        if let Some(packet) = second.get(index) {
            decoder_second.push(packet, &PacketMeta::default(), now, &mut |_| {});
        }
    }
    assert_ne!(expected_first, expected_second);
    assert_eq!(decoder_first.store().pages(), &expected_first[..]);
    assert_eq!(decoder_second.store().pages(), &expected_second[..]);
}

/// Decision E.23 by profile: the X/27/0 flag shows row 24 by default, and
/// the SAA5050 profile never shows it (REQ-PRF-002).
#[test]
fn row_24_follows_the_profile() {
    let mut packets = body();
    packets.push(encode::row(mag(), 24, &text(b"ROW 24 PROMPTS")));
    let (page, _) = decode(&packets);
    assert!(page.links.is_some_and(|links| links.display_row_24));
    let row_24_shown = |config: Config| {
        let mut grid = Grid::EMPTY;
        build_grid(&mut grid, Some(&page), &MagazineData::default(), &config, &DisplayOptions::default());
        grid.cells[24].iter().any(|cell| !cell.glyph.is_blank())
    };
    assert!(row_24_shown(Config::default()));
    assert!(row_24_shown(Config::saa5246a()));
    assert!(!row_24_shown(Config::saa5050()));
}
