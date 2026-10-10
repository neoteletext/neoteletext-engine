//! Page acquisition and the page store (area ACQ, layer L2).
//!
//! [`Decoder`] takes packets one at a time, runs the magazine acquisition
//! machines and the page lifecycle machines of [`crate::fsm`], and keeps
//! every page in a [`PageStore`] whose memory the caller provides
//! (REQ-GEN-003). It raises [`Event`]s through a callback (REQ-ARCH-005).

use crate::config::{Config, ParityPolicy};
use crate::consts::{
    C4_ERASE, C8_UPDATE, C9_INTERRUPTED, CLOCK_COLUMN, COLUMNS, DATA_BYTES, DESIGNATION_CODES, HEADER_DISPLAY_BYTES,
    HEADER_DISPLAY_COLUMN, MAGAZINES, SPACE, STORED_ROWS, TRIPLETS_PER_PACKET,
};
use crate::ecc::has_odd_parity;
use crate::fsm::{
    MagazineAction, MagazineEvent, MagazineState, PageAction, PageEvent, PageKey, PageState, magazine_action_name,
    magazine_event_name, magazine_state_name, magazine_transition, page_action_name, page_event_name, page_state_name,
    page_transition,
};
use crate::packet::{
    Decoded, EnhancementData, Header, Links, Packet, PacketError, PacketMeta, ServicePacket, Triplet, decode,
};
use crate::trace::{Machine, TraceEntry};
use crate::types::{ControlBits, Instant, Magazine, PageAddress, PageNumber, SubCode};

/// One stored page or sub-page (REQ-ACQ-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StoredPage {
    /// The lifecycle state (REQ-FSM-004).
    pub state: PageState,
    /// Address and sub-code; meaningless while `state` is `Empty`.
    pub key: PageKey,
    /// Header control bits of the latest reception.
    pub control: ControlBits,
    /// Rows 0 to 25, raw with parity. Row 0 columns 8 to 39 hold header
    /// bytes 14 to 45; columns 0 to 7 are never written (REQ-SEC-005).
    pub cells: [[u8; COLUMNS]; STORED_ROWS],
    /// Per row, bit c set when the byte in column c passed parity.
    pub parity_ok: [u64; STORED_ROWS],
    /// Per row, bit c set when column c has held a byte that passed parity
    /// since the page was last erased (decision E.8).
    pub ever_good: [u64; STORED_ROWS],
    /// Bit r set when row r has been received.
    pub rows_received: u32,
    /// Packets X/26 by designation code; bit d of `enhancements_present`
    /// marks code d as received.
    pub enhancements: [[Option<Triplet>; TRIPLETS_PER_PACKET]; DESIGNATION_CODES],
    /// Which designation codes of X/26 are present.
    pub enhancements_present: u16,
    /// Packet X/27/0.
    pub links: Option<Links>,
    /// Packets X/27/1 to X/27/3, raw; bit d-1 of `links_raw_present`.
    pub links_raw: [[u8; DATA_BYTES]; 3],
    /// Which of X/27/1 to X/27/3 are present.
    pub links_raw_present: u8,
    /// Packet X/28/0 format 1, if received and decodable.
    pub page_data: Option<EnhancementData>,
    /// Packet X/28/1, raw.
    pub page_data_raw: Option<[u8; DATA_BYTES]>,
    /// Time of the first reception.
    pub first_received: Instant,
    /// Time of the most recent packet.
    pub last_received: Instant,
    /// Number of receptions.
    pub receptions: u32,
    /// Number of parity and Hamming errors seen.
    pub errors: u32,
}

/// A blank row.
const SPACES: [u8; COLUMNS] = [SPACE; COLUMNS];

