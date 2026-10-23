//! The reception of packets (Annex B.3): the decoding of each packet into
//! the store, the rolling header (decision E.11), and the driving of the
//! magazine and page machines of area FSM.

use super::{Decoder, Discard, Event, ROLLING_BYTES, Route, StoredPage, enhancement_index, row_index};
use crate::config::ParityPolicy;
use crate::consts::{
    C4_ERASE, C8_UPDATE, C9_INTERRUPTED, DATA_BYTES, DESIGNATION_LINKS_LEVEL_25, HEADER_DISPLAY_BYTES,
    HEADER_DISPLAY_COLUMN, LINK_FUNCTION_GPOP, LINK_FUNCTION_POP, MAGAZINES, SERVICE_FORMAT_2_BIT,
};
use crate::fsm::{
    MagazineAction, MagazineEvent, MagazineState, PageAction, PageEvent, PageKey, PageState, magazine_action_name,
    magazine_event_name, magazine_state_name, magazine_transition, page_action_name, page_event_name, page_state_name,
    page_transition,
};
use crate::packet::{
    CompositionalLink, Decoded, Header, MagazinePacket, Packet, PacketError, PacketMeta, PagePacket, ServicePacket,
    decode,
};
use crate::trace::{Machine, TraceEntry};
use crate::types::{ControlBits, Instant, Magazine, PageAddress, PageNumber};

