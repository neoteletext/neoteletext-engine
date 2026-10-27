//! The magazine and page machines of area FSM (REQ-FSM-003, REQ-FSM-004):
//! one transition of each, and the actions a magazine transition takes,
//! starting and ending the reception of a page.

use super::{Decoder, Discard, Event, Numbering, StoredPage};
use crate::fsm::{
    MagazineAction, MagazineEvent, PageAction, PageEvent, PageKey, PageState, magazine_action_name,
    magazine_event_name, magazine_state_name, magazine_transition, page_action_name, page_event_name, page_state_name,
    page_transition,
};
use crate::trace::{Machine, TraceEntry};
use crate::types::{Instant, Magazine};

impl Decoder<'_> {
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
        let numbering = self.numbering(key.address);
        if numbering == Numbering::LatestOnly {
            self.release_other_versions(key, now, events);
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
        if let Some(page) = self.store.page_mut(slot) {
            page.numbering = numbering;
        }
        if let Some(receiving) = self.receiving_slot.get_mut(magazine_index) {
            *receiving = Some(slot);
        }
        self.stats.pages_started = self.stats.pages_started.saturating_add(1);
        self.page_event(slot, PageEvent::ReceptionStarted { erase }, now, events);
    }

    /// Releases every stored version of a page other than `key`'s, for a
    /// page that keeps only its latest (REQ-NUM-021): its sub-code is the
    /// time or the like, not a sub-page, so each new one replaces the last.
    fn release_other_versions(&mut self, key: PageKey, now: Instant, events: &mut dyn FnMut(Event)) {
        for index in 0..self.store.pages().len() {
            let other = self.store.slot(index).is_some_and(|page| {
                page.state != PageState::Empty && page.key.address == key.address && page.key != key
            });
            if other && !self.receiving_slot.contains(&Some(index)) {
                self.page_event(index, PageEvent::Evicted, now, events);
            }
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
        // REQ-FSM-010: a transition is a change of state or an action
        // performed, as for the magazine machine.
        if self.config.trace && (next != state || action != PageAction::None) {
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
