//! EP1 page files (REQ-INP-015), as written by Softel Flair and the zxnet
//! and edit.tf editors (`research/notes-formats.md` section 6.1).

use neoteletext_core::consts::{
    CODE_MASK_7BIT, COLUMNS, NATIONAL_INDEX_MASK, NIBBLE_MASK, ROW_ADDRESS_LAST, TRIPLETS_PER_PACKET,
};
use neoteletext_core::consts::{TRIPLET_ADDRESS_MASK, TRIPLET_DATA_FIELD_MASK, TRIPLET_MODE_MASK};
use neoteletext_core::packet::Triplet;

use crate::consts::{
    EP1_BLOCK_MARKER, EP1_ENHANCED, EP1_ENHANCEMENT_HEADER_BYTES, EP1_FLAG, EP1_HEADER_BYTES, EP1_LANGUAGE,
    EP1_LANGUAGES, EP1_MAX_PACKETS, EP1_OFFSET, EP1_PACKET_BYTES, EP1_ROWS, EP1_SIGNATURE, EP1_TERMINATOR_ADDRESS,
    EP1_TERMINATOR_BYTES,
};
use crate::encode::DESIGNATION_GROUP_SHIFT;
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of, saturate_u32};
use crate::page::{EnhancementPacket, PageFile};
use crate::text::copy_prefix;

/// Bytes of the Level 1 data: 24 rows, a further row and the terminator.
const LEVEL1_BYTES: usize = (EP1_ROWS + 1) * COLUMNS + EP1_TERMINATOR_BYTES;
/// The largest EP1 file: header, enhancement block with sixteen packets and
/// the Level 1 data.
pub const EP1_MAX_BYTES: usize =
    EP1_HEADER_BYTES + EP1_ENHANCEMENT_HEADER_BYTES + EP1_MAX_PACKETS * EP1_PACKET_BYTES + LEVEL1_BYTES;

/// Applies a Table 32 designation value to a page: the national option
/// index goes into C12 to C14 and the group into the region.
pub(crate) fn apply_designation(page: &mut PageFile, designation: u8) {
    let group = designation >> DESIGNATION_GROUP_SHIFT;
    page.set_national_index(designation & NATIONAL_INDEX_MASK);
    page.region = if group == 0 { None } else { Some(group & NIBBLE_MASK) };
}

