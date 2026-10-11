//! Raw sampled video to T42 through the software data slicer (REQ-INP-011,
//! area SLC).

use crate::input::{self, CHUNK, Options};
use crate::{Failure, exit};
use neoteletext_slicer::{Counters, Preset, RawGeometry, RawReader};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Slices the input, a raw capture of the geometry `preset` names, and
/// writes the packets found to `output` as T42. Returns the slicer's
/// quality statistics.
pub fn slice(options: &Options, output: &Path, preset: &str) -> Result<Counters, Failure> {
    let preset = match preset {
        "bt8x8" => Preset::Bt8x8,
        "cx88" => Preset::Cx88,
        "saa7131" => Preset::Saa7131,
        "tbc" => Preset::Tbc,
        "tbc-vbi" => Preset::TbcVbi,
        other => return Err((exit::USAGE, format!("unknown preset {other:?} (bt8x8, cx88, saa7131, tbc, tbc-vbi)"))),
    };
    let mut config = neoteletext_slicer::Config::default();
    config.framing_tolerance = input::config(options)?.framing_tolerance;
    let mut raw =
        RawReader::new(RawGeometry::preset(preset), config).map_err(|error| (exit::USAGE, format!("{error:?}")))?;
    let mut input = input::open(options)?;
    let file = File::create(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    let mut writer = neoteletext_formats::write::T42Writer::new(std::io::BufWriter::new(file));
    let mut buffer = vec![0u8; CHUNK];
    let mut failure = None;
    loop {
        let read = input.read(&mut buffer).map_err(|error| (exit::INPUT, error.to_string()))?;
        if read == 0 {
            break;
        }
        raw.feed(&buffer[..read], |packet, _meta| {
            if let Err(error) = writer.write_packet(&packet) {
                failure = Some(error);
            }
        });
    }
    if let Some(error) = failure {
        return Err((exit::INPUT, error.to_string()));
    }
    writer.into_inner().flush().map_err(|error| (exit::INPUT, error.to_string()))?;
    Ok(raw.slicer().counters())
}
