//! Page acquisition and the page store (area ACQ, layer L2).
//!
//! [`Decoder`] takes packets one at a time, runs the magazine acquisition
//! machines and the page lifecycle machines of [`crate::fsm`], and keeps
//! every page in a [`PageStore`] whose memory the caller provides
//! (REQ-GEN-003). It raises [`Event`]s through a callback (REQ-ARCH-005).
//!
//! Enhancement pages (object pages, DRCS pages and the Magazine
//! Organization Table) are kept as received in an [`EnhancementStore`], the
//! part of the store with its own capacity in packets (decision E.56), and
//! the presentation layer reads them through the [`EnhancementLookup`].

use crate::config::{Config, ParityPolicy, PresentationLevel};
use crate::consts::{
    C4_ERASE, C8_UPDATE, C9_INTERRUPTED, CLOCK_COLUMN, COLUMNS, COMPOSITIONAL_LINKS, DATA_BYTES, DESIGNATION_CODES,
    DESIGNATION_LINKS_LEVEL_25, HEADER_DISPLAY_BYTES, HEADER_DISPLAY_COLUMN, LINK_NULL_PAGE, MAGAZINES, MOT_DRCS_LINKS,
    MOT_DRCS_LINKS_LEVEL_25, MOT_DRCS_LINKS_LEVEL_35, MOT_OBJECT_LINKS_LEVEL_25, MOT_OBJECT_LINKS_LEVEL_35,
    MOT_OBJECT_LINKS_PER_PACKET, PAGE_CODING_PARITY, PAGE_FUNCTION_BASIC, PAGE_MOT, SERVICE_FORMAT_2_BIT, SPACE,
    STORED_ROWS, TRIPLETS_PER_PACKET,
};
use crate::ecc::has_odd_parity;
use crate::fsm::{
    MagazineAction, MagazineEvent, MagazineState, PageAction, PageEvent, PageKey, PageState, magazine_action_name,
    magazine_event_name, magazine_state_name, magazine_transition, page_action_name, page_event_name, page_state_name,
    page_transition,
};
use crate::packet::{
    CompositionalLink, Decoded, DrcsColourData, Header, Links, MagazinePacket, Packet, PacketError, PacketMeta,
    PageEnhancement, PageFormat, PagePacket, ServicePacket, Triplet, decode, mot_drcs_link, mot_object_link,
};
use crate::trace::{Machine, TraceEntry};

mod enhancement;

use crate::types::{ControlBits, FixedVec, Instant, Magazine, PageAddress, PageNumber, PresentationFields, SubCode};
use enhancement::Stored;
pub use enhancement::{
    EnhancementKey, EnhancementLookup, EnhancementPacket, EnhancementPage, EnhancementStore, EnhancementSubPage,
    enhancement_index, row_index,
};

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
    /// Packets X/27/4 and X/27/5, indexed by designation code minus 4,
    /// with their decoded links (REQ-PKT-011).
    pub compositional_links: [Option<[Option<CompositionalLink>; COMPOSITIONAL_LINKS]>; 2],
    /// Packet X/28/0, read as format 1 (decision E.44).
    pub page_data: Option<PageEnhancement>,
    /// Packet X/28/1: the DRCS colour lookup tables (REQ-PKT-013).
    pub drcs_colours: Option<DrcsColourData>,
    /// Packet X/28/4: the Level 3.5 page data (REQ-PKT-015).
    pub page_data_level35: Option<PageEnhancement>,
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
        compositional_links: [None; 2],
        page_data: None,
        drcs_colours: None,
        page_data_level35: None,
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
        self.page_data
            .and_then(|data| data.format)
            .is_none_or(|format| format.function == PAGE_FUNCTION_BASIC && format.coding == PAGE_CODING_PARITY)
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
        self.compositional_links = [None; 2];
        self.page_data = None;
        self.drcs_colours = None;
        self.page_data_level35 = None;
    }

    /// Merges received bytes into `row` starting at `column` (decision E.8,
    /// REQ-ACQ-008): rows not received keep their stored content.
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
                // A rotation of an empty slice panics (REQ-GEN-006).
                if let Some(tail) = out.get_mut(position..count).filter(|tail| !tail.is_empty()) {
                    tail.rotate_right(1);
                    if let Some(first) = tail.first_mut() {
                        *first = page.key.subcode;
                    }
                }
            } else if let Some(tail) = out.get_mut(position..).filter(|tail| !tail.is_empty()) {
                // Full: keep the lowest sub-codes. A sub-code above every
                // listed one leaves an empty tail and is not listed.
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

/// The magazine data of one magazine (REQ-ACQ-009, REQ-ACQ-018): the most
/// recent packet of each designation code, each replacing the last.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct MagazineData {
    /// The latest M/29/0 with packet function 0 (REQ-PKT-016).
    pub data: Option<PresentationFields>,
    /// The latest M/29/1: the DRCS colour lookup tables.
    pub drcs_colours: Option<DrcsColourData>,
    /// The latest M/29/4 with packet function 0: the Level 3.5 data.
    pub level35: Option<PresentationFields>,
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
    /// M/29 was received for a magazine, or the magazine's M/29 packets
    /// were discarded because the service changed (REQ-ACQ-018).
    MagazineData(Magazine),
    /// An enhancement sub-page was stored or changed during the reception
    /// that just ended, so pages that use it are to be built again
    /// (REQ-ACQ-022).
    EnhancementPageChanged(EnhancementKey),
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
    /// Packets of enhancement pages dropped because every slot of the
    /// enhancement store belonged to a sub-page being received.
    pub enhancement_dropped: u32,
    /// Faults met while building pages at Levels 2.5 and 3.5 (REQ-L25-051),
    /// added by [`Decoder::record_enhancement`].
    pub enhancement: EnhancementFaults,
}

