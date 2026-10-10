//! A small Portable Network Graphics writer (REQ-RND-007; decision 14 of
//! IMPLEMENTATION-LOG.md).
//!
//! It writes indexed-colour images (PNG colour type 3) with the 144-entry
//! teletext palette and a `tRNS` chunk for transparency. Image data is
//! compressed with fixed-Huffman deflate (RFC 1951 section 3.2.6) using two
//! kinds of match that suit teletext: a run of the previous byte, and the
//! same bytes one row above. Memory use is fixed: two rows of scratch and
//! one output chunk buffer.

use crate::Sink;
use crate::palette::Rgba;
use tellytxt_core::consts::PALETTE_SIZE;

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
/// Bytes of compressed data buffered before an IDAT chunk is written.
const CHUNK_BYTES: usize = 8192;
/// Shortest and longest deflate matches, and the longest distance.
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const MAX_DISTANCE: usize = 32_768;
/// The Adler-32 modulus (RFC 1950 section 8).
const ADLER_MODULUS: u32 = 65_521;

/// Errors from the PNG writer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PngError<E> {
    /// The sink failed.
    Sink(E),
    /// The scratch buffer is smaller than two rows plus two bytes.
    ScratchTooSmall,
    /// The image is empty or rows are longer than the deflate window.
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

/// Bits written least significant first, flushed into IDAT chunks.
struct BitWriter {
    buffer: [u8; CHUNK_BYTES],
    length: usize,
    accumulator: u64,
    bits: u32,
}

impl BitWriter {
    fn push<S: Sink>(&mut self, sink: &mut S, value: u32, count: u32) -> Result<(), PngError<S::Error>> {
        self.accumulator |= u64::from(value) << self.bits;
        self.bits = self.bits.saturating_add(count);
        while self.bits >= 8 {
            self.byte(sink, self.accumulator as u8)?;
            self.accumulator >>= 8;
            self.bits = self.bits.saturating_sub(8);
        }
        Ok(())
    }

    /// A Huffman code, which deflate writes most significant bit first.
    fn code<S: Sink>(&mut self, sink: &mut S, code: u32, length: u32) -> Result<(), PngError<S::Error>> {
        let reversed = code.reverse_bits() >> (32u32.saturating_sub(length));
        self.push(sink, reversed, length)
    }

    fn byte<S: Sink>(&mut self, sink: &mut S, byte: u8) -> Result<(), PngError<S::Error>> {
        if self.length >= CHUNK_BYTES {
            self.flush(sink)?;
        }
        if let Some(slot) = self.buffer.get_mut(self.length) {
            *slot = byte;
            self.length = self.length.saturating_add(1);
        }
        Ok(())
    }

    fn align<S: Sink>(&mut self, sink: &mut S) -> Result<(), PngError<S::Error>> {
        if self.bits > 0 {
            self.push(sink, 0, 8u32.saturating_sub(self.bits))?;
        }
        Ok(())
    }

    fn flush<S: Sink>(&mut self, sink: &mut S) -> Result<(), PngError<S::Error>> {
        if self.length > 0 {
            chunk(sink, *b"IDAT", self.buffer.get(..self.length).unwrap_or(&[]))?;
            self.length = 0;
        }
        Ok(())
    }
}

/// The fixed Huffman code of a literal or length symbol (RFC 1951 3.2.6).
fn literal_code(symbol: u16) -> (u32, u32) {
    let symbol = u32::from(symbol);
    match symbol {
        0..=143 => (0x30_u32.saturating_add(symbol), 8),
        144..=255 => (0x190_u32.saturating_add(symbol.saturating_sub(144)), 9),
        256..=279 => (symbol.saturating_sub(256), 7),
        _ => (0xC0_u32.saturating_add(symbol.saturating_sub(280)), 8),
    }
}

/// Length symbols 257 to 285: base length and extra bits (RFC 1951 3.2.5).
const LENGTH_BASE: [(u16, u32); 29] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 1),
    (13, 1),
    (15, 1),
    (17, 1),
    (19, 2),
    (23, 2),
    (27, 2),
    (31, 2),
    (35, 3),
    (43, 3),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 4),
    (115, 4),
    (131, 5),
    (163, 5),
    (195, 5),
    (227, 5),
    (258, 0),
];
/// Distance codes 0 to 29: base distance and extra bits.
const DISTANCE_BASE: [(u32, u32); 30] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 1),
    (7, 1),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 4),
    (49, 4),
    (65, 5),
    (97, 5),
    (129, 6),
    (193, 6),
    (257, 7),
    (385, 7),
    (513, 8),
    (769, 8),
    (1025, 9),
    (1537, 9),
    (2049, 10),
    (3073, 10),
    (4097, 11),
    (6145, 11),
    (8193, 12),
    (12289, 12),
    (16385, 13),
    (24577, 13),
];
/// The first length symbol.
const LENGTH_SYMBOL_BASE: u16 = 257;
/// The end-of-block symbol.
const END_OF_BLOCK: u16 = 256;
/// Bits of a fixed distance code.
const DISTANCE_CODE_BITS: u32 = 5;

