//! The transport stream reader (REQ-INP-006 and REQ-INP-007).

use tellytxt_core::types::{FixedVec, Magazine, PageAddress, PageNumber};

use crate::consts::{
    CRC32_INITIAL, CRC32_POLYNOMIAL, DATA_SERVICE_EBU_TELETEXT, DATA_SERVICE_INVERTED_TELETEXT,
    DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_TELETEXT, DESCRIPTOR_VBI_DATA, DESCRIPTOR_VBI_TELETEXT, PAT_ENTRIES_START,
    PAT_ENTRY_BYTES, PID_MASK, PID_NULL, PID_PAT, PMT_DESCRIPTORS_START, PMT_INFO_LENGTH, PMT_STREAM_HEADER_BYTES,
    SECTION_CRC_BYTES, SECTION_HEADER_BYTES, SECTION_LENGTH_MASK, SECTION_MAX_BYTES, SECTION_STUFFING,
    SECTION_VERSION_BYTE, SECTION_VERSION_MASK, SECTION_VERSION_SHIFT, STREAM_TYPE_PRIVATE_PES, TABLE_ID_PAT,
    TABLE_ID_PMT, TELETEXT_ENTRY_BYTES, TELETEXT_MAGAZINE_MASK, TELETEXT_TYPE_SHIFT, TS_ADAPTATION_MASK,
    TS_ADAPTATION_PRESENT, TS_ADAPTATION_SHIFT, TS_CONTINUITY_MASK, TS_DISCONTINUITY, TS_FEC_PACKET_BYTES,
    TS_HEADER_BYTES, TS_M2TS_PACKET_BYTES, TS_PACKET_BYTES, TS_PAYLOAD_PRESENT, TS_PAYLOAD_UNIT_START,
    TS_PID_HIGH_MASK, TS_SYNC_BYTE, TS_TRANSPORT_ERROR,
};
use crate::item::{Capacity, Diagnostic, DiagnosticKind, Item, Reader, offset_of, saturate_u32};
use crate::pes::PesParser;

use super::{MAX_PROGRAMS, TeletextEntry, TeletextType, TsConfig, TsPacketSize};

/// The reader's input buffer: enough for four of the largest packets, so
/// that a sync byte can be confirmed two packets ahead.
const BUFFER_BYTES: usize = 4 * TS_FEC_PACKET_BYTES;
/// Packets ahead whose sync bytes must agree before the reader locks to an
/// unknown packet size, and to a known one.
const CONFIRM_UNKNOWN: usize = 2;
const CONFIRM_KNOWN: usize = 1;
/// The most significant bit of the CRC register.
const CRC_TOP_BIT: u32 = 1 << 31;
/// `section_syntax_indicator`, bit 7 of the second section byte.
const SECTION_SYNTAX: u8 = 0x80;
/// `current_next_indicator`, bit 0 of the version byte.
const SECTION_CURRENT: u8 = 0x01;

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

impl Section {
    const fn new() -> Self {
        Self { data: [0; SECTION_MAX_BYTES], len: 0, active: false, continuity: None }
    }

    fn total(&self) -> Option<usize> {
        let high = self.data.get(1).copied()?;
        let low = self.data.get(2).copied()?;
        // REQ-GEN-007: MPEG-2 fields are big-endian whatever the host.
        let length = u16::from_be_bytes([high, low]) & SECTION_LENGTH_MASK;
        Some(SECTION_HEADER_BYTES.saturating_add(usize::from(length)))
    }

    /// Adds one payload byte.
    fn push(&mut self, byte: u8) -> Push {
        if !self.active {
            if byte == SECTION_STUFFING {
                return Push::Stuffing;
            }
            self.active = true;
            self.len = 0;
        }
        if let Some(slot) = self.data.get_mut(self.len) {
            *slot = byte;
            self.len = self.len.saturating_add(1);
        }
        if self.len >= SECTION_HEADER_BYTES {
            let total = self.total().unwrap_or(0);
            if total > SECTION_MAX_BYTES {
                self.active = false;
                return Push::TooLong;
            }
            if self.len == total {
                self.active = false;
                return Push::Complete(total);
            }
        }
        Push::More
    }
}

