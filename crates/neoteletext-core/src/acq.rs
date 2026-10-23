//! Page acquisition and the page store (area ACQ, layer L2).
//!
//! [`Decoder`] takes packets one at a time, runs the magazine acquisition
//! machines and the page lifecycle machines of [`crate::fsm`], and keeps
//! every page in a [`PageStore`] whose memory the caller provides
//! (REQ-GEN-003). It raises [`Event`]s through a callback (REQ-ARCH-005).
//!
//! Enhancement pages (object pages, the Magazine Organization Table, and
//! any other page whose function is not a basic Level 1 page) are kept as received in an [`EnhancementStore`], the
//! part of the store with its own capacity in packets (decision E.56), and
//! the presentation layer reads them through the [`EnhancementLookup`].

use crate::config::Config;
use crate::consts::{CLOCK_COLUMN, COLUMNS, HEADER_DISPLAY_COLUMN, MAGAZINES, MOT_OBJECT_LINKS_PER_PACKET, SPACE};
use crate::fsm::{MagazineEvent, MagazineState, PageEvent, PageState};
use crate::packet::ServicePacket;

mod enhancement;
mod enhancement_pages;
mod events;
mod page;
mod reception;
mod statistics;
mod store;

use crate::types::{ControlBits, FixedVec, Instant, Magazine, PageAddress, PageNumber};
pub use enhancement::{
    EnhancementKey, EnhancementLookup, EnhancementPacket, EnhancementPage, EnhancementStore, EnhancementSubPage,
    enhancement_index, row_index,
};

pub use events::{Discard, Event, MagazineData};
pub use page::StoredPage;
pub use statistics::{EnhancementFaults, EnhancementReport, PageBuild, Statistics};
pub use store::PageStore;

/// Search state of the requested page (REQ-ACQ-012).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SearchStatus {
    /// Not stored.
    Searching,
    /// Stored and current.
    Found,
    /// Stored, but not received in the most recent cycle.
    FoundNotRecent,
}

/// The decoder: magazine machines, page store, magazine and service data.
#[derive(Debug)]
pub struct Decoder<'a> {
    store: PageStore<'a>,
    config: Config,
    // REQ-ACQ-001: the page in progress in each magazine, and the serial
    // or parallel mode of the latest valid header.
    magazines: [MagazineState; MAGAZINES],
    receiving_slot: [Option<usize>; MAGAZINES],
    last_packet: [Instant; MAGAZINES],
    serial: bool,
    serial_active: Option<Magazine>,
    magazine_data: [MagazineData; MAGAZINES],
    service: Option<ServicePacket>,
    stats: Statistics,
    requested: Option<PageAddress>,
    rolling: [u8; CLOCK_COLUMN - HEADER_DISPLAY_COLUMN],
    clock: [u8; COLUMNS - CLOCK_COLUMN],
    enhancement: EnhancementStore<'a>,
    // REQ-ACQ-016: the enhancement sub-page each magazine is receiving,
    // its generation when the reception started (REQ-ACQ-022), and the
    // control bits of the header that started it.
    receiving_enhancement: [Option<usize>; MAGAZINES],
    start_generation: [Option<u32>; MAGAZINES],
    receiving_control: [ControlBits; MAGAZINES],
    // Pages that stored links name as object pages (decision E.56):
    // a bounded hint list, refilled as links arrive.
    named: FixedVec<PageAddress, NAMED_ENHANCEMENT_PAGES>,
    // Where the page of the header being processed is stored.
    pending: Route,
    // The build whose enhancement faults were recorded last.
    last_recorded: Option<PageBuild>,
}

/// Bytes of header display data before the clock: columns 8 to 31.
const ROLLING_BYTES: usize = CLOCK_COLUMN - HEADER_DISPLAY_COLUMN;

