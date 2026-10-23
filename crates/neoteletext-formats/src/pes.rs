//! Teletext in packetised elementary stream packets (REQ-INP-008), from a
//! transport stream or as a raw file of PES packets (REQ-INP-009).
//!
//! The parser is a byte-at-a-time state machine, so a PES packet never has
//! to be held whole: its memory is one data unit.

use neoteletext_core::consts::{FRAMING_CODE_DVB, FRAMING_CODE_DVB_INVERTED, PACKET_BYTES};
use neoteletext_core::ecc::reverse_bits;
use neoteletext_core::packet::{Packet, PacketMeta};

use crate::consts::{
    DATA_IDENTIFIER_EBU_FIRST, DATA_IDENTIFIER_EBU_LAST, DATA_IDENTIFIER_VBI_FIRST, DATA_IDENTIFIER_VBI_LAST,
    DATA_UNIT_FIELD_PARITY, DATA_UNIT_HEADER_BYTES, DATA_UNIT_INVERTED, DATA_UNIT_LINE_MASK, DATA_UNIT_SUBTITLE,
    DATA_UNIT_TELETEXT, DATA_UNIT_TELETEXT_LENGTH, PES_FIXED_HEADER_BYTES, PES_OPTIONAL_HEADER_BYTES, PES_PTS_BYTES,
    PES_PTS_PRESENT, PES_START_CODE, PTS_TO_US_DENOMINATOR, PTS_TO_US_NUMERATOR, STREAM_ID_PRIVATE_1,
};
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of};

/// Bytes of the PES header the parser holds: the fixed part and the three
/// bytes of the optional header up to `PES_header_data_length`.
const HEADER_BYTES: usize = PES_FIXED_HEADER_BYTES + PES_OPTIONAL_HEADER_BYTES;
/// Index of `stream_id` in the header.
const STREAM_ID_INDEX: usize = 3;
/// Index of the first `PES_packet_length` byte.
const LENGTH_INDEX: usize = 4;
/// Index of the flag byte holding `PTS_DTS_flags`.
const FLAGS_INDEX: usize = 7;
/// Index of `PES_header_data_length`.
const HEADER_DATA_LENGTH_INDEX: usize = 8;
/// The largest data unit: its header and a one-byte length's worth of data.
const UNIT_CAPACITY: usize = DATA_UNIT_HEADER_BYTES + u8::MAX as usize;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// Waiting for the transport layer to start a packet.
    Idle,
    /// Searching the byte stream for a start code (raw files only).
    Hunting,
    /// Reading the header up to `PES_header_data_length`.
    Header,
    /// Skipping the rest of the header, keeping the PTS.
    HeaderData,
    /// The `data_identifier` byte.
    DataIdentifier,
    /// Data units.
    Units,
    /// Skipping to the end of the PES packet.
    Skip,
}

/// The streaming PES parser shared by the transport stream reader and the
/// raw PES file reader.
#[derive(Clone, Debug)]
pub struct PesParser {
    state: State,
    /// True for a raw PES file, where the parser finds start codes itself.
    raw: bool,
    source: Option<u8>,
    header: [u8; HEADER_BYTES],
    header_len: usize,
    /// Bytes of the PES packet still to come; `None` when the length is 0.
    remaining: Option<usize>,
    header_data_left: usize,
    pts_wanted: bool,
    pts: [u8; PES_PTS_BYTES],
    pts_len: usize,
    timestamp: Option<u64>,
    unit: [u8; UNIT_CAPACITY],
    unit_len: usize,
    /// Start and count of bytes skipped while hunting.
    skipped: Option<(u64, u32)>,
}

impl PesParser {
    /// A parser. `raw` is true for a raw PES file, false under a transport
    /// stream, where [`PesParser::begin`] marks each packet start.
    #[must_use]
    pub const fn new(raw: bool, source: Option<u8>) -> Self {
        Self {
            state: if raw { State::Hunting } else { State::Idle },
            raw,
            source,
            header: [0; HEADER_BYTES],
            header_len: 0,
            remaining: None,
            header_data_left: 0,
            pts_wanted: false,
            pts: [0; PES_PTS_BYTES],
            pts_len: 0,
            timestamp: None,
            unit: [0; UNIT_CAPACITY],
            unit_len: 0,
            skipped: None,
        }
    }

    /// Starts a new PES packet: the transport layer saw
    /// `payload_unit_start_indicator`.
    pub fn begin(&mut self) {
        self.state = State::Header;
        self.header_len = 0;
        self.unit_len = 0;
        self.timestamp = None;
    }

