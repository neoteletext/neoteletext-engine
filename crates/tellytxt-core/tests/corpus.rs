//! Decoding real broadcast captures from the local corpus (REQ-TST-022).
//! Skips, with a note, when `TELLYTXT_CORPUS` is not set or the file is
//! missing.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use std::path::PathBuf;
use tellytxt_core::acq::{Decoder, StoredPage};
use tellytxt_core::charset::to_unicode;
use tellytxt_core::config::Config;
use tellytxt_core::consts::PACKET_BYTES;
use tellytxt_core::fsm::PageState;
use tellytxt_core::level1::{DisplayOptions, Glyph, Grid, build_grid};
use tellytxt_core::packet::{Packet, PacketMeta};
use tellytxt_core::types::{Instant, PageAddress, SubCode};

fn corpus(name: &str) -> Option<Vec<u8>> {
    let Some(directory) = std::env::var_os("TELLYTXT_CORPUS").map(PathBuf::from) else {
        eprintln!("SKIPPED: TELLYTXT_CORPUS is not set; corpus test for {name} not run");
        return None;
    };
    let bytes = std::fs::read(directory.join(name)).ok();
    if bytes.is_none() {
        eprintln!("SKIPPED: corpus file {name} not found");
    }
    bytes
}

fn row_text(grid: &Grid, row: usize) -> String {
    grid.cells[row]
        .iter()
        .map(|cell| match cell.glyph {
            Glyph::Char(c) => to_unicode(c).base,
            Glyph::Mosaic { .. } => '#',
            Glyph::Space => ' ',
        })
        .collect()
}

#[test]
fn bbc1_1991_decodes() {
    let Some(bytes) = corpus("dan/bbc1_1991-10-31_Dan/bbc1_1991-10-31_v2_squashed.t42") else { return };
    let mut slots = vec![StoredPage::EMPTY; 4096];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    for (index, chunk) in bytes.as_chunks::<PACKET_BYTES>().0.iter().enumerate() {
        let packet = Packet { bytes: *chunk };
        decoder.push(&packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    decoder.tick(Instant(u64::MAX / 2), &mut |_| {});
    let stored = decoder.store().pages().iter().filter(|p| p.state != PageState::Empty).count();
    let stats = decoder.statistics();
    eprintln!("packets per magazine {:?}, pages stored {stored}, address failures {}, parity failures {}", stats.per_magazine, stats.address_failures, stats.parity_failures);
    let address = PageAddress::from_mpp(0x100).unwrap();
    let page = decoder.store().get(address, SubCode::ANY).expect("page 100");
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(page), &decoder.magazine_data(address.magazine), &Config::default(), &DisplayOptions::default());
    for row in 0..25 {
        eprintln!("{:2} |{}|", row, row_text(&grid, row));
    }
    assert!(stored > 100, "a full Ceefax service has hundreds of pages");
    assert!(row_text(&grid, 0).contains("CEEFAX"), "page 100 header names the service");
}
