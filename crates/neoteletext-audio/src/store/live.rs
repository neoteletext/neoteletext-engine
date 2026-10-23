//! A live sound's ring of blocks (REQ-AUD-014): the newest replacing the
//! oldest, and the cursor of the block the sink gets next.

use super::{Entry, Store, Stored};
use crate::port::SoundId;
use crate::record::Data;

impl<const OBJECTS: usize> Store<'_, OBJECTS> {
    /// Records which live block the sink gets next, and when one last
    /// arrived.
    pub fn set_live_cursor(&mut self, sound: SoundId, next: u16, arrived_ms: u64) {
        if let Some(entry) = self.entries.iter_mut().flatten().find(|entry| entry.sound == sound) {
            entry.next = next;
            entry.arrived_ms = arrived_ms;
        }
    }

    /// Block `index` of a live sound, if its slot holds that block.
    #[must_use]
    pub fn live_block(&self, sound: SoundId, index: u16) -> Option<&[u8]> {
        let entry = self.entries.iter().flatten().find(|entry| entry.sound == sound && entry.live)?;
        let slot = index.checked_rem(entry.blocks)?;
        if !self.received(entry, slot) || self.slot_index(entry, slot) != Some(index) {
            return None;
        }
        let start = entry.offset.saturating_add(usize::from(slot).saturating_mul(usize::from(entry.block_bytes)));
        self.arena.get(start..start.saturating_add(usize::from(entry.block_bytes)))
    }

    /// The block index a live sound's slot holds.
    fn slot_index(&self, entry: &Entry, slot: u16) -> Option<u16> {
        let at = entry
            .offset
            .saturating_add(entry.data_bytes())
            .saturating_add(entry.bitmap_bytes())
            .saturating_add(usize::from(slot).saturating_mul(2));
        let bytes = self.arena.get(at..at.saturating_add(2))?;
        Some(u16::from_be_bytes([bytes.first().copied()?, bytes.get(1).copied()?]))
    }

    /// Stores a live sound's blocks, each whole, in the slot of its index
    /// modulo the ring. A block older than the ring is dropped.
    pub(super) fn put_live(&mut self, position: usize, mut entry: Entry, data: &Data<'_>) -> Stored {
        let length = usize::from(entry.block_bytes);
        if length == 0 || !data.blocks.len().is_multiple_of(length) {
            return Stored::Malformed;
        }
        let mut fresh = false;
        let mut count: u16 = 0;
        for (offset, bytes) in (0_u16..).zip(data.blocks.chunks_exact(length)) {
            let index = data.first_block.wrapping_add(offset);
            count = count.saturating_add(1);
            if entry
                .newest
                .is_some_and(|newest| newest.wrapping_sub(index) >= entry.blocks && !crate::serial_newer(index, newest))
            {
                continue;
            }
            let Some(slot) = index.checked_rem(entry.blocks) else { return Stored::Malformed };
            if self.received(&entry, slot) && self.slot_index(&entry, slot) == Some(index) {
                let start = entry.offset.saturating_add(usize::from(slot).saturating_mul(length));
                if self.arena.get(start..start.saturating_add(length)) != Some(bytes) {
                    entry.conflict = true;
                    if let Some(place) = self.entries.get_mut(position) {
                        *place = Some(entry);
                    }
                    return Stored::Conflict;
                }
                continue;
            }
            let start = entry.offset.saturating_add(usize::from(slot).saturating_mul(length));
            if let Some(target) = self.arena.get_mut(start..start.saturating_add(length)) {
                target.copy_from_slice(bytes);
            }
            let table = entry
                .offset
                .saturating_add(entry.data_bytes())
                .saturating_add(entry.bitmap_bytes())
                .saturating_add(usize::from(slot).saturating_mul(2));
            if let Some(target) = self.arena.get_mut(table..table.saturating_add(2)) {
                target.copy_from_slice(&index.to_be_bytes());
            }
            let bit = entry.offset.saturating_add(entry.data_bytes()).saturating_add(usize::from(slot / 8));
            if let Some(byte) = self.arena.get_mut(bit) {
                *byte |= 1 << (slot % 8);
            }
            if entry.newest.is_none_or(|newest| crate::serial_newer(index, newest)) {
                entry.newest = Some(index);
            }
            entry.received = entry.received.saturating_add(1);
            fresh = true;
        }
        if let Some(place) = self.entries.get_mut(position) {
            *place = Some(entry);
        }
        if fresh { Stored::Blocks { first: data.first_block, count } } else { Stored::Repeat }
    }
}
