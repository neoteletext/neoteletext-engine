//! Teletext in packetised elementary stream packets (REQ-INP-008), from a
//! transport stream or as a raw file of PES packets (REQ-INP-009).
//!
//! The parser is a byte-at-a-time state machine, so a PES packet never has
//! to be held whole: its memory is one data unit.

use crate::consts::{
    DATA_IDENTIFIER_EBU_FIRST, DATA_IDENTIFIER_EBU_LAST, DATA_IDENTIFIER_VBI_FIRST, DATA_IDENTIFIER_VBI_LAST,
    DATA_UNIT_HEADER_BYTES, PES_FIXED_HEADER_BYTES, PES_OPTIONAL_HEADER_BYTES, PES_PTS_BYTES,
};
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of};

mod header;
mod units;

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
