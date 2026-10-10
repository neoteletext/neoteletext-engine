//! A decoding session: the decoder over a heap-allocated page store, the
//! magazine and service data, and composing grids for display.

use tellytxt_core::acq::{Decoder, EnhancementPacket, EnhancementStore, EnhancementSubPage, Event, StoredPage};
use tellytxt_core::compose::compose_level_traced;
use tellytxt_core::config::Config;
use tellytxt_core::consts::{
    COLUMNS, ENHANCEMENT_PACKETS_LEVEL_35, HEADER_DISPLAY_BYTES, MAGAZINES, MOT_PACKETS, PAGE_COMPLETION_TIMEOUT_US,
};
use tellytxt_core::fsm::{PageKey, PageState};
use tellytxt_core::level1::{DisplayOptions, Grid, build_grid};
use tellytxt_core::packet::{Packet, PacketMeta};
use tellytxt_core::service::{ServiceInfo, interpret};
use tellytxt_core::trace::TraceSink;
use tellytxt_core::types::{Instant, PageAddress, SubCode};

/// Default number of stored pages and sub-pages.
pub const DEFAULT_PAGES: usize = 4096;

/// Packets kept for enhancement pages (REQ-ACQ-019): the Annex E.8 total
/// for Level 3.5, plus the eight Magazine Organization Tables, which that
/// total leaves out.
const ENHANCEMENT_PACKETS: usize = ENHANCEMENT_PACKETS_LEVEL_35 + MAGAZINES * MOT_PACKETS;

/// Enhancement sub-pages: one for every eight packets, plus the eight
/// tables. Most object and DRCS sub-pages hold more than eight packets.
const ENHANCEMENT_SUB_PAGES: usize = ENHANCEMENT_PACKETS_LEVEL_35 / 8 + MAGAZINES;

/// A session over one input.
pub struct Session {
    decoder: Decoder<'static>,
    capacity: usize,
    clock: u64,
    /// Events seen, kept for `--verbose` tracing.
    pub trace: Vec<Event>,
    tracing: bool,
}

/// Microseconds between packets when the input has no time stamps: one
/// television line, 64 microseconds, so the page timeout never fires
/// between packets of a file read at full speed.
const LINE_PERIOD_US: u64 = 64;

impl Session {
    /// A session storing up to `capacity` pages.
    #[must_use]
    pub fn new(config: Config, capacity: usize) -> Self {
        // One store for the life of the process.
        let slots: &'static mut [StoredPage] = Box::leak(vec![StoredPage::EMPTY; capacity.max(1)].into_boxed_slice());
        let sub_pages = Box::leak(vec![EnhancementSubPage::EMPTY; ENHANCEMENT_SUB_PAGES].into_boxed_slice());
        let packets = Box::leak(vec![EnhancementPacket::EMPTY; ENHANCEMENT_PACKETS].into_boxed_slice());
        let enhancement = EnhancementStore::new(sub_pages, packets);
        let tracing = config.trace;
        Self {
            decoder: Decoder::with_enhancement_store(slots, enhancement, config),
            capacity: capacity.max(1),
            clock: 0,
            trace: Vec::new(),
            tracing,
        }
    }

