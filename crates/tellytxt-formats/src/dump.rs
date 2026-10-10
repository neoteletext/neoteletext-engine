//! Raw screen dumps (REQ-INP-017): 1000 bytes (25 rows of 40) or 960 bytes
//! (24 rows), each byte masked to 7 bits, which also reads the BBC
//! Microcomputer form with every top bit set.
//!
//! The Phase 2 option that treats a repeated row under a double-height row
//! as the ignored lower row is not implemented.

use tellytxt_core::consts::{CODE_MASK_7BIT, COLUMNS};

use crate::consts::{DUMP_BYTES_24, DUMP_BYTES_25};
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of};
use crate::page::PageFile;

/// Reads a whole dump into `page`. Returns false when `data` is neither
/// size.
pub fn parse(data: &[u8], page: &mut PageFile) -> bool {
    if data.len() != DUMP_BYTES_25 && data.len() != DUMP_BYTES_24 {
        return false;
    }
    page.clear();
    for (row, codes) in data.chunks(COLUMNS).enumerate() {
        let mut masked = [0; COLUMNS];
        for (slot, &code) in masked.iter_mut().zip(codes) {
            *slot = code & CODE_MASK_7BIT;
        }
        page.set_row(row, &masked);
    }
    true
}

/// A reader for one screen dump, held whole and read when the input ends.
#[derive(Clone, Debug)]
pub struct DumpReader {
    buffer: [u8; DUMP_BYTES_25],
    len: usize,
    total: u64,
    page: PageFile,
}

impl Default for DumpReader {
    fn default() -> Self {
        Self::new()
    }
}

impl DumpReader {
    /// A reader.
    #[must_use]
    pub const fn new() -> Self {
        Self { buffer: [0; DUMP_BYTES_25], len: 0, total: 0, page: PageFile::new() }
    }
}

impl Reader for DumpReader {
    fn feed(&mut self, bytes: &[u8], _out: &mut dyn FnMut(Item<'_>)) {
        self.total = offset_of(self.total, bytes.len());
        let take = DUMP_BYTES_25.saturating_sub(self.len).min(bytes.len());
        if let (Some(target), Some(source)) =
            (self.buffer.get_mut(self.len..self.len.saturating_add(take)), bytes.get(..take))
        {
            target.copy_from_slice(source);
        }
        self.len = self.len.saturating_add(take);
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        let data = self.buffer;
        let whole = u64::try_from(self.len).is_ok_and(|len| len == self.total);
        if whole && parse(data.get(..self.len).unwrap_or(&[]), &mut self.page) {
            out(Item::Page(&self.page));
        } else {
            let size = u32::try_from(self.total).unwrap_or(u32::MAX);
            out(Item::Diagnostic(Diagnostic::at(0, DiagnosticKind::BadSize { size })));
        }
        self.len = 0;
        self.total = 0;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn read(data: &[u8]) -> (Option<PageFile>, Vec<Diagnostic>) {
        let mut reader = DumpReader::new();
        let mut page = None;
        let mut diagnostics = Vec::new();
        let mut out = |item: Item<'_>| match item {
            Item::Page(found) => page = Some(*found),
            Item::Diagnostic(diagnostic) => diagnostics.push(diagnostic),
            _ => {}
        };
        for byte in data {
            reader.feed(core::slice::from_ref(byte), &mut out);
        }
        reader.finish(&mut out);
        (page, diagnostics)
    }

    #[test]
    fn reads_both_sizes_and_masks_the_top_bit() {
        let mut data = std::vec![0xC1u8; 1000];
        data[40] = 0x8D;
        let (page, diagnostics) = read(&data);
        let page = page.unwrap();
        assert_eq!(diagnostics, []);
        assert_eq!(page.row(0).unwrap()[0], b'A');
        assert_eq!(page.row(1).unwrap()[0], 0x0D);
        assert!(page.has_row(24));
        let (page, _) = read(&data[..960]);
        assert!(!page.unwrap().has_row(24));
        let (page, diagnostics) = read(&data[..999]);
        assert!(page.is_none());
        assert_eq!(diagnostics[0].kind, DiagnosticKind::BadSize { size: 999 });
        let mut long = data.clone();
        long.push(0);
        assert!(read(&long).0.is_none());
    }
}
