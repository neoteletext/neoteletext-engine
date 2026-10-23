//! The viewer (area NAV, layer L6): the viewer state machine of
//! REQ-FSM-005, the receiver-owned status area (decision E.10,
//! REQ-SEC-005), and composing the grid shown on screen.

use crate::acq::EnhancementReport;
use crate::acq::{EnhancementLookup, MagazineData, PageStore, StoredPage};
use crate::compose::compose_level;
use crate::config::{Config, RevealStyle, StepOrder};
use crate::consts::{ALPHA_RED, ALPHA_YELLOW, HEADER_DISPLAY_BYTES, SPACE, STATUS_COLUMNS};
use crate::level1::{DisplayOptions, Grid, Zoom, build_grid};
use crate::nav::{
    EntryResult, History, HistoryEntry, LinkTarget, PageEntry, PageLinks, next_page, previous_page, step_stored,
};
use crate::types::{Instant, PageAddress, SubCode};

/// Viewer keys: the numbered functions of area NAV.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// A digit 0 to 9.
    Digit(u8),
    /// The red Fastext key.
    Red,
    /// The green Fastext key.
    Green,
    /// The yellow Fastext key.
    Yellow,
    /// The cyan Fastext key.
    Cyan,
    /// Index.
    Index,
    /// Home: the initial page (REQ-NAV-008).
    Home,
    /// Next page.
    NextPage,
    /// Previous page.
    PreviousPage,
    /// Next stored sub-page.
    NextSubPage,
    /// Previous stored sub-page.
    PreviousSubPage,
    /// Reveal.
    Reveal,
    /// Hold.
    Hold,
    /// Mix.
    Mix,
    /// Page zoom.
    Zoom,
    /// Back in history.
    Back,
    /// Forward in history.
    Forward,
}

/// States of the viewer machine (REQ-FSM-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewerState {
    /// Before the decoder starts.
    Starting,
    /// One digit entered.
    Entering1,
    /// Two digits entered.
    Entering2,
    /// The requested page is not yet stored.
    Searching,
    /// The requested page is shown.
    Displaying,
    /// Updates are not applied.
    Holding,
    /// Waiting for the same coloured key to confirm a link to another
    /// service (REQ-NAV-012).
    ConfirmingLink(u8),
}

/// What the application must do after a viewer transition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Nothing beyond redrawing.
    None,
    /// Request a page from the current service.
    Request(PageAddress, SubCode),
    /// Follow a link to another service, already confirmed (REQ-NAV-012).
    FollowService(LinkTarget),
    /// Ask for confirmation of a link to another service.
    ConfirmService(LinkTarget),
}

/// Events from the decoder that drive the viewer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewerEvent {
    /// The decoder started.
    Started,
    /// The requested page is now in the store.
    PagePresent,
    /// A header arrived.
    HeaderReceived,
    /// The displayed page changed.
    PageUpdated,
    /// The displayed page is stale.
    PageStale,
}

/// The security indicator shown in the status area (REQ-SEC-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SecurityIndicator {
    /// No indicator: broadcast or encrypted source.
    None,
    /// The page on screen came over plain HTTP (REQ-SEC-008).
    Unencrypted,
    /// Waiting for confirmation before submitting over plain HTTP.
    PlainHttpWarning,
    /// Waiting for confirmation of a link to another service.
    LeavingService,
    /// Waiting for confirmation of the first submission to an origin
    /// (REQ-SEC-009).
    ConfirmSubmission,
}

/// What a key press needs to know about its surroundings.
#[derive(Clone, Copy)]
pub struct KeyContext<'a> {
    /// The links of the page shown (from X/27/0 or the TTI Link Extension).
    pub links: &'a PageLinks,
    /// The page store.
    pub store: &'a PageStore<'a>,
    /// The initial page (REQ-SVC-004).
    pub initial: PageAddress,
    /// The configuration.
    pub config: &'a Config,
    /// The current time.
    pub now: Instant,
    /// Whether another service's origin was confirmed this session.
    pub confirmed: &'a dyn Fn(u8) -> bool,
    /// Row 24 is on screen with the link prompts. When it is not, the
    /// coloured keys do nothing (REQ-SEC-024).
    pub row24_shown: bool,
}

/// What composing the screen needs besides the page.
#[derive(Clone, Copy)]
pub struct ComposeContext<'a> {
    /// The magazine data of the page's magazine.
    pub magazine: &'a MagazineData,
    /// The configuration.
    pub config: &'a Config,
    /// Header columns 8 to 39 rolled from incoming headers.
    pub rolling_header: &'a [u8; HEADER_DISPLAY_BYTES],
    /// The page store.
    pub store: &'a PageStore<'a>,
    /// A receiver-drawn row 24 (REQ-NAV-011).
    pub row24: Option<[u8; crate::consts::COLUMNS]>,
    /// The stored object pages, other enhancement pages and Magazine
    /// Organization Tables, for Levels 2.5 and 3.5 (REQ-ACQ-020).
    pub lookup: EnhancementLookup<'a>,
}

