//! The page header (REQ-ACQ-002, REQ-ACQ-003, REQ-ACQ-010): its event for
//! each of the eight magazine machines, its display bytes and control bits
//! stored, and the rolling header and clock (decision E.11, REQ-ACQ-011).

use super::{Decoder, Event, ROLLING_BYTES, Route};
use crate::consts::{C4_ERASE, C8_UPDATE, C9_INTERRUPTED, HEADER_DISPLAY_BYTES, HEADER_DISPLAY_COLUMN, MAGAZINES};
use crate::fsm::{MagazineEvent, MagazineState, PageKey};
use crate::packet::Header;
use crate::types::{ControlBits, Instant, PageAddress, PageNumber};

impl Decoder<'_> {
    /// Handles a header for all eight magazine machines: it starts its own
    /// page (REQ-ACQ-002) and ends pages as REQ-ACQ-003 lists.
    pub(super) fn header(&mut self, header: &Header, now: Instant, events: &mut dyn FnMut(Event)) {
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
}
