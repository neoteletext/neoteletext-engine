//! The packet decoder and everything after it: acquisition, the page store,
//! Level 1 and Level 1.5 decoding and the raster renderer, from arbitrary
//! 42-byte packets (REQ-TST-040).
#![no_main]

use libfuzzer_sys::fuzz_target;
use tellytxt_core::acq::{Decoder, StoredPage};
use tellytxt_core::config::{CellMode, Config};
use tellytxt_core::consts::{PACKET_BYTES, PAGE_WIDTH, ROWS};
use tellytxt_core::fsm::PageState;
use tellytxt_core::glyphs::GlyphSet;
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::level15;
use tellytxt_core::packet::{Packet, PacketMeta};
use tellytxt_core::render::{ALL_ROWS, IndexBuffer, IndexFormat, RenderOptions, render_screen};
use tellytxt_core::types::Instant;

/// A small store, so that pages are evicted and replaced.
const SLOTS: usize = 8;
/// Microseconds between packets: one television line.
const LINE_US: u64 = 64;

fuzz_target!(|data: &[u8]| {
    let [settings, body @ ..] = data else { return };
    let config = Config {
        level15: settings & 1 != 0,
        cell_mode: if settings & 2 != 0 { CellMode::Saa5246a } else { CellMode::Saa5050 },
        black_colour_codes: settings & 4 != 0,
        strict_row_setting: settings & 8 != 0,
        ..Config::default()
    };
    let mut slots = [StoredPage::EMPTY; SLOTS];
    let mut decoder = Decoder::new(&mut slots, config);
    let mut now = 0u64;
    for bytes in body.as_chunks::<PACKET_BYTES>().0 {
        now += LINE_US;
        decoder.push(&Packet { bytes: *bytes }, &PacketMeta::default(), Instant(now), &mut |_| {});
    }
    decoder.tick(Instant(u64::MAX / 2), &mut |_| {});
    let Some(page) = decoder.store().pages().iter().find(|page| page.state != PageState::Empty) else { return };
    let mut grid = Grid::EMPTY;
    build_grid(
        &mut grid,
        Some(page),
        &decoder.magazine_data(page.key.address.magazine),
        &config,
        &DisplayOptions::default(),
    );
    level15::apply(&mut grid, &page.enhancements, page.enhancements_present, &config, settings & 16 != 0);
    let options = RenderOptions::from_config(&config, settings & 32 != 0);
    let mut pixels = vec![0u8; PAGE_WIDTH * ROWS * options.row_height()];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut pixels, stride: PAGE_WIDTH };
    let _ = render_screen(&grid, ALL_ROWS, &GlyphSet::bedstead(), options, &mut buffer);
});
