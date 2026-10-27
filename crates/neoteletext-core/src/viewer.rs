//! The viewer (area NAV, layer L6): the viewer state machine of
//! REQ-FSM-005, the receiver-owned status area (decision E.10,
//! REQ-SEC-005), and composing the grid shown on screen.

use crate::acq::{EnhancementLookup, MagazineData, PageStore};
use crate::config::Config;
use crate::consts::HEADER_DISPLAY_BYTES;
use crate::level1::Zoom;
use crate::nav::{History, HistoryEntry, LinkTarget, PageEntry, PageLinks};
use crate::types::{Instant, PageAddress, SubCode};

mod keys;
mod screen;
mod status;

pub use screen::Composed;

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
    /// Waiting for confirmation of a link to another service.
    LeavingService,
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
            self.history.visit(HistoryEntry { service: self.service, address, subcode });
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
}

/// The most sub-pages of one page the viewer lists for stepping and
/// numbering ("2/5"); a page with more steps through the first this many.
/// Public so that a port showing the same list names the same bound.
pub const MAX_LISTED_SUBPAGES: usize = 64;

#[cfg(test)]
mod tests;
