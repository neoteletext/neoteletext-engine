//! The part of the page store that keeps enhancement pages: object pages,
//! the Magazine Organization Table, and any other page whose function is
//! not a basic Level 1 page (decision E.56,
//! REQ-ACQ-016 to REQ-ACQ-021), and the read-only lookup through which the
//! presentation layer reaches them (REQ-ACQ-020).
//!
//! The memory is two caller-provided slices (REQ-GEN-003): a directory of
//! sub-pages, and a pool of packets whose length is the capacity in packets
//! that REQ-ACQ-019 asks for. A sub-page holds an index into the pool for
//! each packet it has received, so a full page uses 24 slots and a small
//! object page only a few. Packets are stored as received, with no merge by
//! parity, because their bytes are Hamming coded or 6-bit data
//! (decision E.56).

use super::MagazineData;
use crate::consts::{
    C4_ERASE, DATA_BYTES, DESIGNATION_CODES, ENHANCEMENT_SUBPAGE_PACKETS, MAGAZINES, NIBBLE_MASK, PACKET_ROW_25,
    PAGE_MOT,
};
use crate::packet::PageFormat;
use crate::types::{ControlBits, Instant, Magazine, PageAddress, PageNumber, SubCode};

/// A pool slot that belongs to no sub-page.
const FREE: u16 = u16::MAX;
/// Shift of the update indicator S2 in a sub-code (Annex A.1).
const S2_SHIFT: u32 = 4;
/// Mask of S2 after the shift: three bits.
const S2_MASK: u16 = 0x7;
/// Shift of S3 in a sub-code; S4 S3 together give the last packet number
/// of an enhancement page (Annex A.1).
const S3_SHIFT: u32 = 8;
/// Mask of the six bits of S4 S3 after the shift.
const LAST_PACKET_MASK: u16 = 0x3F;

/// The key of an enhancement sub-page: its page address and the sub-page
/// number S1, which is all of the sub-code that names a sub-page of an
/// enhancement page (Annex A.1: the other digits are the last packet and
/// the update indicator).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct EnhancementKey {
    /// The magazine and page number.
    pub address: PageAddress,
    /// The sub-page number S1, 0 to 15.
    pub s1: u8,
}

impl EnhancementKey {
    /// The key of the sub-page a sub-code belongs to.
    #[must_use]
    pub const fn new(address: PageAddress, subcode: SubCode) -> Self {
        Self { address, s1: subcode.value() as u8 & NIBBLE_MASK }
    }
}

/// One slot of the packet pool: the 40 data bytes of a packet, bytes 6 to
/// 45, as received.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnhancementPacket {
    owner: u16,
    bytes: [u8; DATA_BYTES],
}

impl EnhancementPacket {
    /// An empty slot, for building caller-provided storage.
    pub const EMPTY: Self = Self { owner: FREE, bytes: [0; DATA_BYTES] };
}

/// One stored enhancement sub-page: the directory entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnhancementSubPage {
    used: bool,
    key: EnhancementKey,
    subcode: SubCode,
    control: ControlBits,
    format: Option<PageFormat>,
    received: u64,
    generation: u32,
    last_received: Instant,
    slots: [u16; ENHANCEMENT_SUBPAGE_PACKETS],
}

impl EnhancementSubPage {
    /// An empty entry, for building caller-provided storage.
    pub const EMPTY: Self = Self {
        used: false,
        key: EnhancementKey { address: PageAddress { magazine: Magazine::from_bits(1), page: PageNumber(0) }, s1: 0 },
        subcode: SubCode::new(0),
        control: ControlBits(0),
        format: None,
        received: 0,
        generation: 0,
        last_received: Instant(0),
        slots: [FREE; ENHANCEMENT_SUBPAGE_PACKETS],
    };
}

/// The index of a packet within a sub-page: packets X/1 to X/25 are 0 to
/// 24, and X/26 with designation code d is 25 + d.
#[must_use]
pub const fn row_index(packet: u8) -> Option<usize> {
    if packet >= 1 && packet <= PACKET_ROW_25 { Some((packet as usize).saturating_sub(1)) } else { None }
}

/// The index of packet X/26 with designation code `designation`.
#[must_use]
pub const fn enhancement_index(designation: u8) -> Option<usize> {
    if (designation as usize) < DESIGNATION_CODES {
        Some((PACKET_ROW_25 as usize).saturating_add(designation as usize))
    } else {
        None
    }
}