impl StoredPage {
    /// An empty slot, for building caller-provided storage.
    pub const EMPTY: Self = Self {
        state: PageState::Empty,
        key: PageKey {
            address: PageAddress { magazine: Magazine::from_bits(1), page: PageNumber(0) },
            subcode: SubCode::new(0),
        },
        control: ControlBits(0),
        cells: [SPACES; STORED_ROWS],
        parity_ok: [0; STORED_ROWS],
        ever_good: [0; STORED_ROWS],
        rows_received: 0,
        enhancements: [[None; TRIPLETS_PER_PACKET]; DESIGNATION_CODES],
        enhancements_present: 0,
        links: None,
        links_raw: [[0; DATA_BYTES]; 3],
        links_raw_present: 0,
        page_data: None,
        page_data_raw: None,
        first_received: Instant(0),
        last_received: Instant(0),
        receptions: 0,
        errors: 0,
    };

    /// False when packet X/28/0 format 1 says the page is not a basic Level 1
    /// page coded as 7 bits with odd parity: such pages are not displayed as
    /// text (REQ-ACQ-014).
    #[must_use]
    pub fn is_text_page(&self) -> bool {
        self.page_data.is_none_or(|data| data.page_function == 0 && data.page_coding == 0)
    }

    /// True when row `row` has been received.
    #[must_use]
    pub fn has_row(&self, row: u8) -> bool {
        self.rows_received.checked_shr(u32::from(row)).is_some_and(|bits| bits & 1 == 1)
    }

    /// The stored byte at a position and whether it is displayable under
    /// `policy` (decision E.8): `(byte, passed)`. Never-received positions
    /// give a space.
    #[must_use]
    pub fn byte(&self, row: usize, column: usize, policy: ParityPolicy) -> (u8, bool) {
        let byte = self.cells.get(row).and_then(|cells| cells.get(column)).copied().unwrap_or(SPACE);
        let bit = 1u64.checked_shl(column as u32).unwrap_or(0);
        let passed = self.parity_ok.get(row).is_some_and(|flags| flags & bit != 0);
        let ever = self.ever_good.get(row).is_some_and(|flags| flags & bit != 0);
        match policy {
            ParityPolicy::KeepLastGood => (byte, passed || ever),
            ParityPolicy::AlwaysSpace | ParityPolicy::ErrorMarker => (byte, passed),
        }
    }

    /// Clears rows 1 to 28 and every packet attached to them (REQ-ACQ-004).
    fn erase(&mut self) {
        for (index, row) in self.cells.iter_mut().enumerate() {
            if index > 0 {
                *row = SPACES;
            }
        }
        for (index, (ok, ever)) in self.parity_ok.iter_mut().zip(self.ever_good.iter_mut()).enumerate() {
            if index > 0 {
                *ok = 0;
                *ever = 0;
            }
        }
        self.rows_received &= 1;
        self.enhancements = [[None; TRIPLETS_PER_PACKET]; DESIGNATION_CODES];
        self.enhancements_present = 0;
        self.links = None;
        self.links_raw_present = 0;
        self.page_data = None;
        self.page_data_raw = None;
    }

    /// Merges received bytes into `row` starting at `column` (decision E.8).
    /// Returns the number of parity failures.
    fn merge(&mut self, row: usize, column: usize, bytes: &[u8], policy: ParityPolicy) -> u32 {
        let mut failures = 0u32;
        let (Some(cells), Some(ok), Some(ever)) =
            (self.cells.get_mut(row), self.parity_ok.get_mut(row), self.ever_good.get_mut(row))
        else {
            return 0;
        };
        for (offset, &byte) in bytes.iter().enumerate() {
            let position = column.saturating_add(offset);
            let Some(cell) = cells.get_mut(position) else { break };
            let bit = 1u64.checked_shl(position as u32).unwrap_or(0);
            if has_odd_parity(byte) {
                *cell = byte;
                *ok |= bit;
                *ever |= bit;
            } else {
                failures = failures.saturating_add(1);
                let keep_old = policy == ParityPolicy::KeepLastGood && *ever & bit != 0;
                if !keep_old {
                    *cell = byte;
                    *ok &= !bit;
                }
            }
        }
        if let Some(mask) = 1u32.checked_shl(row as u32) {
            self.rows_received |= mask;
        }
        failures
    }
}

