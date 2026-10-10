//! Behaviour across modules: the reordering tests of REQ-FSM-008 and
//! REQ-FSM-009, non-text pages (REQ-ACQ-014), inhibit display
//! (REQ-L1-016) and the second G0 set (REQ-CHR-009, REQ-L1-013).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use tellytxt_core::acq::{Decoder, MagazineData, StoredPage};
use tellytxt_core::charset::{CharSet, G0Set};
use tellytxt_core::config::Config;
use tellytxt_core::consts::{C10_INHIBIT_DISPLAY, ESC};
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
