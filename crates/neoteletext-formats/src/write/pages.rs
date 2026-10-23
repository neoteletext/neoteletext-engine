//! Writing encoded pages as T42 (REQ-INP-018, REQ-INP-024), compiled with
//! the `pages` feature.

use std::io::{self, Write};

use crate::encode::{encode_page, transmission_class};
use crate::page::PageFile;

use super::T42Writer;

impl<W: Write> T42Writer<W> {
    /// Encodes a set of pages in the order of Annex B.7 (REQ-INP-024):
    /// magazine packets, the Magazine Organization Tables, the object pages,
    /// other data pages and then the Level 1 pages, each class in the order
    /// given. The filter does not apply.
    pub fn write_pages(&mut self, pages: &[&PageFile]) -> io::Result<()> {
        for page in in_transmission_order(pages) {
            self.write_page(page)?;
        }
        Ok(())
    }

    /// Encodes a page and writes its packets in the order of Annex B.2. The
    /// filter does not apply.
    pub fn write_page(&mut self, page: &PageFile) -> io::Result<()> {
        let mut result = Ok(());
        let inner = &mut self.inner;
        encode_page(page, &mut |packet| {
            if result.is_ok() {
                result = inner.write_all(&packet.bytes);
            }
        });
        result
    }
}

/// The pages in the order of Annex B.7, each class keeping the order
/// given ([`crate::encode::TransmissionClass`]).
#[must_use]
pub fn in_transmission_order<'p>(pages: &[&'p PageFile]) -> std::vec::Vec<&'p PageFile> {
    let mut ordered: std::vec::Vec<&PageFile> = pages.to_vec();
    ordered.sort_by_key(|page| transmission_class(page));
    ordered
}