    /// The number of page slots.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// The decoder.
    #[must_use]
    pub fn decoder(&self) -> &Decoder<'static> {
        &self.decoder
    }

    /// The decoder, mutably.
    pub fn decoder_mut(&mut self) -> &mut Decoder<'static> {
        &mut self.decoder
    }

    /// Feeds one packet. Time advances by the packet's own time stamp when
    /// it has one, otherwise by one line period.
    pub fn push(&mut self, packet: &Packet, meta: &PacketMeta) {
        self.clock = match meta.timestamp_us {
            Some(stamp) if stamp >= self.clock => stamp,
            _ => self.clock.saturating_add(LINE_PERIOD_US),
        };
        let tracing = self.tracing;
        let trace = &mut self.trace;
        self.decoder.push(packet, meta, Instant(self.clock), &mut |event| {
            if tracing {
                trace.push(event);
            }
        });
    }

    /// Feeds one packet received now, on the session's real clock (live
    /// sources).
    pub fn push_at(&mut self, packet: &Packet, meta: &PacketMeta, now: Instant) {
        self.clock = self.clock.max(now.0);
        let tracing = self.tracing;
        let trace = &mut self.trace;
        self.decoder.push(packet, meta, Instant(self.clock), &mut |event| {
            if tracing {
                trace.push(event);
            }
        });
    }

    /// Lets time pass: completes pages whose packets stopped (decision
    /// E.19).
    pub fn tick(&mut self, now: Instant) {
        self.clock = self.clock.max(now.0);
        let tracing = self.tracing;
        let trace = &mut self.trace;
        self.decoder.tick(Instant(self.clock), &mut |event| {
            if tracing {
                trace.push(event);
            }
        });
    }

    /// Ends the input: completes every page still being received.
    pub fn finish(&mut self) {
        self.clock = self.clock.saturating_add(PAGE_COMPLETION_TIMEOUT_US.saturating_mul(2));
        let tracing = self.tracing;
        let trace = &mut self.trace;
        self.decoder.tick(Instant(self.clock), &mut |event| {
            if tracing {
                trace.push(event);
            }
        });
    }

    /// Every stored page, sorted by address and sub-code.
    #[must_use]
    pub fn pages(&self) -> Vec<&StoredPage> {
        let mut pages: Vec<&StoredPage> =
            self.decoder.store().pages().iter().filter(|page| page.state != PageState::Empty).collect();
        pages.sort_by_key(|page| (page.key.address, page.key.subcode));
        pages
    }

    /// The interpreted 8/30 service data, if any was received.
    #[must_use]
    pub fn service(&self) -> Option<ServiceInfo> {
        self.decoder.service().map(interpret)
    }

    /// The stored page for a request (decision E.20).
    #[must_use]
    pub fn page(&self, address: PageAddress, subcode: SubCode) -> Option<&StoredPage> {
        self.decoder.store().get(address, subcode)
    }

    /// The grid of a page as `ttx show`, `text` and `render` present it:
    /// row 0 with the status area showing the page number (decision E.10),
    /// Level 1 then the pass of the presentation level (REQ-PRF-006).
    /// `None` when the page is not stored. At Levels 2.5 and 3.5 the faults
    /// met are added to the decoder's statistics (REQ-L25-051). With
    /// tracing on, the transitions of the level's walk are printed
    /// (REQ-FSM-010).
    pub fn grid(&mut self, key: PageKey, row24: Option<[u8; COLUMNS]>) -> Option<Grid> {
        let store = self.decoder.store();
        let page = store.find(key).and_then(|index| store.slot(index))?;
        let mut grid = Grid::EMPTY;
        let address = page.key.address;
        let mut status = [b' '; 7];
        status[0] = b'P';
        status[1..4].copy_from_slice(&address.ascii());
        let options = DisplayOptions { status, row24, ..DisplayOptions::default() };
        let magazine = self.decoder.magazine_data(address.magazine);
        let config = self.decoder.config();
        build_grid(&mut grid, Some(page), &magazine, config, &options);
        let lookup = self.decoder.enhancement_lookup();
        let mut entries = Vec::new();
        let mut keep = |entry| entries.push(Event::Trace(entry));
        let mut trace = TraceSink { time: Instant(self.clock), entries: &mut keep };
        let report = compose_level_traced(&mut grid, page, &magazine, &lookup, config, &options, &mut trace);
        if let Some(report) = report {
            self.decoder.record_enhancement(report.build, &report.faults);
        }
        self.trace.extend(entries);
        self.print_trace();
        Some(grid)
    }

    /// Prints the events kept for tracing, as `-vv` asks (REQ-CLI-002), and
    /// forgets them.
    pub fn print_trace(&mut self) {
        for event in self.trace.drain(..) {
            eprintln!("ttx: {event:?}");
        }
    }

    /// The rolling header text, for viewers searching for a page.
    #[must_use]
    pub fn rolling_header(&self) -> [u8; HEADER_DISPLAY_BYTES] {
        let mut out = [b' '; HEADER_DISPLAY_BYTES];
        let rolling = self.decoder.rolling_header();
        let clock = self.decoder.clock();
        out[..rolling.len()].copy_from_slice(rolling);
        out[rolling.len()..].copy_from_slice(clock);
        out
    }
}
