//! The teletext data units of a PES packet's payload (EN 300 472 clause 4,
//! EN 301 775).

use neoteletext_core::consts::{FRAMING_CODE_DVB, FRAMING_CODE_DVB_INVERTED, PACKET_BYTES};
use neoteletext_core::ecc::reverse_bits;
use neoteletext_core::packet::{Packet, PacketMeta};

use crate::consts::{
    DATA_UNIT_FIELD_PARITY, DATA_UNIT_HEADER_BYTES, DATA_UNIT_INVERTED, DATA_UNIT_LINE_MASK, DATA_UNIT_SUBTITLE,
    DATA_UNIT_TELETEXT, DATA_UNIT_TELETEXT_LENGTH,
};
use crate::item::{Diagnostic, DiagnosticKind, Item};

use super::{PesParser, State};

impl PesParser {
    pub(super) fn unit_byte(&mut self, byte: u8, offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        if self.unit_len == 0 && self.remaining.is_none() && byte == 0 && self.raw {
            // data_unit_id 0 is reserved (EN 300 472 Table 3): in an unbounded
            // packet it can only be the next start code.
            self.state = State::Hunting;
            self.header_len = 0;
            self.hunt(byte, offset);
            return;
        }
        if let Some(slot) = self.unit.get_mut(self.unit_len) {
            *slot = byte;
            self.unit_len = self.unit_len.saturating_add(1);
        }
        let length = usize::from(self.unit.get(1).copied().unwrap_or(0));
        if self.unit_len >= DATA_UNIT_HEADER_BYTES && self.unit_len == DATA_UNIT_HEADER_BYTES.saturating_add(length) {
            let unit_start = offset.saturating_sub(u64::try_from(self.unit_len).unwrap_or(0)).saturating_add(1);
            self.data_unit(unit_start, out);
            self.unit_len = 0;
        }
        self.count_byte();
    }

    fn data_unit(&self, offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        let id = self.unit.first().copied().unwrap_or(0);
        let length = self.unit.get(1).copied().unwrap_or(0);
        let framing = match id {
            DATA_UNIT_TELETEXT | DATA_UNIT_SUBTITLE => FRAMING_CODE_DVB,
            DATA_UNIT_INVERTED => FRAMING_CODE_DVB_INVERTED,
            // Stuffing and every other unit are skipped by their length.
            _ => return,
        };
        if usize::from(length) < DATA_UNIT_TELETEXT_LENGTH {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::ShortDataUnit { id, length })));
            return;
        }
        let data = self.unit.get(DATA_UNIT_HEADER_BYTES..).unwrap_or(&[]);
        let field_line = data.first().copied().unwrap_or(0);
        let found = data.get(1).copied().unwrap_or(0);
        if found != framing {
            let at = offset.saturating_add(3);
            out(Item::Diagnostic(Diagnostic::at(at, DiagnosticKind::BadFramingCode { value: found })));
            return;
        }
        let mut bytes = [0; PACKET_BYTES];
        for (target, &source) in bytes.iter_mut().zip(data.iter().skip(2)) {
            // EN 300 472 clause 4.4: bits are in the order they appear in the
            // VBI, so each byte is the bit-reverse of the internal form.
            *target = reverse_bits(source);
        }
        let line_offset = field_line & DATA_UNIT_LINE_MASK;
        let meta = PacketMeta {
            line: if line_offset == 0 { None } else { Some(u16::from(line_offset)) },
            // field_parity 1 is the first field (EN 300 472 Table 4).
            second_field: if line_offset == 0 { None } else { Some(field_line & DATA_UNIT_FIELD_PARITY == 0) },
            timestamp_us: self.timestamp,
            source: self.source,
            quality: None,
        };
        out(Item::Packet { packet: Packet { bytes }, meta });
    }
}
