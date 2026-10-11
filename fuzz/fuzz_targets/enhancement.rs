//! The whole grid build at Levels 2.5 and 3.5 (REQ-TST-042): a displayed
//! page, its object pages, DRCS pages, Magazine Organization Table and
//! magazine packets, all built from the fuzzer's input, through the
//! decoder with an enhancement store, the enhancement walk, objects, DRCS
//! decoding and the renderer in the resolved colour mode.
//!
//! The input is a list of records, each naming one of the pages below and
//! a packet kind, so that the fuzzer reaches the walk rather than spending
//! its time on page headers: a header, a row of raw bytes, a packet of
//! valid Hamming 24/18 triplets made from the fuzzer's bits (X/26, X/27/4
//! and X/27/5, X/28, M/29, object pages and their pointer tables), or a
//! packet of raw bytes with any packet number. As on air, a packet other
//! than a header belongs to the page whose header came last.
//!
//! It checks that the walk stays within `MAX_WALK_STEPS` (REQ-TST-019) and
//! that row 0, columns 0 to 7, are left as the Level 1 build left them
//! (REQ-L25-050).
#![no_main]

use libfuzzer_sys::fuzz_target;
use tellytxt_core::acq::{Decoder, EnhancementPacket, EnhancementStore, EnhancementSubPage, StoredPage};
use tellytxt_core::config::{CellMode, Config, PresentationLevel};
use tellytxt_core::consts::{
    DATA_BYTES, ENHANCEMENT_PACKETS_LEVEL_35, HEADER_DISPLAY_BYTES, MAGAZINES, MOT_PACKETS, ROWS, TRIPLETS_PER_PACKET,
};
use tellytxt_core::enhance::MAX_WALK_STEPS;
use tellytxt_core::glyphs::GlyphSet;
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::packet::{Packet, PacketMeta, Triplet, encode};
use tellytxt_core::render::{
    ALL_ROWS, ColourMode, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, render_screen, row_width,
};
use tellytxt_core::types::{ControlBits, Instant, Magazine, PageAddress, PageNumber, SubCode};

/// Microseconds between packets: one television line.
const LINE_US: u64 = 64;
/// The pages a record can name, all in magazine 1: the displayed page, a
/// second display page, a POP, a GPOP, a DRCS page, a GDRCS page, the
/// Magazine Organization Table and page FF, which carries M/29.
const PAGES: [u8; 8] = [0x00, 0x01, 0xA0, 0xE0, 0xB0, 0xC0, 0xFE, 0xFF];
/// The displayed page.
const DISPLAYED: u16 = 0x100;
/// Row 0, columns 0 to 7: the receiver's area (REQ-L25-050).
const STATUS_COLUMNS: usize = 8;
/// Small stores, so that pages are evicted and replaced.
const PAGE_SLOTS: usize = 16;
const SUB_PAGE_SLOTS: usize = 32;
/// Bytes of a triplet value taken from the input: 18 bits from three.
const TRIPLET_INPUT_BYTES: usize = 3;
const TRIPLET_VALUE_MASK: u32 = 0x3_FFFF;

/// Takes `n` bytes from the front of `data`.
fn take<'a>(data: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    let (head, tail) = data.split_at_checked(n)?;
    *data = tail;
    Some(head)
}

fn packet_of(selector: u8, data: &mut &[u8]) -> Option<Packet> {
    let magazine = Magazine::new(1)?;
    match (selector >> 3) & 3 {
        0 => {
            let page = PageNumber(PAGES[usize::from(selector & 7)]);
            let [s1, control_low, control_high] = *take(data, 3)? else { return None };
            let subcode = SubCode::new(u16::from(s1 & 0xF));
            let control = ControlBits(u16::from_le_bytes([control_low, control_high]));
            Some(encode::header(magazine, page, subcode, control, &[b' '; HEADER_DISPLAY_BYTES]))
        }
        1 => {
            let [row] = *take(data, 1)? else { return None };
            let bytes: [u8; DATA_BYTES] = take(data, DATA_BYTES)?.try_into().ok()?;
            Some(encode::row(magazine, row % 26, &bytes))
        }
        2 => {
            let [number, designation] = *take(data, 2)? else { return None };
            let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
            for triplet in &mut triplets {
                let [a, b, c] = *take(data, TRIPLET_INPUT_BYTES)? else { return None };
                *triplet = Triplet::from_value(u32::from_le_bytes([a, b, c, 0]) & TRIPLET_VALUE_MASK);
            }
            // Rows 1 to 25 of object pages, and packets 26 to 29.
            Some(encode::triplet_packet(magazine, 1 + number % 29, designation, &triplets))
        }
        _ => {
            let [number] = *take(data, 1)? else { return None };
            let bytes: [u8; DATA_BYTES] = take(data, DATA_BYTES)?.try_into().ok()?;
            Some(encode::row(magazine, number % 32, &bytes))
        }
    }
}

fuzz_target!(|input: &[u8]| {
    let Some((&settings, mut data)) = input.split_first() else { return };
    let config = Config {
        level: if settings & 1 != 0 { PresentationLevel::Level35 } else { PresentationLevel::Level25 },
        cell_mode: if settings & 2 != 0 { CellMode::Saa5246a } else { CellMode::Saa5050 },
        ..Config::default()
    };
    let mut pages = [StoredPage::EMPTY; PAGE_SLOTS];
    let mut sub_pages = [EnhancementSubPage::EMPTY; SUB_PAGE_SLOTS];
    let mut pool = vec![EnhancementPacket::EMPTY; ENHANCEMENT_PACKETS_LEVEL_35 + MAGAZINES * MOT_PACKETS];
    let mut decoder =
        Decoder::with_enhancement_store(&mut pages, EnhancementStore::new(&mut sub_pages, &mut pool), config);
    let mut now = 0u64;
    while let Some((&selector, rest)) = data.split_first() {
        data = rest;
        let Some(packet) = packet_of(selector, &mut data) else { break };
        now += LINE_US;
        decoder.push(&packet, &PacketMeta::default(), Instant(now), &mut |_| {});
    }
    decoder.tick(Instant(u64::MAX / 2), &mut |_| {});
    let Some(address) = PageAddress::from_mpp(DISPLAYED) else { return };
    let Some(page) = decoder.store().get(address, SubCode::ANY) else { return };
    let magazine = decoder.magazine_data(address.magazine);
    let mut grid: Box<Grid> = Box::new(Grid::empty());
    build_grid(&mut grid, Some(page), &magazine, &config, &DisplayOptions::default());
    let status: [_; STATUS_COLUMNS] = core::array::from_fn(|column| grid.cells[0][column]);
    let report = tellytxt_core::compose::compose_level(
        &mut grid,
        page,
        &magazine,
        &decoder.enhancement_lookup(),
        &config,
        &DisplayOptions::default(),
    );
    if let Some(report) = report {
        assert!(report.steps <= MAX_WALK_STEPS, "REQ-TST-019: {} steps", report.steps);
    }
    assert!(status.iter().zip(&grid.cells[0]).all(|(before, after)| before == after), "REQ-L25-050");
    let options = RenderOptions {
        mode: config.cell_mode,
        rounding: settings & 4 != 0,
        reveal: settings & 8 != 0,
        colour: ColourMode::Resolved(FlashPhase::at_unit(settings >> 4)),
    };
    let width = row_width(&grid);
    let mut pixels = vec![0u8; width * ROWS * options.row_height()];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut pixels, stride: width };
    let _ = render_screen(&grid, ALL_ROWS, &GlyphSet::original(), options, &mut buffer);
});