/// What adding a byte to a section did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Push {
    /// The section needs more bytes.
    More,
    /// The byte is stuffing after the last section of the packet.
    Stuffing,
    /// The section claims to be longer than any section can be.
    TooLong,
    /// The section is complete, with this many bytes.
    Complete(usize),
}

/// A program and the PID of its Program Map Table.
#[derive(Clone, Copy, Debug)]
struct Program {
    number: u16,
    pid: u16,
    version: Option<u8>,
    section: Section,
}

/// The outcome of a continuity counter check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Continuity {
    Follows,
    Duplicate,
    Broken,
}

/// Checks a continuity counter (ISO/IEC 13818-1 clause 2.4.3.3): it
/// increments only with a payload, and one duplicate packet is allowed.
fn check_continuity(last: &mut Option<u8>, counter: u8, has_payload: bool, discontinuity: bool) -> Continuity {
    if !has_payload {
        return Continuity::Follows;
    }
    let result = match *last {
        Some(previous) if !discontinuity && counter == previous => Continuity::Duplicate,
        Some(previous) if !discontinuity && counter != (previous.wrapping_add(1) & TS_CONTINUITY_MASK) => {
            Continuity::Broken
        }
        _ => Continuity::Follows,
    };
    *last = Some(counter);
    result
}

