//! The decoder's side of the Opus oracle (`tests/oracles/run_opus.sh`, decision E.123): decodes
//! an `opus_demo` stream as the oracle's C program decodes it with libopus, and writes the same
//! two files, so that the oracle compares them byte for byte.
//!
//! The stream is a sequence of packets, each a 4-byte big-endian length, a 4-byte big-endian
//! final range (the encoder's, which neither side reads) and the payload; a length of 0 is a lost
//! packet. The samples are written as 16-bit little-endian, one 20 ms frame a packet, and the
//! decoder's final range after each packet as 8 hexadecimal digits a line (0 after a concealed
//! frame).
//!
//! Usage: `decode_bit IN.bit OUT.pcm OUT.ranges`

use neoteletext_opus::{Decoder, FRAME_SAMPLES};
use std::fs;
use std::io::{BufWriter, Write};
use std::process::ExitCode;

/// The bytes before each payload: its length and the encoder's final range.
const HEADER: usize = 8;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, input, pcm_path, ranges_path] = arguments.as_slice() else {
        eprintln!("usage: decode_bit IN.bit OUT.pcm OUT.ranges");
        return ExitCode::from(2);
    };
    match run(input, pcm_path, ranges_path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("decode_bit: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &str, pcm_path: &str, ranges_path: &str) -> Result<(), String> {
    let stream = fs::read(input).map_err(|e| format!("{input}: {e}"))?;
    let mut pcm_out = BufWriter::new(fs::File::create(pcm_path).map_err(|e| format!("{pcm_path}: {e}"))?);
    let mut ranges_out = BufWriter::new(fs::File::create(ranges_path).map_err(|e| format!("{ranges_path}: {e}"))?);
    let mut decoder = Decoder::new();
    let mut pcm = [0i16; FRAME_SAMPLES];
    let mut rest = stream.as_slice();
    let mut index = 0usize;
    while let Some((header, after)) = rest.split_first_chunk::<HEADER>() {
        let length = usize::try_from(u32::from_be_bytes([header[0], header[1], header[2], header[3]]))
            .map_err(|_| format!("packet {index}: length does not fit"))?;
        let (packet, after) = after.split_at_checked(length).ok_or_else(|| format!("packet {index}: truncated"))?;
        rest = after;
        if packet.is_empty() {
            decoder.conceal(&mut pcm);
        } else if let Err(error) = decoder.decode(packet, &mut pcm) {
            return Err(format!("packet {index} of {length} bytes: {}", describe(error)));
        }
        for sample in pcm {
            pcm_out.write_all(&sample.to_le_bytes()).map_err(|e| e.to_string())?;
        }
        writeln!(ranges_out, "{:08x}", decoder.final_range()).map_err(|e| e.to_string())?;
        index += 1;
    }
    if !rest.is_empty() {
        return Err(format!("{} bytes after the last packet", rest.len()));
    }
    pcm_out.flush().map_err(|e| e.to_string())?;
    ranges_out.flush().map_err(|e| e.to_string())
}

fn describe(error: neoteletext_opus::DecodeError) -> &'static str {
    use neoteletext_opus::{DecodeError, PacketError};
    match error {
        DecodeError::Corrupt => "corrupt",
        DecodeError::Packet(PacketError::Empty) => "empty",
        DecodeError::Packet(PacketError::TooLong) => "longer than the profile allows",
        DecodeError::Packet(PacketError::Stereo) => "stereo",
        DecodeError::Packet(PacketError::SeveralFrames) => "several frames",
        DecodeError::Packet(PacketError::NotTwentyMs) => "not 20 ms",
        DecodeError::Packet(PacketError::Malformed) => "malformed",
    }
}
