//! The Program Association and Program Map Tables (ISO/IEC 13818-1
//! clause 2.4.4) and the teletext descriptors of EN 300 468 (6.2.43, 6.2.47,
//! 6.2.48) that select the teletext streams.

use neoteletext_core::types::{Magazine, PageAddress, PageNumber};

use crate::consts::{
    DATA_SERVICE_EBU_TELETEXT, DATA_SERVICE_INVERTED_TELETEXT, DESCRIPTOR_HEADER_BYTES, DESCRIPTOR_TELETEXT,
    DESCRIPTOR_VBI_DATA, DESCRIPTOR_VBI_TELETEXT, PAT_ENTRIES_START, PAT_ENTRY_BYTES, PID_MASK, PID_PAT,
    PMT_DESCRIPTORS_START, PMT_INFO_LENGTH, PMT_STREAM_HEADER_BYTES, SECTION_CRC_BYTES, SECTION_LENGTH_MASK,
    SECTION_VERSION_BYTE, SECTION_VERSION_MASK, SECTION_VERSION_SHIFT, STREAM_TYPE_PRIVATE_PES, TABLE_ID_PAT,
    TABLE_ID_PMT, TELETEXT_ENTRY_BYTES, TELETEXT_MAGAZINE_MASK, TELETEXT_TYPE_SHIFT,
};
use crate::item::{Capacity, Diagnostic, DiagnosticKind, Item};
use crate::transport::{TeletextEntry, TeletextType};

use super::section::{Continuity, Push, SECTION_CURRENT, SECTION_SYNTAX, check_continuity, crc32};
use super::{Header, Program, Section, TsReader};

impl TsReader {
    pub(super) fn section_packet(
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