/// The MPEG-2 `CRC_32` (ISO/IEC 13818-1 Annex A); a section with its CRC
/// appended gives 0.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = CRC32_INITIAL;
    for &byte in data {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & CRC_TOP_BIT != 0 { (crc << 1) ^ CRC32_POLYNOMIAL } else { crc << 1 };
        }
    }
    crc
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
            if let Some(source) = self.buffer.get(self.start..self.start.saturating_add(TS_PACKET_BYTES)) {
                packet.copy_from_slice(source);
            }
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

    fn section_packet(
        &mut self,
        section: &mut Section,
        header: &Header,
        payload: &[u8],
        offset: u64,
        out: &mut dyn FnMut(Item<'_>),
    ) {
        let pid = header.pid;
        match check_continuity(&mut section.continuity, header.counter, header.has_payload, header.discontinuity) {
            Continuity::Duplicate => return,
            Continuity::Broken => {
                let kind = DiagnosticKind::ContinuityBreak { pid, expected: 0, found: header.counter };
                out(Item::Diagnostic(Diagnostic::at(offset, kind)));
                section.active = false;
            }
            Continuity::Follows => {}
        }
        let mut ok = true;
        if header.unit_start {
            // pointer_field counts the bytes that finish the previous section
            // (clause 2.4.4.2).
            let pointer = usize::from(payload.first().copied().unwrap_or(0));
            let body = payload.get(1..).unwrap_or(&[]);
            if section.active {
                ok &= self.append_section(section, body.get(..pointer).unwrap_or(body), pid, offset, out);
            }
            section.active = false;
            ok &= self.append_section(section, body.get(pointer..).unwrap_or(&[]), pid, offset, out);
        } else if section.active {
            ok &= self.append_section(section, payload, pid, offset, out);
        }
        if !ok {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::BadSection { pid })));
        }
    }

    /// Adds payload bytes to a section, handling each section completed.
    /// Returns false when a section claimed an impossible length.
    fn append_section(
        &mut self,
        section: &mut Section,
        bytes: &[u8],
        pid: u16,
        offset: u64,
        out: &mut dyn FnMut(Item<'_>),
    ) -> bool {
        for &byte in bytes {
            match section.push(byte) {
                Push::More => {}
                Push::Stuffing => return true,
                Push::TooLong => return false,
                Push::Complete(len) => {
                    let data = section.data;
                    self.section(pid, data.get(..len).unwrap_or(&[]), offset, out);
                }
            }
        }
        true
    }

    fn section(&mut self, pid: u16, data: &[u8], offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        if crc32(data) != 0 {
            out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::SectionCrc { pid })));
            return;
        }
        let syntax = data.get(1).is_some_and(|byte| byte & SECTION_SYNTAX != 0);
        let current = data.get(SECTION_VERSION_BYTE).is_some_and(|byte| byte & SECTION_CURRENT != 0);
        if !syntax || !current {
            return;
        }
        let body_end = data.len().saturating_sub(SECTION_CRC_BYTES);
        match data.first().copied() {
            Some(TABLE_ID_PAT) if pid == PID_PAT => self.pat_section(data.get(..body_end).unwrap_or(&[]), offset, out),
            Some(TABLE_ID_PMT) if pid != PID_PAT => {
                self.pmt_section(pid, data.get(..body_end).unwrap_or(&[]), offset, out);
            }
            _ => {}
        }
    }

    fn pat_section(&mut self, body: &[u8], offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        let entries = body.get(PAT_ENTRIES_START..).unwrap_or(&[]);
        let (chunks, _) = entries.as_chunks::<PAT_ENTRY_BYTES>();
        for &[number_high, number_low, pid_high, pid_low] in chunks {
            let number = u16::from_be_bytes([number_high, number_low]);
            let pid = u16::from_be_bytes([pid_high, pid_low]) & PID_MASK;
            // Program 0 names the network information PID (Table 2-30).
            if number == 0 || self.programs.as_slice().iter().any(|program| program.pid == pid) {
                continue;
            }
            let program = Program { number, pid, version: None, section: Section::new() };
            if !self.programs.push(program) && !self.programs_reported {
                self.programs_reported = true;
                let kind = DiagnosticKind::CapacityExceeded { what: Capacity::Programs };
                out(Item::Diagnostic(Diagnostic::at(offset, kind)));
            }
        }
    }

    fn pmt_section(&mut self, pid: u16, body: &[u8], offset: u64, out: &mut dyn FnMut(Item<'_>)) {
        let number = u16::from_be_bytes([body.get(3).copied().unwrap_or(0), body.get(4).copied().unwrap_or(0)]);
        let version = body.get(SECTION_VERSION_BYTE).map(|byte| (byte >> SECTION_VERSION_SHIFT) & SECTION_VERSION_MASK);
        let Some(program) =
            self.programs.as_mut_slice().iter_mut().find(|program| program.pid == pid && program.number == number)
        else {
            return;
        };
        if program.version == version {
            return;
        }
        program.version = version;
        let info_length = read_length(body, PMT_INFO_LENGTH);
        let mut cursor = PMT_DESCRIPTORS_START.saturating_add(info_length);
        while let Some(stream) = body.get(cursor..cursor.saturating_add(PMT_STREAM_HEADER_BYTES)) {
            let stream_type = stream.first().copied().unwrap_or(0);
            let stream_pid =
                u16::from_be_bytes([stream.get(1).copied().unwrap_or(0), stream.get(2).copied().unwrap_or(0)])
                    & PID_MASK;
            let info_length = read_length(stream, 3);
            let descriptors_start = cursor.saturating_add(PMT_STREAM_HEADER_BYTES);
            let descriptors_end = descriptors_start.saturating_add(info_length);
            let Some(descriptors) = body.get(descriptors_start..descriptors_end) else {
                out(Item::Diagnostic(Diagnostic::at(offset, DiagnosticKind::BadSection { pid })));
                return;
            };
            if stream_type == STREAM_TYPE_PRIVATE_PES {
                self.stream_descriptors(number, stream_pid, descriptors, out);
            }
            cursor = descriptors_end;
        }
    }

    /// Reports the teletext entries of one elementary stream and selects it
    /// when it is the one configured or the first found.
    fn stream_descriptors(&mut self, program: u16, pid: u16, descriptors: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut teletext = false;
        let mut subtitle: Option<PageAddress> = None;
        let mut cursor = 0usize;
        while let Some(&[tag, length]) = descriptors.get(cursor..cursor.saturating_add(DESCRIPTOR_HEADER_BYTES)) {
            let start = cursor.saturating_add(DESCRIPTOR_HEADER_BYTES);
            let end = start.saturating_add(usize::from(length));
            let Some(contents) = descriptors.get(start..end) else { break };
            match tag {
                DESCRIPTOR_TELETEXT | DESCRIPTOR_VBI_TELETEXT => {
                    teletext = true;
                    let (entries, _) = contents.as_chunks::<TELETEXT_ENTRY_BYTES>();
                    for &[l0, l1, l2, type_magazine, page] in entries {
                        let address = PageAddress {
                            magazine: Magazine::from_bits(type_magazine & TELETEXT_MAGAZINE_MASK),
                            page: PageNumber(page),
                        };
                        let kind = TeletextType::from_bits(type_magazine >> TELETEXT_TYPE_SHIFT);
                        if kind.is_subtitle() && subtitle.is_none() {
                            subtitle = Some(address);
                        }
                        let entry = TeletextEntry { language: [l0, l1, l2], kind, page: address, descriptor: tag };
                        out(Item::TeletextEntry { program, pid, entry });
                    }
                }
                DESCRIPTOR_VBI_DATA => teletext |= vbi_data_lists_teletext(contents),
                _ => {}
            }
            cursor = end;
        }
        if !teletext {
            return;
        }
        let wanted = match (self.config.pid, self.config.program) {
            (Some(wanted), _) => wanted == pid,
            (None, Some(wanted)) => wanted == program,
            (None, None) => true,
        };
        if self.selected.is_none() && wanted {
            self.selected = Some(pid);
            self.selected_program = Some(program);
            out(Item::StreamSelected { program: Some(program), pid });
        }
        if self.selected == Some(pid)
            && !self.subtitle_reported
            && let Some(address) = subtitle
        {
            self.subtitle_reported = true;
            out(Item::SubtitlePage(address));
        }
    }
}

