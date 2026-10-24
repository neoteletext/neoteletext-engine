//! The reception of packets (Annex B.3): the decoding of each packet into
//! the store, and the row-like packets' path to the page their magazine is
//! receiving.

use super::{Decoder, Discard, Event, StoredPage, enhancement_index, row_index};
use crate::config::ParityPolicy;
use crate::consts::{
    DATA_BYTES, DESIGNATION_LINKS_LEVEL_25, LINK_FUNCTION_GPOP, LINK_FUNCTION_POP, SERVICE_FORMAT_2_BIT,
};
use crate::fsm::{MagazineAction, MagazineEvent, PageEvent};
use crate::packet::{
    CompositionalLink, Decoded, Enhancement, MagazinePacket, Packet, PacketError, PacketMeta, PagePacket, Row,
    ServicePacket, decode,
};
use crate::types::{Instant, Magazine, PageAddress};

impl Decoder<'_> {
    /// Processes one packet.
    pub fn push(&mut self, packet: &Packet, _meta: &PacketMeta, now: Instant, events: &mut dyn FnMut(Event)) {
        let decoded = match decode(packet) {
            Ok(decoded) => decoded,
            Err(error) => {
                self.count_failure(error);
                events(Event::Discarded(Discard::Packet(error)));
                return;
            }
        };
        match decoded {
            Decoded::Header(header) => {
                self.count(header.magazine, 0);
                self.header(&header, now, events);
            }
            Decoded::Row(row) => self.display_row(&row, now, events),
            Decoded::Enhancement(enhancement) => self.enhancement_packet(enhancement, packet, now, events),
            Decoded::Links(links) => {
                self.count(links.magazine, crate::consts::PACKET_LINKS);
                self.row_like(links.magazine, now, events, None, |page, _| {
                    page.links = Some(links);
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::LinksRaw { magazine, designation, bytes } => {
                self.count(magazine, crate::consts::PACKET_LINKS);
                self.row_like(magazine, now, events, None, |page, _| {
                    let index = usize::from(designation.saturating_sub(1));
                    if let Some(slot) = page.links_raw.get_mut(index) {
                        *slot = bytes;
                        page.links_raw_present |= 1u8.checked_shl(index as u32).unwrap_or(0);
                    }
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::CompositionalLinks { .. } | Decoded::PageData { .. } | Decoded::Magazine { .. } => {
                self.enhancement_data(decoded, now, events);
            }
            Decoded::Service(service) => self.service_packet(service, events),
            Decoded::Ignored(address) => {
                self.count(address.magazine, address.number);
                self.stats.ignored = self.stats.ignored.saturating_add(1);
            }
        }
    }

    /// Counts a packet whose address or designation failed to decode.
    fn count_failure(&mut self, error: PacketError) {
        match error {
            PacketError::BadAddress => {
                self.stats.address_failures = self.stats.address_failures.saturating_add(1);
            }
            PacketError::BadDesignation => {
                self.stats.designation_failures = self.stats.designation_failures.saturating_add(1);
            }
        }
    }

    /// Stores a display row packet X/1 to X/25 into its page.
    fn display_row(&mut self, row: &Row, now: Instant, events: &mut dyn FnMut(Event)) {
        self.count(row.magazine, row.row);
        let as_received = row_index(row.row).map(|index| (index, &row.bytes));
        self.row_like(row.magazine, now, events, as_received, |page, policy| {
            let failures = page.merge(usize::from(row.row), 0, &row.bytes, policy);
            (row.row, failures)
        });
    }

    /// Stores an X/26 enhancement packet's triplets into its page, counting
    /// the triplets that failed to decode.
    fn enhancement_packet(
        &mut self,
        enhancement: Enhancement,
        packet: &Packet,
        now: Instant,
        events: &mut dyn FnMut(Event),
    ) {
        self.count(enhancement.magazine, crate::consts::PACKET_ENHANCEMENT);
        let failures = enhancement.triplets.iter().filter(|t| t.is_none()).count() as u32;
        self.stats.triplet_failures = self.stats.triplet_failures.saturating_add(failures);
        let data = packet.data();
        let as_received = enhancement_index(enhancement.designation).map(|index| (index, &data));
        self.row_like(enhancement.magazine, now, events, as_received, |page, _| {
            let code = usize::from(enhancement.designation);
            if let Some(slot) = page.enhancements.get_mut(code) {
                *slot = enhancement.triplets;
                page.enhancements_present |= 1u16.checked_shl(code as u32).unwrap_or(0);
            }
            (crate::consts::PACKET_ENHANCEMENT, 0)
        });
    }

    /// Takes a broadcast service data packet 8/30 (REQ-ACQ-018).
    fn service_packet(&mut self, service: ServicePacket, events: &mut dyn FnMut(Event)) {
        self.count(Magazine::from_bits(0), crate::consts::PACKET_SERVICE);
        // REQ-ACQ-018: a new network identification in format 1
        // means a new service, whose magazine data differs.
        let format_1 = |packet: &ServicePacket| packet.designation & SERVICE_FORMAT_2_BIT == 0;
        if format_1(&service) && self.service.is_some_and(|old| format_1(&old) && old.network_id != service.network_id)
        {
            self.discard_magazine_data(events);
        }
        self.service = Some(service);
        events(Event::ServiceData);
    }

    /// Stores the page-related packets X/27/4, X/27/5 and X/28, and the
    /// magazine-related packets M/29 (REQ-ACQ-007, REQ-ACQ-009).
    fn enhancement_data(&mut self, decoded: Decoded, now: Instant, events: &mut dyn FnMut(Event)) {
        match decoded {
            Decoded::CompositionalLinks { magazine, designation, links } => {
                self.count(magazine, crate::consts::PACKET_LINKS);
                // Only object page links are followed (decision E.95).
                let object_link =
                    |link: &&CompositionalLink| matches!(link.function, LINK_FUNCTION_GPOP | LINK_FUNCTION_POP);
                for link in
                    links.iter().flatten().filter(object_link).filter(|link| link.is_format_1() && !link.is_null())
                {
                    self.name(PageAddress { magazine: link.magazine, page: link.page });
                }
                self.row_like(magazine, now, events, None, |page, _| {
                    let index = usize::from(designation.saturating_sub(DESIGNATION_LINKS_LEVEL_25));
                    if let Some(slot) = page.compositional_links.get_mut(index) {
                        *slot = Some(links);
                    }
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::PageData { magazine, packet } => {
                self.count(magazine, crate::consts::PACKET_PAGE_ENHANCEMENT);
                let stored = self.row_like(magazine, now, events, None, |page, _| {
                    match packet {
                        PagePacket::Format1(data) => page.page_data = Some(data),
                        PagePacket::Level35(data) => page.page_data_level35 = Some(data),
                        // X/28/3 belongs to pattern download pages, which
                        // are not text pages (REQ-OBJ-041, decision E.117).
                        PagePacket::PageFunction(format) => page.download_function = format,
                    }
                    (crate::consts::PACKET_PAGE_ENHANCEMENT, 0)
                });
                let format = match packet {
                    PagePacket::Format1(data) => data.format,
                    PagePacket::PageFunction(format) => format,
                    PagePacket::Level35(_) => None,
                };
                if let (Some(index), Some(format)) = (stored, format) {
                    self.page_format(index, format, now, events);
                }
            }
            Decoded::Magazine { magazine, packet } => {
                self.count(magazine, crate::consts::PACKET_MAGAZINE_ENHANCEMENT);
                // Magazine packets repeat with the cycle; only one that
                // changes what every page of the magazine may draw from is
                // an event (decision E.103).
                let changed = self.magazine_data.get_mut(magazine.index()).is_some_and(|slot| {
                    let before = *slot;
                    match packet {
                        MagazinePacket::Data(fields) => slot.data = Some(fields),
                        MagazinePacket::Level35(fields) => slot.level35 = Some(fields),
                    }
                    *slot != before
                });
                if changed {
                    events(Event::MagazineData(magazine));
                }
            }
            _ => {}
        }
    }

    /// Handles a row-like packet (rows 1 to 28) of a magazine. In serial
    /// mode a packet of another magazine is discarded and counted
    /// (REQ-ACQ-002, Annex B.3). `as_received` is the packet's index and
    /// data bytes for the enhancement store, where it has a place there.
    /// Returns the index of the magazine whose page took the packet.
    fn row_like(
        &mut self,
        magazine: Magazine,
        now: Instant,
        events: &mut dyn FnMut(Event),
        as_received: Option<(usize, &[u8; DATA_BYTES])>,
        store: impl FnOnce(&mut StoredPage, ParityPolicy) -> (u8, u32),
    ) -> Option<usize> {
        let (index, event) = match self.serial_active {
            Some(active) if self.serial && active != magazine => {
                (active.index(), MagazineEvent::RowOfOtherMagazineSerial)
            }
            _ => (magazine.index(), MagazineEvent::Row),
        };
        let action = self.magazine_event(index, event, now, events);
        if let MagazineAction::Store(_) = action {
            if let Some(slot) = self.receiving_slot.get(index).copied().flatten() {
                let policy = self.config.parity_policy;
                let mut stored = None;
                if let Some(page) = self.store.page_mut(slot) {
                    let (row, failures) = store(page, policy);
                    page.last_received = now;
                    page.errors = page.errors.saturating_add(failures);
                    stored = Some((row, failures));
                }
                if let Some((row, failures)) = stored {
                    self.stats.parity_failures = self.stats.parity_failures.saturating_add(failures);
                    self.page_event(slot, PageEvent::RowStored(row), now, events);
                }
            }
            if let (Some(entry), Some((packet, bytes))) =
                (self.receiving_enhancement.get(index).copied().flatten(), as_received)
            {
                self.store_enhancement(entry, packet, bytes, now);
            }
            if let Some(last) = self.last_packet.get_mut(index) {
                *last = now;
            }
            return Some(index);
        }
        None
    }
}
