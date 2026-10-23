//! The cost of a flashing page at each change of its flash phase, as a
//! port pays it (REQ-RND-032, REQ-TST-050, decision E.107): rebuilding the
//! grid and drawing every row, against drawing only the rows
//! [`ChangeTracker`] says changed.
//! Run with `cargo test -p neoteletext-core --release --test flash_cost --
//! --ignored --nocapture`.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss,
    missing_docs
)]

mod common;

use common::{DISPLAY, Receiver, display_page};
use neoteletext_core::acq::Decoder;
use neoteletext_core::compose::compose_level;
use neoteletext_core::config::Config;
use neoteletext_core::consts::{COLUMNS, FLASH, PAGE_WIDTH, STEADY};
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::{DisplayOptions, Grid, build_grid};
use neoteletext_core::render::{
    ALL_ROWS, ChangeTracker, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, render_screen,
};
use neoteletext_core::types::{PageAddress, SubCode};
use std::time::Instant as Clock;

/// Flash units on either side of slow flash's change (decision E.48: on
/// in units 0 to 8, off in 9 to 11).
const SLOW_ON: u8 = 0;
const SLOW_OFF: u8 = 9;
/// Phase changes timed in each batch, and batches: the fastest batch is
/// reported, as the one least disturbed by the rest of the computer.
const ROUNDS: u32 = 500;
const BATCHES: u32 = 15;
/// Display rows 1 to 23 hold text; row 24 is Fastext.
const LAST_TEXT_ROW: u8 = 23;
/// Lines of a whole screen at the 12-by-20 cell.
const SCREEN_LINES: usize = 500;

/// The rows of a page of text on rows 1 to 23, those in `flashing`
/// flashing from their first column.
fn rows(flashing: &[u8]) -> Vec<(u8, Vec<u8>)> {
    (1..=LAST_TEXT_ROW)
        .map(|number| {
            let mut codes = Vec::with_capacity(COLUMNS);
            codes.push(if flashing.contains(&number) { FLASH } else { STEADY });
            codes.extend(format!("ROW {number:02} THE QUICK BROWN FOX JUMPS OVER").bytes());
            codes.truncate(COLUMNS);
            (number, codes)
        })
        .collect()
}

fn grid_of(decoder: &Decoder<'_>, config: &Config) -> Grid {
    let address = PageAddress::from_mpp(DISPLAY).unwrap();
    let stored = decoder.store().get(address, SubCode::ANY).unwrap();
    let magazine = decoder.magazine_data(address.magazine);
    let options = DisplayOptions::default();
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(stored), &magazine, config, &options);
    let _ = compose_level(&mut grid, stored, &magazine, &decoder.enhancement_lookup(), config, &options);
    grid
}

/// Microseconds per phase change of `step`, the phase alternating between
/// slow flash's on and off: the fastest of `BATCHES` batches of `ROUNDS`.
fn time(mut step: impl FnMut(FlashPhase) -> u32) -> (f64, u32) {
    let mut rows = 0;
    let mut best = f64::MAX;
    for _ in 0..BATCHES {
        let start = Clock::now();
        for round in 0..ROUNDS {
            let unit = if round % 2 == 0 { SLOW_OFF } else { SLOW_ON };
            rows |= step(FlashPhase::at_unit(unit));
        }
        best = best.min(start.elapsed().as_secs_f64() * 1e6 / f64::from(ROUNDS));
    }
    (best, rows)
}

#[test]
#[ignore = "measurement; run in release mode"]
fn a_flashing_page_redrawn_at_each_phase() {
    let config = Config::default();
    let glyphs = GlyphSet::bedstead();
    for flashing in [&[12_u8][..], &[3, 12, 20], &(1..=LAST_TEXT_ROW).collect::<Vec<_>>()] {
        let rows = rows(flashing);
        let borrowed: Vec<(u8, &[u8])> = rows.iter().map(|(number, codes)| (*number, codes.as_slice())).collect();
        let packets = display_page(0, &borrowed, Vec::new());
        Receiver::new().run(&packets, config, |decoder| {
            let mut pixels = vec![0_u8; PAGE_WIDTH * SCREEN_LINES];
            let mut draw = |grid: &Grid, rows: u32, phase: FlashPhase| {
                let options = RenderOptions::resolved(&config, false, phase).with_underline(false);
                let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut pixels, stride: PAGE_WIDTH };
                render_screen(grid, rows, &glyphs, options, &mut buffer).unwrap();
            };
            let kept = grid_of(decoder, &config);
            let (rebuild_all, _) = time(|phase| {
                let grid = grid_of(decoder, &config);
                draw(&grid, ALL_ROWS, phase);
                ALL_ROWS
            });
            let (all, _) = time(|phase| {
                draw(&kept, ALL_ROWS, phase);
                ALL_ROWS
            });
            let mut tracker = ChangeTracker::new();
            let _ = tracker.changed_rows(&kept, RenderOptions::resolved(&config, false, FlashPhase::at_unit(SLOW_ON)));
            let (changed, drawn) = time(|phase| {
                let options = RenderOptions::resolved(&config, false, phase).with_underline(false);
                let rows = tracker.changed_rows(&kept, options);
                draw(&kept, rows, phase);
                rows
            });
            let mut tracker = ChangeTracker::new();
            let _ = tracker.changed_rows(&kept, RenderOptions::resolved(&config, false, FlashPhase::at_unit(SLOW_ON)));
            let (rebuild_changed, _) = time(|phase| {
                let grid = grid_of(decoder, &config);
                let options = RenderOptions::resolved(&config, false, phase).with_underline(false);
                let rows = tracker.changed_rows(&grid, options);
                draw(&grid, rows, phase);
                rows
            });
            let mut tracker = ChangeTracker::new();
            let _ = tracker.changed_rows(&kept, RenderOptions::resolved(&config, false, FlashPhase::at_unit(SLOW_ON)));
            let (tracking, _) = time(|phase| {
                tracker.changed_rows(&kept, RenderOptions::resolved(&config, false, phase).with_underline(false))
            });
            let (rebuild, _) = time(|_| {
                let grid = grid_of(decoder, &config);
                u32::from(grid.boxed_mode)
            });
            let expected = flashing.iter().fold(0_u32, |mask, &row| mask | 1 << row);
            assert_eq!(drawn, expected, "the tracker drew exactly the flashing rows");
            println!(
                "{} flashing rows, per phase change: rebuild + all rows {rebuild_all:.1} µs; all rows {all:.1} µs; \
                 changed rows {changed:.1} µs; rebuild + changed rows {rebuild_changed:.1} µs; \
                 of which the tracker {tracking:.1} µs and the rebuild {rebuild:.1} µs",
                flashing.len()
            );
        });
    }
}
