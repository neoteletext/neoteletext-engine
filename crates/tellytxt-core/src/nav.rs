//! Navigation (area NAV, layer L6): page arithmetic, links and history.

use crate::acq::{PageStore, StoredPage};
use crate::consts::{FIRST_PAGE, LAST_PAGE, MAGAZINE_ZERO_MEANS};
use crate::types::{FixedVec, PageAddress, PageNumber, SubCode};

/// A link target: a page, a sub-code (3F7F for any), and the service it
/// belongs to (`None` for the current service).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkTarget {
    /// The page.
    pub address: PageAddress,
    /// The sub-code, or `SubCode::ANY`.
    pub subcode: SubCode,
    /// Index of another service in the descriptor's list (Appendix E), or
    /// `None` for this service.
    pub service: Option<u8>,
}

/// The links of a page: four coloured keys and Index (decision E.36,
/// REQ-NAV-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PageLinks {
    /// Red, green, yellow and cyan.
    pub keys: [Option<LinkTarget>; 4],
    /// The Index key.
    pub index: Option<LinkTarget>,
}

/// Link positions in X/27/0: links 0 to 3 are the coloured keys and link 5
/// the Index key (Annex H, REQ-NAV-003); link 4 is not a viewer link.
const X27_INDEX_LINK: usize = 5;

impl PageLinks {
    /// The links of a broadcast page, from packet X/27/0 (REQ-NAV-003).
    #[must_use]
    pub fn from_page(page: &StoredPage) -> Self {
        let Some(links) = page.links else { return Self::default() };
        let target = |index: usize| {
            links.links.get(index).copied().flatten().and_then(|link| {
                let mpp = (u16::from(link.magazine.number()) << 8) | u16::from(link.page.0);
                PageAddress::from_mpp(mpp).map(|address| LinkTarget { address, subcode: link.subcode, service: None })
            })
        };
        Self { keys: [target(0), target(1), target(2), target(3)], index: target(X27_INDEX_LINK) }
    }

    /// True when no coloured key has a link.
    #[must_use]
    pub fn no_coloured_links(&self) -> bool {
        self.keys.iter().all(Option::is_none)
    }
}

/// The page after `address` in numerical order; 899 wraps to 100
/// (REQ-NAV-002). Hexadecimal pages are skipped.
#[must_use]
pub fn next_page(address: PageAddress) -> PageAddress {
    step(address, true)
}

/// The page before `address` in numerical order; 100 wraps to 899.
#[must_use]
pub fn previous_page(address: PageAddress) -> PageAddress {
    step(address, false)
}

fn decimal(value: u16) -> Option<PageAddress> {
    let hundreds = value / 100;
    let tens = (value / 10) % 10;
    let units = value % 10;
    // Magazines above 8 do not exist; from_mpp rejects them.
    PageAddress::from_mpp((hundreds << 8) | (tens << 4) | units)
}

fn to_decimal(address: PageAddress) -> u16 {
    let page = address.page;
    u16::from(address.magazine.number())
        .saturating_mul(100)
        .saturating_add(u16::from(page.tens().min(9)).saturating_mul(10))
        .saturating_add(u16::from(page.units().min(9)))
}

/// First and last pages as decimal numbers.
const FIRST_DECIMAL: u16 = 100;
const LAST_DECIMAL: u16 = 899;
/// Number of decimal pages, bounding the search for a stored page.
const PAGE_COUNT: u16 = LAST_DECIMAL - FIRST_DECIMAL + 1;

fn step(address: PageAddress, forward: bool) -> PageAddress {
    let value = to_decimal(address);
    let next = if forward {
        if value >= LAST_DECIMAL { FIRST_DECIMAL } else { value.saturating_add(1) }
    } else if value <= FIRST_DECIMAL {
        LAST_DECIMAL
    } else {
        value.saturating_sub(1)
    };
    decimal(next).or_else(|| PageAddress::from_mpp(FIRST_PAGE)).unwrap_or(address)
}

/// The next or previous page present in the store (REQ-NAV-002), or the
/// numerical neighbour when none is stored. Bounded: at most 800 steps.
#[must_use]
pub fn step_stored(address: PageAddress, forward: bool, store: &PageStore<'_>) -> PageAddress {
    let mut candidate = address;
    for _ in 0..PAGE_COUNT {
        candidate = step(candidate, forward);
        if store.contains(candidate) {
            return candidate;
        }
    }
    step(address, forward)
}

/// Three-digit page entry (REQ-NAV-001).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PageEntry {
    digits: [u8; 3],
    count: u8,
}

/// What a digit did to the entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryResult {
    /// The digit was refused: it cannot form a valid page number.
    Rejected,
    /// More digits are needed.
    Partial,
    /// A page number is complete.
    Complete(PageAddress),
}

impl PageEntry {
    /// Number of digits entered so far.
    #[must_use]
    pub const fn count(&self) -> u8 {
        self.count
    }

    /// Clears the entry.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Adds a digit 0 to 9.
    pub fn push(&mut self, digit: u8) -> EntryResult {
        if digit > 9 {
            return EntryResult::Rejected;
        }
        // The first digit is the magazine, 1 to 8.
        if self.count == 0 && !(1..=MAGAZINE_ZERO_MEANS).contains(&digit) {
            return EntryResult::Rejected;
        }
        if let Some(slot) = self.digits.get_mut(usize::from(self.count)) {
            *slot = digit;
        }
        self.count = self.count.saturating_add(1);
        if self.count < 3 {
            return EntryResult::Partial;
        }
        let [m, t, u] = self.digits;
        self.clear();
        let mpp = (u16::from(m) << 8) | (u16::from(t) << 4) | u16::from(u);
        PageAddress::from_mpp(mpp).map_or(EntryResult::Rejected, EntryResult::Complete)
    }