/// History capacity (REQ-NAV-010 default).
pub const HISTORY_ENTRIES: usize = 32;

/// The viewer.
#[derive(Clone, Copy, Debug)]
pub struct Viewer {
    state: ViewerState,
    requested: PageAddress,
    subcode: SubCode,
    entry: PageEntry,
    history: History<HISTORY_ENTRIES>,
    reveal: bool,
    reveal_until: Option<Instant>,
    mix: bool,
    zoom: Zoom,
    stale: bool,
    indicator: SecurityIndicator,
    service: u8,
}

impl Viewer {
    /// A viewer that will start on `initial`.
    #[must_use]
    pub fn new(initial: PageAddress) -> Self {
        Self {
            state: ViewerState::Starting,
            requested: initial,
            subcode: SubCode::ANY,
            entry: PageEntry::default(),
            history: History::new(),
            reveal: false,
            reveal_until: None,
            mix: false,
            zoom: Zoom::Off,
            stale: false,
            indicator: SecurityIndicator::None,
            service: 0,
        }
    }

    /// The machine state.
    #[must_use]
    pub const fn state(&self) -> ViewerState {
        self.state
    }

    /// The requested page and sub-code.
    #[must_use]
    pub const fn requested(&self) -> (PageAddress, SubCode) {
        (self.requested, self.subcode)
    }

    /// Reveal is on.
    #[must_use]
    pub fn reveal(&self, now: Instant) -> bool {
        self.reveal || self.reveal_until.is_some_and(|until| now < until)
    }

    /// Page zoom.
    #[must_use]
    pub const fn zoom(&self) -> Zoom {
        self.zoom
    }

    /// Mix mode.
    #[must_use]
    pub const fn mix(&self) -> bool {
        self.mix
    }

    /// Sets the security indicator (from the web module's events).
    pub fn set_indicator(&mut self, indicator: SecurityIndicator) {
        self.indicator = indicator;
    }

    /// Sets the service the viewer is showing, for history entries.
    pub fn set_service(&mut self, service: u8) {
        self.service = service;
    }

    /// The history.
    #[must_use]
    pub const fn history(&self) -> &History<HISTORY_ENTRIES> {
        &self.history
    }

    /// Wipes the history (REQ-NAV-010).
    pub fn wipe_history(&mut self) {
        self.history.wipe();
    }

    fn select(&mut self, address: PageAddress, subcode: SubCode, record: bool) -> Action {
        if address != self.requested {
            // REQ-NAV-007: reveal clears when a different page is selected.
            self.reveal = false;
            self.reveal_until = None;
        }
        self.requested = address;
        self.subcode = subcode;
        self.state = ViewerState::Searching;
        self.stale = false;
        if record {
            self.history.visit(HistoryEntry { service: self.service, address, subcode, from_form: false });
        }
        Action::Request(address, subcode)
    }

    /// Handles a decoder event (REQ-FSM-005).
    pub fn event(&mut self, event: ViewerEvent) -> Action {
        match (self.state, event) {
            (ViewerState::Starting, ViewerEvent::Started) => self.select(self.requested, SubCode::ANY, true),
            (ViewerState::Searching, ViewerEvent::PagePresent) => {
                self.state = ViewerState::Displaying;
                Action::None
            }
            (ViewerState::Displaying, ViewerEvent::PageStale) => {
                self.stale = true;
                Action::None
            }
            (ViewerState::Displaying, ViewerEvent::PageUpdated) => {
                self.stale = false;
                Action::None
            }
            _ => Action::None,
        }
    }