fn emit_match<S: Sink>(
    writer: &mut BitWriter,
    sink: &mut S,
    length: usize,
    distance: usize,
) -> Result<(), PngError<S::Error>> {
    let length = u16::try_from(length).unwrap_or(0);
    let index = LENGTH_BASE.iter().rposition(|&(base, _)| base <= length).unwrap_or(0);
    let (base, extra) = LENGTH_BASE.get(index).copied().unwrap_or((3, 0));
    let symbol = LENGTH_SYMBOL_BASE.saturating_add(index as u16);
    let (code, bits) = literal_code(symbol);
    writer.code(sink, code, bits)?;
    writer.push(sink, u32::from(length.saturating_sub(base)), extra)?;
    let distance = u32::try_from(distance).unwrap_or(1);
    let index = DISTANCE_BASE.iter().rposition(|&(base, _)| base <= distance).unwrap_or(0);
    let (base, extra) = DISTANCE_BASE.get(index).copied().unwrap_or((1, 0));
    writer.code(sink, index as u32, DISTANCE_CODE_BITS)?;
    writer.push(sink, distance.saturating_sub(base), extra)
}

/// Writes a `width` by `height` indexed image. `row` fills row `y` (width
/// bytes of palette indices). `scratch` needs at least `2 × (width + 1)`
/// bytes.
pub fn write<S: Sink>(
    sink: &mut S,
    width: usize,
    height: usize,
    palette: &[Rgba; PALETTE_SIZE],
    row: &mut dyn FnMut(usize, &mut [u8]),
    scratch: &mut [u8],
) -> Result<(), PngError<S::Error>> {
    let stream_row = width.checked_add(1).ok_or(PngError::BadSize)?;
    if width == 0 || height == 0 || stream_row > MAX_DISTANCE {
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
    chunk(sink, *b"PLTE", &plte)?;
    chunk(sink, *b"tRNS", &trns)?;

    let mut writer = BitWriter { buffer: [0; CHUNK_BYTES], length: 0, accumulator: 0, bits: 0 };
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
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::comparison_chain)]
mod tests {
    use super::*;
    use crate::palette::{CLUT0, palette};
    use std::vec;
    use std::vec::Vec;

    /// A minimal inflate for fixed-Huffman blocks, to check the encoder
    /// independently of it.
    fn inflate_fixed(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut bit = 0usize;
        let read = |count: u32, bit: &mut usize| -> u32 {
            let mut value = 0;
            for k in 0..count {
                let b = (data[*bit / 8] >> (*bit % 8)) & 1;
                value |= u32::from(b) << k;
                *bit += 1;
            }
            value
        };
        let read_code = |bit: &mut usize, count: u32| -> u32 {
            let mut value = 0;
            for _ in 0..count {
                value = (value << 1) | u32::from((data[*bit / 8] >> (*bit % 8)) & 1);
                *bit += 1;
            }
            value
        };
        assert_eq!(read(3, &mut bit), 0b011);
        loop {
            let mut code = read_code(&mut bit, 7);
            let symbol = if code <= 0x17 {
                code + 256
            } else {
                code = (code << 1) | read_code(&mut bit, 1);
                if (0x30..=0xBF).contains(&code) {
                    code - 0x30
                } else if (0xC0..=0xC7).contains(&code) {
                    code - 0xC0 + 280
                } else {
                    code = (code << 1) | read_code(&mut bit, 1);
                    code - 0x190 + 144
                }
            };
            if symbol < 256 {
                out.push(symbol as u8);
            } else if symbol == 256 {
                break;
            } else {
                let (base, extra) = LENGTH_BASE[(symbol - 257) as usize];
                let length = usize::from(base) + read(extra, &mut bit) as usize;
                let dcode = read_code(&mut bit, 5) as usize;
                let (dbase, dextra) = DISTANCE_BASE[dcode];
                let distance = (dbase + read(dextra, &mut bit)) as usize;
                for _ in 0..length {
                    out.push(out[out.len() - distance]);
                }
            }
        }
        out
    }

    #[test]
    fn round_trip_through_an_independent_inflate() {
        let (width, height) = (37usize, 9usize);
        let image: Vec<u8> =
            (0..width * height).map(|i| if (i / width) % 3 == 0 { 7 } else { (i % 5) as u8 }).collect();
        let mut sink = VecSink(Vec::new());
        let mut scratch = vec![0u8; 2 * (width + 1)];
        let colours = palette(&CLUT0, Rgba::TRANSPARENT, true, false);
        write(
            &mut sink,
            width,
            height,
            &colours,
            &mut |y, row: &mut [u8]| row.copy_from_slice(&image[y * width..(y + 1) * width]),
            &mut scratch,
        )
        .unwrap();
        let out = sink.0;
        assert_eq!(&out[..8], &SIGNATURE);
        // Gather IDAT data.
        let mut position = 8;
        let mut zlib = Vec::new();
        while position < out.len() {
            let length = u32::from_be_bytes(out[position..position + 4].try_into().unwrap()) as usize;
            let kind = &out[position + 4..position + 8];
            let body = &out[position + 8..position + 8 + length];
            let crc = u32::from_be_bytes(out[position + 8 + length..position + 12 + length].try_into().unwrap());
            assert_eq!(crc, !crc_update(crc_update(u32::MAX, kind), body));
            if kind == b"IDAT" {
                zlib.extend_from_slice(body);
            }
            position += 12 + length;
        }
        assert_eq!(&zlib[..2], &ZLIB_HEADER);
        let raw = inflate_fixed(&zlib[2..zlib.len() - 4]);
        let mut expected = Vec::new();
        for y in 0..height {
            expected.push(0);
            expected.extend_from_slice(&image[y * width..(y + 1) * width]);
        }
        assert_eq!(raw, expected);
    }

    struct VecSink(Vec<u8>);

    impl Sink for VecSink {
        type Error = ();
        fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }
}
