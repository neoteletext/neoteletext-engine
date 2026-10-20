//! Writers (feature `std`): T42 packet streams (REQ-INP-018), whose page
//! encoding needs the `pages` feature, TTI page files (REQ-INP-014), which
//! need the `tti` feature, and 12 by 10 BDF fonts (REQ-FNT-002), which need
//! the `bdf` feature.

use std::io::{self, Write};

use neoteletext_core::consts::{MAGAZINES, PACKET_HEADER, PACKET_MAGAZINE_ENHANCEMENT};
use neoteletext_core::packet::{Decoded, Packet, decode, decode_address};
use neoteletext_core::types::{Magazine, PageNumber};

#[cfg(feature = "pages")]
use crate::encode::{encode_page, transmission_class};
#[cfg(feature = "pages")]
use crate::page::PageFile;

/// Which received packets a [`T42Writer`] passes through.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PacketFilter {
    /// Only this magazine.
    pub magazine: Option<Magazine>,
    /// Only this page: its header and the packets that follow it in its
    /// magazine.
    pub page: Option<PageNumber>,
}

/// Writes T42: 42 bytes per packet, no header (REQ-INP-018).
#[derive(Debug)]
pub struct T42Writer<W: Write> {
    inner: W,
    filter: PacketFilter,
    current: [Option<PageNumber>; MAGAZINES],
}

impl<W: Write> T42Writer<W> {
    /// A writer passing every packet.
    pub fn new(inner: W) -> Self {
        Self::with_filter(inner, PacketFilter::default())
    }

    /// A writer passing only the packets the filter selects.
    pub fn with_filter(inner: W, filter: PacketFilter) -> Self {
        Self { inner, filter, current: [None; MAGAZINES] }
    }

    /// The underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// True when the filter passes a packet. Packets with an uncorrectable
    /// address pass only when there is no filter.
    fn passes(&mut self, packet: &Packet) -> bool {
        let Ok(address) = decode_address(packet) else {
            return self.filter == PacketFilter::default();
        };
        if self.filter.magazine.is_some_and(|magazine| magazine != address.magazine) {
            return false;
        }
        let Some(wanted) = self.filter.page else { return true };
        let current = self.current.get_mut(address.magazine.index());
        if address.number == PACKET_HEADER {
            let page = match decode(packet) {
                Ok(Decoded::Header(header)) => header.page,
                _ => None,
            };
            if let Some(slot) = current {
                *slot = page;
            }
            return page == Some(wanted);
        }
        // Magazine packets 29 and service packets belong to no page.
        address.number < PACKET_MAGAZINE_ENHANCEMENT && current.is_some_and(|page| *page == Some(wanted))
    }

    /// Writes a received packet if the filter passes it. Returns whether it
    /// was written.
    pub fn write_packet(&mut self, packet: &Packet) -> io::Result<bool> {
        if !self.passes(packet) {
            return Ok(false);
        }
        self.inner.write_all(&packet.bytes)?;
        Ok(true)
    }

    /// Encodes a set of pages in the order of Annex B.7 (REQ-INP-024):
    /// magazine packets, the Magazine Organization Tables, the object pages,
    /// other data pages and then the Level 1 pages, each class in the order
    /// given. The filter does not apply.
    #[cfg(feature = "pages")]
    pub fn write_pages(&mut self, pages: &[&PageFile]) -> io::Result<()> {
        for page in in_transmission_order(pages) {
            self.write_page(page)?;
        }
        Ok(())
    }

    /// Encodes a page and writes its packets in the order of Annex B.2. The
    /// filter does not apply.
    #[cfg(feature = "pages")]
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
#[cfg(feature = "pages")]
#[must_use]
pub fn in_transmission_order<'p>(pages: &[&'p PageFile]) -> std::vec::Vec<&'p PageFile> {
    let mut ordered: std::vec::Vec<&PageFile> = pages.to_vec();
    ordered.sort_by_key(|page| transmission_class(page));
    ordered
}

#[cfg(feature = "tti")]
mod tti;
#[cfg(feature = "tti")]
pub use tti::write_tti;
#[cfg(feature = "bdf")]
mod font;
#[cfg(feature = "bdf")]
pub use font::{FontText, write_font};

// The round trips read back what they write.
#[cfg(all(test, feature = "tti"))]
mod tests;