impl Decoder<'_> {
    /// Processes one packet.
    pub fn push(&mut self, packet: &Packet, _meta: &PacketMeta, now: Instant, events: &mut dyn FnMut(Event)) {
        let decoded = match decode(packet) {
            Ok(decoded) => decoded,
            Err(error) => {
                match error {
                    PacketError::BadAddress => {
                        self.stats.address_failures = self.stats.address_failures.saturating_add(1);
                    }
                    PacketError::BadDesignation => {
                        self.stats.designation_failures = self.stats.designation_failures.saturating_add(1);
                    }
                }
                events(Event::Discarded(Discard::Packet(error)));
                return;
            }
        };
        match decoded {
            Decoded::Header(header) => {
                self.count(header.magazine, 0);
                self.header(&header, now, events);
            }
            Decoded::Row(row) => {
                self.count(row.magazine, row.row);
                let as_received = row_index(row.row).map(|index| (index, &row.bytes));
                self.row_like(row.magazine, now, events, as_received, |page, policy| {
                    let failures = page.merge(usize::from(row.row), 0, &row.bytes, policy);
                    (row.row, failures)
                });
            }
            Decoded::Enhancement(enhancement) => {
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
            Decoded::Service(service) => {
                self.count(Magazine::from_bits(0), crate::consts::PACKET_SERVICE);
                // REQ-ACQ-018: a new network identification in format 1
                // means a new service, whose magazine data differs.
                let format_1 = |packet: &ServicePacket| packet.designation & SERVICE_FORMAT_2_BIT == 0;
                if format_1(&service)
                    && self.service.is_some_and(|old| format_1(&old) && old.network_id != service.network_id)
                {
                    self.discard_magazine_data(events);
                }
                self.service = Some(service);
                events(Event::ServiceData);
            }
            Decoded::Ignored(address) => {
                self.count(address.magazine, address.number);
                self.stats.ignored = self.stats.ignored.saturating_add(1);
            }
        }
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
                        // are not text pages (REQ-OBJ-041).
                        PagePacket::PageFunction(_) => {}
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

    /// Handles a header for all eight magazine machines: it starts its own
    /// page (REQ-ACQ-002) and ends pages as REQ-ACQ-003 lists.
    fn header(&mut self, header: &Header, now: Instant, events: &mut dyn FnMut(Event)) {
        if let Some(serial) = header.serial() {
            self.serial = serial;
        }
        let own = header.magazine;
        let time_filling = header.page.is_some_and(PageNumber::is_time_filling);
        self.pending = header.page.map_or(Route::default(), |page| self.route(PageAddress { magazine: own, page }));
        if let (Some(control), Some(slot)) = (header.control, self.receiving_control.get_mut(own.index())) {
            *slot = control;
        }
        for index in 0..MAGAZINES {
            let event = if index == own.index() {
                self.own_header_event(header)
            } else if self.serial {
                if time_filling { MagazineEvent::TimeFillingHeader } else { MagazineEvent::HeaderOfOtherMagazineSerial }
            } else {
                MagazineEvent::UnrelatedHeader
            };
            self.magazine_event(index, event, now, events);
        }
        if let Some(last) = self.last_packet.get_mut(own.index()) {
            *last = now;
        }
        self.serial_active = if self.serial { Some(own) } else { None };
        self.header_store(header, now, events);
        self.roll(header);
        events(Event::HeaderReceived { magazine: own, page: header.page });
    }

    fn own_header_event(&self, header: &Header) -> MagazineEvent {
        let (Some(page), Some(subcode), Some(control)) = (header.page, header.subcode, header.control) else {
            return MagazineEvent::BadHeader;
        };
        if page.is_time_filling() {
            return MagazineEvent::TimeFillingHeader;
        }
        if !self.pending.main && !self.pending.enhancement {
            return MagazineEvent::NonStorableHeader;
        }
        let key = PageKey { address: PageAddress { magazine: header.magazine, page }, subcode };
        MagazineEvent::StorableHeader { key, erase: control.has(C4_ERASE) }
    }

    /// Updates the rolling header and the clock (decision E.11,
    /// REQ-ACQ-011).
    fn roll(&mut self, header: &Header) {
        let Some(control) = header.control else { return };
        if !header.is_complete() {
            return;
        }
        if let Some(clock) = header.display.get(ROLLING_BYTES..HEADER_DISPLAY_BYTES) {
            self.clock.copy_from_slice(clock);
        }
        let interrupted = control.has(C9_INTERRUPTED);
        let wanted = self.serial || self.requested.is_none_or(|requested| requested.magazine == header.magazine);
        if self.config.rolling_header
            && !interrupted
            && wanted
            && let Some(text) = header.display.get(..ROLLING_BYTES)
        {
            self.rolling.copy_from_slice(text);
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

    /// Runs one magazine machine transition and performs its action.
    pub(super) fn magazine_event(
        &mut self,
        index: usize,
        event: MagazineEvent,
        now: Instant,
        events: &mut dyn FnMut(Event),
    ) -> MagazineAction {
        let Some(state) = self.magazines.get(index).copied() else { return MagazineAction::None };
        let (next, action) = magazine_transition(state, event);
        if let Some(slot) = self.magazines.get_mut(index) {
            *slot = next;
        }
        if self.config.trace && (next != state || action != MagazineAction::None) {
            events(Event::Trace(TraceEntry {
                machine: Machine::Magazine(Magazine::from_bits(index as u8).number()),
                time: now,
                from: magazine_state_name(state),
                event: magazine_event_name(event),
                action: magazine_action_name(action),
                to: magazine_state_name(next),
            }));
        }
        match action {
            // Storing is done by the caller, which holds the packet; absorbed
            // rows are discarded silently (REQ-FSM-003).
            MagazineAction::None | MagazineAction::Store(_) | MagazineAction::DiscardAbsorbed => {}
            MagazineAction::DiscardWithoutHeader => self.discard(Discard::RowWithoutHeader, events),
            MagazineAction::DiscardOtherMagazine => self.discard(Discard::OtherMagazine, events),
            MagazineAction::Start { key, erase } => self.start(index, key, erase, now, events),
            MagazineAction::End(_) => self.end(index, now, events),
            MagazineAction::EndAndStart { started, erase, .. } => {
                self.end(index, now, events);
                self.start(index, started, erase, now, events);
            }
        }
        action
    }

    fn discard(&mut self, reason: Discard, events: &mut dyn FnMut(Event)) {
        self.stats.discarded = self.stats.discarded.saturating_add(1);
        events(Event::Discarded(reason));
    }

    fn start(&mut self, magazine_index: usize, key: PageKey, erase: bool, now: Instant, events: &mut dyn FnMut(Event)) {
        let route = self.pending;
        if route.enhancement {
            let control = self.receiving_control.get(magazine_index).copied().unwrap_or_default();
            self.start_enhancement(magazine_index, key, control, now);
        }
        if !route.main {
            return;
        }
        let Some((slot, evict)) = self.store.allocate(key, &self.receiving_slot) else {
            return;
        };
        if evict {
            self.page_event(slot, PageEvent::Evicted, now, events);
        }
        if let Some(page) = self.store.page_mut(slot)
            && page.state == PageState::Empty
        {
            *page = StoredPage::EMPTY;
            page.key = key;
            page.first_received = now;
        }
        if let Some(receiving) = self.receiving_slot.get_mut(magazine_index) {
            *receiving = Some(slot);
        }
        self.stats.pages_started = self.stats.pages_started.saturating_add(1);
        self.page_event(slot, PageEvent::ReceptionStarted { erase }, now, events);
    }

    /// Stores the display bytes and control bits of a header into the page
    /// it has just started, and raises the update indicator for C8
    /// (REQ-ACQ-010).
    fn header_store(&mut self, header: &Header, now: Instant, events: &mut dyn FnMut(Event)) {
        let index = header.magazine.index();
        let Some(control) = header.control else { return };
        let Some(MagazineState::Receiving(key)) = self.magazines.get(index).copied() else { return };
        let Some(slot) = self.receiving_slot.get(index).copied().flatten() else { return };
        self.store_header(slot, &header.display, control, now);
        if control.has(C8_UPDATE) {
            events(Event::UpdateIndicator(key));
        }
    }

    /// Stores the header of the page just started.
    fn store_header(
        &mut self,
        slot: usize,
        header_display: &[u8; HEADER_DISPLAY_BYTES],
        control: ControlBits,
        now: Instant,
    ) {
        let policy = self.config.parity_policy;
        if let Some(page) = self.store.page_mut(slot) {
            page.control = control;
            page.receptions = page.receptions.saturating_add(1);
            page.last_received = now;
            let failures = page.merge(0, HEADER_DISPLAY_COLUMN, header_display, policy);
            page.errors = page.errors.saturating_add(failures);
        }
    }

    fn end(&mut self, magazine_index: usize, now: Instant, events: &mut dyn FnMut(Event)) {
        let slot = self.receiving_slot.get_mut(magazine_index).and_then(Option::take);
        if let Some(slot) = slot {
            self.stats.pages_completed = self.stats.pages_completed.saturating_add(1);
            self.page_event(slot, PageEvent::ReceptionEnded, now, events);
        }
        // REQ-ACQ-022: report an enhancement sub-page whose content or update
        // indicator changed during the reception.
        let entry = self.receiving_enhancement.get_mut(magazine_index).and_then(Option::take);
        if let Some(entry) = entry {
            let started = self.start_generation.get(magazine_index).copied().flatten();
            if let (Some(key), Some(generation)) = (self.enhancement.key(entry), self.enhancement.generation(entry))
                && started != Some(generation)
            {
                events(Event::EnhancementPageChanged(key));
            }
        }
    }

    /// Runs one page lifecycle transition and performs its action.
    pub(super) fn page_event(&mut self, slot: usize, event: PageEvent, now: Instant, events: &mut dyn FnMut(Event)) {
        let Some(page) = self.store.page_mut(slot) else { return };
        let key = page.key;
        let state = page.state;
        let (next, action) = page_transition(state, event);
        page.state = next;
        if self.config.trace && next != state {
            events(Event::Trace(TraceEntry {
                machine: Machine::Page(key.address.mpp()),
                time: now,
                from: page_state_name(state),
                event: page_event_name(event),
                action: page_action_name(action),
                to: page_state_name(next),
            }));
        }
        match action {
            PageAction::None => {}
            PageAction::Started | PageAction::MergeAndStart => events(Event::PageStarted(key)),
            PageAction::Erase => {
                page.erase();
                events(Event::PageErased(key));
                events(Event::PageStarted(key));
            }
            PageAction::Updated(row) => events(Event::PageUpdated { key, row }),
            PageAction::Completed => events(Event::PageCompleted(key)),
            PageAction::UpdateIndicator => events(Event::UpdateIndicator(key)),
            PageAction::Stale => events(Event::PageStale(key)),
            PageAction::Release => *page = StoredPage::EMPTY,
        }
    }
}