/// Addresses kept in the list of pages named by links: every object link
/// of both levels of the eight Magazine Organization Tables (Table 20: 8
/// object links each), so a complete set of tables always fits. Links of
/// X/27/4 and X/27/5 share the list; when it is full the oldest address is
/// dropped, and it returns with the next table or link packet that names
/// it.
const NAMED_ENHANCEMENT_PAGES: usize = 2 * MAGAZINES * MOT_OBJECT_LINKS_PER_PACKET * 2;

/// Where the page of a header is stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Route {
    /// In the page store, with the parity merge of decision E.8.
    main: bool,
    /// In the enhancement store, as received (decision E.56).
    enhancement: bool,
}

impl<'a> Decoder<'a> {
    /// A decoder storing pages in `pages` (REQ-GEN-003), with no store for
    /// enhancement pages: object pages and the Magazine Organization Table
    /// are not kept.
    pub fn new(pages: &'a mut [StoredPage], config: Config) -> Self {
        Self::with_enhancement_store(pages, EnhancementStore::new(&mut [], &mut []), config)
    }

    /// A decoder storing pages in `pages` and enhancement pages in
    /// `enhancement` (REQ-GEN-003, REQ-ACQ-019).
    pub fn with_enhancement_store(
        pages: &'a mut [StoredPage],
        enhancement: EnhancementStore<'a>,
        config: Config,
    ) -> Self {
        Self {
            store: PageStore::new(pages),
            config,
            magazines: [MagazineState::Idle; MAGAZINES],
            receiving_slot: [None; MAGAZINES],
            last_packet: [Instant(0); MAGAZINES],
            serial: false,
            serial_active: None,
            magazine_data: [MagazineData::default(); MAGAZINES],
            service: None,
            stats: Statistics::default(),
            requested: None,
            rolling: [SPACE; ROLLING_BYTES],
            clock: [SPACE; COLUMNS - CLOCK_COLUMN],
            enhancement,
            receiving_enhancement: [None; MAGAZINES],
            start_generation: [None; MAGAZINES],
            receiving_control: [ControlBits(0); MAGAZINES],
            named: FixedVec::new(PageAddress { magazine: Magazine::from_bits(1), page: PageNumber(0) }),
            pending: Route::default(),
            last_recorded: None,
        }
    }