/// A read-only view of one stored enhancement sub-page.
#[derive(Clone, Copy, Debug)]
pub struct EnhancementPage<'s> {
    entry: &'s EnhancementSubPage,
    packets: &'s [EnhancementPacket],
}

impl<'s> EnhancementPage<'s> {
    /// The page address and sub-page number.
    #[must_use]
    pub const fn key(&self) -> EnhancementKey {
        self.entry.key
    }

    /// The sub-code of the latest header (Annex A.1).
    #[must_use]
    pub const fn subcode(&self) -> SubCode {
        self.entry.subcode
    }

    /// The control bits of the latest header. On enhancement pages C12
    /// means a continued fragment and C13 a partial page (Annex B.6).
    #[must_use]
    pub const fn control(&self) -> ControlBits {
        self.entry.control
    }

    /// The last packet number of an uninterrupted transmission, from S4 and
    /// S3 of the sub-code: 1 to 25 for X/1 to X/25, 26 to 41 for X/26/0 to
    /// X/26/15 (Annex A.1; REQ-ACQ-021).
    #[must_use]
    pub const fn last_packet(&self) -> u8 {
        ((self.entry.subcode.value() >> S3_SHIFT) & LAST_PACKET_MASK) as u8
    }

    /// The page function and coding from the page's X/28/0 or X/28/3, if
    /// one was received.
    #[must_use]
    pub const fn format(&self) -> Option<PageFormat> {
        self.entry.format
    }

    /// A number that changes whenever the stored content or the update
    /// indicator S2 changes, so that decoded data taken from the sub-page is
    /// not reused once stale (REQ-ACQ-021).
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.entry.generation
    }

    /// The time of the most recent packet.
    #[must_use]
    pub const fn last_received(&self) -> Instant {
        self.entry.last_received
    }

    /// Which packets have been received: bit i for the packet with index i
    /// ([`row_index`], [`enhancement_index`]).
    #[must_use]
    pub const fn received(&self) -> u64 {
        self.entry.received
    }

    fn packet(&self, index: Option<usize>) -> Option<&'s [u8; DATA_BYTES]> {
        let slot = *self.entry.slots.get(index?)?;
        self.packets.get(usize::from(slot)).map(|packet| &packet.bytes)
    }

    /// The stored data bytes of packet X/`packet` (1 to 25), as received.
    #[must_use]
    pub fn row(&self, packet: u8) -> Option<&'s [u8; DATA_BYTES]> {
        self.packet(row_index(packet))
    }

    /// The stored data bytes of X/26 with designation code `designation`,
    /// byte 6 being the Hamming-coded designation code.
    #[must_use]
    pub fn enhancement(&self, designation: u8) -> Option<&'s [u8; DATA_BYTES]> {
        self.packet(enhancement_index(designation))
    }
}

/// What happened to a packet offered to the store.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Stored {
    /// Stored; `changed` when its bytes differ from those it replaced.
    Yes {
        /// The content changed.
        changed: bool,
    },
    /// Dropped: every slot belongs to a sub-page being received.
    Dropped,
}

/// The enhancement part of the page store (REQ-ACQ-019).
#[derive(Debug)]
pub struct EnhancementStore<'a> {
    sub_pages: &'a mut [EnhancementSubPage],
    packets: &'a mut [EnhancementPacket],
    next_free: usize,
}

impl<'a> EnhancementStore<'a> {
    /// A store over a directory of sub-pages and a pool of packets; both
    /// are reset to empty. The pool's length is the capacity in packets;
    /// Annex E.8 gives 500 at Level 2.5 and 1200 at Level 3.5
    /// ([`crate::consts::ENHANCEMENT_PACKETS_LEVEL_25`] and
    /// [`crate::consts::ENHANCEMENT_PACKETS_LEVEL_35`]). Empty slices give a
    /// store that keeps nothing.
    pub fn new(sub_pages: &'a mut [EnhancementSubPage], packets: &'a mut [EnhancementPacket]) -> Self {
        sub_pages.fill(EnhancementSubPage::EMPTY);
        packets.fill(EnhancementPacket::EMPTY);
        Self { sub_pages, packets, next_free: 0 }
    }