/// The page store in cache mode (REQ-ACQ-005), over caller-provided slots.
#[derive(Debug)]
pub struct PageStore<'a> {
    pages: &'a mut [StoredPage],
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

    /// The slot holding a page and sub-code.
    #[must_use]
    pub fn find(&self, key: PageKey) -> Option<usize> {
        self.pages.iter().position(|page| page.state != PageState::Empty && page.key == key)
    }

    /// The most recently received sub-page of an address (REQ-NAV-005's
    /// rotating display, decision E.20 "any sub-page").
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

    /// The stored sub-codes of an address in ascending order, written into
    /// `out`; returns how many were written.
    pub fn subcodes(&self, address: PageAddress, out: &mut [SubCode]) -> usize {
        let mut count = 0usize;
        for page in self.pages.iter().filter(|page| page.state != PageState::Empty && page.key.address == address) {
            // Insertion into the sorted prefix; bounded by `out.len()`.
            let Some(prefix) = out.get_mut(..count) else { break };
            let position = prefix.iter().position(|&s| s > page.key.subcode).unwrap_or(count);
            if count < out.len() {
                count = count.saturating_add(1);
                if let Some(tail) = out.get_mut(position..count) {
                    tail.rotate_right(1);
                    if let Some(first) = tail.first_mut() {
                        *first = page.key.subcode;
                    }
                }
            } else if let Some(tail) = out.get_mut(position..) {
                // Full: keep the lowest sub-codes.
                tail.rotate_right(1);
                if let Some(first) = tail.first_mut() {
                    *first = page.key.subcode;
                }
            }
        }
        count
    }

    /// True when any sub-page of an address is stored.
    #[must_use]
    pub fn contains(&self, address: PageAddress) -> bool {
        self.pages.iter().any(|page| page.state != PageState::Empty && page.key.address == address)
    }

    /// Finds or allocates a slot for a page, evicting the least recently
    /// received page that is not being received when the store is full.
    fn allocate(&mut self, key: PageKey, busy: &[Option<usize>; MAGAZINES]) -> Option<(usize, bool)> {
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

    fn page_mut(&mut self, index: usize) -> Option<&mut StoredPage> {
        self.pages.get_mut(index)
    }
}

/// The magazine data of one magazine (REQ-ACQ-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MagazineData {
    /// The latest valid M/29/0.
    pub data: Option<EnhancementData>,
    /// The latest M/29/1, raw.
    pub raw: Option<[u8; DATA_BYTES]>,
}

/// Why a packet was discarded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Discard {
    /// An uncorrectable address or designation byte.
    Packet(PacketError),
    /// A row with no page being received in its magazine.
    RowWithoutHeader,
    /// A row of another magazine in serial mode (Annex B.3).
    OtherMagazine,
}

/// An event raised by the decoder (REQ-ARCH-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    /// A header was received, for the rolling header.
    HeaderReceived {
        /// Its magazine.
        magazine: Magazine,
        /// Its page number, if decodable.
        page: Option<PageNumber>,
    },
    /// A page reception started.
    PageStarted(PageKey),
    /// A row changed.
    PageUpdated {
        /// The page.
        key: PageKey,
        /// The packet number.
        row: u8,
    },
    /// A page reception completed.
    PageCompleted(PageKey),
    /// A page was erased by C4.
    PageErased(PageKey),
    /// C8 was seen, or the content changed (REQ-ACQ-010).
    UpdateIndicator(PageKey),
    /// The page has not been received for the configured period.
    PageStale(PageKey),
    /// Packet 8/30 was received.
    ServiceData,
    /// M/29 was received for a magazine.
    MagazineData(Magazine),
    /// A packet was discarded.
    Discarded(Discard),
    /// A state machine transition (REQ-FSM-010).
    Trace(TraceEntry),
}

/// Counters the caller can read and reset (REQ-ARCH-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Statistics {
    /// Packets received per magazine, index 0 for magazine 8.
    pub per_magazine: [u32; MAGAZINES],
    /// Packets received per packet number.
    pub per_packet_number: [u32; 32],
    /// Packets discarded for an uncorrectable address.
    pub address_failures: u32,
    /// Uncorrectable designation codes.
    pub designation_failures: u32,
    /// Triplets that failed Hamming 24/18.
    pub triplet_failures: u32,
    /// Bytes that failed parity.
    pub parity_failures: u32,
    /// Rows discarded with no page in progress, or out of sequence.
    pub discarded: u32,
    /// Packets counted and ignored.
    pub ignored: u32,
    /// Pages started.
    pub pages_started: u32,
    /// Pages completed.
    pub pages_completed: u32,
}

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
}