    /// The store of enhancement pages.
    #[must_use]
    pub const fn enhancement_store(&self) -> &EnhancementStore<'a> {
        &self.enhancement
    }

    /// The read-only lookup through which the presentation layer reaches
    /// stored object pages, Magazine Organization Tables, other enhancement
    /// pages and
    /// M/29 packets (REQ-ACQ-020).
    #[must_use]
    pub fn enhancement_lookup(&self) -> EnhancementLookup<'_> {
        self.enhancement.lookup(&self.magazine_data)
    }

    /// Tells the decoder that a new input starts: the stored M/29 packets
    /// of every magazine are discarded (REQ-ACQ-018).
    pub fn start_new_input(&mut self, events: &mut dyn FnMut(Event)) {
        self.discard_magazine_data(events);
    }

    /// Discards every stored M/29 packet, raising an event for each
    /// magazine that had any (REQ-ACQ-018).
    fn discard_magazine_data(&mut self, events: &mut dyn FnMut(Event)) {
        for (index, data) in self.magazine_data.iter_mut().enumerate() {
            if *data != MagazineData::default() {
                *data = MagazineData::default();
                events(Event::MagazineData(Magazine::from_bits(index as u8)));
            }
        }
    }

    /// The page store.
    #[must_use]
    pub fn store(&self) -> &PageStore<'a> {
        &self.store
    }

    /// The configuration in use.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Replaces the configuration; stored pages are kept (REQ-ARCH-008).
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    /// Statistics so far.
    #[must_use]
    pub const fn statistics(&self) -> &Statistics {
        &self.stats
    }

    /// Resets the statistics.
    pub fn reset_statistics(&mut self) {
        self.stats = Statistics::default();
    }

    /// Adds the faults that building a page met at Levels 2.5 and 3.5 to
    /// the statistics (REQ-L25-051, REQ-ARCH-007), and returns whether they
    /// were added. Call it with the `build` and `faults` of the
    /// `EnhancementReport` that building the grid returned, after the
    /// grid is built, because building borrows the decoder through
    /// [`Decoder::enhancement_lookup`].
    ///
    /// A build is counted once: a report for the same build as the one
    /// recorded last is ignored. A viewer composes the shown page on every
    /// redraw, and each redraw meets the same faults, so counting them all
    /// would make the counts measure how long the page was on screen. A
    /// build is the page, its reception count and the level asked for.
    /// Keying on the reception, not on every stored row, counts each fault
    /// once per reception of the page, as the acquisition counts count each
    /// damaged packet once per reception. The reception count is used
    /// rather than the content because it is already kept and costs no
    /// comparison of page bytes.
    pub fn record_enhancement(&mut self, build: PageBuild, faults: &EnhancementFaults) -> bool {
        if self.last_recorded == Some(build) {
            return false;
        }
        self.last_recorded = Some(build);
        self.stats.enhancement.add(faults);
        true
    }

    /// The magazine data of a magazine.
    #[must_use]
    pub fn magazine_data(&self, magazine: Magazine) -> MagazineData {
        self.magazine_data.get(magazine.index()).copied().unwrap_or_default()
    }

    /// The latest 8/30 packet.
    #[must_use]
    pub const fn service(&self) -> Option<&ServicePacket> {
        self.service.as_ref()
    }

    /// Tells the decoder which page the viewer wants, for the rolling header
    /// (decision E.11).
    pub fn set_requested(&mut self, requested: Option<PageAddress>) {
        self.requested = requested;
    }

    /// Header columns 8 to 31 rolled from incoming headers (decision E.11).
    #[must_use]
    pub const fn rolling_header(&self) -> &[u8; ROLLING_BYTES] {
        &self.rolling
    }

    /// Header columns 32 to 39: the clock (decision E.11).
    #[must_use]
    pub const fn clock(&self) -> &[u8; COLUMNS - CLOCK_COLUMN] {
        &self.clock
    }

    /// The search state of a page (REQ-ACQ-012).
    #[must_use]
    pub fn search_status(&self, address: PageAddress) -> SearchStatus {
        match self.store.latest(address).map(|page| page.state) {
            None | Some(PageState::Empty) => SearchStatus::Searching,
            Some(PageState::Stale) => SearchStatus::FoundNotRecent,
            Some(PageState::Partial | PageState::Complete) => SearchStatus::Found,
        }
    }

    /// Completes pages whose packets stopped arriving (decision E.19,
    /// REQ-ACQ-003) and marks pages stale (REQ-FSM-004). Call regularly with
    /// the current time.
    pub fn tick(&mut self, now: Instant, events: &mut dyn FnMut(Event)) {
        for index in 0..MAGAZINES {
            let state = self.magazines.get(index).copied().unwrap_or(MagazineState::Idle);
            let last = self.last_packet.get(index).copied().unwrap_or_default();
            let receiving = !matches!(state, MagazineState::Idle);
            if receiving && now.since(last) > self.config.page_completion_timeout_us {
                self.magazine_event(index, MagazineEvent::Timeout, now, events);
            }
        }
        let stale_after = self.config.stale_after_us;
        for index in 0..self.store.pages().len() {
            let Some(page) = self.store.slot(index) else { continue };
            if page.state == PageState::Complete && now.since(page.last_received) > stale_after {
                self.page_event(index, PageEvent::NotReceived, now, events);
            }
        }
    }

    fn count(&mut self, magazine: Magazine, number: u8) {
        if let Some(count) = self.stats.per_magazine.get_mut(magazine.index()) {
            *count = count.saturating_add(1);
        }
        if let Some(count) = self.stats.per_packet_number.get_mut(usize::from(number)) {
            *count = count.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests;