/// A 12-bit length at `index` of a section (`program_info_length` and
/// `ES_info_length`).
fn read_length(bytes: &[u8], index: usize) -> usize {
    let high = bytes.get(index).copied().unwrap_or(0);
    let low = bytes.get(index.saturating_add(1)).copied().unwrap_or(0);
    usize::from(u16::from_be_bytes([high, low]) & SECTION_LENGTH_MASK)
}

/// True when a VBI data descriptor lists EBU or inverted teletext
/// (EN 300 468 clause 6.2.47).
fn vbi_data_lists_teletext(contents: &[u8]) -> bool {
    let mut cursor = 0usize;
    while let Some(&[service, length]) = contents.get(cursor..cursor.saturating_add(DESCRIPTOR_HEADER_BYTES)) {
        if service == DATA_SERVICE_EBU_TELETEXT || service == DATA_SERVICE_INVERTED_TELETEXT {
            return true;
        }
        cursor = cursor.saturating_add(DESCRIPTOR_HEADER_BYTES).saturating_add(usize::from(length));
    }
    false
}

impl Reader for TsReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut rest = bytes;
        while !rest.is_empty() {
            if self.start > 0 {
                self.buffer.copy_within(self.start..self.end, 0);
                self.end = self.end.saturating_sub(self.start);
                self.start = 0;
            }
            let take = BUFFER_BYTES.saturating_sub(self.end).min(rest.len());
            let (now, later) = rest.split_at(take);
            if let Some(target) = self.buffer.get_mut(self.end..self.end.saturating_add(take)) {
                target.copy_from_slice(now);
            }
            self.end = self.end.saturating_add(take);
            rest = later;
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
