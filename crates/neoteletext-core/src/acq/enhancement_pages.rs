//! Which pages are enhancement pages, and their reception into the
//! enhancement store as received (REQ-ACQ-013, REQ-ACQ-016, REQ-ACQ-017,
//! decision E.56).

use super::enhancement::Stored;
use super::{Decoder, EnhancementKey, Event, Route};
use crate::consts::{
    DATA_BYTES, LINK_NULL_PAGE, MOT_OBJECT_LINKS_LEVEL_25, MOT_OBJECT_LINKS_LEVEL_35, MOT_OBJECT_LINKS_PER_PACKET,
    PAGE_FUNCTION_BASIC, PAGE_MOT,
};
use crate::fsm::{MagazineState, PageEvent, PageKey};
use crate::limits::{PAGE_MIP, is_reserved_page};
use crate::packet::{PageFormat, mot_object_link};
use crate::types::{ControlBits, Instant, PageAddress};

impl Decoder<'_> {
    /// Acts on the page function of a page being received: a page whose
    /// X/28/0 or X/28/3 function is not "basic Level 1 page" is an
    /// enhancement page, stored as received from this packet on
    /// (REQ-ACQ-016, decision E.56).
    pub(super) fn page_format(
        &mut self,
        index: usize,
        format: PageFormat,
        now: Instant,
        events: &mut dyn FnMut(Event),
    ) {
        if format.function == PAGE_FUNCTION_BASIC {
            return;
        }
        let receiving = self.receiving_enhancement.get(index).copied().flatten();
        let entry = match receiving {
            Some(entry) => Some(entry),
            None => self.switch_to_enhancement(index, now, events),
        };
        if let Some(entry) = entry {
            self.enhancement.set_format(entry, format);
        }
    }

    /// Moves the reception in magazine `index` to the enhancement store. A
    /// copy in the page store holding only this reception is released; an
    /// older one keeps its content and ends its reception here.
    fn switch_to_enhancement(&mut self, index: usize, now: Instant, events: &mut dyn FnMut(Event)) -> Option<usize> {
        if !self.enhancement.is_enabled() {
            return None;
        }
        let Some(MagazineState::Receiving(key)) = self.magazines.get(index).copied() else { return None };
        let control = self.receiving_control.get(index).copied().unwrap_or_default();
        let entry = self.start_enhancement(index, key, control, now)?;
        if let Some(slot) = self.receiving_slot.get_mut(index).and_then(Option::take) {
            let fresh = self.store.slot(slot).is_some_and(|page| page.receptions <= 1);
            self.page_event(slot, if fresh { PageEvent::Evicted } else { PageEvent::ReceptionEnded }, now, events);
        }
        Some(entry)
    }

    /// Starts the reception of an enhancement sub-page in magazine `index`.
    /// The generation before the reception is kept, so that a change of
    /// S2 or C4 at its start is reported too; a new sub-page has none.
    pub(super) fn start_enhancement(
        &mut self,
        index: usize,
        key: PageKey,
        control: ControlBits,
        now: Instant,
    ) -> Option<usize> {
        let before = self.enhancement.get(EnhancementKey::new(key.address, key.subcode)).map(|page| page.generation());
        let busy = self.receiving_enhancement;
        let entry = self.enhancement.start(key.address, key.subcode, control, now, &busy)?;
        if let Some(slot) = self.receiving_enhancement.get_mut(index) {
            *slot = Some(entry);
        }
        if let Some(generation) = self.start_generation.get_mut(index) {
            *generation = before;
        }
        Some(entry)
    }

    /// Adds an address to the list of pages named by links.
    pub(super) fn name(&mut self, address: PageAddress) {
        if address.page.0 == LINK_NULL_PAGE || self.named.as_slice().contains(&address) {
            return;
        }
        if self.named.is_full() {
            self.named.remove(0);
        }
        self.named.push(address);
    }

    /// Adds the pages that a stored packet X/19, X/20, X/22 or X/23 of a
    /// Magazine Organization Table links to (Table 20; decision E.52: the
    /// magazine of a table link is absolute).
    fn name_from_table(&mut self, packet: u8, bytes: &[u8; DATA_BYTES]) {
        if !MOT_OBJECT_LINKS_LEVEL_25.contains(&packet) && !MOT_OBJECT_LINKS_LEVEL_35.contains(&packet) {
            return;
        }
        for link in 0..MOT_OBJECT_LINKS_PER_PACKET {
            if let Some(link) = mot_object_link(bytes, link) {
                self.name(PageAddress { magazine: link.magazine, page: link.page });
            }
        }
    }

    /// Where the page of a header is stored (REQ-ACQ-013, REQ-ACQ-016,
    /// REQ-ACQ-017). Decimal pages always go to the page store, and also to
    /// the enhancement store when they are enhancement pages, so that a
    /// wrong link never hides a display page. Pages with hexadecimal numbers
    /// go to the enhancement store when they are enhancement pages, and
    /// otherwise to the page store at Levels 2.5 and 3.5 or under the
    /// diagnostic setting.
    ///
    /// A reserved page number (mFD, mFE, mFF, 1BE, 1F0) is never a display
    /// page: it is read only as the function it is reserved for, so it never
    /// goes to the page store (Annex A.10; REQ-NUM-008, REQ-NUM-010). The
    /// MOT and the MIP go to the enhancement store, as the tables they are.
    pub(super) fn route(&self, address: PageAddress) -> Route {
        let table = address.page.0 == PAGE_MOT || address.page.0 == PAGE_MIP;
        let enhancement = self.enhancement.is_enabled()
            && (table || self.enhancement.contains_address(address) || self.named.as_slice().contains(&address));
        let hexadecimal = self.config.store_hexadecimal_pages || (self.config.level.is_enhanced() && !enhancement);
        let reserved = is_reserved_page(address.magazine.number(), address.page.0);
        Route { main: !reserved && (address.page.is_decimal() || hexadecimal), enhancement }
    }

    /// Stores a packet of an enhancement sub-page as received (decision
    /// E.56), and learns the links of a Magazine Organization Table.
    pub(super) fn store_enhancement(&mut self, entry: usize, packet: usize, bytes: &[u8; DATA_BYTES], now: Instant) {
        let busy = self.receiving_enhancement;
        match self.enhancement.store(entry, packet, bytes, now, &busy) {
            Stored::Yes { .. } => {
                let table = self.enhancement.key(entry).filter(|key| key.address.page.0 == PAGE_MOT && key.s1 == 0);
                if table.is_some() {
                    // Packet index i is packet X/(i + 1).
                    self.name_from_table(packet.saturating_add(1) as u8, bytes);
                }
            }
            Stored::Dropped => self.stats.enhancement_dropped = self.stats.enhancement_dropped.saturating_add(1),
        }
    }
}
