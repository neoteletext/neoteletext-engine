//! Tests of the viewer (REQ-FSM-005): page entry, the keys, links and their confirmation, sub-pages,
//! history, hold, reveal, mix and zoom, and the status area.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::acq::{Decoder, StoredPage};
use crate::config::{RevealStyle, StepOrder};
use crate::consts::{ALPHA_RED, ALPHA_YELLOW, COLOUR_GREEN, PAGE_TIME_FILLING, SPACE};
use crate::ecc::with_odd_parity;
use crate::fsm::PageState;
use crate::level1::{Cell, Grid};
use crate::packet::{PacketMeta, encode};
use crate::types::ControlBits;
use std::vec::Vec;

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

/// Feeds a header for each page and sub-code, in order, then a
/// time-filling header in every magazine, so that each is stored.
fn receive(decoder: &mut Decoder<'_>, pages: &[(u16, u16)]) {
    let header = |mpp: u16, subcode: u16| {
        let address = page(mpp);
        let display = [with_odd_parity(b' '); HEADER_DISPLAY_BYTES];
        encode::header(address.magazine, address.page, SubCode::new(subcode), ControlBits(0), &display).unwrap()
    };
    let filling = (1..=8u16).map(|magazine| (magazine << 8 | u16::from(PAGE_TIME_FILLING), 0));
    for (index, (mpp, subcode)) in pages.iter().copied().chain(filling).enumerate() {
        decoder.push(&header(mpp, subcode), &PacketMeta::default(), Instant(index as u64 * 1000), &mut |_| {});
    }
}

/// A viewer that has found its initial page 100 and shows it.
fn displaying() -> Viewer {
    let mut viewer = Viewer::new(page(0x100));
    viewer.event(ViewerEvent::Started);
    viewer.event(ViewerEvent::PagePresent);
    viewer
}

fn context<'a>(
    links: &'a PageLinks,
    store: &'a PageStore<'a>,
    config: &'a Config,
    confirmed: &'a dyn Fn(u8) -> bool,
) -> KeyContext<'a> {
    KeyContext { links, store, initial: page(0x100), config, now: Instant(0), confirmed, row24_shown: true }
}

/// REQ-NAV-007, REQ-FSM-005: reveal, mix and zoom are viewer settings that
/// do not change the machine's state; zoom cycles top, bottom and off;
/// momentary reveal lasts 160 milliseconds after the last request; and
/// Hold acts only while displaying or holding.
#[test]
fn reveal_mix_zoom_and_hold_settings() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let decoder = Decoder::new(&mut slots, Config::default());
    let links = PageLinks::default();
    let momentary = Config { reveal: RevealStyle::Momentary, ..Config::default() };
    let no = |_| false;
    let mut viewer = displaying();
    let keys = context(&links, decoder.store(), &momentary, &no);
    let pressed_at = KeyContext { now: Instant(1_000), ..keys };
    assert_eq!(viewer.key(Key::Reveal, &pressed_at), Action::None);
    let until = 1_000 + momentary.momentary_reveal_us;
    assert!(viewer.reveal(Instant(until - 1)), "on until the timeout");
    assert!(!viewer.reveal(Instant(until)), "off at the timeout");
    assert!(!viewer.mix());
    viewer.key(Key::Mix, &keys);
    assert!(viewer.mix());
    viewer.key(Key::Mix, &keys);
    assert!(!viewer.mix());
    let zooms: Vec<Zoom> = (0..3)
        .map(|_| {
            viewer.key(Key::Zoom, &keys);
            viewer.zoom()
        })
        .collect();
    assert_eq!(zooms, [Zoom::Top, Zoom::Bottom, Zoom::Off]);
    assert_eq!(viewer.state(), ViewerState::Displaying, "settings leave the state");
    // Hold while searching does nothing.
    viewer.key(Key::NextPage, &keys);
    assert_eq!(viewer.state(), ViewerState::Searching);
    viewer.key(Key::Hold, &keys);
    assert_eq!(viewer.state(), ViewerState::Searching);
}

/// REQ-FSM-005, REQ-NAV-006: the decoder events of the viewer table; any
/// event the table does not list for a state leaves it unchanged.
#[test]
fn decoder_events_mark_the_page_stale_and_fresh() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let decoder = Decoder::new(&mut slots, Config::default());
    let mut viewer = Viewer::new(page(0x100));
    assert_eq!(viewer.event(ViewerEvent::PagePresent), Action::None);
    assert_eq!(viewer.state(), ViewerState::Starting, "nothing before the decoder starts");
    viewer.event(ViewerEvent::Started);
    assert_eq!(viewer.event(ViewerEvent::HeaderReceived), Action::None);
    assert_eq!(viewer.state(), ViewerState::Searching);
    viewer.event(ViewerEvent::PagePresent);
    assert_eq!(viewer.requested(), (page(0x100), SubCode::ANY));
    assert_eq!(viewer.event(ViewerEvent::PageStale), Action::None);
    assert_eq!(&viewer.status(decoder.store()), b"P100 ? ", "not recently received");
    assert_eq!(viewer.event(ViewerEvent::PageUpdated), Action::None);
    assert_eq!(&viewer.status(decoder.store()), b"P100   ");
    assert_eq!(viewer.event(ViewerEvent::Started), Action::None);
    assert_eq!(viewer.state(), ViewerState::Displaying);
}

