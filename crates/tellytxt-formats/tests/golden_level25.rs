//! Golden rasters of the authored Level 2.5 and 3.5 stream (REQ-TST-024,
//! REQ-TST-030): every display page of `author::streams` is encoded,
//! decoded with an enhancement store, built and enhanced at Levels 2.5 and
//! 3.5, and rendered in the resolved colour mode (REQ-RND-031) in both cell
//! sizes; the checksum of the Colour Map indices must match
//! `tests/golden/authored-level25.txt`.
//!
//! The stream's pages are kept as TTI source in `tests/pages/level25/`, one
//! file per page address, as REQ-TST-024 asks; a second test checks that
//! the builder of REQ-INP-025 still writes exactly those files. Each page
//! was checked by eye, and cell by cell against zvbi where zvbi decodes it
//! correctly (`tests/oracles/run_cells.sh`, REQ-TST-023), before its
//! checksum was taken.
//!
//! The file carries a format version. A deliberate change to the stream,
//! the enhancement processor or the renderer raises the version and
//! regenerates the file in the same change, with the reason in the commit:
//! run with `TELLYTXT_UPDATE_GOLDEN=1`.
#![cfg(feature = "std")]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use std::fmt::Write as _;
use std::path::PathBuf;
use tellytxt_core::acq::{Decoder, EnhancementPacket, EnhancementStore, EnhancementSubPage, StoredPage};
use tellytxt_core::compose::compose_level;
use tellytxt_core::config::{CellMode, Config, PresentationLevel};
use tellytxt_core::consts::{ENHANCEMENT_PACKETS_LEVEL_35, MAGAZINES, MOT_PACKETS, ROWS};
use tellytxt_core::glyphs::GlyphSet;
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::packet::PacketMeta;
use tellytxt_core::render::{
    ALL_ROWS, ColourMode, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, render_screen, row_width,
};
use tellytxt_core::types::{Instant, SubCode};
use tellytxt_formats::author::streams;

/// Raise when the stream, enhancement or rendering changes on purpose
/// (REQ-TST-030).
const FORMAT_VERSION: u32 = 3;
/// One line period between packets, as the command-line program uses.
const LINE_US: u64 = 64;
/// The flash units of decision E.48 a still page is drawn at: the start of
/// the cycle, and half a cycle on, where every slow flash is off.
const STILL_UNITS: [u8; 2] = [0, 6];
/// Every unit of the flash cycle, for the flash page.
const ALL_UNITS: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
/// The flash page of the stream.
const FLASH_PAGE: u16 = 0x110;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3))
}

fn checksums() -> String {
    let stream = streams::stream().unwrap();
    let packets = stream.packets();
    let mut out = format!("format {FORMAT_VERSION}\n");
    let cases = [
        ("2.5", PresentationLevel::Level25, "12x20", CellMode::Saa5050),
        ("3.5", PresentationLevel::Level35, "12x20", CellMode::Saa5050),
        ("3.5", PresentationLevel::Level35, "12x10", CellMode::Saa5246a),
    ];
    for (level_name, level, size, cell_mode) in cases {
        let config = Config { level, cell_mode, rounding: true, ..Config::default() };
        let mut pages = vec![StoredPage::EMPTY; 256];
        let mut sub_pages = vec![EnhancementSubPage::EMPTY; 256];
        let mut pool = vec![EnhancementPacket::EMPTY; ENHANCEMENT_PACKETS_LEVEL_35 + MAGAZINES * MOT_PACKETS];
        let mut decoder =
            Decoder::with_enhancement_store(&mut pages, EnhancementStore::new(&mut sub_pages, &mut pool), config);
        let mut now = 0;
        for packet in &packets {
            now += LINE_US;
            decoder.push(packet, &PacketMeta::default(), Instant(now), &mut |_| {});
        }
        decoder.tick(Instant(now + 1_000_000), &mut |_| {});
        let lookup = decoder.enhancement_lookup();
        for address in streams::display_pages().unwrap() {
            let page = decoder.store().get(address, SubCode::ANY).unwrap();
            let magazine = decoder.magazine_data(address.magazine);
            let mut grid: Box<Grid> = Box::new(Grid::empty());
            build_grid(&mut grid, Some(page), &magazine, &config, &DisplayOptions::default());
            let _ = compose_level(&mut grid, page, &magazine, &lookup, &config, &DisplayOptions::default());
            let mpp = String::from_utf8_lossy(&address.ascii()).into_owned();
            let units: &[u8] = if address == tellytxt_core::types::PageAddress::from_mpp(FLASH_PAGE).unwrap() {
                &ALL_UNITS
            } else {
                &STILL_UNITS
            };
            for &unit in units {
                for reveal in [false, true] {
                    let options = RenderOptions {
                        mode: config.cell_mode,
                        rounding: config.rounding,
                        reveal,
                        colour: ColourMode::Resolved(FlashPhase::at_unit(unit)),
                    };
                    let width = row_width(&grid);
                    let height = ROWS * options.row_height();
                    let mut indices = vec![0u8; width * height];
                    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut indices, stride: width };
                    render_screen(&grid, ALL_ROWS, &GlyphSet::original(), options, &mut buffer).unwrap();
                    let _ = writeln!(
                        out,
                        "{mpp} level={level_name} {size} unit={unit} reveal={reveal} {width}x{height} {:016x}",
                        fnv(&indices)
                    );
                }
            }
        }
    }
    out
}

#[test]
fn authored_golden_rasters_match() {
    let path = root().join("tests/golden/authored-level25.txt");
    let actual = checksums();
    if std::env::var_os("TELLYTXT_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .expect("tests/golden/authored-level25.txt; generate with TELLYTXT_UPDATE_GOLDEN=1");
    assert_eq!(actual, expected, "authored rendering changed: if deliberate, raise FORMAT_VERSION and regenerate");
}

/// REQ-TST-024: the TTI source of the stream in the repository is what the
/// builder writes. With `TELLYTXT_UPDATE_GOLDEN=1` it is written again.
#[test]
fn authored_tti_source_is_current() {
    let directory = root().join("tests/pages/level25");
    let files = streams::stream().unwrap().tti_files().unwrap();
    if std::env::var_os("TELLYTXT_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(&directory).unwrap();
        for (name, text) in &files {
            std::fs::write(directory.join(name), text).unwrap();
        }
    }
    let mut stored: Vec<String> =
        std::fs::read_dir(&directory).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    stored.sort();
    let mut names: Vec<String> = files.iter().map(|(name, _)| name.clone()).collect();
    names.sort();
    assert_eq!(stored, names, "tests/pages/level25 holds other files than the builder writes");
    for (name, text) in &files {
        let on_disk = std::fs::read(directory.join(name)).unwrap();
        assert!(
            on_disk == text.as_slice(),
            "{name} differs from the builder's: regenerate with TELLYTXT_UPDATE_GOLDEN=1"
        );
    }
}
