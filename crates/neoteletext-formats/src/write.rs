//! Writers (feature `std`): T42 packet streams (REQ-INP-018), whose page
//! encoding needs the `pages` feature, TTI page files (REQ-INP-014), which
//! need the `tti` feature, and 12 by 10 BDF fonts (REQ-FNT-002), which need
//! the `bdf` feature.

use std::io::{self, Write};

use neoteletext_core::consts::{MAGAZINES, PACKET_HEADER, PACKET_MAGAZINE_ENHANCEMENT};
use neoteletext_core::packet::{Decoded, Packet, decode, decode_address};
use neoteletext_core::types::{Magazine, PageNumber, SubCode};

/// Which received packets a [`T42Writer`] passes through.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PacketFilter {
    /// Only this magazine.
    pub magazine: Option<Magazine>,
    /// Only this page: its header and the packets that follow it in its
    /// magazine.
    pub page: Option<PageNumber>,
    /// Of that page, only this sub-page.
    pub subcode: Option<SubCode>,
}

/// Writes T42: 42 bytes per packet, no header (REQ-INP-018).
#[derive(Debug)]
pub struct T42Writer<W: Write> {
    inner: W,
    filter: PacketFilter,
    // Whether each magazine's page in progress is the selected one.
    selected: [bool; MAGAZINES],
}

impl<W: Write> T42Writer<W> {
    /// A writer passing every packet.
    pub fn new(inner: W) -> Self {
        Self::with_filter(inner, PacketFilter::default())
    }

    /// A writer passing only the packets the filter selects.
    pub fn with_filter(inner: W, filter: PacketFilter) -> Self {
        Self { inner, filter, selected: [false; MAGAZINES] }
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
        let selected = self.selected.get_mut(address.magazine.index());
        if address.number == PACKET_HEADER {
            let header = match decode(packet) {
                Ok(Decoded::Header(header)) => Some(header),
                _ => None,
            };
            let wanted_page = header.is_some_and(|header| {
                header.page == Some(wanted) && self.filter.subcode.is_none_or(|code| header.subcode == Some(code))
            });
            if let Some(slot) = selected {
                *slot = wanted_page;
            }
            return wanted_page;
        }
        // Magazine packets 29 and service packets belong to no page.
        address.number < PACKET_MAGAZINE_ENHANCEMENT && selected.is_some_and(|slot| *slot)
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
}

#[cfg(feature = "pages")]
mod pages;
#[cfg(feature = "pages")]
pub use pages::in_transmission_order;
#[cfg(feature = "tti")]
mod tti;
#[cfg(feature = "tti")]
pub use tti::{write_log_line, write_tti};
#[cfg(feature = "bdf")]
mod font;
#[cfg(feature = "bdf")]
pub use font::{FontText, write_font};

// The round trips read back what they write.
#[cfg(all(test, feature = "tti"))]
mod tests;
