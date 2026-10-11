//! Headerless packet streams: T42 (REQ-INP-002), the 43-byte form with the
//! framing code and the 45-byte form with clock run-in (REQ-INP-003), and
//! bit-reversed T42 (REQ-INP-004).

use neoteletext_core::consts::{
    CLOCK_RUN_IN, FRAMING_CODE, PACKET_BYTES, PACKET_BYTES_WITH_FRAMING, PACKET_BYTES_WITH_RUN_IN,
};
use neoteletext_core::ecc::reverse_bits;
use neoteletext_core::packet::{Packet, PacketMeta};

use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of, saturate_u32};
use crate::text::copy_prefix;

/// The leading bytes of the 45-byte form, in transmission order.
const RUN_IN_PREFIX: [u8; 3] = [CLOCK_RUN_IN, CLOCK_RUN_IN, FRAMING_CODE];
/// The leading byte of the 43-byte form.
const FRAMING_PREFIX: [u8; 1] = [FRAMING_CODE];

/// The packet stream layouts.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum T42Variant {
    /// 42-byte packets in transmission bit order (REQ-INP-002).
    Plain,
    /// 42-byte packets with every byte bit-reversed (REQ-INP-004).
    Reversed,
    /// 43-byte packets beginning with the framing code (REQ-INP-003).
    Framed,
    /// 45-byte packets beginning with the clock run-in and framing code
    /// (REQ-INP-003).
    RunIn,
}

impl T42Variant {
    /// Every variant, in the order detection prefers on a tie.
    pub const ALL: [Self; 4] = [Self::Plain, Self::Reversed, Self::Framed, Self::RunIn];

    /// Bytes per packet in the stream.
    #[must_use]
    pub const fn unit_bytes(self) -> usize {
        match self {
            Self::Plain | Self::Reversed => PACKET_BYTES,
            Self::Framed => PACKET_BYTES_WITH_FRAMING,
            Self::RunIn => PACKET_BYTES_WITH_RUN_IN,
        }
    }

    /// The bytes that must lead each packet, which the reader checks and
    /// removes.
    #[must_use]
    pub const fn prefix(self) -> &'static [u8] {
        match self {
            Self::Plain | Self::Reversed => &[],
            Self::Framed => &FRAMING_PREFIX,
            Self::RunIn => &RUN_IN_PREFIX,
        }
    }

    /// Converts one whole unit to a packet, or `None` when its leading bytes
    /// are wrong or it is the wrong length.
    #[must_use]
    pub fn packet(self, unit: &[u8]) -> Option<Packet> {
        if unit.len() != self.unit_bytes() || !unit.starts_with(self.prefix()) {
            return None;
        }
        let body = unit.get(self.prefix().len()..)?;
        let mut bytes = [0; PACKET_BYTES];
        // The length check above makes the body exactly one packet long.
        copy_prefix(&mut bytes, body);
        if self == Self::Reversed {
            for byte in &mut bytes {
                *byte = reverse_bits(*byte);
            }
        }
        Some(Packet { bytes })
    }
}

/// A streaming reader for one [`T42Variant`].
///
/// For the variants with leading bytes, a unit whose leading bytes are wrong
/// is not used: the reader slides forward one byte at a time until the
/// leading bytes match again and reports the bytes it skipped (REQ-INP-020).
#[derive(Clone, Debug)]
pub struct T42Reader {
    variant: T42Variant,
    source: Option<u8>,
    unit: [u8; PACKET_BYTES_WITH_RUN_IN],
    len: usize,
    /// Byte offset of `unit[0]`.
    unit_offset: u64,
    /// Offset of the next byte to arrive.
    position: u64,
    /// Bytes still to discard before the first unit.
    skip: usize,
    /// Start and length of the run of bytes skipped while resynchronising.
    skipped: Option<(u64, u32)>,
}

impl T42Reader {
    /// A reader for `variant`, marking packets with `source`.
    #[must_use]
    pub const fn new(variant: T42Variant, source: Option<u8>) -> Self {
        Self {
            variant,
            source,
            unit: [0; PACKET_BYTES_WITH_RUN_IN],
            len: 0,
            unit_offset: 0,
            position: 0,
            skip: 0,
            skipped: None,
        }
    }

    /// A reader that first discards `skip` bytes, as detection asks when a
    /// stream starts part-way through a packet.
    #[must_use]
    pub const fn with_skip(variant: T42Variant, source: Option<u8>, skip: usize) -> Self {
        let mut reader = Self::new(variant, source);
        reader.skip = skip;
        reader
    }

    /// The variant read.
    #[must_use]
    pub const fn variant(&self) -> T42Variant {
        self.variant
    }

    fn report_skipped(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if let Some((offset, count)) = self.skipped.take() {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::SkippedBytes { count })));
        }
    }

    /// True when the bytes held so far agree with the leading bytes.
    fn prefix_matches(&self) -> bool {
        let prefix = self.variant.prefix();
        let held = self.unit.get(..self.len.min(prefix.len())).unwrap_or(&[]);
        prefix.starts_with(held)
    }

    /// Drops leading bytes until the bytes held could start a unit.
    fn resynchronise(&mut self) {
        while self.len > 0 && !self.prefix_matches() {
            let (start, count) = self.skipped.unwrap_or((self.unit_offset, 0));
            self.skipped = Some((start, count.saturating_add(1)));
            if let Some(held) = self.unit.get_mut(..self.len) {
                held.rotate_left(1);
            }
            self.len = self.len.saturating_sub(1);
            self.unit_offset = self.unit_offset.saturating_add(1);
        }
    }

    fn push(&mut self, byte: u8, offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        if self.len == 0 {
            self.unit_offset = offset;
        }
        if let Some(slot) = self.unit.get_mut(self.len) {
            *slot = byte;
            self.len = self.len.saturating_add(1);
        }
        if !self.prefix_matches() {
            self.resynchronise();
            return;
        }
        if self.len < self.variant.unit_bytes() {
            return;
        }
        let packet = self.unit.get(..self.len).and_then(|unit| self.variant.packet(unit));
        self.len = 0;
        if let Some(packet) = packet {
            self.report_skipped(out);
            let meta = PacketMeta { source: self.source, ..PacketMeta::default() };
            out(Item::Packet { packet, meta });
        }
    }
}

