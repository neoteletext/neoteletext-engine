//! Tests of navigation: links, page stepping, entry and history (area NAV).
#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::acq::Decoder;
use crate::config::Config;
use crate::consts::PAGE_TIME_FILLING;
use crate::ecc::with_odd_parity;
use crate::packet::{Link, Links, PacketMeta, encode};
use crate::types::{ControlBits, Instant, Magazine};

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
    let entry = |mpp| HistoryEntry { service: 0, address: page(mpp), subcode: SubCode::ANY };
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

/// REQ-NAV-003, Annex H: packet X/27/0's links 1 to 4 (0 to 3 here) are
/// the red, green, yellow and cyan keys and link 6 (5) is Index, each with
/// its sub-code; link 5 (4) is for pre-fetching only; a null link leaves
/// its key without effect, and a page without X/27/0 has no links.
#[test]
fn x27_0_links_give_the_coloured_keys_and_index() {
    let link = |mpp: u16, subcode: u16| {
        let address = page(mpp);
        Some(Link { magazine: address.magazine, page: address.page, subcode: SubCode::new(subcode) })
    };
    let target =
        |mpp: u16, subcode: u16| Some(LinkTarget { address: page(mpp), subcode: SubCode::new(subcode), service: None });
    let mut stored = StoredPage::EMPTY;
    assert_eq!(PageLinks::from_page(&stored), PageLinks::default());
    assert!(PageLinks::from_page(&stored).no_coloured_links());
    let links =
        [link(0x102, 0x3F7F), None, link(0x300, 0x0001), link(0x8A0, 0x3F7F), link(0x555, 0x3F7F), link(0x199, 0x3F7F)];
    stored.links =
        Some(Links { magazine: Magazine::new(1).unwrap(), links, display_row_24: true, checksum_bytes: [0; 2] });
    let found = PageLinks::from_page(&stored);
    assert_eq!(found.keys, [target(0x102, 0x3F7F), None, target(0x300, 0x0001), target(0x8A0, 0x3F7F)]);
    assert_eq!(found.index, target(0x199, 0x3F7F));
    assert!(!found.no_coloured_links());
    let only_index = PageLinks { keys: [None; 4], ..found };
    assert!(only_index.no_coloured_links());
}

/// REQ-NAV-002: in the stored order, next and previous step to the
/// neighbouring stored page, wrapping between 899 and 100; hexadecimal
/// pages are never steps; with no decimal page stored, the step is the
/// numerical neighbour.
#[test]
fn stored_order_steps_to_stored_pages() {
    let receive = |decoder: &mut Decoder<'_>, pages: &[u16]| {
        let filling = (1..=8u16).map(|magazine| magazine << 8 | u16::from(PAGE_TIME_FILLING));
        for (index, mpp) in pages.iter().copied().chain(filling).enumerate() {
            let address = page(mpp);
            let display = [with_odd_parity(b' '); 32];
            let header =
                encode::header(address.magazine, address.page, SubCode::new(0), ControlBits(0), &display).unwrap();
            decoder.push(&header, &PacketMeta::default(), Instant(index as u64 * 1000), &mut |_| {});
        }
    };
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    receive(&mut decoder, &[0x1A0]);
    assert_eq!(step_stored(page(0x100), true, decoder.store()), page(0x101), "only a hexadecimal page");
    assert_eq!(step_stored(page(0x100), false, decoder.store()), page(0x899));
    receive(&mut decoder, &[0x150, 0x300]);
    assert_eq!(step_stored(page(0x100), true, decoder.store()), page(0x150));
    assert_eq!(step_stored(page(0x150), true, decoder.store()), page(0x300));
    assert_eq!(step_stored(page(0x300), true, decoder.store()), page(0x150), "wraps through 899");
    assert_eq!(step_stored(page(0x150), false, decoder.store()), page(0x300), "wraps through 100");
}

/// REQ-NAV-001: a value above 9 is never a digit; hexadecimal page numbers
/// cannot be entered.
#[test]
fn entry_refuses_hexadecimal_digits() {
    let mut entry = PageEntry::default();
    assert_eq!(entry.push(1), EntryResult::Partial);
    assert_eq!(entry.push(0xA), EntryResult::Rejected);
    assert_eq!((entry.count(), &entry.display()), (1, b"1--"), "the digits so far are kept");
}

/// REQ-NAV-010: a new history is empty; selecting the page already shown
/// records nothing; Back is possible from the second entry on; wiping
/// erases everything.
#[test]
fn history_ignores_repeats_and_can_be_wiped() {
    let mut history: History<4> = History::default();
    assert_eq!((history.current(), history.can_go_back(), history.can_go_forward()), (None, false, false));
    let entry = |mpp| HistoryEntry { service: 0, address: page(mpp), subcode: SubCode::ANY };
    history.visit(entry(0x100));
    history.visit(entry(0x100));
    assert!(!history.can_go_back(), "the same page twice is one entry");
    history.visit(entry(0x200));
    assert!(history.can_go_back());
    assert_eq!(history.back(), Some(entry(0x100)));
    assert!(history.can_go_forward());
    history.wipe();
    assert_eq!((history.current(), history.can_go_back(), history.can_go_forward()), (None, false, false));
    assert_eq!(history.forward(), None);
}
