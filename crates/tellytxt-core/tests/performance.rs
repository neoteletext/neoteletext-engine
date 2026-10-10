//! Throughput measurements (REQ-GEN-013, REQ-TST-050). Run with
//! `cargo test -p tellytxt-core --release --test performance -- --ignored --nocapture`.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    missing_docs
)]

mod common;

use std::time::Instant as Clock;
use tellytxt_core::acq::{Decoder, StoredPage};
use tellytxt_core::config::Config;
use tellytxt_core::consts::{PACKET_BYTES, PAGE_WIDTH};
use tellytxt_core::glyphs::GlyphSet;
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::packet::{Packet, PacketMeta};
use tellytxt_core::render::{RenderOptions, cell_line};
use tellytxt_core::types::{Instant, SubCode};
use tellytxt_core::viewer::compose_level;

/// REQ-GEN-013: twenty times 35 packets per frame at 25 frames per second.
const REQUIRED_PACKETS_PER_SECOND: f64 = 17_500.0;

fn capture() -> Option<Vec<u8>> {
    let directory = std::env::var_os("TELLYTXT_CORPUS")?;
    std::fs::read(std::path::Path::new(&directory).join("dan/bbc1_1991-10-31_Dan/bbc1_1991-10-31_v2.t42")).ok()
}

#[test]
#[ignore = "measurement; run in release mode"]
fn packet_decoder_throughput() {
    let Some(bytes) = capture() else {
        eprintln!("SKIPPED: TELLYTXT_CORPUS with Dan's BBC1 capture is needed");
        return;
    };
    let packets: Vec<Packet> =
        bytes.as_chunks::<PACKET_BYTES>().0.iter().map(|chunk| Packet { bytes: *chunk }).collect();
    let mut slots = vec![StoredPage::EMPTY; 4096];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    let start = Clock::now();
    for (index, packet) in packets.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    let seconds = start.elapsed().as_secs_f64();
    let rate = packets.len() as f64 / seconds;
    println!(
        "decoded {} packets in {seconds:.3} s: {rate:.0} packets per second (required {REQUIRED_PACKETS_PER_SECOND})",
        packets.len()
    );
    assert!(rate >= REQUIRED_PACKETS_PER_SECOND);
}

#[test]
#[ignore = "measurement; run in release mode"]
fn presentation_and_drawing_costs() {
    let Some(bytes) = capture() else {
        eprintln!("SKIPPED: TELLYTXT_CORPUS with Dan's BBC1 capture is needed");
        return;
    };
    let mut slots = vec![StoredPage::EMPTY; 4096];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    for (index, chunk) in bytes.as_chunks::<PACKET_BYTES>().0.iter().enumerate() {
        decoder.push(&Packet { bytes: *chunk }, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    let config = Config::default();
    let address = tellytxt_core::types::PageAddress::from_mpp(0x100).unwrap();
    let page = decoder.store().get(address, SubCode::ANY).unwrap();
    let mut grid = Grid::EMPTY;
    let rounds = 2000;
    let magazine = decoder.magazine_data(address.magazine);
    let options = DisplayOptions::default();
    let start = Clock::now();
    for _ in 0..rounds {
        build_grid(&mut grid, Some(page), &magazine, &config, &options);
        let _ = compose_level(&mut grid, page, &magazine, &decoder.enhancement_lookup(), &config, &options);
    }
    let grids = f64::from(rounds) / start.elapsed().as_secs_f64();
    let glyphs = GlyphSet::bedstead();
    let options = RenderOptions::from_config(&config, false);
    let start = Clock::now();
    let mut draws = 0u64;
    let mut sink = 0u16;
    for _ in 0..20 {
        for row in 0..25 {
            for line in 0..20 {
                for cell in &grid.cells[row] {
                    sink ^= cell_line(cell, &grid, line, &glyphs, options).mask;
                    draws += 1;
                }
            }
        }
    }
    let rate = draws as f64 / start.elapsed().as_secs_f64();
    println!(
        "grids built per second: {grids:.0}; cell-line draws per second: {rate:.0} (a full redraw of every field needs 500,000; page width {PAGE_WIDTH}); {sink}"
    );
}

/// REQ-TST-019: the time to build the worst-case page of the object tests
/// at Level 3.5.
#[test]
#[ignore = "measurement; run in release mode"]
fn worst_case_grid_at_level_35() {
    let packets = common::worst_case_page();
    let config = Config::default();
    common::Receiver::new().run(&packets, config, |decoder| {
        let rounds = 20;
        let start = Clock::now();
        let mut steps = 0;
        for _ in 0..rounds {
            steps = common::grid_of(decoder, &config).1.map_or(0, |report| report.steps);
        }
        let seconds = start.elapsed().as_secs_f64() / f64::from(rounds);
        println!("worst-case grid at Level 3.5: {:.1} ms, {steps} triplets read", seconds * 1000.0);
    });
}
