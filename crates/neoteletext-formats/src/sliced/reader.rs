//! The reader of Linux sliced VBI records (REQ-INP-010), compiled with the
//! `sliced` feature (decision E.61).

use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::packet::{Packet, PacketMeta};

use crate::consts::{
    SLICED_FIELD_OFFSET, SLICED_HEADER_BYTES, SLICED_LINE_OFFSET, SLICED_RECORD_BYTES, SLICED_TELETEXT_B,
};
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of, saturate_u32};
use crate::text::copy_prefix;

use super::ByteOrder;

/// Converts one record to a packet, or `None` for records that carry no
/// teletext (service identifier 0 or another service).
#[must_use]
pub fn record_packet(record: &[u8; SLICED_RECORD_BYTES], order: ByteOrder) -> Option<(Packet, PacketMeta)> {
    if order.word(record, 0)? != SLICED_TELETEXT_B {
        return None;
    }
    let field = order.word(record, SLICED_FIELD_OFFSET)?;
    let line = order.word(record, SLICED_LINE_OFFSET)?;
    let data = record.get(SLICED_HEADER_BYTES..SLICED_HEADER_BYTES.saturating_add(PACKET_BYTES))?;
    let mut bytes = [0; PACKET_BYTES];
    bytes.copy_from_slice(data);
    let meta = PacketMeta {
        // V4L2 counts lines from 1; 0 means the driver did not say.
        line: u16::try_from(line).ok().filter(|&line| line != 0),
        second_field: match field {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        ..PacketMeta::default()
    };
    Some((Packet { bytes }, meta))
}

/// A streaming reader of sliced records.
#[derive(Clone, Debug)]
pub struct SlicedReader {
    order: ByteOrder,
    source: Option<u8>,
    record: [u8; SLICED_RECORD_BYTES],
    len: usize,
    position: u64,
    record_offset: u64,
}

impl SlicedReader {
    /// A reader with the given field byte order.
    #[must_use]
    pub const fn new(order: ByteOrder, source: Option<u8>) -> Self {
        Self { order, source, record: [0; SLICED_RECORD_BYTES], len: 0, position: 0, record_offset: 0 }
    }
}

impl Reader for SlicedReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let base = self.position;
        self.position = offset_of(base, bytes.len());
        let mut index = 0usize;
        while let Some(rest) = bytes.get(index..) {
            if rest.is_empty() {
                break;
            }
            if self.len == 0 {
                self.record_offset = offset_of(base, index);
            }
            let take = copy_prefix(self.record.get_mut(self.len..).unwrap_or(&mut []), rest);
            self.len = self.len.saturating_add(take);
            index = index.saturating_add(take);
            if self.len == SLICED_RECORD_BYTES {
                self.len = 0;
                if let Some((packet, mut meta)) = record_packet(&self.record, self.order) {
                    meta.source = self.source;
                    out(Item::Packet { packet, meta });
                }
            }
        }
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if self.len > 0 {
            let kind = DiagnosticKind::Truncated { bytes: saturate_u32(self.len) };
            out(Item::Diagnostic(Diagnostic::at(self.record_offset, kind)));
            self.len = 0;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn record(order: ByteOrder, id: u32, field: u32, line: u32, seed: u8) -> Vec<u8> {
        let words = [id, field, line, 0];
        let mut out = Vec::new();
        for word in words {
            out.extend_from_slice(&match order {
                ByteOrder::Little => word.to_le_bytes(),
                ByteOrder::Big => word.to_be_bytes(),
            });
        }
        out.extend((0..48u8).map(|index| seed.wrapping_add(index)));
        out
    }

    type Output = Vec<(Option<(Packet, PacketMeta)>, Option<Diagnostic>)>;

    fn read(order: ByteOrder, input: &[u8], chunk: usize) -> Output {
        let mut reader = SlicedReader::new(order, None);
        let mut items = Vec::new();
        let mut out = |item: Item<'_>| match item {
            Item::Packet { packet, meta } => items.push((Some((packet, meta)), None)),
            Item::Diagnostic(diagnostic) => items.push((None, Some(diagnostic))),
            _ => {}
        };
        for piece in input.chunks(chunk) {
            reader.feed(piece, &mut out);
        }
        reader.finish(&mut out);
        items
    }

    #[test]
    fn reads_teletext_records_in_both_byte_orders() {
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let mut input = record(order, 1, 0, 7, 10);
            input.extend(record(order, 0, 0, 0, 0));
            input.extend(record(order, 4, 0, 16, 0));
            input.extend(record(order, 1, 1, 320, 20));
            input.extend_from_slice(&[1, 2, 3]);
            let whole = read(order, &input, input.len());
            assert_eq!(whole.len(), 3);
            let (packet, meta) = whole[0].0.unwrap();
            assert_eq!(packet.bytes[0], 10);
            assert_eq!((meta.line, meta.second_field), (Some(7), Some(false)));
            let (packet, meta) = whole[1].0.unwrap();
            assert_eq!(packet.bytes[41], 20 + 41);
            assert_eq!((meta.line, meta.second_field), (Some(320), Some(true)));
            assert_eq!(whole[2].1, Some(Diagnostic::at(256, DiagnosticKind::Truncated { bytes: 3 })));
            for chunk in [1, 63, 64, 65] {
                assert_eq!(read(order, &input, chunk), whole);
            }
        }
    }
}