/// Bytes of header display data before the clock: columns 8 to 31.
const ROLLING_BYTES: usize = CLOCK_COLUMN - HEADER_DISPLAY_COLUMN;

impl<'a> Decoder<'a> {
    /// A decoder storing pages in `pages` (REQ-GEN-003).
    pub fn new(pages: &'a mut [StoredPage], config: Config) -> Self {
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

    /// Processes one packet.
    pub fn push(&mut self, packet: &Packet, _meta: &PacketMeta, now: Instant, events: &mut dyn FnMut(Event)) {
        let decoded = match decode(packet) {
            Ok(decoded) => decoded,
            Err(error) => {
                match error {
                    PacketError::BadAddress => {
                        self.stats.address_failures = self.stats.address_failures.saturating_add(1);
                    }
                    PacketError::BadDesignation => {
                        self.stats.designation_failures = self.stats.designation_failures.saturating_add(1);
                    }
                }
                events(Event::Discarded(Discard::Packet(error)));
                return;
            }
        };
        match decoded {
            Decoded::Header(header) => {
                self.count(header.magazine, 0);
                self.header(&header, now, events);
            }
            Decoded::Row(row) => {
                self.count(row.magazine, row.row);
                self.row_like(row.magazine, now, events, |page, policy| {
                    let failures = page.merge(usize::from(row.row), 0, &row.bytes, policy);
                    (row.row, failures)
                });
            }
            Decoded::Enhancement(enhancement) => {
                self.count(enhancement.magazine, crate::consts::PACKET_ENHANCEMENT);
                let failures = enhancement.triplets.iter().filter(|t| t.is_none()).count() as u32;
                self.stats.triplet_failures = self.stats.triplet_failures.saturating_add(failures);
                self.row_like(enhancement.magazine, now, events, |page, _| {
                    let code = usize::from(enhancement.designation);
                    if let Some(slot) = page.enhancements.get_mut(code) {
                        *slot = enhancement.triplets;
                        page.enhancements_present |= 1u16.checked_shl(code as u32).unwrap_or(0);
                    }
                    (crate::consts::PACKET_ENHANCEMENT, 0)
                });
            }
            Decoded::Links(links) => {
                self.count(links.magazine, crate::consts::PACKET_LINKS);
                self.row_like(links.magazine, now, events, |page, _| {
                    page.links = Some(links);
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::LinksRaw { magazine, designation, bytes } => {
                self.count(magazine, crate::consts::PACKET_LINKS);
                self.row_like(magazine, now, events, |page, _| {
                    let index = usize::from(designation.saturating_sub(1));
                    if let Some(slot) = page.links_raw.get_mut(index) {
                        *slot = bytes;
                        page.links_raw_present |= 1u8.checked_shl(index as u32).unwrap_or(0);
                    }
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::PageData { magazine, data } => {
                self.count(magazine, crate::consts::PACKET_PAGE_ENHANCEMENT);
                self.row_like(magazine, now, events, |page, _| {
                    page.page_data = data;
                    (crate::consts::PACKET_PAGE_ENHANCEMENT, 0)
                });
            }
            Decoded::PageDataRaw { magazine, bytes } => {
                self.count(magazine, crate::consts::PACKET_PAGE_ENHANCEMENT);
                self.row_like(magazine, now, events, |page, _| {
                    page.page_data_raw = Some(bytes);
                    (crate::consts::PACKET_PAGE_ENHANCEMENT, 0)
                });
            }
            Decoded::Magazine { magazine, data } => {
                self.count(magazine, crate::consts::PACKET_MAGAZINE_ENHANCEMENT);
                if let (Some(slot), Some(data)) = (self.magazine_data.get_mut(magazine.index()), data) {
                    slot.data = Some(data);
                }
                events(Event::MagazineData(magazine));
            }
            Decoded::MagazineRaw { magazine, bytes } => {
                self.count(magazine, crate::consts::PACKET_MAGAZINE_ENHANCEMENT);
                if let Some(slot) = self.magazine_data.get_mut(magazine.index()) {
                    slot.raw = Some(bytes);
                }
                events(Event::MagazineData(magazine));
            }
            Decoded::Service(service) => {
                self.count(Magazine::from_bits(0), crate::consts::PACKET_SERVICE);
                self.service = Some(service);
                events(Event::ServiceData);
            }
            Decoded::Ignored(address) => {
                self.count(address.magazine, address.number);
                self.stats.ignored = self.stats.ignored.saturating_add(1);
            }
        }
    }

    /// Completes pages whose packets stopped arriving (decision E.19) and
    /// marks pages stale (REQ-FSM-004). Call regularly with the current time.
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

    /// Handles a header for all eight magazine machines.
    fn header(&mut self, header: &Header, now: Instant, events: &mut dyn FnMut(Event)) {
        if let Some(serial) = header.serial() {
            self.serial = serial;
        }
        let own = header.magazine;
        let time_filling = header.page.is_some_and(PageNumber::is_time_filling);
        for index in 0..MAGAZINES {
            let event = if index == own.index() {
                self.own_header_event(header)
            } else if self.serial {
                if time_filling { MagazineEvent::TimeFillingHeader } else { MagazineEvent::HeaderOfOtherMagazineSerial }
            } else {
                MagazineEvent::UnrelatedHeader
            };
            self.magazine_event(index, event, now, events);
        }
        if let Some(last) = self.last_packet.get_mut(own.index()) {
            *last = now;
        }
        self.serial_active = if self.serial { Some(own) } else { None };
        self.header_store(header, now, events);
        self.roll(header);
        events(Event::HeaderReceived { magazine: own, page: header.page });
    }

    fn own_header_event(&self, header: &Header) -> MagazineEvent {
        let (Some(page), Some(subcode), Some(control)) = (header.page, header.subcode, header.control) else {
            return MagazineEvent::BadHeader;
        };
        if page.is_time_filling() {
            return MagazineEvent::TimeFillingHeader;
        }
        if !page.is_decimal() && !self.config.store_hexadecimal_pages {
            return MagazineEvent::NonStorableHeader;
        }
        let key = PageKey { address: PageAddress { magazine: header.magazine, page }, subcode };
        MagazineEvent::StorableHeader { key, erase: control.has(C4_ERASE) }
    }

    /// Updates the rolling header and the clock (decision E.11).
    fn roll(&mut self, header: &Header) {
        let Some(control) = header.control else { return };
        if !header.is_complete() {
            return;
        }
        if let Some(clock) = header.display.get(ROLLING_BYTES..HEADER_DISPLAY_BYTES) {
            self.clock.copy_from_slice(clock);
        }
        let interrupted = control.has(C9_INTERRUPTED);
        let wanted = self.serial || self.requested.is_none_or(|requested| requested.magazine == header.magazine);
        if self.config.rolling_header
            && !interrupted
            && wanted
            && let Some(text) = header.display.get(..ROLLING_BYTES)
        {
            self.rolling.copy_from_slice(text);
        }
    }

    /// Handles a row-like packet (rows 1 to 28) of a magazine.
    fn row_like(
        &mut self,
        magazine: Magazine,
        now: Instant,
        events: &mut dyn FnMut(Event),
        store: impl FnOnce(&mut StoredPage, ParityPolicy) -> (u8, u32),
    ) {
        let (index, event) = match self.serial_active {
            Some(active) if self.serial && active != magazine => {
                (active.index(), MagazineEvent::RowOfOtherMagazineSerial)
            }
            _ => (magazine.index(), MagazineEvent::Row),
        };
        let action = self.magazine_event(index, event, now, events);
        if let MagazineAction::Store(_) = action {
            if let Some(slot) = self.receiving_slot.get(index).copied().flatten() {
                let policy = self.config.parity_policy;
                let mut stored = None;
                if let Some(page) = self.store.page_mut(slot) {
                    let (row, failures) = store(page, policy);
                    page.last_received = now;
                    page.errors = page.errors.saturating_add(failures);
                    stored = Some((row, failures));
                }
                if let Some((row, failures)) = stored {
                    self.stats.parity_failures = self.stats.parity_failures.saturating_add(failures);
                    self.page_event(slot, PageEvent::RowStored(row), now, events);
                }
            }
            if let Some(last) = self.last_packet.get_mut(index) {
                *last = now;
            }
        }
    }

    /// Runs one magazine machine transition and performs its action.
    fn magazine_event(
        &mut self,
        index: usize,
        event: MagazineEvent,
        now: Instant,
        events: &mut dyn FnMut(Event),
    ) -> MagazineAction {
        let Some(state) = self.magazines.get(index).copied() else { return MagazineAction::None };
        let (next, action) = magazine_transition(state, event);
        if let Some(slot) = self.magazines.get_mut(index) {
            *slot = next;
        }
        if self.config.trace && (next != state || action != MagazineAction::None) {
            events(Event::Trace(TraceEntry {
                machine: Machine::Magazine(Magazine::from_bits(index as u8).number()),
                time: now,
                from: magazine_state_name(state),
                event: magazine_event_name(event),
                action: magazine_action_name(action),
                to: magazine_state_name(next),
            }));
        }
        match action {
            // Storing is done by the caller, which holds the packet; absorbed
            // rows are discarded silently (REQ-FSM-003).
            MagazineAction::None | MagazineAction::Store(_) | MagazineAction::DiscardAbsorbed => {}
            MagazineAction::DiscardWithoutHeader => self.discard(Discard::RowWithoutHeader, events),
            MagazineAction::DiscardOtherMagazine => self.discard(Discard::OtherMagazine, events),
            MagazineAction::Start { key, erase } => self.start(index, key, erase, now, events),
            MagazineAction::End(_) => self.end(index, now, events),
            MagazineAction::EndAndStart { started, erase, .. } => {
                self.end(index, now, events);
                self.start(index, started, erase, now, events);
            }
        }
        action
    }

    fn discard(&mut self, reason: Discard, events: &mut dyn FnMut(Event)) {
        self.stats.discarded = self.stats.discarded.saturating_add(1);
        events(Event::Discarded(reason));
    }

    fn start(&mut self, magazine_index: usize, key: PageKey, erase: bool, now: Instant, events: &mut dyn FnMut(Event)) {
        let Some((slot, evict)) = self.store.allocate(key, &self.receiving_slot) else {
            return;
        };
        if evict {
            self.page_event(slot, PageEvent::Evicted, now, events);
        }
        if let Some(page) = self.store.page_mut(slot)
            && page.state == PageState::Empty
        {
            *page = StoredPage::EMPTY;
            page.key = key;
            page.first_received = now;
        }
        if let Some(receiving) = self.receiving_slot.get_mut(magazine_index) {
            *receiving = Some(slot);
        }
        self.stats.pages_started = self.stats.pages_started.saturating_add(1);
        self.page_event(slot, PageEvent::ReceptionStarted { erase }, now, events);
    }

    /// Stores the display bytes and control bits of a header into the page
    /// it has just started, and raises the update indicator for C8
    /// (REQ-ACQ-010).
    fn header_store(&mut self, header: &Header, now: Instant, events: &mut dyn FnMut(Event)) {
        let index = header.magazine.index();
        let Some(control) = header.control else { return };
        let Some(MagazineState::Receiving(key)) = self.magazines.get(index).copied() else { return };
        let Some(slot) = self.receiving_slot.get(index).copied().flatten() else { return };
        self.store_header(slot, &header.display, control, now);
        if control.has(C8_UPDATE) {
            events(Event::UpdateIndicator(key));
        }
    }

    /// Stores the header of the page just started.
    fn store_header(
        &mut self,
        slot: usize,
        header_display: &[u8; HEADER_DISPLAY_BYTES],
        control: ControlBits,
        now: Instant,
    ) {
        let policy = self.config.parity_policy;
        if let Some(page) = self.store.page_mut(slot) {
            page.control = control;
            page.receptions = page.receptions.saturating_add(1);
            page.last_received = now;
            let failures = page.merge(0, HEADER_DISPLAY_COLUMN, header_display, policy);
            page.errors = page.errors.saturating_add(failures);
        }
    }

    fn end(&mut self, magazine_index: usize, now: Instant, events: &mut dyn FnMut(Event)) {
        let slot = self.receiving_slot.get_mut(magazine_index).and_then(Option::take);
        if let Some(slot) = slot {
            self.stats.pages_completed = self.stats.pages_completed.saturating_add(1);
            self.page_event(slot, PageEvent::ReceptionEnded, now, events);
        }
    }

    /// Runs one page lifecycle transition and performs its action.
    fn page_event(&mut self, slot: usize, event: PageEvent, now: Instant, events: &mut dyn FnMut(Event)) {
        let Some(page) = self.store.page_mut(slot) else { return };
        let key = page.key;
        let state = page.state;
        let (next, action) = page_transition(state, event);
        page.state = next;
        if self.config.trace && next != state {
            events(Event::Trace(TraceEntry {
                machine: Machine::Page(key.address.mpp()),
                time: now,
                from: page_state_name(state),
                event: page_event_name(event),
                action: page_action_name(action),
                to: page_state_name(next),
            }));
        }
        match action {
            PageAction::None => {}
            PageAction::Started | PageAction::MergeAndStart => events(Event::PageStarted(key)),
            PageAction::Erase => {
                page.erase();
                events(Event::PageErased(key));
                events(Event::PageStarted(key));
            }
            PageAction::Updated(row) => events(Event::PageUpdated { key, row }),
            PageAction::Completed => events(Event::PageCompleted(key)),
            PageAction::UpdateIndicator => events(Event::UpdateIndicator(key)),
            PageAction::Stale => events(Event::PageStale(key)),
            PageAction::Release => *page = StoredPage::EMPTY,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]
mod tests {
    extern crate std;
    use super::*;
    use crate::consts::{C4_ERASE, C11_SERIAL};
    use crate::ecc::with_odd_parity;
    use crate::packet::encode;
    use std::vec::Vec;

    fn mag(n: u8) -> Magazine {
        Magazine::new(n).unwrap()
    }

    fn text(s: &[u8]) -> [u8; 40] {
        let mut out = [with_odd_parity(b' '); 40];
        for (slot, &b) in out.iter_mut().zip(s) {
            *slot = with_odd_parity(b);
        }
        out
    }

    fn header(m: u8, page: u8, sub: u16, control: u16) -> Packet {
        let mut display = [with_odd_parity(b' '); 32];
        display[..4].copy_from_slice(&text(b"HDR!")[..4]);
        encode::header(mag(m), PageNumber(page), SubCode::new(sub), ControlBits(control), &display)
    }

    fn feed(decoder: &mut Decoder<'_>, packets: &[Packet], events: &mut Vec<Event>) {
        for (i, packet) in packets.iter().enumerate() {
            decoder.push(packet, &PacketMeta::default(), Instant(i as u64 * 1000), &mut |e| events.push(e));
        }
    }

    fn key(mpp: u16, sub: u16) -> PageKey {
        PageKey { address: PageAddress::from_mpp(mpp).unwrap(), subcode: SubCode::new(sub) }
    }

    #[test]
    fn parallel_page_is_stored_and_completed() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(
            &mut decoder,
            &[
                header(1, 0x00, 0, 0),
                encode::row(mag(1), 1, &text(b"HELLO")),
                encode::row(mag(2), 1, &text(b"OTHER")),
                header(1, 0x01, 0, 0),
            ],
            &mut events,
        );
        let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
        assert_eq!(&page.cells[1][..5], &text(b"HELLO")[..5]);
        assert_eq!(page.state, PageState::Complete);
        assert_eq!(&page.cells[0][8..12], &text(b"HDR!")[..4]);
        assert!(events.contains(&Event::PageCompleted(key(0x100, 0))));
        assert!(events.contains(&Event::Discarded(Discard::RowWithoutHeader)));
    }

    #[test]
    fn serial_mode_header_of_any_magazine_ends_page() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(
            &mut decoder,
            &[
                header(1, 0x00, 0, C11_SERIAL),
                encode::row(mag(1), 1, &text(b"ONE")),
                header(2, 0x00, 0, C11_SERIAL),
                encode::row(mag(1), 2, &text(b"LATE")),
            ],
            &mut events,
        );
        assert!(events.contains(&Event::PageCompleted(key(0x100, 0))));
        assert!(events.contains(&Event::Discarded(Discard::OtherMagazine)));
        let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
        assert!(!page.has_row(2));
    }

    #[test]
    fn erase_bit_clears_rows_and_parity_merge_keeps_good_bytes() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        let mut corrupted = text(b"GOOD");
        corrupted[0] ^= 0x01; // breaks parity of the first byte
        feed(
            &mut decoder,
            &[
                header(1, 0x00, 0, 0),
                encode::row(mag(1), 1, &text(b"GOOD")),
                encode::row(mag(1), 3, &text(b"KEEP")),
                header(1, 0x00, 0, 0),
                encode::row(mag(1), 1, &corrupted),
                header(1, 0x01, 0, 0),
            ],
            &mut events,
        );
        let address = PageAddress::from_mpp(0x100).unwrap();
        let page = decoder.store().get(address, SubCode::ANY).unwrap();
        assert_eq!(page.cells[1][0], with_odd_parity(b'G'), "E.8: good byte kept");
        assert!(page.has_row(3), "rows not received keep their content");
        feed(&mut decoder, &[header(1, 0x00, 0, C4_ERASE), header(1, 0x02, 0, 0)], &mut events);
        let page = decoder.store().get(address, SubCode::ANY).unwrap();
        assert!(!page.has_row(3) && !page.has_row(1), "C4 erased rows");
        assert!(events.contains(&Event::PageErased(key(0x100, 0))));
    }

    #[test]
    fn timeout_completes_and_hex_pages_are_not_stored() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(&mut decoder, &[header(3, 0x45, 0, 0), header(4, 0xA5, 0, 0)], &mut events);
        decoder.tick(Instant(1_000_000), &mut |e| events.push(e));
        assert!(events.contains(&Event::PageCompleted(key(0x345, 0))));
        assert!(!decoder.store().contains(PageAddress::from_mpp(0x4A5).unwrap()));
    }

    #[test]
    fn subpages_are_kept_separately_and_eviction_is_lru() {
        let mut slots = std::vec![StoredPage::EMPTY; 2];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(
            &mut decoder,
            &[header(1, 0x00, 1, 0), header(1, 0x00, 2, 0), header(1, 0x01, 0, 0), header(1, 0x02, 0, 0)],
            &mut events,
        );
        let address = PageAddress::from_mpp(0x100).unwrap();
        let mut subcodes = [SubCode::default(); 4];
        let count = decoder.store().subcodes(address, &mut subcodes);
        // 100/0001 was evicted first, then 100/0002; 101 and 102 remain.
        assert_eq!(count, 0);
        assert!(decoder.store().contains(PageAddress::from_mpp(0x101).unwrap()));
        assert!(decoder.store().contains(PageAddress::from_mpp(0x102).unwrap()));
    }

    #[test]
    fn subcode_listing_is_sorted() {
        let mut slots = std::vec![StoredPage::EMPTY; 8];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(
            &mut decoder,
            &[header(1, 0x00, 3, 0), header(1, 0x00, 1, 0), header(1, 0x00, 2, 0), header(1, 0x01, 0, 0)],
            &mut events,
        );
        let mut subcodes = [SubCode::default(); 8];
        let count = decoder.store().subcodes(PageAddress::from_mpp(0x100).unwrap(), &mut subcodes);
        assert_eq!(&subcodes[..count], &[SubCode::new(1), SubCode::new(2), SubCode::new(3)]);
    }
}