    /// True when the store can keep anything.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        !self.sub_pages.is_empty() && !self.packets.is_empty()
    }

    /// The directory and pool capacities.
    #[must_use]
    pub const fn capacity(&self) -> (usize, usize) {
        (self.sub_pages.len(), self.packets.len())
    }

    /// The stored sub-page with a key.
    #[must_use]
    pub fn get(&self, key: EnhancementKey) -> Option<EnhancementPage<'_>> {
        lookup(self.sub_pages, self.packets, key)
    }

    /// Every stored sub-page, in directory order.
    pub fn sub_pages(&self) -> impl Iterator<Item = EnhancementPage<'_>> {
        let packets = &*self.packets;
        self.sub_pages.iter().filter(|entry| entry.used).map(move |entry| EnhancementPage { entry, packets })
    }

    /// True when any sub-page of an address is stored.
    #[must_use]
    pub fn contains_address(&self, address: PageAddress) -> bool {
        self.sub_pages.iter().any(|entry| entry.used && entry.key.address == address)
    }

    /// The number of packets in use.
    #[must_use]
    pub fn packets_used(&self) -> usize {
        self.packets.iter().filter(|packet| packet.owner != FREE).count()
    }

    fn entry(&self, index: usize) -> Option<&EnhancementSubPage> {
        self.sub_pages.get(index).filter(|entry| entry.used)
    }

    /// The key of the entry at `index`.
    pub(super) fn key(&self, index: usize) -> Option<EnhancementKey> {
        self.entry(index).map(|entry| entry.key)
    }

    /// The generation of the entry at `index`.
    pub(super) fn generation(&self, index: usize) -> Option<u32> {
        self.entry(index).map(|entry| entry.generation)
    }

    /// Frees every packet of an entry.
    fn release_packets(&mut self, index: usize) {
        let Some(entry) = self.sub_pages.get_mut(index) else { return };
        for slot in &mut entry.slots {
            if let Some(packet) = self.packets.get_mut(usize::from(*slot)) {
                packet.owner = FREE;
            }
            *slot = FREE;
        }
        entry.received = 0;
    }

    /// The least recently received entry that is not in `busy`.
    fn victim(&self, busy: &[Option<usize>]) -> Option<usize> {
        self.sub_pages
            .iter()
            .enumerate()
            .filter(|(index, entry)| entry.used && !busy.contains(&Some(*index)))
            .min_by_key(|(_, entry)| entry.last_received)
            .map(|(index, _)| index)
    }

    /// Evicts an entry, freeing its packets.
    fn evict(&mut self, index: usize) {
        self.release_packets(index);
        if let Some(entry) = self.sub_pages.get_mut(index) {
            *entry = EnhancementSubPage::EMPTY;
        }
    }

    /// Starts a reception of a sub-page (REQ-ACQ-021): finds or allocates
    /// its entry, evicting the least recently received sub-page that is not
    /// being received when the directory is full, and erases its packets
    /// when the header has C4 set. A change of S2 changes the generation.
    pub(super) fn start(
        &mut self,
        address: PageAddress,
        subcode: SubCode,
        control: ControlBits,
        now: Instant,
        busy: &[Option<usize>],
    ) -> Option<usize> {
        let key = EnhancementKey::new(address, subcode);
        let found = self.sub_pages.iter().position(|entry| entry.used && entry.key == key);
        let index = if let Some(index) = found {
            index
        } else {
            let index = if let Some(index) = self.sub_pages.iter().position(|entry| !entry.used) {
                index
            } else {
                let victim = self.victim(busy)?;
                self.evict(victim);
                victim
            };
            if let Some(entry) = self.sub_pages.get_mut(index) {
                *entry = EnhancementSubPage { used: true, key, subcode, ..EnhancementSubPage::EMPTY };
            }
            index
        };
        if control.has(C4_ERASE) {
            self.release_packets(index);
            if let Some(entry) = self.sub_pages.get_mut(index) {
                entry.format = None;
                entry.generation = entry.generation.wrapping_add(1);
            }
        }
        let entry = self.sub_pages.get_mut(index)?;
        let update = |subcode: SubCode| (subcode.value() >> S2_SHIFT) & S2_MASK;
        if update(entry.subcode) != update(subcode) {
            entry.generation = entry.generation.wrapping_add(1);
        }
        entry.subcode = subcode;
        entry.control = control;
        entry.last_received = now;
        Some(index)
    }

    /// A free pool slot, evicting least recently received sub-pages other
    /// than `busy` ones until one is free.
    fn free_slot(&mut self, busy: &[Option<usize>]) -> Option<usize> {
        loop {
            let length = self.packets.len();
            let found = (0..length)
                .map(|offset| self.next_free.saturating_add(offset).checked_rem(length).unwrap_or(0))
                .find(|&slot| self.packets.get(slot).is_some_and(|packet| packet.owner == FREE));
            if let Some(slot) = found {
                self.next_free = slot.saturating_add(1);
                return Some(slot);
            }
            let victim = self.victim(busy)?;
            self.evict(victim);
        }
    }

    /// Stores the data bytes of the packet with index `packet` of the entry
    /// at `index`, replacing what was there (decision E.56).
    pub(super) fn store(
        &mut self,
        index: usize,
        packet: usize,
        bytes: &[u8; DATA_BYTES],
        now: Instant,
        busy: &[Option<usize>],
    ) -> Stored {
        let existing = self.sub_pages.get(index).and_then(|entry| entry.slots.get(packet)).copied();
        let Some(existing) = existing else { return Stored::Dropped };
        let (slot, changed) = if existing == FREE {
            let Some(slot) = self.free_slot(busy) else { return Stored::Dropped };
            (slot, true)
        } else {
            let same = self.packets.get(usize::from(existing)).is_some_and(|stored| stored.bytes == *bytes);
            (usize::from(existing), !same)
        };
        let (Some(target), Some(entry)) = (self.packets.get_mut(slot), self.sub_pages.get_mut(index)) else {
            return Stored::Dropped;
        };
        target.owner = index as u16;
        target.bytes = *bytes;
        if let Some(entry_slot) = entry.slots.get_mut(packet) {
            *entry_slot = slot as u16;
        }
        entry.received |= 1u64.wrapping_shl(packet as u32);
        entry.last_received = now;
        if changed {
            entry.generation = entry.generation.wrapping_add(1);
        }
        Stored::Yes { changed }
    }

    /// Records the page function and coding of the page's X/28/0 or X/28/3.
    pub(super) fn set_format(&mut self, index: usize, format: PageFormat) {
        if let Some(entry) = self.sub_pages.get_mut(index)
            && entry.format != Some(format)
        {
            entry.format = Some(format);
            entry.generation = entry.generation.wrapping_add(1);
        }
    }

    /// The read-only lookup over this store and the magazine data.
    pub(super) fn lookup<'s>(&'s self, magazines: &'s [MagazineData; MAGAZINES]) -> EnhancementLookup<'s> {
        EnhancementLookup { sub_pages: self.sub_pages, packets: self.packets, magazines }
    }
}