    /// The digits entered, as ASCII, with `-` for the missing ones.
    #[must_use]
    pub fn display(&self) -> [u8; 3] {
        let mut out = [b'-'; 3];
        for (index, (slot, &digit)) in out.iter_mut().zip(self.digits.iter()).enumerate() {
            if index < usize::from(self.count) {
                *slot = b'0'.saturating_add(digit);
            }
        }
        out
    }
}

/// A history entry (REQ-NAV-010).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HistoryEntry {
    /// The service: 0 for the current or broadcast service.
    pub service: u8,
    /// The page.
    pub address: PageAddress,
    /// The sub-page.
    pub subcode: SubCode,
    /// The page came from a form response.
    pub from_form: bool,
}

/// Back and forward history in caller-sized memory (REQ-NAV-010).
#[derive(Clone, Copy, Debug)]
pub struct History<const N: usize> {
    entries: FixedVec<HistoryEntry, N>,
    position: usize,
}

const EMPTY_ENTRY: HistoryEntry = HistoryEntry {
    service: 0,
    address: PageAddress { magazine: crate::types::Magazine::from_bits(1), page: PageNumber(0) },
    subcode: SubCode::ANY,
    from_form: false,
};

impl<const N: usize> Default for History<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> History<N> {
    /// An empty history.
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: FixedVec::new(EMPTY_ENTRY), position: 0 }
    }

    /// Records a newly selected page, clearing the forward history. When
    /// full, the oldest entry is dropped.
    pub fn visit(&mut self, entry: HistoryEntry) {
        if self.current() == Some(entry) {
            return;
        }
        self.entries.truncate(self.position);
        if self.entries.is_full() {
            let _ = self.entries.remove(0);
        }
        let _ = self.entries.push(entry);
        self.position = self.entries.len();
    }

    /// The entry being shown.
    #[must_use]
    pub fn current(&self) -> Option<HistoryEntry> {
        self.position.checked_sub(1).and_then(|index| self.entries.as_slice().get(index).copied())
    }

    /// Moves back, returning the entry to show.
    pub fn back(&mut self) -> Option<HistoryEntry> {
        if self.position <= 1 {
            return None;
        }
        self.position = self.position.saturating_sub(1);
        self.current()
    }

    /// Moves forward, undoing a back.
    pub fn forward(&mut self) -> Option<HistoryEntry> {
        if self.position >= self.entries.len() {
            return None;
        }
        self.position = self.position.saturating_add(1);
        self.current()
    }

    /// True when Back has somewhere to go.
    #[must_use]
    pub const fn can_go_back(&self) -> bool {
        self.position > 1
    }

    /// True when Forward has somewhere to go.
    #[must_use]
    pub const fn can_go_forward(&self) -> bool {
        self.position < self.entries.len()
    }

    /// Erases the history (REQ-NAV-010, as REQ-SEC-013 for cookies).
    pub fn wipe(&mut self) {
        self.entries.clear();
        self.position = 0;
    }
}

const _: () = assert!(FIRST_PAGE == 0x100 && LAST_PAGE == 0x899);

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn page(mpp: u16) -> PageAddress {
        PageAddress::from_mpp(mpp).unwrap()
    }

    #[test]
    fn stepping_wraps_and_skips_hex() {
        assert_eq!(next_page(page(0x899)), page(0x100));
        assert_eq!(previous_page(page(0x100)), page(0x899));
        assert_eq!(next_page(page(0x109)), page(0x110));
        assert_eq!(next_page(page(0x199)), page(0x200));
        assert_eq!(previous_page(page(0x200)), page(0x199));
    }

    #[test]
    fn entry_rejects_invalid_digits() {
        let mut entry = PageEntry::default();
        assert_eq!(entry.push(0), EntryResult::Rejected);
        assert_eq!(entry.push(9), EntryResult::Rejected);
        assert_eq!(entry.push(3), EntryResult::Partial);
        assert_eq!(&entry.display(), b"3--");
        assert_eq!(entry.push(0), EntryResult::Partial);
        assert_eq!(entry.push(2), EntryResult::Complete(page(0x302)));
        assert_eq!(entry.count(), 0);
    }

    #[test]
    fn history_back_forward_and_clearing() {
        let mut history: History<3> = History::new();
        let entry = |mpp| HistoryEntry { service: 0, address: page(mpp), subcode: SubCode::ANY, from_form: false };
        history.visit(entry(0x100));
        history.visit(entry(0x200));
        history.visit(entry(0x300));
        assert_eq!(history.back(), Some(entry(0x200)));
        assert_eq!(history.forward(), Some(entry(0x300)));
        assert_eq!(history.forward(), None);
        history.back();
        history.visit(entry(0x400));
        assert!(!history.can_go_forward(), "a new page clears forward history");
        history.visit(entry(0x500));
        assert_eq!(history.back(), Some(entry(0x400)));
        assert_eq!(history.back(), Some(entry(0x200)), "oldest dropped when full");
        assert_eq!(history.back(), None);
    }
}
