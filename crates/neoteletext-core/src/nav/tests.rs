//! Tests of navigation: links, page stepping, entry and history (area NAV).
#![allow(clippy::unwrap_used)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
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