fn lookup<'s>(
    sub_pages: &'s [EnhancementSubPage],
    packets: &'s [EnhancementPacket],
    key: EnhancementKey,
) -> Option<EnhancementPage<'s>> {
    sub_pages.iter().find(|entry| entry.used && entry.key == key).map(|entry| EnhancementPage { entry, packets })
}

/// The read-only lookup that the presentation layer uses to reach
/// enhancement data (REQ-ACQ-020; the L2 to L3 boundary of
/// `ARCHITECTURE.md` section 5). It has no side effects, and each call is
/// a bounded search of the directory.
#[derive(Clone, Copy, Debug)]
pub struct EnhancementLookup<'s> {
    sub_pages: &'s [EnhancementSubPage],
    packets: &'s [EnhancementPacket],
    magazines: &'s [MagazineData; MAGAZINES],
}

impl<'s> EnhancementLookup<'s> {
    /// The stored packets of an enhancement sub-page, named by its
    /// magazine, page number and S1 (decision E.52: invocations name the
    /// sub-page by S1 directly).
    #[must_use]
    pub fn sub_page(&self, magazine: Magazine, page: PageNumber, s1: u8) -> Option<EnhancementPage<'s>> {
        lookup(self.sub_pages, self.packets, EnhancementKey { address: PageAddress { magazine, page }, s1 })
    }

    /// The Magazine Organization Table of a magazine: page mFE, sub-page
    /// S1 = 0 only (decision E.52).
    #[must_use]
    pub fn magazine_organization_table(&self, magazine: Magazine) -> Option<EnhancementPage<'s>> {
        self.sub_page(magazine, PageNumber(PAGE_MOT), 0)
    }

    /// The stored M/29 packets of a magazine (REQ-ACQ-018).
    #[must_use]
    pub fn magazine_data(&self, magazine: Magazine) -> Option<&'s MagazineData> {
        self.magazines.get(magazine.index())
    }
}
