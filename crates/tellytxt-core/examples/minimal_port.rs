//! A complete minimal port (REQ-ARCH-009): reads T42 packets from standard
//! input, decodes them, and prints page 100 as a block of palette-index
//! characters, using only the platform interface and the row renderer.
//!
//! Run: `cargo run -p tellytxt-core --example minimal_port < capture.t42`

use std::io::Read;
use tellytxt_core::acq::{Decoder, StoredPage};
use tellytxt_core::config::Config;
use tellytxt_core::consts::{CELL_HEIGHT, PACKET_BYTES, PAGE_WIDTH};
use tellytxt_core::glyphs::GlyphSet;
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::level15;
use tellytxt_core::packet::{Packet, PacketMeta};
use tellytxt_core::port::{Clock, DataSource};
use tellytxt_core::render::{Field, IndexBuffer, IndexFormat, RenderOptions, render_row};
use tellytxt_core::types::{Instant, PageAddress, SubCode};

/// The data source: standard input.
struct Stdin(std::io::Stdin);

impl DataSource for Stdin {
    type Error = std::io::Error;
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(buffer)
    }
}

/// The time source: one television line per packet.
struct LineClock(u64);

impl Clock for LineClock {
    fn now(&self) -> Instant {
        Instant(self.0)
    }
}

fn main() -> Result<(), std::io::Error> {
    let mut source = Stdin(std::io::stdin());
    let mut clock = LineClock(0);
    let mut slots = vec![StoredPage::EMPTY; 512];
    let mut decoder = Decoder::new(&mut slots, Config::default());

    // Feed whole 42-byte packets from the source.
    let mut packet = [0u8; PACKET_BYTES];
    let mut filled = 0;
    loop {
        let read = source.read(&mut packet[filled..])?;
        if read == 0 {
            break;
        }
        filled += read;
        if filled == PACKET_BYTES {
            clock.0 += 64;
            decoder.push(&Packet { bytes: packet }, &PacketMeta::default(), clock.now(), &mut |_| {});
            filled = 0;
        }
    }
    decoder.tick(Instant(clock.0 + 1_000_000), &mut |_| {});

    // Build the grid of page 100 and pull its rows from the renderer.
    let Some(address) = PageAddress::from_mpp(0x100) else { return Ok(()) };
    let Some(page) = decoder.store().get(address, SubCode::ANY) else {
        eprintln!("page 100 not found");
        return Ok(());
    };
    let config = Config::default();
    let mut grid = Grid::EMPTY;
    build_grid(&mut grid, Some(page), &decoder.magazine_data(address.magazine), &config, &DisplayOptions::default());
    level15::apply(&mut grid, &page.enhancements, page.enhancements_present, &config, true);

    // The port's buffer: one text row of 480 by 20 palette indices.
    let mut row = vec![0u8; PAGE_WIDTH * CELL_HEIGHT];
    let options = RenderOptions::from_config(&config, false);
    for text_row in 0..25 {
        let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut row, stride: PAGE_WIDTH };
        render_row(&grid, text_row, Field::First, &GlyphSet::bedstead(), options, &mut buffer)
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
        // Show every fourth line and every sixth pixel as a character.
        for line in (0..10).step_by(4) {
            let pixels = &row[line * PAGE_WIDTH..(line + 1) * PAGE_WIDTH];
            let text: String = pixels.iter().step_by(6).map(|&index| if index == 0 { ' ' } else { '#' }).collect();
            println!("{}", text.trim_end());
        }
    }
    Ok(())
}
