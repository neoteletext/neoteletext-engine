//! Golden raster checksums (REQ-TST-030). The project's own test card is
//! read as TTI, encoded to packets, decoded, built into a grid and rendered
//! in each cell mode; the checksum of the palette indices must match
//! `tests/golden/raster.txt`.
//!
//! The file carries a format version. A deliberate rendering change raises
//! the version and regenerates the file in the same change, with the reason
//! in the commit: run with `TELLYTXT_UPDATE_GOLDEN=1`.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use std::fmt::Write as _;
use std::path::PathBuf;
use tellytxt_core::acq::{Decoder, StoredPage};
use tellytxt_core::config::{CellMode, Config};
use tellytxt_core::consts::PAGE_WIDTH;
use tellytxt_core::glyphs::GlyphSet;
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::level15;
use tellytxt_core::packet::PacketMeta;
use tellytxt_core::render::{ALL_ROWS, IndexBuffer, IndexFormat, RenderOptions, render_screen};
use tellytxt_core::types::{Instant, PageAddress, SubCode};
use tellytxt_formats::Item;
use tellytxt_formats::encode::encode_page;
use tellytxt_formats::item::Reader;
use tellytxt_formats::tti::TtiReader;

/// Raise when rendering changes on purpose (REQ-TST-030).
const FORMAT_VERSION: u32 = 1;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3))
}

fn grid_of(file: &str, config: &Config) -> Grid {
    let text = std::fs::read(root().join(file)).unwrap();
    let mut reader = TtiReader::default();
    let mut packets = Vec::new();
    let mut collect = |item: Item<'_>| {
        if let Item::Page(page) = item {
            encode_page(page, &mut |packet| packets.push(packet));
        }
    };
    reader.feed(&text, &mut collect);
    reader.finish(&mut collect);
    let mut slots = vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, *config);
    for (index, packet) in packets.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    decoder.tick(Instant(10_000_000), &mut |_| {});
    let address = PageAddress::from_mpp(0x199).unwrap();
    let page = decoder.store().get(address, SubCode::ANY).unwrap();
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(page), &decoder.magazine_data(address.magazine), config, &DisplayOptions::default());
    level15::apply(&mut grid, &page.enhancements, page.enhancements_present, config, true);
    grid
}

fn checksums() -> String {
    let mut out = format!("format {FORMAT_VERSION}\n");
    let modes = [
        ("12x20-rounded", CellMode::Saa5050, true),
        ("12x20-unrounded", CellMode::Saa5050, false),
        ("12x10", CellMode::Saa5246a, false),
    ];
    for reveal in [false, true] {
        for (name, mode, rounding) in modes {
            let mut config = Config::default();
            config.cell_mode = mode;
            config.rounding = rounding;
            let grid = grid_of("tests/pages/testcard.tti", &config);
            let options = RenderOptions::from_config(&config, reveal);
            let mut pixels = vec![0u8; PAGE_WIDTH * 25 * options.row_height()];
            let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut pixels, stride: PAGE_WIDTH };
            render_screen(&grid, ALL_ROWS, &GlyphSet::bedstead(), options, &mut buffer).unwrap();
            let _ = writeln!(out, "testcard {name} reveal={reveal} {:016x}", fnv(&pixels));
        }
    }
    out
}

#[test]
fn golden_rasters_match() {
    let path = root().join("tests/golden/raster.txt");
    let actual = checksums();
    if std::env::var_os("TELLYTXT_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).expect("tests/golden/raster.txt; generate with TELLYTXT_UPDATE_GOLDEN=1");
    assert_eq!(actual, expected, "rendering changed: if deliberate, raise FORMAT_VERSION and regenerate");
}
