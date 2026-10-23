//! The transport stream reader (REQ-INP-006 and REQ-INP-007).

use neoteletext_core::types::FixedVec;

use crate::consts::{
    PID_NULL, PID_PAT, SECTION_MAX_BYTES, TS_ADAPTATION_MASK, TS_ADAPTATION_PRESENT, TS_ADAPTATION_SHIFT,
    TS_CONTINUITY_MASK, TS_DISCONTINUITY, TS_FEC_PACKET_BYTES, TS_HEADER_BYTES, TS_M2TS_PACKET_BYTES, TS_PACKET_BYTES,
    TS_PAYLOAD_PRESENT, TS_PAYLOAD_UNIT_START, TS_PID_HIGH_MASK, TS_SYNC_BYTE, TS_TRANSPORT_ERROR,
};
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of, saturate_u32};
use crate::pes::PesParser;
use crate::text::copy_prefix;

use super::{MAX_PROGRAMS, TsConfig, TsPacketSize};

mod section;
mod tables;

pub use section::crc32;
use section::{Continuity, check_continuity};

/// The reader's input buffer: enough for four of the largest packets, so
/// that a sync byte can be confirmed two packets ahead.
const BUFFER_BYTES: usize = 4 * TS_FEC_PACKET_BYTES;
/// Packets ahead whose sync bytes must agree before the reader locks to an
/// unknown packet size, and to a known one.
const CONFIRM_UNKNOWN: usize = 2;
const CONFIRM_KNOWN: usize = 1;

impl TsPacketSize {
    /// Bytes before the first sync byte of a well-formed stream.
    const fn leading_bytes(self) -> usize {
        match self {
            Self::M2ts => TS_M2TS_PACKET_BYTES.saturating_sub(TS_PACKET_BYTES),
            Self::Standard | Self::Fec => 0,
        }
    }
}

/// A table section being reassembled.
#[derive(Clone, Copy, Debug)]
struct Section {
    data: [u8; SECTION_MAX_BYTES],
    len: usize,
    active: bool,
    continuity: Option<u8>,
}

/// A program and the PID of its Program Map Table.
#[derive(Clone, Copy, Debug)]
struct Program {
    number: u16,
    pid: u16,
    version: Option<u8>,
    section: Section,
}

/// The fields of a transport packet header (clause 2.4.3.2).
#[derive(Clone, Copy, Debug)]
struct Header {
    pid: u16,
    unit_start: bool,
    counter: u8,
    has_payload: bool,
    discontinuity: bool,
    payload_start: usize,
}

/// A streaming transport stream reader.
#[derive(Clone, Debug)]
pub struct TsReader {
    config: TsConfig,
    size: Option<TsPacketSize>,
    buffer: [u8; BUFFER_BYTES],
    start: usize,
    end: usize,
    /// Offset of `buffer[start]`.
    buffer_offset: u64,
    locked: bool,
    skipped: Option<(u64, u32)>,
    pat: Section,
    programs: FixedVec<Program, MAX_PROGRAMS>,
    programs_reported: bool,
    selected: Option<u16>,
    selected_program: Option<u16>,
    subtitle_reported: bool,
    teletext_continuity: Option<u8>,
    pes: PesParser,
}

impl TsReader {
    /// A reader with the given configuration.
    #[must_use]
    pub const fn new(config: TsConfig) -> Self {
        let filler = Program { number: 0, pid: 0, version: None, section: Section::new() };
        Self {
            config,
            size: config.packet_size,
            buffer: [0; BUFFER_BYTES],
            start: 0,
            end: 0,
            buffer_offset: 0,
            locked: false,
            skipped: None,
            pat: Section::new(),
            programs: FixedVec::new(filler),
            programs_reported: false,
            selected: config.pid,
            selected_program: None,
            subtitle_reported: false,
            teletext_continuity: None,
            pes: PesParser::new(false, config.source),
        }
    }

    /// The PID being decoded as teletext, once chosen.
    #[must_use]
    pub const fn selected_pid(&self) -> Option<u16> {
        self.selected
    }