impl Reader for T42Reader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let base = self.position;
        self.position = offset_of(base, bytes.len());
        let skip = self.skip.min(bytes.len());
        self.skip = self.skip.saturating_sub(skip);
        for (index, &byte) in bytes.iter().enumerate().skip(skip) {
            self.push(byte, offset_of(base, index), out);
        }
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.report_skipped(out);
        if self.len > 0 {
            let bytes = saturate_u32(self.len);
            out(Item::Diagnostic(Diagnostic::at(self.unit_offset, DiagnosticKind::Truncated { bytes })));
            self.len = 0;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use neoteletext_core::packet::encode;
    use neoteletext_core::types::Magazine;
    use std::vec::Vec;

    pub(crate) fn sample_packets(count: usize) -> Vec<Packet> {
        (0..count)
            .map(|index| {
                let mut packet = encode::address(Magazine::new((index % 8 + 1) as u8).unwrap(), (index % 26) as u8);
                for (byte_index, byte) in packet.bytes.iter_mut().enumerate().skip(2) {
                    *byte = (index * 7 + byte_index) as u8;
                }
                packet
            })
            .collect()
    }

    pub(crate) fn serialise(variant: T42Variant, packets: &[Packet]) -> Vec<u8> {
        let mut out = Vec::new();
        for packet in packets {
            out.extend_from_slice(variant.prefix());
            for &byte in &packet.bytes {
                out.push(if variant == T42Variant::Reversed { reverse_bits(byte) } else { byte });
            }
        }
        out
    }

    fn read(variant: T42Variant, input: &[u8], chunk: usize) -> (Vec<Packet>, Vec<Diagnostic>) {
        let mut reader = T42Reader::new(variant, None);
        let mut packets = Vec::new();
        let mut diagnostics = Vec::new();
        let mut out = |item: Item<'_>| match item {
            Item::Packet { packet, .. } => packets.push(packet),
            Item::Diagnostic(diagnostic) => diagnostics.push(diagnostic),
            _ => {}
        };
        for piece in input.chunks(chunk.max(1)) {
            reader.feed(piece, &mut out);
        }
        reader.finish(&mut out);
        (packets, diagnostics)
    }

    #[test]
    fn every_variant_round_trips_in_any_chunk_size() {
        let packets = sample_packets(20);
        for variant in T42Variant::ALL {
            let stream = serialise(variant, &packets);
            let whole = read(variant, &stream, stream.len());
            assert_eq!(whole.0, packets, "{variant:?}");
            assert_eq!(whole.1, []);
            for chunk in [1, 5, 41, 42, 43, 100] {
                assert_eq!(read(variant, &stream, chunk), whole, "{variant:?} chunk {chunk}");
            }
        }
    }

    #[test]
    fn framed_streams_resynchronise_after_garbage() {
        let packets = sample_packets(5);
        for variant in [T42Variant::Framed, T42Variant::RunIn] {
            let mut stream = serialise(variant, &packets[..2]);
            let garbage_at = stream.len();
            stream.extend_from_slice(&[1, 2, 3]);
            stream.extend_from_slice(&serialise(variant, &packets[2..]));
            for chunk in [1, 7, stream.len()] {
                let (got, diagnostics) = read(variant, &stream, chunk);
                assert_eq!(got, packets, "{variant:?}");
                assert_eq!(diagnostics, [Diagnostic::at(garbage_at as u64, DiagnosticKind::SkippedBytes { count: 3 })]);
            }
        }
    }

    #[test]
    fn run_in_with_an_extra_run_in_byte_resynchronises() {
        let packets = sample_packets(2);
        let mut stream = std::vec![CLOCK_RUN_IN];
        stream.extend_from_slice(&serialise(T42Variant::RunIn, &packets));
        let (got, diagnostics) = read(T42Variant::RunIn, &stream, 1);
        assert_eq!(got, packets);
        assert_eq!(diagnostics, [Diagnostic::at(0, DiagnosticKind::SkippedBytes { count: 1 })]);
    }

    #[test]
    fn truncated_tail_is_reported_with_its_offset() {
        let packets = sample_packets(3);
        let mut stream = serialise(T42Variant::Plain, &packets);
        stream.truncate(100);
        let (got, diagnostics) = read(T42Variant::Plain, &stream, 9);
        assert_eq!(got.len(), 2);
        assert_eq!(diagnostics, [Diagnostic::at(84, DiagnosticKind::Truncated { bytes: 16 })]);
    }

    #[test]
    fn skip_discards_a_partial_first_packet() {
        let packets = sample_packets(3);
        let mut stream = std::vec![9, 9, 9];
        stream.extend_from_slice(&serialise(T42Variant::Plain, &packets));
        let mut reader = T42Reader::with_skip(T42Variant::Plain, Some(4), 3);
        let mut got = Vec::new();
        for byte in &stream {
            reader.feed(core::slice::from_ref(byte), &mut |item| {
                if let Item::Packet { packet, meta } = item {
                    assert_eq!(meta.source, Some(4));
                    got.push(packet);
                }
            });
        }
        assert_eq!(got, packets);
    }
}
