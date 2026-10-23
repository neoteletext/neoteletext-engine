//! Packed enhancement lines (`OL,26` to `OL,29`, REQ-INP-023).

use neoteletext_core::consts::{NIBBLE_MASK, PACKET_ENHANCEMENT, PACKET_LINKS, PACKET_PAGE_ENHANCEMENT};
use neoteletext_core::packet::{Decoded, PageEnhancement, PagePacket, decode};
use neoteletext_core::types::PageAddress;

use crate::consts::TTI_PACKED_CHARACTERS;
use crate::item::{Capacity, DiagnosticKind, Item};
use crate::page::{EnhancementPacket, FastextLink, PageFunction, RawBody, RawPacket};

use super::TtiReader;
use super::row_text::{PACKED_LINE, decode_text};

impl TtiReader {
    pub(super) fn packed_line(&mut self, number: u8, text: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let decoded = decode_text(text, false, PACKED_LINE);
        // REQ-INP-023: exactly the designation code and 39 characters.
        if decoded.count != PACKED_LINE || decoded.overflow || decoded.bad_escape {
            self.diagnostic(DiagnosticKind::BadArgument { command: *b"OL" }, out);
        }
        let designation = decoded.codes.first().copied().unwrap_or(0) & NIBBLE_MASK;
        let values = decoded.codes.get(1..).unwrap_or(&[]);
        let magazine = self.page.address.magazine;
        if number == PACKET_ENHANCEMENT {
            let packet = EnhancementPacket { designation, triplets: crate::encode::unpack_triplets(values) };
            let packets = self.page.enhancements.as_mut_slice();
            if let Some(existing) = packets.iter_mut().find(|packet| packet.designation == designation) {
                *existing = packet;
            } else if !self.page.enhancements.push(packet) {
                self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Enhancements }, out);
            }
            return;
        }
        // Designation codes 0 to 3 of X/27 carry Hamming 8/4 nibbles, one per
        // character (vbit2).
        let nibbles = number == PACKET_LINKS && designation <= 3;
        let body = if nibbles {
            let mut values_4 = [0; TTI_PACKED_CHARACTERS];
            for (slot, &value) in values_4.iter_mut().zip(values) {
                *slot = value & NIBBLE_MASK;
            }
            RawBody::Nibbles(values_4)
        } else {
            RawBody::Triplets(crate::encode::unpack_triplets(values))
        };
        let raw = RawPacket { number, designation, body };
        let packet = crate::encode::raw_packet(magazine, &raw);
        if number == PACKET_LINKS && designation == 0 {
            if let Ok(Decoded::Links(links)) = decode(&packet) {
                for (slot, link) in self.page.fastext.iter_mut().zip(links.links) {
                    *slot = link.map(|link| FastextLink {
                        address: PageAddress { magazine: link.magazine, page: link.page },
                        subcode: link.subcode,
                    });
                }
                self.page.fastext_row_24 = links.display_row_24;
            }
            return;
        }
        // REQ-INP-023: X/28/0, X/28/3 and X/28/4 also set the page function
        // and coding, since they are what a decoder receives. They win over a
        // `PF` line, and the lowest designation code wins among them, before
        // or after in the file alike, so that writing a page read from a
        // file and reading it back gives the same page.
        if number == PACKET_PAGE_ENHANCEMENT
            && self.function_designation.is_none_or(|earlier| designation <= earlier)
            && let Ok(Decoded::PageData { packet, .. }) = decode(&packet)
        {
            let format = match packet {
                PagePacket::Format1(PageEnhancement { format, .. })
                | PagePacket::Level35(PageEnhancement { format, .. })
                | PagePacket::PageFunction(format) => format,
            };
            if let Some(format) = format {
                self.page.function = Some(PageFunction { function: format.function, coding: format.coding });
                self.function_designation = Some(designation);
            }
        }
        let packets = self.page.raw_packets.as_mut_slice();
        if let Some(existing) =
            packets.iter_mut().find(|packet| packet.number == number && packet.designation == designation)
        {
            *existing = raw;
        } else if !self.page.raw_packets.push(raw) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::RawPackets }, out);
        }
    }
}
