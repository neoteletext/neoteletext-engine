//! Tests of the viewer (REQ-FSM-005): page entry, link confirmation, hold, reveal and its clearing.
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::acq::{Decoder, StoredPage};
use crate::consts::{COLOUR_GREEN, SPACE};
use crate::fsm::PageState;
use crate::level1::{Cell, Grid};

fn page(mpp: u16) -> PageAddress {
    PageAddress::from_mpp(mpp).unwrap()
}

#[test]
fn digit_entry_requests_a_page() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let decoder = Decoder::new(&mut slots, Config::default());
    let mut viewer = Viewer::new(page(0x100));
    assert_eq!(viewer.event(ViewerEvent::Started), Action::Request(page(0x100), SubCode::ANY));
    let links = PageLinks::default();
    let config = Config::default();
    let no = |_| false;
    let context = KeyContext {
        links: &links,
        store: decoder.store(),
        initial: page(0x100),
        config: &config,
        now: Instant(0),
        confirmed: &no,
        row24_shown: true,
    };
    assert_eq!(viewer.key(Key::Digit(2), &context), Action::None);
    assert_eq!(viewer.state(), ViewerState::Entering1);
    assert_eq!(&viewer.status(decoder.store())[..4], b"P2--");
    viewer.key(Key::Digit(0), &context);
    let action = viewer.key(Key::Digit(5), &context);
    assert_eq!(action, Action::Request(page(0x205), SubCode::ANY));
    assert_eq!(viewer.state(), ViewerState::Searching);
    assert_eq!(viewer.key(Key::Back, &context), Action::Request(page(0x100), SubCode::ANY));
}

#[test]
fn other_service_links_need_confirmation() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let decoder = Decoder::new(&mut slots, Config::default());
    let mut viewer = Viewer::new(page(0x100));
    viewer.event(ViewerEvent::Started);
    viewer.event(ViewerEvent::PagePresent);
    let target = LinkTarget { address: page(0x400), subcode: SubCode::ANY, service: Some(1) };
    let links = PageLinks { keys: [None, None, Some(target), None], index: None };
    let config = Config::default();
    let no = |_| false;
    let context = KeyContext {
        links: &links,
        store: decoder.store(),
        initial: page(0x100),
        config: &config,
        now: Instant(0),
        confirmed: &no,
        row24_shown: true,
    };
    let action = viewer.key(Key::Yellow, &context);
    assert_eq!(action, Action::ConfirmService(target));
    assert_eq!(viewer.state(), ViewerState::ConfirmingLink(2));
    assert_eq!(&viewer.status(decoder.store())[1..7], b"LEAVE?");
    let action = viewer.key(Key::Yellow, &context);
    assert_eq!(action, Action::FollowService(target));
    // Cancelling with another key.
    viewer.event(ViewerEvent::PagePresent);
    viewer.key(Key::Yellow, &context);
    assert_eq!(viewer.key(Key::Red, &context), Action::None);
    assert_eq!(viewer.state(), ViewerState::Displaying);
    // REQ-SEC-024: with row 24 off screen the coloured keys do nothing.
    let hidden = KeyContext { row24_shown: false, ..context };
    assert_eq!(viewer.key(Key::Yellow, &hidden), Action::None);
    assert_eq!(viewer.state(), ViewerState::Displaying);
}

/// REQ-NAV-006: a held page is not recomposed until hold is released.
#[test]
fn hold_reveal_and_reveal_clearing() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let decoder = Decoder::new(&mut slots, Config::default());
    let mut viewer = Viewer::new(page(0x100));
    viewer.event(ViewerEvent::Started);
    viewer.event(ViewerEvent::PagePresent);
    let links = PageLinks::default();
    let config = Config::default();
    let no = |_| false;
    let context = KeyContext {
        links: &links,
        store: decoder.store(),
        initial: page(0x100),
        config: &config,
        now: Instant(0),
        confirmed: &no,
        row24_shown: true,
    };
    viewer.key(Key::Reveal, &context);
    assert!(viewer.reveal(Instant(0)));
    viewer.key(Key::Hold, &context);
    assert_eq!(viewer.state(), ViewerState::Holding);
    viewer.key(Key::Digit(3), &context);
    assert_eq!(viewer.state(), ViewerState::Holding, "digits do nothing while holding");
    let magazine = MagazineData::default();
    let rolling = [SPACE; HEADER_DISPLAY_BYTES];
    let compose = ComposeContext {
        magazine: &magazine,
        config: &config,
        rolling_header: &rolling,
        store: decoder.store(),
        row24: None,
        lookup: decoder.enhancement_lookup(),
    };
    let mut grid = Grid::EMPTY;
    let marker = Cell { foreground: COLOUR_GREEN, ..Cell::BLANK };
    grid.cells[5][5] = marker;
    let received = StoredPage { state: PageState::Complete, ..StoredPage::EMPTY };
    assert_eq!(viewer.compose(&mut grid, Some(&received), &compose), Composed::Held, "held: not shown");
    assert_eq!(grid.cells[5][5], marker);
    viewer.key(Key::Hold, &context);
    assert!(matches!(viewer.compose(&mut grid, Some(&received), &compose), Composed::Built(_)), "released");
    assert_ne!(grid.cells[5][5], marker);
    viewer.key(Key::NextPage, &context);
    assert!(!viewer.reveal(Instant(0)), "a different page clears reveal");
}
