//! A small Portable Network Graphics writer (REQ-RND-007; decision 14 of
//! IMPLEMENTATION-LOG.md).
//!
//! It writes indexed-colour images (PNG colour type 3) with the 144-entry
//! teletext palette, or the 32 entries of a Colour Map (REQ-RND-030), and a
//! `tRNS` chunk for transparency. Image data is
//! compressed with fixed-Huffman deflate (RFC 1951 section 3.2.6) using two
//! kinds of match that suit teletext: a run of the previous byte, and the
//! same bytes one row above. Memory use is fixed: two rows of scratch and
//! one output chunk buffer.

use crate::Sink;
use crate::palette::Rgba;
use neoteletext_core::consts::PALETTE_SIZE;

mod deflate;

use deflate::{BitWriter, END_OF_BLOCK, MAX_DISTANCE, MAX_MATCH, MIN_MATCH, emit_match, literal_code};

/// The PNG file signature (PNG specification section 5.2).
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// Bit depth 8, colour type 3 (indexed), compression 0, filter 0, no
/// interlace (section 11.2.2).
const BIT_DEPTH: u8 = 8;
const COLOUR_TYPE_INDEXED: u8 = 3;
/// Filter type 0 (None) at the start of every scanline (section 9).
const FILTER_NONE: u8 = 0;
/// zlib header: deflate with a 32 KiB window, no dictionary, check bits
/// making the 16-bit value a multiple of 31 (RFC 1950 section 2.2).
const ZLIB_HEADER: [u8; 2] = [0x78, 0x01];
/// The Adler-32 modulus (RFC 1950 section 8).
const ADLER_MODULUS: u32 = 65_521;

/// Errors from the PNG writer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PngError<E> {
    /// The sink failed.
    Sink(E),
    /// The scratch buffer is smaller than two rows plus two bytes.
    ScratchTooSmall,
    /// The image is empty, rows are longer than the deflate window, or the
    /// palette is empty or longer than 144 entries.
    BadSize,
}

// Generated during compilation: a bad index stops the build rather than
// panicking at run time.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 == 1 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

/// CRC-32 of the PNG specification (section 5.5, annex D).
static CRC_TABLE: [u32; 256] = crc_table();

fn crc_update(crc: u32, bytes: &[u8]) -> u32 {
    bytes.iter().fold(crc, |crc, &byte| {
        let index = usize::from((crc as u8) ^ byte);
        CRC_TABLE.get(index).copied().unwrap_or(0) ^ (crc >> 8)
    })
}

fn chunk<S: Sink>(sink: &mut S, kind: [u8; 4], data: &[u8]) -> Result<(), PngError<S::Error>> {
    let length = u32::try_from(data.len()).map_err(|_| PngError::BadSize)?;
    sink.write_all(&length.to_be_bytes()).map_err(PngError::Sink)?;
    sink.write_all(&kind).map_err(PngError::Sink)?;
    sink.write_all(data).map_err(PngError::Sink)?;
    let crc = !crc_update(crc_update(u32::MAX, &kind), data);
    sink.write_all(&crc.to_be_bytes()).map_err(PngError::Sink)
}