/// REQ-SEC-005, REQ-SEC-008, REQ-NAV-012, decision E.10: the status area
/// shows each security indicator in the receiver's own columns, red for the
/// plain-HTTP marker, yellow for the confirmation of leaving the service,
/// and the page number again when it is cleared.
#[test]
fn status_area_shows_each_security_indicator() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let decoder = Decoder::new(&mut slots, Config::default());
    let mut viewer = displaying();
    let mut status = |indicator| {
        viewer.set_indicator(indicator);
        viewer.status(decoder.store())
    };
    assert_eq!(status(SecurityIndicator::Unencrypted), [ALPHA_RED, b'1', b'0', b'0', b' ', b'!', b' ']);
    assert_eq!(status(SecurityIndicator::LeavingService), [ALPHA_YELLOW, b'L', b'E', b'A', b'V', b'E', b'?']);
    assert_eq!(&status(SecurityIndicator::None), b"P100   ");
}

/// REQ-NAV-003, REQ-NAV-008, REQ-NAV-012, REQ-SVC-004, decision E.36: the
/// coloured keys follow links in the same service at once, and links to a
/// confirmed service at once; a key without a link does nothing; an
/// unconfirmed link asks only while the page is displayed. Index follows
/// the page's index link in the service, and otherwise, like Home,
/// requests the initial page.
#[test]
fn coloured_index_and_home_keys() {
    let mut slots = std::vec![StoredPage::EMPTY; 2];
    let decoder = Decoder::new(&mut slots, Config::default());
    let config = Config::default();
    let here = |mpp, subcode| Some(LinkTarget { address: page(mpp), subcode: SubCode::new(subcode), service: None });
    let away = LinkTarget { address: page(0x400), subcode: SubCode::ANY, service: Some(1) };
    let links =
        PageLinks { keys: [None, here(0x200, 0x3F7F), Some(away), here(0x300, 0x0002)], index: here(0x199, 0x3F7F) };
    let yes = |service| service == 1;
    let no = |_| false;
    let mut viewer = displaying();
    let confirmed = context(&links, decoder.store(), &config, &yes);
    assert_eq!(viewer.key(Key::Red, &confirmed), Action::None, "no red link");
    assert_eq!(viewer.key(Key::Green, &confirmed), Action::Request(page(0x200), SubCode::ANY));
    assert_eq!(viewer.key(Key::Cyan, &confirmed), Action::Request(page(0x300), SubCode::new(0x0002)));
    assert_eq!(viewer.requested(), (page(0x300), SubCode::new(0x0002)));
    assert_eq!(viewer.key(Key::Yellow, &confirmed), Action::FollowService(away), "already confirmed");
    assert_eq!(viewer.state(), ViewerState::Searching);
    // Unconfirmed while still searching: nothing; the question is asked
    // only of a page on screen.
    let unconfirmed = context(&links, decoder.store(), &config, &no);
    assert_eq!(viewer.key(Key::Yellow, &unconfirmed), Action::None);
    assert_eq!(viewer.state(), ViewerState::Searching);
    // A key that is not a coloured key cancels a confirmation.
    viewer.event(ViewerEvent::PagePresent);
    assert_eq!(viewer.key(Key::Yellow, &unconfirmed), Action::ConfirmService(away));
    assert_eq!(viewer.key(Key::Digit(1), &unconfirmed), Action::None);
    assert_eq!(viewer.state(), ViewerState::Displaying);
    assert_eq!(&viewer.status(decoder.store())[..1], b"P", "the indicator is cleared");
    assert_eq!(viewer.key(Key::Index, &unconfirmed), Action::Request(page(0x199), SubCode::ANY));
    assert_eq!(viewer.key(Key::Home, &unconfirmed), Action::Request(page(0x100), SubCode::ANY));
    // Without an index link, or with one to another service, Index stays
    // in the service and requests the initial page.
    for index in [None, Some(away)] {
        let links = PageLinks { index, ..links };
        viewer.key(Key::Digit(5), &unconfirmed);
        viewer.key(Key::Digit(0), &unconfirmed);
        viewer.key(Key::Digit(0), &unconfirmed);
        let keys = context(&links, decoder.store(), &config, &no);
        assert_eq!(viewer.key(Key::Index, &keys), Action::Request(page(0x100), SubCode::ANY), "{index:?}");
    }
}