    /// Handles a key (REQ-FSM-005). `links` are the links of the page shown
    /// (from X/27/0 or the TTI Link Extension); `confirmed` says whether
    /// another service's origin has been confirmed this session.
    pub fn key(&mut self, key: Key, context: &KeyContext<'_>) -> Action {
        let KeyContext { links, store, initial, config, now, confirmed, row24_shown } = *context;
        if let ViewerState::ConfirmingLink(waiting) = self.state {
            let index = colour_index(key);
            if index == Some(waiting)
                && let Some(target) = links.keys.get(usize::from(waiting)).copied().flatten()
            {
                self.state = ViewerState::Searching;
                self.indicator = SecurityIndicator::None;
                return Action::FollowService(target);
            }
            // Any other key cancels and restores row 24.
            self.state = ViewerState::Displaying;
            self.indicator = SecurityIndicator::None;
            return Action::None;
        }
        match key {
            Key::Digit(digit) if self.state != ViewerState::Holding => self.digit(digit),
            Key::Digit(_) => Action::None,
            Key::Hold => {
                self.state = match self.state {
                    ViewerState::Displaying => ViewerState::Holding,
                    ViewerState::Holding => ViewerState::Displaying,
                    other => other,
                };
                Action::None
            }
            Key::Reveal => {
                match config.reveal {
                    RevealStyle::Latched => self.reveal = !self.reveal,
                    RevealStyle::Momentary => {
                        self.reveal_until = Some(Instant(now.0.saturating_add(config.momentary_reveal_us)));
                    }
                }
                Action::None
            }
            Key::Mix => {
                self.mix = !self.mix;
                Action::None
            }
            Key::Zoom => {
                self.zoom = match self.zoom {
                    Zoom::Off => Zoom::Top,
                    Zoom::Top => Zoom::Bottom,
                    Zoom::Bottom => Zoom::Off,
                };
                Action::None
            }
            _ if !matches!(self.state, ViewerState::Displaying | ViewerState::Searching) => Action::None,
            // REQ-SEC-024: a link may be followed only while its prompt is
            // shown whole in row 24.
            Key::Red | Key::Green | Key::Yellow | Key::Cyan if !row24_shown => Action::None,
            Key::Red | Key::Green | Key::Yellow | Key::Cyan => {
                let index = colour_index(key).unwrap_or(0);
                match links.keys.get(usize::from(index)).copied().flatten() {
                    Some(target) if target.service.is_none() => self.select(target.address, target.subcode, true),
                    Some(target) => {
                        let service = target.service.unwrap_or(0);
                        if confirmed(service) {
                            self.state = ViewerState::Searching;
                            Action::FollowService(target)
                        } else if self.state == ViewerState::Displaying {
                            self.state = ViewerState::ConfirmingLink(index);
                            self.indicator = SecurityIndicator::LeavingService;
                            Action::ConfirmService(target)
                        } else {
                            Action::None
                        }
                    }
                    None => Action::None,
                }
            }
            Key::Index => match links.index {
                Some(target) if target.service.is_none() => self.select(target.address, target.subcode, true),
                _ => self.select(initial, SubCode::ANY, true),
            },
            Key::Home => self.select(initial, SubCode::ANY, true),
            Key::NextPage | Key::PreviousPage => {
                let forward = key == Key::NextPage;
                let target = match config.step_order {
                    StepOrder::Numerical if forward => next_page(self.requested),
                    StepOrder::Numerical => previous_page(self.requested),
                    StepOrder::Stored => step_stored(self.requested, forward, store),
                };
                self.select(target, SubCode::ANY, true)
            }
            Key::NextSubPage | Key::PreviousSubPage => self.step_subpage(key == Key::NextSubPage, store),
            Key::Back => match self.history.back() {
                Some(entry) => self.select(entry.address, entry.subcode, false),
                None => Action::None,
            },
            Key::Forward => match self.history.forward() {
                Some(entry) => self.select(entry.address, entry.subcode, false),
                None => Action::None,
            },
        }
    }

    fn digit(&mut self, digit: u8) -> Action {
        match self.entry.push(digit) {
            EntryResult::Rejected => Action::None,
            EntryResult::Partial => {
                self.state = if self.entry.count() == 1 { ViewerState::Entering1 } else { ViewerState::Entering2 };
                Action::None
            }
            EntryResult::Complete(address) => self.select(address, SubCode::ANY, true),
        }
    }

    fn step_subpage(&mut self, forward: bool, store: &PageStore<'_>) -> Action {
        if self.state != ViewerState::Displaying {
            return Action::None;
        }
        let mut codes = [SubCode::ANY; MAX_LISTED_SUBPAGES];
        let count = store.subcodes(self.requested, &mut codes);
        let listed = codes.get(..count).unwrap_or(&[]);
        if listed.is_empty() {
            return Action::None;
        }
        let current = if self.subcode.is_any() {
            store.latest(self.requested).map(|page| page.key.subcode)
        } else {
            Some(self.subcode)
        };
        let position = current.and_then(|current| listed.iter().position(|&s| s == current)).unwrap_or(0);
        let next = if forward {
            position.saturating_add(1).checked_rem(listed.len()).unwrap_or(0)
        } else {
            position.checked_sub(1).unwrap_or(listed.len().saturating_sub(1))
        };
        let subcode = listed.get(next).copied().unwrap_or(SubCode::ANY);
        self.subcode = subcode;
        Action::Request(self.requested, subcode)
    }