/// The faults one build of a page met at Levels 2.5 and 3.5, one count for
/// each kind (REQ-L25-051). The enhancement processor fills it; the decoder
/// keeps the sum in its [`Statistics`]. It is defined here, with the
/// statistics, because acquisition may not use the presentation layer
/// (ARCHITECTURE.md section 9).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EnhancementFaults {
    /// Triplets that failed their Hamming 24/18 check and were skipped.
    pub damaged_triplets: u32,
    /// Object invocations skipped by the nesting rules of REQ-OBJ-008 or
    /// with an illegal source.
    pub forbidden_invocations: u32,
    /// Invocations whose object was absent: no link, no page, no pointer,
    /// no matching definition, or a missing packet (REQ-OBJ-003 to
    /// REQ-OBJ-006).
    pub absent_objects: u32,
    /// Invocations ignored because a limit of Annex A.8 was reached.
    pub limited_invocations: u32,
    /// Characters and attributes discarded by REQ-OBJ-014 and REQ-L25-050.
    pub discarded_writes: u32,
    /// Modified designations ignored: reserved values, or a third pair at
    /// Level 2.5 (REQ-L25-012).
    pub ignored_designations: u32,
    /// Cells showing a space for an undefined G2 or G3 position (decision
    /// E.50).
    pub undefined_characters: u32,
    /// DRCS cells whose data is missing or not yet usable, including those
    /// beyond the grid's pattern capacity (REQ-DRC-007, REQ-DRC-009).
    pub drcs_missing: u32,
    /// DRCS cells whose data is invalid (REQ-DRC-009).
    pub drcs_invalid: u32,
}

impl EnhancementFaults {
    /// No faults.
    pub const NONE: Self = Self {
        damaged_triplets: 0,
        forbidden_invocations: 0,
        absent_objects: 0,
        limited_invocations: 0,
        discarded_writes: 0,
        ignored_designations: 0,
        undefined_characters: 0,
        drcs_missing: 0,
        drcs_invalid: 0,
    };

    /// Adds `other`'s counts to these, saturating.
    pub const fn add(&mut self, other: &Self) {
        self.damaged_triplets = self.damaged_triplets.saturating_add(other.damaged_triplets);
        self.forbidden_invocations = self.forbidden_invocations.saturating_add(other.forbidden_invocations);
        self.absent_objects = self.absent_objects.saturating_add(other.absent_objects);
        self.limited_invocations = self.limited_invocations.saturating_add(other.limited_invocations);
        self.discarded_writes = self.discarded_writes.saturating_add(other.discarded_writes);
        self.ignored_designations = self.ignored_designations.saturating_add(other.ignored_designations);
        self.undefined_characters = self.undefined_characters.saturating_add(other.undefined_characters);
        self.drcs_missing = self.drcs_missing.saturating_add(other.drcs_missing);
        self.drcs_invalid = self.drcs_invalid.saturating_add(other.drcs_invalid);
    }
}

/// What one build of a grid found, for the decoder's statistics and the
/// trace (REQ-L25-051, REQ-ARCH-007, REQ-TST-019). Fault counts are of the
/// page-level run of the walk and of the row resolution; `steps` counts
/// every triplet read in every run. Pass `build` and `faults` to
/// `Decoder::record_enhancement` once the grid is built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnhancementReport {
    /// The page, reception and level the grid was built from.
    pub build: PageBuild,
    /// The level the page was processed at: Level 2.5 when a Level 3.5
    /// decoder falls back to the table's Level 2.5 links (clause 10.6.1).
    pub level: PresentationLevel,
    /// The faults met, one count for each kind.
    pub faults: EnhancementFaults,
    /// Objects run.
    pub objects: u32,
    /// Triplets read, over every run of the walk.
    pub steps: u32,
    /// Modified G0 and G2 Character Set Designation triplets applied
    /// (Table 34, REQ-L25-012, REQ-BRW-011).
    pub designations: u32,
}

#[cfg(feature = "level25")]
impl EnhancementReport {
    pub(crate) const fn new(build: PageBuild, level: PresentationLevel) -> Self {
        Self { build, level, faults: EnhancementFaults::NONE, objects: 0, steps: 0, designations: 0 }
    }
}

/// Which build of a page an enhancement report belongs to: the page, the
/// reception it was built from, and the presentation level asked for. The
/// decoder uses it so that redrawing a page does not count its faults
/// again ([`Decoder::record_enhancement`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PageBuild {
    /// The page and sub-page.
    pub key: PageKey,
    /// The page's reception count when it was built ([`StoredPage::receptions`]).
    pub receptions: u32,
    /// The presentation level of the configuration it was built with.
    pub level: PresentationLevel,
}

impl PageBuild {
    /// The build of `page` at `level`.
    #[must_use]
    pub const fn of(page: &StoredPage, level: PresentationLevel) -> Self {
        Self { key: page.key, receptions: page.receptions, level }
    }
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
    // Pages that stored links name as object or DRCS pages (decision E.56):
    // a bounded hint list, refilled as links arrive.
    named: FixedVec<PageAddress, NAMED_ENHANCEMENT_PAGES>,
    // Where the page of the header being processed is stored.
    pending: Route,
    // The build whose enhancement faults were recorded last.
    last_recorded: Option<PageBuild>,
}

/// Bytes of header display data before the clock: columns 8 to 31.
const ROLLING_BYTES: usize = CLOCK_COLUMN - HEADER_DISPLAY_COLUMN;

