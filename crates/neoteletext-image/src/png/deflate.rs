//! The deflate encoder of the PNG writer (RFC 1951): a bit writer that
//! flushes into IDAT chunks, the fixed Huffman codes of section 3.2.6, and
//! the length and distance codes of section 3.2.5.

use crate::Sink;

use super::{PngError, chunk};
/// Bytes of compressed data buffered before an IDAT chunk is written.
const CHUNK_BYTES: usize = 8192;
/// Shortest and longest deflate matches, and the longest distance.
pub(super) const MIN_MATCH: usize = 3;
pub(super) const MAX_MATCH: usize = 258;
pub(super) const MAX_DISTANCE: usize = 32_768;

/// Bits written least significant first, flushed into IDAT chunks.
pub(super) struct BitWriter {
    buffer: [u8; CHUNK_BYTES],
    length: usize,
    accumulator: u64,
    bits: u32,
}

impl BitWriter {
    /// An empty writer.
    pub(super) const fn new() -> Self {
        Self { buffer: [0; CHUNK_BYTES], length: 0, accumulator: 0, bits: 0 }
    }

    pub(super) fn push<S: Sink>(&mut self, sink: &mut S, value: u32, count: u32) -> Result<(), PngError<S::Error>> {
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
    pub(super) fn code<S: Sink>(&mut self, sink: &mut S, code: u32, length: u32) -> Result<(), PngError<S::Error>> {
        let reversed = code.reverse_bits() >> (32u32.saturating_sub(length));
        self.push(sink, reversed, length)
    }

    pub(super) fn byte<S: Sink>(&mut self, sink: &mut S, byte: u8) -> Result<(), PngError<S::Error>> {
        if self.length >= CHUNK_BYTES {
            self.flush(sink)?;
        }
        if let Some(slot) = self.buffer.get_mut(self.length) {
            *slot = byte;
            self.length = self.length.saturating_add(1);
        }
        Ok(())
    }

    pub(super) fn align<S: Sink>(&mut self, sink: &mut S) -> Result<(), PngError<S::Error>> {
        if self.bits > 0 {
            self.push(sink, 0, 8u32.saturating_sub(self.bits))?;
        }
        Ok(())
    }

    pub(super) fn flush<S: Sink>(&mut self, sink: &mut S) -> Result<(), PngError<S::Error>> {
        if self.length > 0 {
            chunk(sink, *b"IDAT", self.buffer.get(..self.length).unwrap_or(&[]))?;
            self.length = 0;
        }
        Ok(())
    }
}

/// The fixed Huffman code of a literal or length symbol (RFC 1951 3.2.6).
pub(super) fn literal_code(symbol: u16) -> (u32, u32) {
    let symbol = u32::from(symbol);
    match symbol {
        0..=143 => (0x30_u32.saturating_add(symbol), 8),
        144..=255 => (0x190_u32.saturating_add(symbol.saturating_sub(144)), 9),
        256..=279 => (symbol.saturating_sub(256), 7),
        _ => (0xC0_u32.saturating_add(symbol.saturating_sub(280)), 8),
    }
}

/// Length symbols 257 to 285: base length and extra bits (RFC 1951 3.2.5).
pub(super) const LENGTH_BASE: [(u16, u32); 29] = [
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
pub(super) const DISTANCE_BASE: [(u32, u32); 30] = [
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
pub(super) const END_OF_BLOCK: u16 = 256;
/// Bits of a fixed distance code.
const DISTANCE_CODE_BITS: u32 = 5;

pub(super) fn emit_match<S: Sink>(
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