    /// Row 0 columns 0 to 6 (decision E.10, REQ-SEC-005).
    #[must_use]
    pub fn status(&self, store: &PageStore<'_>) -> [u8; STATUS_COLUMNS] {
        let mut status = [SPACE; STATUS_COLUMNS];
        let digits = match self.state {
            ViewerState::Entering1 | ViewerState::Entering2 => self.entry.display(),
            _ => self.requested.ascii(),
        };
        let write = |status: &mut [u8; STATUS_COLUMNS], at: usize, bytes: &[u8]| {
            for (slot, &byte) in status.iter_mut().skip(at).zip(bytes) {
                *slot = byte;
            }
        };
        match self.indicator {
            SecurityIndicator::None => {
                write(&mut status, 0, b"P");
                write(&mut status, 1, &digits);
                if !self.subcode.is_any() {
                    // Specific sub-page: "S" and its number of stored
                    // sub-pages (REQ-NAV-005), as hexadecimal digits.
                    let mut codes = [SubCode::ANY; MAX_LISTED_SUBPAGES];
                    let count = store.subcodes(self.requested, &mut codes);
                    let index = codes.get(..count).and_then(|c| c.iter().position(|&s| s == self.subcode));
                    let number = index.map_or(0, |i| i.saturating_add(1));
                    let [tens, units] = two_digits(number);
                    write(&mut status, 4, &[b'/', tens, units]);
                } else if self.stale {
                    write(&mut status, 5, b"?");
                }
            }
            SecurityIndicator::Unencrypted => {
                // A red marker the page cannot forge: the number and "!".
                write(&mut status, 0, &[ALPHA_RED]);
                write(&mut status, 1, &digits);
                write(&mut status, 5, b"!");
            }
            SecurityIndicator::PlainHttpWarning => {
                write(&mut status, 0, &[ALPHA_RED]);
                write(&mut status, 1, b"HTTP?");
            }
            SecurityIndicator::LeavingService => {
                write(&mut status, 0, &[ALPHA_YELLOW]);
                write(&mut status, 1, b"LEAVE?");
            }
            SecurityIndicator::ConfirmSubmission => {
                write(&mut status, 0, &[ALPHA_YELLOW]);
                write(&mut status, 1, b"SEND?");
            }
        }
        status
    }

    /// Builds the grid the viewer shows (REQ-ARCH-006): Level 1, then the
    /// pass of the presentation level, with the receiver's status area,
    /// rolling header and row 24. While holding it leaves the grid alone.
    ///
    /// At Levels 2.5 and 3.5 the result carries the enhancement processor's
    /// report. The caller records it with `Decoder::record_enhancement`
    /// once `context` no longer borrows the decoder (REQ-L25-051); the
    /// decoder counts each build once however often the page is redrawn.
    #[must_use]
    pub fn compose<const PANEL_ROWS: usize>(
        &self,
        grid: &mut Grid<PANEL_ROWS>,
        page: Option<&StoredPage>,
        context: &ComposeContext<'_>,
    ) -> Composed {
        let ComposeContext { magazine, config, rolling_header, store, row24, lookup } = *context;
        // REQ-NAV-006: while held, new receptions do not reach the screen.
        if self.state == ViewerState::Holding {
            return Composed::Held;
        }
        let searching = page.is_none() || self.state == ViewerState::Searching;
        let options = DisplayOptions {
            status: self.status(store),
            searching,
            rolling_header: if page.is_none() { Some(*rolling_header) } else { None },
            row24,
            security_indicator: self.indicator != SecurityIndicator::None,
            mix: self.mix,
            boxed: false,
        };
        build_grid(grid, page, magazine, config, &options);
        Composed::Built(page.and_then(|page| compose_level(grid, page, magazine, &lookup, config, &options)))
    }
}

/// What [`Viewer::compose`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Composed {
    /// The viewer is holding, and the grid was left alone (REQ-NAV-006).
    Held,
    /// The grid was built; at Levels 2.5 and 3.5, with the report of the
    /// faults the enhancement processor met, for the decoder's statistics.
    Built(Option<EnhancementReport>),
}

/// The most sub-pages of one page the viewer lists for stepping and
/// numbering ("2/5"); a page with more steps through the first this many.
/// Public so that a port showing the same list names the same bound.
pub const MAX_LISTED_SUBPAGES: usize = 64;

fn colour_index(key: Key) -> Option<u8> {
    match key {
        Key::Red => Some(0),
        Key::Green => Some(1),
        Key::Yellow => Some(2),
        Key::Cyan => Some(3),
        _ => None,
    }
}

fn two_digits(value: usize) -> [u8; 2] {
    let clamped = value.min(99) as u8;
    [b'0'.saturating_add(clamped / 10), b'0'.saturating_add(clamped % 10)]
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::acq::Decoder;
    use crate::consts::COLOUR_GREEN;
    use crate::fsm::PageState;
    use crate::level1::Cell;

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
}
