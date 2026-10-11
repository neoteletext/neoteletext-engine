//! The software data slicer on arbitrary sample data, for each capture
//! geometry (REQ-TST-040, REQ-SLC-001).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_slicer::{Config, Preset, RawGeometry, RawReader};

const PRESETS: [Preset; 5] = [Preset::Bt8x8, Preset::Cx88, Preset::Saa7131, Preset::Tbc, Preset::TbcVbi];

fuzz_target!(|data: &[u8]| {
    let [selector, piece_high, body @ ..] = data else { return };
    let preset = PRESETS[usize::from(*selector) % PRESETS.len()];
    let Ok(mut reader) = RawReader::new(RawGeometry::preset(preset), Config::default()) else { return };
    // Pieces up to 64 KiB, so that lines and fields straddle feeds.
    let piece = (usize::from(*piece_high) << 8).max(1);
    for chunk in body.chunks(piece) {
        reader.feed(chunk, |packet, meta| {
            core::hint::black_box((packet, meta));
        });
    }
});