/// REQ-NAV-001, REQ-NAV-002, REQ-NAV-010: a first digit that cannot be a
/// magazine is rejected and changes nothing; previous page steps down in
/// numerical order, and in the stored order to the neighbouring stored
/// page; Back and Forward move through the history, recording nothing,
/// and do nothing at its ends; the history records the service and can be
/// wiped.
#[test]
fn digits_stepping_and_history() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    receive(&mut decoder, &[(0x150, 0), (0x300, 0)]);
    let links = PageLinks::default();
    let numerical = Config::default();
    let stored = Config { step_order: StepOrder::Stored, ..Config::default() };
    let no = |_| false;
    let mut viewer = displaying();
    viewer.set_service(2);
    let keys = context(&links, decoder.store(), &numerical, &no);
    assert_eq!(viewer.key(Key::Digit(0), &keys), Action::None);
    assert_eq!((viewer.state(), &viewer.status(decoder.store())[..4]), (ViewerState::Displaying, &b"P100"[..]));
    assert_eq!(viewer.key(Key::Forward, &keys), Action::None, "nothing ahead");
    assert_eq!(viewer.key(Key::Back, &keys), Action::None, "nothing behind");
    assert_eq!(viewer.key(Key::PreviousPage, &keys), Action::Request(page(0x899), SubCode::ANY));
    let keys = context(&links, decoder.store(), &stored, &no);
    assert_eq!(viewer.key(Key::NextPage, &keys), Action::Request(page(0x150), SubCode::ANY), "899 wraps to 150");
    assert_eq!(viewer.key(Key::NextPage, &keys), Action::Request(page(0x300), SubCode::ANY));
    assert_eq!(viewer.key(Key::PreviousPage, &keys), Action::Request(page(0x150), SubCode::ANY));
    let entry = viewer.history().current().unwrap();
    assert_eq!((entry.service, entry.address), (2, page(0x150)));
    assert_eq!(viewer.key(Key::Back, &keys), Action::Request(page(0x300), SubCode::ANY));
    assert_eq!(viewer.key(Key::Back, &keys), Action::Request(page(0x150), SubCode::ANY));
    assert_eq!(viewer.key(Key::Forward, &keys), Action::Request(page(0x300), SubCode::ANY));
    assert_eq!(viewer.history().current().unwrap().address, page(0x300), "Back and Forward record nothing");
    viewer.wipe_history();
    assert_eq!(viewer.history().current(), None);
    assert_eq!(viewer.key(Key::Back, &keys), Action::None);
}

/// REQ-NAV-005, REQ-FSM-005: the sub-page keys step through the stored
/// sub-pages in ascending order from the one shown, wrapping at both ends,
/// only while a page is displayed; the status area shows the sub-page's
/// number among those stored, and 00 for one not stored.
#[test]
fn sub_page_stepping_and_numbering() {
    let mut slots = std::vec![StoredPage::EMPTY; 8];
    let mut decoder = Decoder::new(&mut slots, Config::default());
    // Sub-page 2 of page 100 arrives last, so it is the one shown.
    receive(&mut decoder, &[(0x100, 0x0005), (0x100, 0x0001), (0x100, 0x0002)]);
    let config = Config::default();
    let links = PageLinks::default();
    let no = |_| false;
    let keys = context(&links, decoder.store(), &config, &no);
    let mut viewer = displaying();
    let mut step = |key| match viewer.key(key, &keys) {
        Action::Request(address, subcode) => {
            assert_eq!(address, page(0x100));
            let status = viewer.status(decoder.store());
            (subcode.value(), std::string::String::from_utf8(status.to_vec()).unwrap())
        }
        other => panic!("{key:?} gave {other:?}"),
    };
    assert_eq!(step(Key::NextSubPage), (0x0005, "P100/03".into()));
    assert_eq!(step(Key::NextSubPage), (0x0001, "P100/01".into()), "wraps to the first");
    assert_eq!(step(Key::PreviousSubPage), (0x0005, "P100/03".into()), "wraps to the last");
    assert_eq!(step(Key::PreviousSubPage), (0x0002, "P100/02".into()));
    // Not while searching, and not for a page with no stored sub-page.
    let mut viewer = Viewer::new(page(0x100));
    viewer.event(ViewerEvent::Started);
    assert_eq!(viewer.key(Key::NextSubPage, &keys), Action::None);
    let mut viewer = displaying();
    viewer.key(Key::NextPage, &keys);
    viewer.event(ViewerEvent::PagePresent);
    assert_eq!(viewer.key(Key::PreviousSubPage, &keys), Action::None, "page 101 is not stored");
    // A sub-page that is not stored is number 00.
    let target = LinkTarget { address: page(0x100), subcode: SubCode::new(0x0007), service: None };
    let links = PageLinks { index: Some(target), ..PageLinks::default() };
    viewer.key(Key::Index, &context(&links, decoder.store(), &config, &no));
    assert_eq!(&viewer.status(decoder.store()), b"P100/00");
}