/// Reads one EP1 file held whole in `data` into `page`. Returns false, with
/// diagnostics, when the file cannot be read.
pub fn parse(data: &[u8], page: &mut PageFile, out: &mut dyn FnMut(Item<'_>)) -> bool {
    page.clear();
    if !data.starts_with(&EP1_SIGNATURE) {
        out(Item::Diagnostic(Diagnostic::at(0, DiagnosticKind::BadSignature)));
        return false;
    }
    let byte = |index: usize| data.get(index).copied().unwrap_or(0);
    let language = byte(EP1_LANGUAGE);
    match EP1_LANGUAGES.iter().find(|(code, _)| *code == language) {
        Some(&(_, designation)) => apply_designation(page, designation),
        None => out(Item::Diagnostic(Diagnostic::at(
            u64::try_from(EP1_LANGUAGE).unwrap_or(0),
            DiagnosticKind::UnmappedLanguage { code: language },
        ))),
    }
    // REQ-GEN-007: the offset is little-endian whatever the host.
    let offset = usize::from(u16::from_le_bytes([byte(EP1_OFFSET), byte(EP1_OFFSET.saturating_add(1))]));
    let level1 = EP1_HEADER_BYTES.saturating_add(offset);
    let expected = level1.saturating_add(LEVEL1_BYTES);
    if expected > EP1_MAX_BYTES || data.len() < expected {
        out(Item::Diagnostic(Diagnostic::at(0, DiagnosticKind::BadSize { size: saturate_u32(data.len()) })));
        return false;
    }
    if data.len() > expected {
        let kind = DiagnosticKind::SkippedBytes { count: saturate_u32(data.len().saturating_sub(expected)) };
        out(Item::Diagnostic(Diagnostic::at(u64::try_from(expected).unwrap_or(u64::MAX), kind)));
    }
    if byte(EP1_FLAG) == EP1_ENHANCED {
        enhancements(data, offset, page, out);
    }
    for row in 0..=EP1_ROWS {
        let start = level1.saturating_add(row.saturating_mul(COLUMNS));
        if let Some(codes) = data.get(start..start.saturating_add(COLUMNS)) {
            let mut masked = [0; COLUMNS];
            for (slot, &code) in masked.iter_mut().zip(codes) {
                *slot = code & CODE_MASK_7BIT;
            }
            page.set_row(row, &masked);
        }
    }
    true
}

/// Reads the enhancement block: up to sixteen packets of a designation code
/// and thirteen unprotected triplets stored as address, mode and data bytes.
fn enhancements(data: &[u8], offset: usize, page: &mut PageFile, out: &mut dyn FnMut(Item<'_>)) {
    let block = EP1_HEADER_BYTES;
    if data.get(block..block.saturating_add(EP1_BLOCK_MARKER.len())) != Some(&EP1_BLOCK_MARKER[..]) {
        out(Item::Diagnostic(Diagnostic::at(u64::try_from(block).unwrap_or(0), DiagnosticKind::BadSignature)));
        return;
    }
    let count = offset.saturating_sub(EP1_ENHANCEMENT_HEADER_BYTES).checked_div(EP1_PACKET_BYTES).unwrap_or(0);
    let first = block.saturating_add(EP1_ENHANCEMENT_HEADER_BYTES);
    for index in 0..count.min(EP1_MAX_PACKETS) {
        let start = first.saturating_add(index.saturating_mul(EP1_PACKET_BYTES));
        let Some(stored) = data.get(start..start.saturating_add(EP1_PACKET_BYTES)) else { break };
        let Some((&designation, triplet_bytes)) = stored.split_first() else { break };
        let mut packet = EnhancementPacket { designation: designation & NIBBLE_MASK, ..EnhancementPacket::default() };
        let (chunks, _) = triplet_bytes.as_chunks::<3>();
        for (slot, &[address, mode, value]) in packet.triplets.iter_mut().take(TRIPLETS_PER_PACKET).zip(chunks) {
            let address = if address == EP1_TERMINATOR_ADDRESS { ROW_ADDRESS_LAST } else { address };
            *slot = Triplet {
                address: address & TRIPLET_ADDRESS_MASK as u8,
                mode: mode & TRIPLET_MODE_MASK as u8,
                data: value & TRIPLET_DATA_FIELD_MASK as u8,
            };
        }
        let _ = page.enhancements.push(packet);
    }
}

/// A reader for one EP1 file. The file is held whole, at most
/// [`EP1_MAX_BYTES`], and read when the input ends.
#[derive(Clone, Debug)]
pub struct Ep1Reader {
    buffer: [u8; EP1_MAX_BYTES],
    len: usize,
    total: u64,
    page: PageFile,
}

impl Default for Ep1Reader {
    fn default() -> Self {
        Self::new()
    }
}

impl Ep1Reader {
    /// A reader.
    #[must_use]
    pub const fn new() -> Self {
        Self { buffer: [0; EP1_MAX_BYTES], len: 0, total: 0, page: PageFile::new() }
    }
}

impl Reader for Ep1Reader {
    fn feed(&mut self, bytes: &[u8], _out: &mut dyn FnMut(Item<'_>)) {
        self.total = offset_of(self.total, bytes.len());
        let take = copy_prefix(self.buffer.get_mut(self.len..).unwrap_or(&mut []), bytes);
        self.len = self.len.saturating_add(take);
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if self.total > u64::try_from(EP1_MAX_BYTES).unwrap_or(u64::MAX) {
            let size = u32::try_from(self.total).unwrap_or(u32::MAX);
            out(Item::Diagnostic(Diagnostic::at(0, DiagnosticKind::BadSize { size })));
            return;
        }
        let data = self.buffer;
        if parse(data.get(..self.len).unwrap_or(&[]), &mut self.page, out) {
            out(Item::Page(&self.page));
        }
        self.len = 0;
        self.total = 0;
    }
}

#[cfg(test)]
pub(crate) mod tests;
