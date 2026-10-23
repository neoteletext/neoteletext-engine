//! The page store in cache mode (REQ-ACQ-005), over caller-provided slots.

use super::StoredPage;
use crate::consts::MAGAZINES;
use crate::fsm::{PageKey, PageState};
use crate::types::{PageAddress, SubCode};

/// The page store in cache mode (REQ-ACQ-005), over caller-provided slots.
#[derive(Debug)]
pub struct PageStore<'a> {
    pages: &'a mut [StoredPage],
}

impl<'a> PageStore<'a> {
    /// A store using the given slots; they are reset to empty.
    pub fn new(pages: &'a mut [StoredPage]) -> Self {
        for page in pages.iter_mut() {
            *page = StoredPage::EMPTY;
        }
        Self { pages }
    }

    /// All slots.
    #[must_use]
    pub fn pages(&self) -> &[StoredPage] {
        self.pages
    }

    /// The slot holding a page and sub-code.
    #[must_use]
    pub fn find(&self, key: PageKey) -> Option<usize> {
        self.pages.iter().position(|page| page.state != PageState::Empty && page.key == key)
    }

    /// The most recently received sub-page of an address (REQ-NAV-005's
    /// rotating display, decision E.20 "any sub-page").
    #[must_use]
    pub fn latest(&self, address: PageAddress) -> Option<&StoredPage> {
        self.pages
            .iter()
            .filter(|page| page.state != PageState::Empty && page.key.address == address)
            .max_by_key(|page| page.last_received)
    }

    /// The stored page for a request (decision E.20): a specific sub-code, or
    /// the most recent sub-page when `subcode` is `SubCode::ANY`.
    #[must_use]
    pub fn get(&self, address: PageAddress, subcode: SubCode) -> Option<&StoredPage> {
        if subcode.is_any() {
            return self.latest(address);
        }
        self.find(PageKey { address, subcode }).and_then(|index| self.pages.get(index))
    }

    /// The page in a slot.
    #[must_use]
    pub fn slot(&self, index: usize) -> Option<&StoredPage> {
        self.pages.get(index)
    }

    /// The stored sub-codes of an address in ascending order, written into
    /// `out`; returns how many were written.
    pub fn subcodes(&self, address: PageAddress, out: &mut [SubCode]) -> usize {
        let mut count = 0usize;
        for page in self.pages.iter().filter(|page| page.state != PageState::Empty && page.key.address == address) {
            // Insertion into the sorted prefix; bounded by `out.len()`.
            let Some(prefix) = out.get_mut(..count) else { break };
            let position = prefix.iter().position(|&s| s > page.key.subcode).unwrap_or(count);
            if count < out.len() {
                count = count.saturating_add(1);
                // A rotation of an empty slice panics (REQ-GEN-006).
                if let Some(tail) = out.get_mut(position..count).filter(|tail| !tail.is_empty()) {
                    tail.rotate_right(1);
                    if let Some(first) = tail.first_mut() {
                        *first = page.key.subcode;
                    }
                }
            } else if let Some(tail) = out.get_mut(position..).filter(|tail| !tail.is_empty()) {
                // Full: keep the lowest sub-codes. A sub-code above every
                // listed one leaves an empty tail and is not listed.
                tail.rotate_right(1);
                if let Some(first) = tail.first_mut() {
                    *first = page.key.subcode;
                }
            }
        }
        count
    }

    /// True when any sub-page of an address is stored.
    #[must_use]
    pub fn contains(&self, address: PageAddress) -> bool {
        self.pages.iter().any(|page| page.state != PageState::Empty && page.key.address == address)
    }

    /// Finds or allocates a slot for a page, evicting the least recently
    /// received page that is not being received when the store is full.
    pub(super) fn allocate(&mut self, key: PageKey, busy: &[Option<usize>; MAGAZINES]) -> Option<(usize, bool)> {
        if let Some(index) = self.find(key) {
            return Some((index, false));
        }
        if let Some(index) = self.pages.iter().position(|page| page.state == PageState::Empty) {
            return Some((index, false));
        }
        let victim = self
            .pages
            .iter()
            .enumerate()
            .filter(|(index, _)| !busy.contains(&Some(*index)))
            .min_by_key(|(_, page)| page.last_received)
            .map(|(index, _)| index)?;
        Some((victim, true))
    }

    pub(super) fn page_mut(&mut self, index: usize) -> Option<&mut StoredPage> {
        self.pages.get_mut(index)
    }
}