    /// Abandons the packet in progress, after lost transport packets.
    pub fn abort(&mut self) {
        self.state = if self.raw { State::Hunting } else { State::Idle };
        self.header_len = 0;
        self.unit_len = 0;
    }

    /// Reads bytes whose first byte is at `offset`.
    pub fn feed(&mut self, bytes: &[u8], offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        for (index, &byte) in bytes.iter().enumerate() {
            self.step(byte, offset_of(offset, index), out);
        }
    }

    /// Reports bytes skipped while hunting that no start code followed.
    pub fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.report_skipped(out);
    }

    fn report_skipped(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if let Some((offset, count)) = self.skipped.take() {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::SkippedBytes { count })));
        }
    }

    fn note_skipped(&mut self, offset: u64, count: usize) {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        let (start, total) = self.skipped.unwrap_or((offset, 0));
        self.skipped = Some((start, total.saturating_add(count)));
    }

    /// Ends the packet when its length is used up.
    fn end_of_packet(&mut self) {
        self.state = if self.raw { State::Hunting } else { State::Idle };
        self.header_len = 0;
        self.unit_len = 0;
    }

    fn step(&mut self, byte: u8, offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        match self.state {
            State::Idle => {}
            State::Hunting => self.hunt(byte, offset),
            State::Header => self.header_byte(byte, offset, out),
            State::HeaderData => {
                if self.pts_wanted
                    && let Some(slot) = self.pts.get_mut(self.pts_len)
                {
                    *slot = byte;
                    self.pts_len = self.pts_len.saturating_add(1);
                }
                self.header_data_left = self.header_data_left.saturating_sub(1);
                if self.header_data_left == 0 {
                    self.finish_header();
                }
                self.count_byte();
            }
            State::DataIdentifier => {
                let accepted = (DATA_IDENTIFIER_EBU_FIRST..=DATA_IDENTIFIER_EBU_LAST).contains(&byte)
                    || (DATA_IDENTIFIER_VBI_FIRST..=DATA_IDENTIFIER_VBI_LAST).contains(&byte);
                if accepted {
                    self.state = State::Units;
                } else {
                    out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::BadDataIdentifier { value: byte })));
                    self.state = State::Skip;
                }
                self.count_byte();
            }
            State::Units => self.unit_byte(byte, offset, out),
            State::Skip => self.count_byte(),
        }
    }

    /// Counts one byte of the packet body against `PES_packet_length`.
    fn count_byte(&mut self) {
        if let Some(remaining) = self.remaining {
            let left = remaining.saturating_sub(1);
            self.remaining = Some(left);
            if left == 0 {
                self.end_of_packet();
            }
        }
    }

    fn hunt(&mut self, byte: u8, offset: u64) {
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

    fn header_byte(&mut self, byte: u8, offset: u64, out: &mut dyn FnMut(Item<'_>)) {
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

    fn finish_header(&mut self) {
        if self.pts_wanted && self.pts_len == PES_PTS_BYTES {
            self.timestamp = pts_microseconds(self.pts);
        }
        self.state = State::DataIdentifier;
    }

    fn unit_byte(&mut self, byte: u8, offset: u64, out: &mut dyn FnMut(Item<'_>)) {
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

/// Converts a coded PTS (ISO/IEC 13818-1 clause 2.4.3.7) to microseconds.
fn pts_microseconds(coded: [u8; PES_PTS_BYTES]) -> Option<u64> {
    let [b0, b1, b2, b3, b4] = coded.map(u64::from);
    // PTS[32..30] in bits 3..1 of byte 0; PTS[29..15] in bytes 1 and 2;
    // PTS[14..0] in bytes 3 and 4; the low bit of bytes 0, 2 and 4 is a
    // marker.
    let pts = ((b0 >> 1) & 0x7) << 30 | b1 << 22 | (b2 >> 1) << 15 | b3 << 7 | b4 >> 1;
    pts.checked_mul(PTS_TO_US_NUMERATOR)?.checked_div(PTS_TO_US_DENOMINATOR)
}

/// A reader for raw files of PES packets (REQ-INP-009).
#[derive(Clone, Debug)]
pub struct PesReader {
    parser: PesParser,
    position: u64,
}

impl PesReader {
    /// A reader marking packets with `source`.
    #[must_use]
    pub const fn new(source: Option<u8>) -> Self {
        Self { parser: PesParser::new(true, source), position: 0 }
    }
}

impl Reader for PesReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        self.parser.feed(bytes, self.position, out);
        self.position = offset_of(self.position, bytes.len());
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.parser.finish(out);
    }
}

#[cfg(test)]
pub(crate) mod tests;
