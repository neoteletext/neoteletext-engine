//! The page store in cache mode (REQ-ACQ-005), over caller-provided slots,
//! and the listing of a page's sub-pages through it (REQ-NAV-005,
//! REQ-NUM-030).
//!
//! The store holds no list of its own: every listing, count and step is a
//! bounded pass over the slots, so a page may have every sub-page the
//! standard addresses (8,190, `limits::MAX_SUBPAGES`) and a port's stack
//! does not grow with that maximum. What a store of a given capacity holds
//! at once is the caller's budget; a sub-page that falls out is stored
//! again when it next arrives (REQ-NUM-030).

use super::StoredPage;
use crate::consts::MAGAZINES;
use crate::fsm::{PageKey, PageState};
use crate::types::{PageAddress, SubCode};

/// The page store in cache mode (REQ-ACQ-005), over caller-provided slots.
#[derive(Debug)]
pub struct PageStore<'a> {
    pages: &'a mut [StoredPage],
}

/// The listed sub-codes of one page in ascending order
/// ([`PageStore::subcodes`]). Each step is one pass over the slots.
#[derive(Clone, Debug)]
pub struct SubCodes<'s> {
    pages: &'s [StoredPage],
    address: PageAddress,
    after: Option<SubCode>,
}

impl Iterator for SubCodes<'_> {
    type Item = SubCode;

    fn next(&mut self) -> Option<SubCode> {
        let next = match self.after {
            None => listed(self.pages, self.address).min(),
            Some(after) => listed(self.pages, self.address).filter(|&subcode| subcode > after).min(),
        }?;
        self.after = Some(next);
        Some(next)
    }
}

/// The sub-codes of an address's listed sub-pages (REQ-NAV-005), in slot
/// order. A sub-code is in at most one slot: [`PageStore::allocate`]
/// finds a stored key before it takes a slot.
fn listed(pages: &[StoredPage], address: PageAddress) -> impl Iterator<Item = SubCode> + '_ {
    pages
        .iter()
        .filter(move |page| page.key.address == address && page.is_listed_sub_page())
        .map(|page| page.key.subcode)
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

    /// The slot holding a page and sub-code. The key is the magazine, the
    /// page number and all thirteen bits of the sub-code (REQ-NUM-006).
    #[must_use]
    pub fn find(&self, key: PageKey) -> Option<usize> {
        self.pages.iter().position(|page| page.state != PageState::Empty && page.key == key)
    }

    /// The most recently received sub-page of an address (REQ-NAV-005's
    /// rotating display, decision E.20 "any sub-page"). Eviction takes the
    /// least recently received slot first (REQ-ACQ-005), so the latest
    /// sub-page of a page is always the last of its sub-pages to go
    /// (REQ-NUM-030).
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

    /// The sub-codes of an address's stored sub-pages in ascending order
    /// (REQ-NAV-005): the sub-pages of a display page, without the
    /// versions of a page that does not follow the sub-code rules
    /// (REQ-NUM-021) or the sub-codes of a data page (REQ-NUM-022,
    /// REQ-NUM-024, REQ-NUM-028). Every one is yielded, up to the 8,190 a
    /// page may have; each step is a pass over the slots, and nothing is
    /// held but the last sub-code yielded.
    #[must_use]
    pub fn subcodes(&self, address: PageAddress) -> SubCodes<'_> {
        SubCodes { pages: self.pages, address, after: None }
    }

    /// How many sub-pages of an address [`PageStore::subcodes`] lists.
    #[must_use]
    pub fn subpage_count(&self, address: PageAddress) -> usize {
        listed(self.pages, address).count()
    }

    /// The lowest listed sub-code of an address.
    #[must_use]
    pub fn first_subpage(&self, address: PageAddress) -> Option<SubCode> {
        listed(self.pages, address).min()
    }

    /// The highest listed sub-code of an address.
    #[must_use]
    pub fn last_subpage(&self, address: PageAddress) -> Option<SubCode> {
        listed(self.pages, address).max()
    }

    /// The lowest listed sub-code of an address above `after`, or `None`
    /// when `after` is the highest.
    #[must_use]
    pub fn next_subpage(&self, address: PageAddress, after: SubCode) -> Option<SubCode> {
        listed(self.pages, address).filter(|&subcode| subcode > after).min()
    }

    /// The highest listed sub-code of an address below `before`, or `None`
    /// when `before` is the lowest.
    #[must_use]
    pub fn previous_subpage(&self, address: PageAddress, before: SubCode) -> Option<SubCode> {
        listed(self.pages, address).filter(|&subcode| subcode < before).max()
    }

    /// The place of a listed sub-code among its page's, from 1 in
    /// ascending order ("3 of 12"), or `None` when it is not listed.
    #[must_use]
    pub fn subpage_place(&self, address: PageAddress, subcode: SubCode) -> Option<usize> {
        let mut found = false;
        let mut below = 0usize;
        for listed in listed(self.pages, address) {
            if listed == subcode {
                found = true;
            } else if listed < subcode {
                below = below.saturating_add(1);
            }
        }
        found.then(|| below.saturating_add(1))
    }

    /// True when any sub-page of an address is stored.
    #[must_use]
    pub fn contains(&self, address: PageAddress) -> bool {
        self.pages.iter().any(|page| page.state != PageState::Empty && page.key.address == address)
    }

    /// True when a display page is stored at an address: one the viewer may
    /// step to, not a data page by its page function or its MIP code
    /// (REQ-NUM-027, REQ-NUM-028).
    #[must_use]
    pub fn has_display_page(&self, address: PageAddress) -> bool {
        self.pages.iter().any(|page| page.key.address == address && page.is_display_page())
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