/// Writes a `width` by `height` indexed image. `row` fills row `y` (width
/// bytes of palette indices). `scratch` needs at least `2 × (width + 1)`
/// bytes. `palette` has 1 to 144 entries, and every index must be below
/// its length.
pub fn write<S: Sink>(
    sink: &mut S,
    width: usize,
    height: usize,
    palette: &[Rgba],
    row: &mut dyn FnMut(usize, &mut [u8]),
    scratch: &mut [u8],
) -> Result<(), PngError<S::Error>> {
    let stream_row = width.checked_add(1).ok_or(PngError::BadSize)?;
    if width == 0 || height == 0 || stream_row > MAX_DISTANCE || palette.is_empty() || palette.len() > PALETTE_SIZE {
        return Err(PngError::BadSize);
    }
    let (previous, current) =
        scratch.get_mut(..stream_row.saturating_mul(2)).ok_or(PngError::ScratchTooSmall)?.split_at_mut(stream_row);
    sink.write_all(&SIGNATURE).map_err(PngError::Sink)?;
    let mut header = [0u8; 13];
    let width_bytes = u32::try_from(width).map_err(|_| PngError::BadSize)?.to_be_bytes();
    let height_bytes = u32::try_from(height).map_err(|_| PngError::BadSize)?.to_be_bytes();
    let fields = width_bytes.into_iter().chain(height_bytes).chain([BIT_DEPTH, COLOUR_TYPE_INDEXED, 0, 0, 0]);
    for (slot, value) in header.iter_mut().zip(fields) {
        *slot = value;
    }
    chunk(sink, *b"IHDR", &header)?;
    let mut plte = [0u8; PALETTE_SIZE * 3];
    let mut trns = [0u8; PALETTE_SIZE];
    for ((colour, rgb), alpha) in palette.iter().zip(plte.chunks_mut(3)).zip(trns.iter_mut()) {
        if let [r, g, b] = rgb {
            (*r, *g, *b) = (colour.r, colour.g, colour.b);
        }
        *alpha = colour.a;
    }
    chunk(sink, *b"PLTE", plte.get(..palette.len().saturating_mul(3)).unwrap_or(&plte))?;
    chunk(sink, *b"tRNS", trns.get(..palette.len()).unwrap_or(&trns))?;

    let mut writer = BitWriter::new();
    for byte in ZLIB_HEADER {
        writer.byte(sink, byte)?;
    }
    // One final block with fixed codes: BFINAL = 1, BTYPE = 01.
    writer.push(sink, 0b011, 3)?;
    let (mut adler_a, mut adler_b) = (1u32, 0u32);
    for y in 0..height {
        if let Some((filter, pixels)) = current.split_first_mut() {
            *filter = FILTER_NONE;
            row(y, pixels);
        }
        for &byte in current.iter() {
            adler_a = adler_a.saturating_add(u32::from(byte)) % ADLER_MODULUS;
            adler_b = adler_b.saturating_add(adler_a) % ADLER_MODULUS;
        }
        // Byte at stream position `j` relative to the start of this row;
        // negative positions are in the row above.
        let byte_at = |j: isize, previous: &[u8], current: &[u8]| -> Option<u8> {
            if j >= 0 {
                current.get(j.unsigned_abs()).copied()
            } else {
                previous.get(stream_row.checked_sub(j.unsigned_abs())?).copied()
            }
        };
        let mut position = 0usize;
        while position < stream_row {
            let i = position;
            let remaining = stream_row.saturating_sub(i).min(MAX_MATCH);
            let mut best = (0usize, 0usize);
            for distance in [1usize, stream_row] {
                let available = if y == 0 { i } else { i.saturating_add(stream_row) };
                if distance > available {
                    continue;
                }
                let mut length = 0usize;
                while length < remaining {
                    let here = isize::try_from(i.saturating_add(length)).unwrap_or(0);
                    let back = here.saturating_sub(isize::try_from(distance).unwrap_or(0));
                    match (byte_at(here, previous, current), byte_at(back, previous, current)) {
                        (Some(a), Some(b)) if a == b => length = length.saturating_add(1),
                        _ => break,
                    }
                }
                if length > best.0 {
                    best = (length, distance);
                }
            }
            if best.0 >= MIN_MATCH {
                emit_match(&mut writer, sink, best.0, best.1)?;
                position = position.saturating_add(best.0);
            } else {
                let (code, bits) = literal_code(u16::from(current.get(i).copied().unwrap_or(0)));
                writer.code(sink, code, bits)?;
                position = position.saturating_add(1);
            }
        }
        previous.copy_from_slice(current);
    }
    let (code, bits) = literal_code(END_OF_BLOCK);
    writer.code(sink, code, bits)?;
    writer.align(sink)?;
    let adler = (adler_b << 16) | adler_a;
    for byte in adler.to_be_bytes() {
        writer.byte(sink, byte)?;
    }
    writer.flush(sink)?;
    chunk(sink, *b"IEND", &[])
}

#[cfg(test)]
mod tests;
