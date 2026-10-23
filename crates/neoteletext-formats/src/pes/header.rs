//! The PES packet header (ISO/IEC 13818-1 clause 2.4.3.6): the start code
//! hunt, the header bytes and the PTS (clause 2.4.3.7).

use crate::consts::{
    PES_FIXED_HEADER_BYTES, PES_PTS_BYTES, PES_PTS_PRESENT, PES_START_CODE, PTS_TO_US_DENOMINATOR, PTS_TO_US_NUMERATOR,
    STREAM_ID_PRIVATE_1,
};
use crate::item::{Diagnostic, DiagnosticKind, Item};

use super::{FLAGS_INDEX, HEADER_BYTES, HEADER_DATA_LENGTH_INDEX, LENGTH_INDEX, PesParser, STREAM_ID_INDEX, State};

impl PesParser {
    pub(super) fn hunt(&mut self, byte: u8, offset: u64) {
        let expected = PES_START_CODE.get(self.header_len).copied();
        if expected == Some(byte) {
            if let Some(slot) = self.header.get_mut(self.header_len) {
                *slot = byte;
            }
            self.header_len = self.header_len.saturating_add(1);
            if self.header_len == PES_START_CODE.len() {
                self.state = State::Header;
            }
        } else if byte == 0 && self.header_len == PES_START_CODE.len().saturating_sub(1) {
            // Three zeros: the oldest is not part of a start code.
            self.note_skipped(offset.saturating_sub(2), 1);
        } else {
            let start = offset.saturating_sub(u64::try_from(self.header_len).unwrap_or(0));
            self.note_skipped(start, self.header_len.saturating_add(1));
            self.header_len = 0;
        }
    }

    pub(super) fn header_byte(&mut self, byte: u8, offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        if let Some(slot) = self.header.get_mut(self.header_len) {
            *slot = byte;
        }
        self.header_len = self.header_len.saturating_add(1);
        if self.header_len <= PES_START_CODE.len() {
            if PES_START_CODE.get(self.header_len.saturating_sub(1)) != Some(&byte) {
                out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::BadPesHeader)));
                self.end_of_packet();
            }
            return;
        }
        if self.header_len < PES_FIXED_HEADER_BYTES {
            return;
        }
        if self.header_len == PES_FIXED_HEADER_BYTES {
            self.report_skipped(out);
            let high = self.header.get(LENGTH_INDEX).copied().unwrap_or(0);
            let low = self.header.get(LENGTH_INDEX.saturating_add(1)).copied().unwrap_or(0);
            // REQ-GEN-007: assembled big-endian, as ISO/IEC 13818-1 orders it.
            let length = usize::from(u16::from_be_bytes([high, low]));
            self.remaining = if length == 0 { None } else { Some(length) };
            let stream_id = self.header.get(STREAM_ID_INDEX).copied().unwrap_or(0);
            if stream_id != STREAM_ID_PRIVATE_1 {
                out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::NotPrivateStream { stream_id })));
                if self.remaining.is_none() {
                    // An unbounded packet of another stream cannot be skipped.
                    self.end_of_packet();
                } else {
                    self.state = State::Skip;
                }
                return;
            }
            if self.remaining.is_none() {
                out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::UnboundedPes)));
            }
            return;
        }
        self.count_byte();
        if self.header_len == HEADER_BYTES && self.state == State::Header {
            let flags = self.header.get(FLAGS_INDEX).copied().unwrap_or(0);
            self.pts_wanted = flags & PES_PTS_PRESENT != 0;
            self.pts_len = 0;
            self.timestamp = None;
            self.header_data_left = usize::from(self.header.get(HEADER_DATA_LENGTH_INDEX).copied().unwrap_or(0));
            if self.header_data_left == 0 {
                self.finish_header();
            } else {
                self.state = State::HeaderData;
            }
        }
    }

    pub(super) fn finish_header(&mut self) {
        if self.pts_wanted && self.pts_len == PES_PTS_BYTES {
            self.timestamp = pts_microseconds(self.pts);
        }
        self.state = State::DataIdentifier;
    }
}

/// Converts a coded PTS (ISO/IEC 13818-1 clause 2.4.3.7) to microseconds.
fn pts_microseconds(coded: [u8; PES_PTS_BYTES]) -> Option<u64> {
    let [b0, b1, b2, b3, b4] = coded.map(u64::from);
    // PTS[32..30] in bits 3..1 of byte 0; PTS[29..15] in bytes 1 and 2;
    // PTS[14..0] in bytes 3 and 4; the low bit of bytes 0, 2 and 4 is a
    // marker.
    let pts = ((b0 >> 1) & 0x7) << 30 | b1 << 22 | (b2 >> 1) << 15 | b3 << 7 | b4 >> 1;
    pts.checked_mul(PTS_TO_US_NUMERATOR)?.checked_div(PTS_TO_US_DENOMINATOR)
}