    fn available(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    fn byte_at(&self, index: usize) -> Option<u8> {
        self.buffer.get(self.start.saturating_add(index)).copied()
    }

    fn consume(&mut self, count: usize) {
        let count = count.min(self.available());
        self.start = self.start.saturating_add(count);
        self.buffer_offset = offset_of(self.buffer_offset, count);
    }

    fn skip(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        let (start, total) = self.skipped.unwrap_or((self.buffer_offset, 0));
        self.skipped = Some((start, total.saturating_add(saturate_u32(count))));
        self.consume(count);
    }

    fn report_skipped(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if let Some((offset, count)) = self.skipped.take() {
            let expected_prefix = self.size.map_or(0, TsPacketSize::leading_bytes);
            // The prefix of an M2TS stream's first packet is not an error.
            if !(offset == 0 && usize::try_from(count).is_ok_and(|count| count == expected_prefix)) {
                out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::SkippedBytes { count })));
            }
        }
    }

    /// True when sync bytes stand at `index` and `confirm` packets on.
    /// `None` when there are not yet enough bytes to tell.
    fn confirmed(&self, index: usize, size: usize, confirm: usize, at_end: bool) -> Option<bool> {
        for step in 1..=confirm {
            let position = index.saturating_add(size.saturating_mul(step));
            match self.byte_at(position) {
                Some(byte) if position < self.available() => {
                    if byte != TS_SYNC_BYTE {
                        return Some(false);
                    }
                }
                _ if at_end => return Some(index.saturating_add(TS_PACKET_BYTES) <= self.available()),
                _ => return None,
            }
        }
        Some(true)
    }

    /// Searches for sync. Returns true once locked.
    fn lock(&mut self, at_end: bool) -> bool {
        let sizes: &[TsPacketSize] = match &self.config.packet_size {
            Some(size) => core::slice::from_ref(size),
            None => &TsPacketSize::ALL,
        };
        let confirm = if self.config.packet_size.is_some() { CONFIRM_KNOWN } else { CONFIRM_UNKNOWN };
        let available = self.available();
        for index in 0..available {
            if self.byte_at(index) != Some(TS_SYNC_BYTE) {
                continue;
            }
            let mut undecided = false;
            for &size in sizes {
                match self.confirmed(index, size.bytes(), confirm, at_end) {
                    Some(true) => {
                        self.size = Some(size);
                        self.skip(index);
                        self.locked = true;
                        return true;
                    }
                    Some(false) => {}
                    None => undecided = true,
                }
            }
            if undecided {
                self.skip(index);
                return false;
            }
        }
        self.skip(available);
        false
    }

    fn process(&mut self, out: &mut dyn FnMut(Item<'_>), at_end: bool) {
        loop {
            if !self.locked && !self.lock(at_end) {
                return;
            }
            let Some(size) = self.size else { return };
            let need = if at_end { TS_PACKET_BYTES } else { size.bytes() };
            if self.available() < need {
                if at_end && self.available() > 0 {
                    let bytes = saturate_u32(self.available());
                    out(Item::Diagnostic(Diagnostic::at(self.buffer_offset, DiagnosticKind::Truncated { bytes })));
                    self.consume(self.available());
                }
                return;
            }
            if self.byte_at(0) != Some(TS_SYNC_BYTE) {
                self.locked = false;
                continue;
            }
            self.report_skipped(out);
            let mut packet = [0; TS_PACKET_BYTES];
            // At least one whole packet is available, checked above.
            copy_prefix(&mut packet, self.buffer.get(self.start..).unwrap_or(&[]));
            let offset = self.buffer_offset;
            self.consume(size.bytes());
            self.packet(&packet, offset, out);
        }
    }

    fn header(packet: &[u8; TS_PACKET_BYTES]) -> Option<Header> {
        let [_, flags, pid_low, control] = *packet.first_chunk::<TS_HEADER_BYTES>()?;
        let pid = (u16::from(flags & TS_PID_HIGH_MASK) << 8) | u16::from(pid_low);
        let adaptation = (control >> TS_ADAPTATION_SHIFT) & TS_ADAPTATION_MASK;
        let mut payload_start = TS_HEADER_BYTES;
        let mut discontinuity = false;
        if adaptation & TS_ADAPTATION_PRESENT != 0 {
            let length = usize::from(packet.get(TS_HEADER_BYTES).copied()?);
            if length > 0 {
                discontinuity = packet.get(TS_HEADER_BYTES.saturating_add(1)).copied()? & TS_DISCONTINUITY != 0;
            }
            payload_start = TS_HEADER_BYTES.saturating_add(1).saturating_add(length);
        }
        Some(Header {
            pid,
            unit_start: flags & TS_PAYLOAD_UNIT_START != 0,
            counter: control & TS_CONTINUITY_MASK,
            has_payload: adaptation & TS_PAYLOAD_PRESENT != 0,
            discontinuity,
            payload_start,
        })
    }

    fn packet(&mut self, packet: &[u8; TS_PACKET_BYTES], offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        let flags = packet.get(1).copied().unwrap_or(0);
        let Some(header) = Self::header(packet) else { return };
        let pid = header.pid;
        if flags & TS_TRANSPORT_ERROR != 0 {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::TransportError { pid })));
            return;
        }
        if pid == PID_NULL {
            return;
        }
        let control = packet.get(3).copied().unwrap_or(0);
        if (control >> TS_ADAPTATION_SHIFT) & TS_ADAPTATION_MASK == 0 {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::ReservedAdaptationControl { pid })));
            return;
        }
        // EN 300 472 clause 4.1 allows only an adaptation field or a payload,
        // but REQ-INP-006 accepts both.
        let Some(payload) = packet.get(header.payload_start..) else {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::BadAdaptationField { pid })));
            return;
        };
        let payload = if header.has_payload { payload } else { &[] };
        let payload_offset = offset_of(offset, header.payload_start);
        if pid == PID_PAT {
            let mut section = self.pat;
            self.section_packet(&mut section, &header, payload, offset, out);
            self.pat = section;
        } else if let Some(index) = self.programs.as_slice().iter().position(|program| program.pid == pid)
            && let Some(mut section) = self.programs.as_slice().get(index).map(|program| program.section)
        {
            // The section is worked on as a copy because handling it may
            // add programs.
            self.section_packet(&mut section, &header, payload, offset, out);
            if let Some(program) = self.programs.as_mut_slice().get_mut(index) {
                program.section = section;
            }
        }
        if self.selected == Some(pid) {
            self.teletext_packet(&header, payload, payload_offset, offset, out);
        }
    }

    fn teletext_packet(
        &mut self,
        header: &Header,
        payload: &[u8],
        payload_offset: u64,
        offset: u64,
        out: &mut dyn FnMut(Item<'_>),
    ) {
        let last = self.teletext_continuity;
        match check_continuity(&mut self.teletext_continuity, header.counter, header.has_payload, header.discontinuity)
        {
            Continuity::Duplicate => return,
            Continuity::Broken => {
                let expected = last.map_or(0, |value| value.wrapping_add(1) & TS_CONTINUITY_MASK);
                let kind = DiagnosticKind::ContinuityBreak { pid: header.pid, expected, found: header.counter };
                out(Item::Diagnostic(Diagnostic::at(offset, kind)));
                self.pes.abort();
            }
            Continuity::Follows => {}
        }
        if header.unit_start {
            self.pes.begin();
        }
        self.pes.feed(payload, payload_offset, out);
    }
}

impl Reader for TsReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut rest = bytes;
        while !rest.is_empty() {
            if self.start > 0 {
                // REQ-GEN-019: a byte loop rather than `copy_within`, whose
                // range check the optimiser cannot remove. Moving forward is
                // correct for the overlap, as the source is never behind
                // the target.
                for index in 0..self.end.saturating_sub(self.start) {
                    if let Some(byte) = self.buffer.get(self.start.saturating_add(index)).copied()
                        && let Some(slot) = self.buffer.get_mut(index)
                    {
                        *slot = byte;
                    }
                }
                self.end = self.end.saturating_sub(self.start);
                self.start = 0;
            }
            let take = copy_prefix(self.buffer.get_mut(self.end..).unwrap_or(&mut []), rest);
            self.end = self.end.saturating_add(take);
            rest = rest.get(take..).unwrap_or(&[]);
            self.process(out, false);
        }
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.process(out, true);
        self.report_skipped(out);
        self.pes.finish(out);
        if self.selected.is_none() {
            out(Item::Diagnostic(Diagnostic::at(self.buffer_offset, DiagnosticKind::NoTeletextStream)));
        }
    }
}