/// Addresses kept in the list of pages named by links: every object and
/// DRCS link of both levels of the eight Magazine Organization Tables
/// (Tables 20 and 22: 8 object and 8 DRCS links each), so a complete set of
/// tables always fits. Links of X/27/4 and X/27/5 share the list; when it
/// is full the oldest address is dropped, and it returns with the next
/// table or link packet that names it.
const NAMED_ENHANCEMENT_PAGES: usize = 2 * MAGAZINES * (MOT_OBJECT_LINKS_PER_PACKET * 2 + MOT_DRCS_LINKS);

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
    /// enhancement pages: object pages, DRCS pages and the Magazine
    /// Organization Table are not kept.
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
    /// stored object pages, DRCS pages, Magazine Organization Tables and
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
                let as_received = row_index(row.row).map(|index| (index, &row.bytes));
                self.row_like(row.magazine, now, events, as_received, |page, policy| {
                    let failures = page.merge(usize::from(row.row), 0, &row.bytes, policy);
                    (row.row, failures)
                });
            }
            Decoded::Enhancement(enhancement) => {
                self.count(enhancement.magazine, crate::consts::PACKET_ENHANCEMENT);
                let failures = enhancement.triplets.iter().filter(|t| t.is_none()).count() as u32;
                self.stats.triplet_failures = self.stats.triplet_failures.saturating_add(failures);
                let data = packet.data();
                let as_received = enhancement_index(enhancement.designation).map(|index| (index, &data));
                self.row_like(enhancement.magazine, now, events, as_received, |page, _| {
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
                self.row_like(links.magazine, now, events, None, |page, _| {
                    page.links = Some(links);
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::LinksRaw { magazine, designation, bytes } => {
                self.count(magazine, crate::consts::PACKET_LINKS);
                self.row_like(magazine, now, events, None, |page, _| {
                    let index = usize::from(designation.saturating_sub(1));
                    if let Some(slot) = page.links_raw.get_mut(index) {
                        *slot = bytes;
                        page.links_raw_present |= 1u8.checked_shl(index as u32).unwrap_or(0);
                    }
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::CompositionalLinks { .. } | Decoded::PageData { .. } | Decoded::Magazine { .. } => {
                self.enhancement_data(decoded, now, events);
            }
            Decoded::Service(service) => {
                self.count(Magazine::from_bits(0), crate::consts::PACKET_SERVICE);
                // REQ-ACQ-018: a new network identification in format 1
                // means a new service, whose magazine data differs.
                let format_1 = |packet: &ServicePacket| packet.designation & SERVICE_FORMAT_2_BIT == 0;
                if format_1(&service)
                    && self.service.is_some_and(|old| format_1(&old) && old.network_id != service.network_id)
                {
                    self.discard_magazine_data(events);
                }
                self.service = Some(service);
                events(Event::ServiceData);
            }
            Decoded::Ignored(address) => {
                self.count(address.magazine, address.number);
                self.stats.ignored = self.stats.ignored.saturating_add(1);
            }
        }
    }

    /// Stores the page-related packets X/27/4, X/27/5 and X/28, and the
    /// magazine-related packets M/29 (REQ-ACQ-007, REQ-ACQ-009).
    fn enhancement_data(&mut self, decoded: Decoded, now: Instant, events: &mut dyn FnMut(Event)) {
        match decoded {
            Decoded::CompositionalLinks { magazine, designation, links } => {
                self.count(magazine, crate::consts::PACKET_LINKS);
                for link in links.iter().flatten().filter(|link| link.is_format_1() && !link.is_null()) {
                    self.name(PageAddress { magazine: link.magazine, page: link.page });
                }
                self.row_like(magazine, now, events, None, |page, _| {
                    let index = usize::from(designation.saturating_sub(DESIGNATION_LINKS_LEVEL_25));
                    if let Some(slot) = page.compositional_links.get_mut(index) {
                        *slot = Some(links);
                    }
                    (crate::consts::PACKET_LINKS, 0)
                });
            }
            Decoded::PageData { magazine, packet } => {
                self.count(magazine, crate::consts::PACKET_PAGE_ENHANCEMENT);
                let stored = self.row_like(magazine, now, events, None, |page, _| {
                    match packet {
                        PagePacket::Format1(data) => page.page_data = Some(data),
                        PagePacket::DrcsColours(data) => page.drcs_colours = Some(data),
                        PagePacket::Level35(data) => page.page_data_level35 = Some(data),
                        // X/28/3 belongs to DRCS pages, which are not text
                        // pages (REQ-OBJ-041).
                        PagePacket::DrcsModes(_) => {}
                    }
                    (crate::consts::PACKET_PAGE_ENHANCEMENT, 0)
                });
                let format = match packet {
                    PagePacket::Format1(data) => data.format,
                    PagePacket::DrcsModes(modes) => modes.format,
                    PagePacket::DrcsColours(_) | PagePacket::Level35(_) => None,
                };
                if let (Some(index), Some(format)) = (stored, format) {
                    self.page_format(index, format, packet, now, events);
                }
            }
            Decoded::Magazine { magazine, packet } => {
                self.count(magazine, crate::consts::PACKET_MAGAZINE_ENHANCEMENT);
                if let Some(slot) = self.magazine_data.get_mut(magazine.index()) {
                    match packet {
                        MagazinePacket::Data(fields) => slot.data = Some(fields),
                        MagazinePacket::DrcsColours(data) => slot.drcs_colours = Some(data),
                        MagazinePacket::Level35(fields) => slot.level35 = Some(fields),
                    }
                }
                events(Event::MagazineData(magazine));
            }
            _ => {}
        }
    }

    /// Acts on the page function of a page being received: a page whose
    /// X/28/0 or X/28/3 function is not "basic Level 1 page" is an
    /// enhancement page, stored as received from this packet on
    /// (REQ-ACQ-016, decision E.56).
    fn page_format(
        &mut self,
        index: usize,
        format: PageFormat,
        packet: PagePacket,
        now: Instant,
        events: &mut dyn FnMut(Event),
    ) {
        if format.function == PAGE_FUNCTION_BASIC {
            return;
        }
        let receiving = self.receiving_enhancement.get(index).copied().flatten();
        let entry = match receiving {
            Some(entry) => Some(entry),
            None => self.switch_to_enhancement(index, now, events),
        };
        if let Some(entry) = entry {
            self.enhancement.set_format(entry, format);
            if let PagePacket::DrcsModes(modes) = packet {
                self.enhancement.set_drcs_modes(entry, modes);
            }
        }
    }

    /// Moves the reception in magazine `index` to the enhancement store. A
    /// copy in the page store holding only this reception is released; an
    /// older one keeps its content and ends its reception here.
    fn switch_to_enhancement(&mut self, index: usize, now: Instant, events: &mut dyn FnMut(Event)) -> Option<usize> {
        if !self.enhancement.is_enabled() {
            return None;
        }
        let Some(MagazineState::Receiving(key)) = self.magazines.get(index).copied() else { return None };
        let control = self.receiving_control.get(index).copied().unwrap_or_default();
        let entry = self.start_enhancement(index, key, control, now)?;
        if let Some(slot) = self.receiving_slot.get_mut(index).and_then(Option::take) {
            let fresh = self.store.slot(slot).is_some_and(|page| page.receptions <= 1);
            self.page_event(slot, if fresh { PageEvent::Evicted } else { PageEvent::ReceptionEnded }, now, events);
        }
        Some(entry)
    }

    /// Starts the reception of an enhancement sub-page in magazine `index`.
    /// The generation before the reception is kept, so that a change of
    /// S2 or C4 at its start is reported too; a new sub-page has none.
    fn start_enhancement(&mut self, index: usize, key: PageKey, control: ControlBits, now: Instant) -> Option<usize> {
        let before = self.enhancement.get(EnhancementKey::new(key.address, key.subcode)).map(|page| page.generation());
        let busy = self.receiving_enhancement;
        let entry = self.enhancement.start(key.address, key.subcode, control, now, &busy)?;
        if let Some(slot) = self.receiving_enhancement.get_mut(index) {
            *slot = Some(entry);
        }
        if let Some(generation) = self.start_generation.get_mut(index) {
            *generation = before;
        }
        Some(entry)
    }

    /// Adds an address to the list of pages named by links.
    fn name(&mut self, address: PageAddress) {
        if address.page.0 == LINK_NULL_PAGE || self.named.as_slice().contains(&address) {
            return;
        }
        if self.named.is_full() {
            self.named.remove(0);
        }
        self.named.push(address);
    }

    /// Adds the pages that a stored packet X/19 to X/24 of a Magazine
    /// Organization Table links to (Tables 20 and 22; decision E.52: the
    /// magazine of a table link is absolute).
    fn name_from_table(&mut self, packet: u8, bytes: &[u8; DATA_BYTES]) {
        let mut found = [None; MOT_DRCS_LINKS];
        if MOT_OBJECT_LINKS_LEVEL_25.contains(&packet) || MOT_OBJECT_LINKS_LEVEL_35.contains(&packet) {
            for (link, slot) in found.iter_mut().enumerate().take(MOT_OBJECT_LINKS_PER_PACKET) {
                *slot =
                    mot_object_link(bytes, link).map(|link| PageAddress { magazine: link.magazine, page: link.page });
            }
        } else if packet == MOT_DRCS_LINKS_LEVEL_25 || packet == MOT_DRCS_LINKS_LEVEL_35 {
            for (link, slot) in found.iter_mut().enumerate() {
                *slot = mot_drcs_link(bytes, link).map(|link| PageAddress { magazine: link.magazine, page: link.page });
            }
        }
        for address in found.into_iter().flatten() {
            self.name(address);
        }
    }

    /// Where the page of a header is stored (REQ-ACQ-013, REQ-ACQ-016,
    /// REQ-ACQ-017). Decimal pages always go to the page store, and also to
    /// the enhancement store when they are enhancement pages, so that a
    /// wrong link never hides a display page. Pages with hexadecimal numbers
    /// go to the enhancement store when they are enhancement pages, and
    /// otherwise to the page store at Levels 2.5 and 3.5 or under the
    /// diagnostic setting.
    fn route(&self, address: PageAddress) -> Route {
        let enhancement = self.enhancement.is_enabled()
            && (address.page.0 == PAGE_MOT
                || self.enhancement.contains_address(address)
                || self.named.as_slice().contains(&address));
        let hexadecimal = self.config.store_hexadecimal_pages || (self.config.level.is_enhanced() && !enhancement);
        Route { main: address.page.is_decimal() || hexadecimal, enhancement }
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

    /// Handles a header for all eight magazine machines: it starts its own
    /// page (REQ-ACQ-002) and ends pages as REQ-ACQ-003 lists.
    fn header(&mut self, header: &Header, now: Instant, events: &mut dyn FnMut(Event)) {
        if let Some(serial) = header.serial() {
            self.serial = serial;
        }
        let own = header.magazine;
        let time_filling = header.page.is_some_and(PageNumber::is_time_filling);
        self.pending = header.page.map_or(Route::default(), |page| self.route(PageAddress { magazine: own, page }));
        if let (Some(control), Some(slot)) = (header.control, self.receiving_control.get_mut(own.index())) {
            *slot = control;
        }
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
        if !self.pending.main && !self.pending.enhancement {
            return MagazineEvent::NonStorableHeader;
        }
        let key = PageKey { address: PageAddress { magazine: header.magazine, page }, subcode };
        MagazineEvent::StorableHeader { key, erase: control.has(C4_ERASE) }
    }

    /// Updates the rolling header and the clock (decision E.11,
    /// REQ-ACQ-011).
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

    /// Handles a row-like packet (rows 1 to 28) of a magazine. In serial
    /// mode a packet of another magazine is discarded and counted
    /// (REQ-ACQ-002, Annex B.3). `as_received` is the packet's index and
    /// data bytes for the enhancement store, where it has a place there.
    /// Returns the index of the magazine whose page took the packet.
    fn row_like(
        &mut self,
        magazine: Magazine,
        now: Instant,
        events: &mut dyn FnMut(Event),
        as_received: Option<(usize, &[u8; DATA_BYTES])>,
        store: impl FnOnce(&mut StoredPage, ParityPolicy) -> (u8, u32),
    ) -> Option<usize> {
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
            if let (Some(entry), Some((packet, bytes))) =
                (self.receiving_enhancement.get(index).copied().flatten(), as_received)
            {
                self.store_enhancement(entry, packet, bytes, now);
            }
            if let Some(last) = self.last_packet.get_mut(index) {
                *last = now;
            }
            return Some(index);
        }
        None
    }

    /// Stores a packet of an enhancement sub-page as received (decision
    /// E.56), and learns the links of a Magazine Organization Table.
    fn store_enhancement(&mut self, entry: usize, packet: usize, bytes: &[u8; DATA_BYTES], now: Instant) {
        let busy = self.receiving_enhancement;
        match self.enhancement.store(entry, packet, bytes, now, &busy) {
            Stored::Yes { .. } => {
                let table = self.enhancement.key(entry).filter(|key| key.address.page.0 == PAGE_MOT && key.s1 == 0);
                if table.is_some() {
                    // Packet index i is packet X/(i + 1).
                    self.name_from_table(packet.saturating_add(1) as u8, bytes);
                }
            }
            Stored::Dropped => self.stats.enhancement_dropped = self.stats.enhancement_dropped.saturating_add(1),
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
        let route = self.pending;
        if route.enhancement {
            let control = self.receiving_control.get(magazine_index).copied().unwrap_or_default();
            self.start_enhancement(magazine_index, key, control, now);
        }
        if !route.main {
            return;
        }
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
        // REQ-ACQ-022: report an enhancement sub-page whose content or update
        // indicator changed during the reception.
        let entry = self.receiving_enhancement.get_mut(magazine_index).and_then(Option::take);
        if let Some(entry) = entry {
            let started = self.start_generation.get(magazine_index).copied().flatten();
            if let (Some(key), Some(generation)) = (self.enhancement.key(entry), self.enhancement.generation(entry))
                && started != Some(generation)
            {
                events(Event::EnhancementPageChanged(key));
            }
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
    use crate::consts::{C4_ERASE, C11_SERIAL, CODE_MASK_7BIT};
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

    /// REQ-ACQ-002, REQ-ACQ-003: parallel mode.
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

    /// REQ-ACQ-002, REQ-ACQ-003: serial mode.
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

    /// REQ-ACQ-008: a corrupted retransmission replaces the row byte by byte,
    /// keeping the good stored byte where the new one failed parity.
    #[test]
    fn erase_bit_clears_rows_and_parity_merge_keeps_good_bytes() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        let mut corrupted = text(b"BNEW");
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
        assert_eq!(&page.cells[1][1..4], &text(b"NEW")[..3], "good new bytes replace the old");
        assert!(page.has_row(3), "rows not received keep their content");
        feed(&mut decoder, &[header(1, 0x00, 0, C4_ERASE), header(1, 0x02, 0, 0)], &mut events);
        let page = decoder.store().get(address, SubCode::ANY).unwrap();
        assert!(!page.has_row(3) && !page.has_row(1), "C4 erased rows");
        assert!(events.contains(&Event::PageErased(key(0x100, 0))));
    }

    /// REQ-ACQ-003: the decision E.19 timeout. REQ-ACQ-013: at Level 1.5
    /// pages with hexadecimal numbers are not stored.
    #[test]
    fn timeout_completes_and_hex_pages_are_not_stored() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let level_15 = Config { level: PresentationLevel::Level15, ..Config::default() };
        let mut decoder = Decoder::new(&mut slots, level_15);
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

    /// REQ-GEN-006: a listing buffer smaller than the stored sub-pages keeps
    /// the lowest sub-codes, without panicking when a later sub-code is
    /// higher than every one already listed.
    #[test]
    fn subcode_listing_into_a_full_buffer_keeps_the_lowest() {
        let mut slots = std::vec![StoredPage::EMPTY; 8];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(
            &mut decoder,
            &[header(1, 0x00, 1, 0), header(1, 0x00, 2, 0), header(1, 0x00, 3, 0), header(1, 0x01, 0, 0)],
            &mut events,
        );
        let mut subcodes = [SubCode::default(); 2];
        let count = decoder.store().subcodes(PageAddress::from_mpp(0x100).unwrap(), &mut subcodes);
        assert_eq!(&subcodes[..count], &[SubCode::new(1), SubCode::new(2)]);
    }

    /// REQ-ACQ-001, REQ-ACQ-002: parallel magazines interleave, the mode
    /// follows C11 of the latest header, and in serial mode rows of another
    /// magazine are discarded and counted.
    #[test]
    fn interleaved_magazines_and_serial_mode_follow_c11() {
        let mut slots = std::vec![StoredPage::EMPTY; 8];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        feed(
            &mut decoder,
            &[
                header(1, 0x00, 0, 0),
                header(2, 0x00, 0, 0),
                encode::row(mag(1), 1, &text(b"ONE")),
                encode::row(mag(2), 1, &text(b"TWO")),
                encode::row(mag(1), 2, &text(b"ONE2")),
            ],
            &mut events,
        );
        assert!(!decoder.serial);
        let receiving = |decoder: &Decoder<'_>, magazine: u8| decoder.magazines[mag(magazine).index()];
        assert_eq!(receiving(&decoder, 1), MagazineState::Receiving(key(0x100, 0)));
        assert_eq!(receiving(&decoder, 2), MagazineState::Receiving(key(0x200, 0)));
        let one = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
        assert_eq!((&one.cells[1][..3], &one.cells[2][..4]), (&text(b"ONE")[..3], &text(b"ONE2")[..4]));
        let two = decoder.store().get(PageAddress::from_mpp(0x200).unwrap(), SubCode::ANY).unwrap();
        assert_eq!(&two.cells[1][..3], &text(b"TWO")[..3]);
        assert!(!two.has_row(2));
        let discarded = decoder.statistics().discarded;
        events.clear();
        feed(
            &mut decoder,
            &[header(3, 0x00, 0, C11_SERIAL), encode::row(mag(3), 1, &text(b"S")), encode::row(mag(4), 1, &text(b"X"))],
            &mut events,
        );
        assert!(decoder.serial);
        assert_eq!(decoder.statistics().discarded, discarded + 1, "the magazine 4 row");
        let reasons: Vec<&Event> = events.iter().filter(|event| matches!(event, Event::Discarded(_))).collect();
        assert_eq!(reasons, [&Event::Discarded(Discard::OtherMagazine)], "Annex B.3");
        let three = decoder.store().get(PageAddress::from_mpp(0x300).unwrap(), SubCode::ANY).unwrap();
        assert_eq!(three.cells[1][0], with_odd_parity(b'S'));
        feed(&mut decoder, &[header(3, 0x01, 0, 0)], &mut events);
        assert!(!decoder.serial, "C11 clear: parallel again");
    }

    /// REQ-ACQ-003: time-filling and uncorrectable headers end the page in
    /// progress and start nothing.
    #[test]
    fn time_filling_and_bad_headers_end_a_page_without_starting_one() {
        let mut bad = header(1, 0x01, 0, 0);
        // Two bit errors in the page units byte are uncorrectable.
        bad.bytes[2] ^= 0b11;
        let Ok(Decoded::Header(bad_header)) = decode(&bad) else { panic!("not a header") };
        assert_eq!(bad_header.page, None);
        for ender in [header(1, 0xFF, 0, 0), bad] {
            let mut slots = std::vec![StoredPage::EMPTY; 4];
            let mut decoder = Decoder::new(&mut slots, Config::default());
            let mut events = Vec::new();
            feed(
                &mut decoder,
                &[
                    header(1, 0x00, 0, 0),
                    encode::row(mag(1), 1, &text(b"IN")),
                    ender,
                    encode::row(mag(1), 2, &text(b"OUT")),
                ],
                &mut events,
            );
            assert!(events.contains(&Event::PageCompleted(key(0x100, 0))));
            assert_eq!(events.iter().filter(|event| matches!(event, Event::PageStarted(_))).count(), 1);
            let page = decoder.store().get(PageAddress::from_mpp(0x100).unwrap(), SubCode::ANY).unwrap();
            assert!(page.has_row(1) && !page.has_row(2));
            assert_eq!(decoder.store().pages().iter().filter(|page| page.state != PageState::Empty).count(), 1);
        }
    }

    /// Storage for the enhancement store in tests.
    struct EnhancementMemory {
        sub_pages: Vec<EnhancementSubPage>,
        packets: Vec<EnhancementPacket>,
    }

    impl EnhancementMemory {
        fn new(sub_pages: usize, packets: usize) -> Self {
            Self {
                sub_pages: std::vec![EnhancementSubPage::EMPTY; sub_pages],
                packets: std::vec![EnhancementPacket::EMPTY; packets],
            }
        }

        fn store(&mut self) -> EnhancementStore<'_> {
            EnhancementStore::new(&mut self.sub_pages, &mut self.packets)
        }
    }

    /// Forty data bytes of Hamming 24/18 triplets with an even-parity
    /// first byte, which the parity merge of decision E.8 would refuse.
    fn object_bytes(seed: u8) -> [u8; DATA_BYTES] {
        let triplets = [Triplet { address: seed % 40, mode: 0x10, data: 0x41 }; TRIPLETS_PER_PACKET];
        let data = encode::triplet_packet(mag(1), 1, 1, &triplets).data();
        assert!(!data[1..].iter().all(|&byte| has_odd_parity(byte)));
        data
    }

    fn object_row(m: u8, row: u8, seed: u8) -> Packet {
        let mut packet = encode::address(mag(m), row);
        packet.bytes[2..].copy_from_slice(&object_bytes(seed));
        packet
    }

    fn table_with_pop_link(m: u8, target: u8) -> [u8; DATA_BYTES] {
        let mut nibbles = [0u8; DATA_BYTES];
        // POP link 1 at packet bytes 16 to 25 (Table 20).
        let link = crate::packet::MotObjectLink {
            magazine: mag(m),
            x27_flag: false,
            page: PageNumber(target),
            subpage_count: 0,
            fallback: crate::types::FallbackFields::default(),
            default_objects: [None; 2],
        };
        nibbles[10..20].copy_from_slice(&encode::mot_object_link(&link));
        nibbles
    }

    /// REQ-ACQ-016, REQ-ACQ-020, REQ-ACQ-021, REQ-ACQ-022: the Magazine
    /// Organization Table and a page it names are stored as received, read
    /// back through the lookup, replaced by new receptions, erased by C4,
    /// and reported when they change.
    #[test]
    fn enhancement_pages_are_stored_as_received() {
        let mut slots = std::vec![StoredPage::EMPTY; 8];
        let mut memory = EnhancementMemory::new(8, 100);
        let mut decoder = Decoder::with_enhancement_store(&mut slots, memory.store(), Config::default());
        let mut events = Vec::new();
        let table = table_with_pop_link(1, 0xA0);
        // A sub-code of S4 S3 = 25 (last packet), S2 = 0, S1 = 0.
        let table_subcode = 25 << 8;
        let table_packet = encode::nibble_packet(mag(1), 19, &table);
        feed(&mut decoder, &[header(1, 0xFE, table_subcode, 0), table_packet, header(1, 0x00, 0, 0)], &mut events);
        let lookup = decoder.enhancement_lookup();
        let mot = lookup.magazine_organization_table(mag(1)).unwrap();
        assert_eq!((mot.row(19), mot.last_packet()), (Some(&table_packet.data()), 25));
        assert!(!decoder.store().contains(PageAddress::from_mpp(0x1FE).unwrap()), "kept only as an enhancement page");
        assert!(events.contains(&Event::EnhancementPageChanged(EnhancementKey {
            address: PageAddress::from_mpp(0x1FE).unwrap(),
            s1: 0
        })));
        // Page 1A0, named by the table, sub-page S1 = 2: stored as received,
        // then replaced byte for byte by a reception whose bytes differ.
        let object_key = EnhancementKey { address: PageAddress::from_mpp(0x1A0).unwrap(), s1: 2 };
        for seed in [3, 7] {
            events.clear();
            feed(
                &mut decoder,
                &[header(1, 0xA0, 0x0002, 0), object_row(1, 1, seed), header(1, 0x00, 0, 0)],
                &mut events,
            );
            let page = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap();
            assert_eq!(page.row(1), Some(&object_bytes(seed)), "seed {seed}: no parity merge");
            assert!(events.contains(&Event::EnhancementPageChanged(object_key)));
        }
        let generation = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap().generation();
        events.clear();
        feed(&mut decoder, &[header(1, 0xA0, 0x0002, 0), object_row(1, 1, 7), header(1, 0x00, 0, 0)], &mut events);
        assert!(!events.iter().any(|event| matches!(event, Event::EnhancementPageChanged(_))), "nothing changed");
        // A new update indicator S2 is reported even with the same bytes.
        feed(&mut decoder, &[header(1, 0xA0, 0x0012, 0), object_row(1, 1, 7), header(1, 0x00, 0, 0)], &mut events);
        let page = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap();
        assert_ne!(page.generation(), generation);
        assert!(events.contains(&Event::EnhancementPageChanged(object_key)));
        // C4 erases the stored packets before the new ones.
        feed(
            &mut decoder,
            &[header(1, 0xA0, 0x0002, C4_ERASE), object_row(1, 2, 1), header(1, 0x00, 0, 0)],
            &mut events,
        );
        let page = decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 2).unwrap();
        assert_eq!((page.row(1), page.row(2)), (None, Some(&object_bytes(1))));
        assert_eq!(page.received(), 1 << row_index(2).unwrap());
        assert!(decoder.enhancement_lookup().sub_page(mag(1), PageNumber(0xA0), 0).is_none());
    }

    /// REQ-ACQ-016: a page that becomes an enhancement page after it was
    /// first stored is stored as received from its next reception on, both
    /// when an X/27/4 link names it and when its X/28/0 says so.
    #[test]
    fn pages_become_enhancement_pages_by_link_or_page_function() {
        let mut slots = std::vec![StoredPage::EMPTY; 8];
        let mut memory = EnhancementMemory::new(8, 100);
        let mut decoder = Decoder::with_enhancement_store(&mut slots, memory.store(), Config::default());
        let mut events = Vec::new();
        let object = [header(2, 0xB0, 0, 0), object_row(2, 3, 5), header(2, 0x00, 0, 0)];
        feed(&mut decoder, &object, &mut events);
        let address = PageAddress::from_mpp(0x2B0).unwrap();
        assert!(decoder.store().contains(address), "REQ-ACQ-017: stored at Level 3.5");
        assert!(decoder.enhancement_lookup().sub_page(mag(2), PageNumber(0xB0), 0).is_none());
        let link = CompositionalLink {
            function: 0b01,
            validity: 0b11,
            magazine: mag(2),
            page: PageNumber(0xB0),
            subpages: 1,
            marker_first: true,
            marker_second: false,
        };
        let links = encode::compositional_links(mag(2), 4, &[link; 6]);
        feed(&mut decoder, &[header(2, 0x00, 0, 0), links, header(2, 0x01, 0, 0)], &mut events);
        assert_eq!(
            decoder.store().get(PageAddress::from_mpp(0x200).unwrap(), SubCode::ANY).unwrap().compositional_links[0],
            Some([Some(link); 6])
        );
        feed(&mut decoder, &object, &mut events);
        let page = decoder.enhancement_lookup().sub_page(mag(2), PageNumber(0xB0), 0).unwrap();
        assert_eq!(page.row(3), Some(&object_bytes(5)));
        // X/28/0 with page function 0011 (POP), before the rows.
        let format = PageFormat { function: 0b0011, coding: 0b010 };
        let page_data = encode::page_enhancement(mag(3), 0, format, &PresentationFields::default());
        feed(
            &mut decoder,
            &[header(3, 0xC0, 0, 0), page_data, object_row(3, 1, 9), header(3, 0x00, 0, 0)],
            &mut events,
        );
        let page = decoder.enhancement_lookup().sub_page(mag(3), PageNumber(0xC0), 0).unwrap();
        assert_eq!((page.format(), page.row(1)), (Some(format), Some(&object_bytes(9))));
        assert!(
            !decoder.store().contains(PageAddress::from_mpp(0x3C0).unwrap()),
            "the copy of one reception is released"
        );
    }

    /// REQ-ACQ-019: the enhancement store keeps to its capacity in packets,
    /// replacing the least recently received sub-page, and drops packets
    /// only when every slot belongs to a sub-page being received.
    #[test]
    fn enhancement_store_capacity_and_eviction() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut memory = EnhancementMemory::new(4, 3);
        let mut decoder = Decoder::with_enhancement_store(&mut slots, memory.store(), Config::default());
        let mut events = Vec::new();
        let rows = |page: u8, count: u8| {
            let mut packets = std::vec![header(4, page, 0, 0)];
            packets.extend((1..=count).map(|row| object_row(4, row, row)));
            packets
        };
        feed(&mut decoder, &[header(4, 0xFE, 0, 0), object_row(4, 1, 1), header(4, 0x00, 0, 0)], &mut events);
        // A sub-page of five packets in a pool of three: the table is
        // evicted, and two packets are dropped.
        let mut five = rows(0xFE, 5);
        five[0] = header(4, 0xFE, 1, 0);
        feed(&mut decoder, &five, &mut events);
        assert_eq!(decoder.enhancement_store().packets_used(), 3);
        assert!(decoder.enhancement_lookup().magazine_organization_table(mag(4)).is_none(), "S1 = 0 was evicted");
        let page = decoder.enhancement_lookup().sub_page(mag(4), PageNumber(0xFE), 1).unwrap();
        assert_eq!(page.received(), 0b111);
        assert_eq!(decoder.statistics().enhancement_dropped, 2);
        // A decoder without an enhancement store keeps no enhancement pages.
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        feed(&mut decoder, &rows(0xFE, 2), &mut events);
        assert!(!decoder.enhancement_store().is_enabled());
        assert!(decoder.enhancement_lookup().magazine_organization_table(mag(4)).is_none());
    }

    /// REQ-ACQ-018: M/29/0, M/29/1 and M/29/4 are kept per magazine, each
    /// replacing the last, and are discarded when the network
    /// identification changes or a new input starts.
    #[test]
    fn magazine_data_is_kept_until_the_service_changes() {
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        let mut events = Vec::new();
        let fields = PresentationFields {
            colours: Some(crate::types::DefaultColourFields { screen: 3, ..Default::default() }),
            ..PresentationFields::default()
        };
        let colours = DrcsColourData { global_4: Some([1, 2, 3, 4]), ..Default::default() };
        let service = |network_id: u16| {
            let mut packet = encode::address(mag(8), 30);
            packet.bytes[2] = crate::ecc::hamming84_encode(0);
            let [high, low] = network_id.to_be_bytes();
            packet.bytes[9] = high.reverse_bits();
            packet.bytes[10] = low.reverse_bits();
            packet
        };
        feed(
            &mut decoder,
            &[
                service(0x1234),
                encode::magazine_enhancement(mag(5), 0, &fields),
                encode::drcs_colours(mag(5), 29, &colours),
                encode::magazine_enhancement(mag(5), 4, &fields),
                service(0x1234),
            ],
            &mut events,
        );
        let data = decoder.magazine_data(mag(5));
        let colours_of = |fields: Option<PresentationFields>| fields.and_then(|fields| fields.colours);
        assert_eq!((colours_of(data.data), colours_of(data.level35)), (fields.colours, fields.colours));
        assert_eq!(data.drcs_colours.unwrap().global_4, colours.global_4);
        assert_eq!(decoder.enhancement_lookup().magazine_data(mag(5)), Some(&data));
        events.clear();
        feed(&mut decoder, &[service(0x4321)], &mut events);
        assert_eq!(decoder.magazine_data(mag(5)), MagazineData::default());
        assert!(events.contains(&Event::MagazineData(mag(5))));
        feed(&mut decoder, &[encode::magazine_enhancement(mag(5), 0, &fields)], &mut events);
        decoder.start_new_input(&mut |_| {});
        assert_eq!(decoder.magazine_data(mag(5)), MagazineData::default());
    }

    /// REQ-ACQ-017: pages with hexadecimal numbers are stored at Levels 2.5
    /// and 3.5, and not at Levels 1 and 1.5.
    #[test]
    fn hexadecimal_pages_follow_the_level() {
        use crate::config::PresentationLevel as L;
        for (level, stored) in [(L::Level1, false), (L::Level15, false), (L::Level25, true), (L::Level35, true)] {
            let mut slots = std::vec![StoredPage::EMPTY; 4];
            let mut decoder = Decoder::new(&mut slots, Config { level, ..Config::default() });
            let mut events = Vec::new();
            feed(&mut decoder, &[header(6, 0x3D, 0, 0), header(6, 0x00, 0, 0)], &mut events);
            assert_eq!(decoder.store().contains(PageAddress::from_mpp(0x63D).unwrap()), stored, "{level:?}");
        }
    }

    /// REQ-ACQ-011, decision E.11: the rolling header takes every magazine
    /// in serial mode and only the requested one in parallel mode, never a
    /// header with C9 set; the clock takes every valid header.
    #[test]
    fn rolling_header_and_clock_follow_decision_e11() {
        let with_text = |m: u8, control: u16, rolling: &[u8], clock: &[u8]| {
            let mut display = [with_odd_parity(b' '); HEADER_DISPLAY_BYTES];
            display[..rolling.len()].copy_from_slice(&text(rolling)[..rolling.len()]);
            display[ROLLING_BYTES..ROLLING_BYTES + clock.len()].copy_from_slice(&text(clock)[..clock.len()]);
            encode::header(mag(m), PageNumber(0x10), SubCode::new(0), ControlBits(control), &display)
        };
        let shown =
            |bytes: &[u8]| -> std::string::String { bytes.iter().map(|&b| char::from(b & CODE_MASK_7BIT)).collect() };
        let mut slots = std::vec![StoredPage::EMPTY; 4];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        decoder.set_requested(Some(PageAddress::from_mpp(0x100).unwrap()));
        let mut events = Vec::new();
        feed(&mut decoder, &[with_text(2, 0, b"OTHER", b"10:00:01")], &mut events);
        assert_eq!(shown(&decoder.rolling_header()[..5]), "     ", "parallel: other magazine");
        assert_eq!(shown(decoder.clock()), "10:00:01");
        feed(&mut decoder, &[with_text(1, 0, b"WANTED", b"10:00:02")], &mut events);
        assert_eq!(shown(&decoder.rolling_header()[..6]), "WANTED");
        feed(&mut decoder, &[with_text(1, C9_INTERRUPTED, b"BROKEN", b"10:00:03")], &mut events);
        assert_eq!(shown(&decoder.rolling_header()[..6]), "WANTED", "C9 set");
        assert_eq!(shown(decoder.clock()), "10:00:03");
        feed(&mut decoder, &[with_text(2, C11_SERIAL, b"SERIAL", b"10:00:04")], &mut events);
        assert_eq!(shown(&decoder.rolling_header()[..6]), "SERIAL", "serial: every magazine");
        assert_eq!(shown(decoder.clock()), "10:00:04");
    }
}
