//! The object store (REQ-AUD-009): sounds kept compressed in an arena the
//! port provides, each with a bitmap of the blocks received. An object is
//! admitted only if it fits once complete; the oldest objects nothing needs
//! are evicted to make room.

use crate::Codec;
use crate::port::SoundId;
use crate::record::{Data, Object};

/// A stored sound's definition and state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    /// Its handle and generation.
    pub sound: SoundId,
    /// Stable across generations.
    pub id: u32,
    /// The codec.
    pub codec: Codec,
    /// Samples a second.
    pub rate_hz: u32,
    /// Bytes in each whole block.
    pub block_bytes: u16,
    /// Samples in the whole sound.
    pub samples: u32,
    /// Blocks in the whole sound.
    pub blocks: u16,
    /// Blocks received so far.
    pub received: u16,
    /// A block arrived twice with different bytes: the sound is not played
    /// until a newer generation replaces it.
    pub conflict: bool,
    /// The sink holds the sound's samples.
    pub at_sink: bool,
    /// A live sound (REQ-AUD-014): `blocks` slots of a ring, filled by block
    /// index modulo their count.
    pub live: bool,
    /// The newest block index of a live sound.
    pub newest: Option<u16>,
    /// The next block index of a live sound to give the sink.
    pub next: u16,
    /// When the last new block of a live sound arrived, in the receiver's
    /// milliseconds.
    pub arrived_ms: u64,
    offset: usize,
    stamp: u32,
}

impl Entry {
    /// Every block has arrived.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.received == self.blocks
    }

    /// Samples in a whole block.
    #[must_use]
    pub fn samples_per_block(&self) -> u32 {
        self.codec.samples_per_block(self.block_bytes)
    }

    /// Samples in block `index`: whole, or what is left in the final block.
    #[must_use]
    pub fn block_samples(&self, index: u16) -> u32 {
        let per_block = self.samples_per_block();
        if self.live {
            return per_block;
        }
        let before = u32::from(index).saturating_mul(per_block);
        self.samples.saturating_sub(before).min(per_block)
    }

    /// Bytes in block `index`.
    #[must_use]
    pub fn block_length(&self, index: u16) -> usize {
        usize::try_from(self.codec.block_bytes_for(self.block_samples(index))).unwrap_or(usize::MAX)
    }

    fn data_bytes(&self) -> usize {
        usize::from(self.blocks).saturating_mul(usize::from(self.block_bytes))
    }

    fn bitmap_bytes(&self) -> usize {
        usize::from(self.blocks).div_ceil(8)
    }

    /// A live sound's table of the block index in each slot.
    fn index_table_bytes(&self) -> usize {
        if self.live { usize::from(self.blocks).saturating_mul(2) } else { 0 }
    }

    fn size(&self) -> usize {
        self.data_bytes().saturating_add(self.bitmap_bytes()).saturating_add(self.index_table_bytes())
    }
}

/// What an OBJECT record did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Defined {
    /// A new sound; it may replace an older generation of its handle.
    New {
        /// The older generation it replaced.
        replaced: Option<Entry>,
        /// Sounds evicted to make room.
        evicted: usize,
    },
    /// The same definition again: nothing changes.
    Repeat,
    /// The codec is reserved: the object is ignored.
    Ignored,
    /// An older generation, or a different definition of the same
    /// generation: refused.
    Conflict,
    /// It would not fit even with every evictable sound gone.
    ResourceLimit,
}

/// What a DATA record did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stored {
    /// New blocks were stored: the first new block and how many.
    Blocks {
        /// The first block in the record.
        first: u16,
        /// Blocks in the record.
        count: u16,
    },
    /// Every block was there already.
    Repeat,
    /// No such sound or generation: discarded.
    Unknown,
    /// Blocks of the wrong length, or beyond the sound's end: discarded.
    Malformed,
    /// A block differed from the one stored: the sound is now in conflict.
    Conflict,
}

/// Compressed sounds in an arena of bytes.
pub struct Store<'a, const OBJECTS: usize> {
    arena: &'a mut [u8],
    entries: [Option<Entry>; OBJECTS],
    clock: u32,
}

impl<'a, const OBJECTS: usize> Store<'a, OBJECTS> {
    /// An empty store over `arena`.
    pub fn new(arena: &'a mut [u8]) -> Self {
        Self { arena, entries: [None; OBJECTS], clock: 0 }
    }

    /// The arena's size in bytes.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.arena.len()
    }

    /// Forgets every sound.
    pub fn clear(&mut self) {
        self.entries = [None; OBJECTS];
    }

    /// The stored sound with this handle.
    #[must_use]
    pub fn entry(&self, handle: u8) -> Option<&Entry> {
        self.entries.iter().flatten().find(|entry| entry.sound.handle == handle)
    }

    /// Every stored sound.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().flatten()
    }

    /// Marks whether the sink holds a sound's samples.
    pub fn set_at_sink(&mut self, sound: SoundId, at_sink: bool) {
        if let Some(entry) = self.entries.iter_mut().flatten().find(|entry| entry.sound == sound) {
            entry.at_sink = at_sink;
        }
    }

    /// Marks that the sink holds nothing.
    pub fn sink_lost(&mut self) {
        for entry in self.entries.iter_mut().flatten() {
            entry.at_sink = false;
        }
    }

    /// Block `index` of a sound, if it has arrived.
    #[must_use]
    pub fn block(&self, sound: SoundId, index: u16) -> Option<&[u8]> {
        let entry = self.entries.iter().flatten().find(|entry| entry.sound == sound)?;
        if !self.received(entry, index) {
            return None;
        }
        let start = entry.offset.saturating_add(usize::from(index).saturating_mul(usize::from(entry.block_bytes)));
        self.arena.get(start..start.saturating_add(entry.block_length(index)))
    }

    /// Whole blocks received from the start, contiguously.
    #[must_use]
    pub fn contiguous(&self, sound: SoundId) -> u16 {
        let Some(entry) = self.entries.iter().flatten().find(|entry| entry.sound == sound) else { return 0 };
        (0..entry.blocks).take_while(|&index| self.received(entry, index)).count().try_into().unwrap_or(0)
    }

    fn received(&self, entry: &Entry, index: u16) -> bool {
        let at = entry.offset.saturating_add(entry.data_bytes()).saturating_add(usize::from(index / 8));
        index < entry.blocks && self.arena.get(at).is_some_and(|byte| byte & (1 << (index % 8)) != 0)
    }

    /// Adds or refreshes a sound's definition. `busy` says whether the
    /// receiver still needs a sound, which protects it from eviction.
    pub fn define(&mut self, object: &Object<'_>, busy: &dyn Fn(SoundId) -> bool) -> Defined {
        let Some(codec) = object.codec else { return Defined::Ignored };
        let live = object.samples == 0;
        let sound = SoundId { handle: object.handle, generation: object.generation };
        let fresh = Entry {
            sound,
            id: object.id,
            codec,
            rate_hz: object.rate_hz,
            block_bytes: object.block_bytes,
            samples: object.samples,
            blocks: if live { crate::LIVE_RING_BLOCKS } else { object.blocks },
            received: 0,
            conflict: false,
            at_sink: false,
            live,
            newest: None,
            next: 0,
            arrived_ms: 0,
            offset: 0,
            stamp: 0,
        };
        let mut replaced = None;
        if let Some(index) =
            self.entries.iter().position(|entry| entry.is_some_and(|entry| entry.sound.handle == object.handle))
        {
            let existing = self.entries.get(index).copied().flatten();
            if let Some(existing) = existing {
                if existing.sound.generation == object.generation {
                    let same = existing.id == fresh.id
                        && existing.codec == fresh.codec
                        && existing.rate_hz == fresh.rate_hz
                        && existing.block_bytes == fresh.block_bytes
                        && existing.samples == fresh.samples;
                    return if same { Defined::Repeat } else { Defined::Conflict };
                }
                if !crate::serial_newer(object.generation, existing.sound.generation) {
                    return Defined::Conflict;
                }
                replaced = Some(existing);
                if let Some(slot) = self.entries.get_mut(index) {
                    *slot = None;
                }
            }
        }
        let size = fresh.size();
        if size > self.arena.len() {
            return Defined::ResourceLimit;
        }
        let mut evicted: usize = 0;
        let offset = loop {
            if let Some(offset) = self.find_space(size) {
                break offset;
            }
            // Evict the oldest sound nothing needs.
            let victim = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| entry.filter(|entry| !busy(entry.sound)).map(|entry| (index, entry.stamp)))
                .min_by_key(|&(_, stamp)| stamp)
                .map(|(index, _)| index);
            match victim {
                Some(index) => {
                    if let Some(slot) = self.entries.get_mut(index) {
                        *slot = None;
                    }
                    evicted = evicted.saturating_add(1);
                }
                None => return Defined::ResourceLimit,
            }
        };
        let Some(slot) = self.entries.iter().position(Option::is_none) else { return Defined::ResourceLimit };
        self.clock = self.clock.wrapping_add(1);
        let entry = Entry { offset, stamp: self.clock, ..fresh };
        // A clear bitmap: no block received.
        let bitmap = offset.saturating_add(entry.data_bytes());
        if let Some(bits) = self.arena.get_mut(bitmap..bitmap.saturating_add(entry.bitmap_bytes())) {
            bits.fill(0);
        }
        if let Some(place) = self.entries.get_mut(slot) {
            *place = Some(entry);
        }
        Defined::New { replaced, evicted }
    }

    /// First fit: the lowest offset where `size` bytes are free.
    fn find_space(&self, size: usize) -> Option<usize> {
        let mut taken: [(usize, usize); OBJECTS] = [(0, 0); OBJECTS];
        let mut count = 0;
        for entry in self.entries.iter().flatten() {
            if let Some(place) = taken.get_mut(count) {
                *place = (entry.offset, entry.offset.saturating_add(entry.size()));
                count = count.saturating_add(1);
            }
        }
        let taken = taken.get_mut(..count)?;
        taken.sort_unstable();
        let mut start = 0;
        for &(from, to) in taken.iter() {
            if from.saturating_sub(start) >= size {
                return Some(start);
            }
            start = start.max(to);
        }
        (self.arena.len().saturating_sub(start) >= size).then_some(start)
    }

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
    fn put_live(&mut self, position: usize, mut entry: Entry, data: &Data<'_>) -> Stored {
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

    /// Stores a DATA record's blocks.
    pub fn put(&mut self, data: &Data<'_>) -> Stored {
        let Some(index) =
            self.entries.iter().position(|entry| entry.is_some_and(|entry| entry.sound.handle == data.handle))
        else {
            return Stored::Unknown;
        };
        let Some(mut entry) = self.entries.get(index).copied().flatten() else { return Stored::Unknown };
        if entry.sound.generation != data.generation {
            return Stored::Unknown;
        }
        if entry.live {
            return self.put_live(index, entry, data);
        }
        // The record's blocks: whole ones, then possibly the final, shorter one.
        let mut position = 0;
        let mut block = data.first_block;
        let mut count: u16 = 0;
        let mut fresh = false;
        while position < data.blocks.len() {
            if block >= entry.blocks {
                return Stored::Malformed;
            }
            let length = entry.block_length(block);
            let Some(bytes) = data.blocks.get(position..position.saturating_add(length)) else {
                return Stored::Malformed;
            };
            let start = entry.offset.saturating_add(usize::from(block).saturating_mul(usize::from(entry.block_bytes)));
            let received = self.received(&entry, block);
            let Some(target) = self.arena.get_mut(start..start.saturating_add(length)) else {
                return Stored::Malformed;
            };
            if received {
                if target != bytes {
                    entry.conflict = true;
                    if let Some(slot) = self.entries.get_mut(index) {
                        *slot = Some(entry);
                    }
                    return Stored::Conflict;
                }
            } else {
                target.copy_from_slice(bytes);
                let bit = entry.offset.saturating_add(entry.data_bytes()).saturating_add(usize::from(block / 8));
                if let Some(byte) = self.arena.get_mut(bit) {
                    *byte |= 1 << (block % 8);
                }
                entry.received = entry.received.saturating_add(1);
                fresh = true;
            }
            position = position.saturating_add(length);
            block = block.saturating_add(1);
            count = count.saturating_add(1);
        }
        if let Some(slot) = self.entries.get_mut(index) {
            *slot = Some(entry);
        }
        if fresh { Stored::Blocks { first: data.first_block, count } } else { Stored::Repeat }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-005, REQ-AUD-009 and REQ-AUD-014.
    use super::*;

    fn object(handle: u8, generation: u16, samples: u32) -> Object<'static> {
        // PCM in 100-byte blocks: 50 samples a block.
        Object {
            handle,
            generation,
            id: u32::from(handle),
            codec: Some(Codec::Pcm16),
            rate_hz: 8_000,
            block_bytes: 100,
            samples,
            blocks: u16::try_from(samples.div_ceil(50)).unwrap(),
            label: None,
            language: None,
        }
    }

    const IDLE: &dyn Fn(SoundId) -> bool = &|_| false;

    #[test]
    fn blocks_are_stored_and_a_sound_completes() {
        let mut arena = [0_u8; 1000];
        let mut store = Store::<4>::new(&mut arena);
        assert!(matches!(store.define(&object(1, 1, 120), IDLE), Defined::New { replaced: None, evicted: 0 }));
        // Three blocks: 50, 50 and a final 20 samples (40 bytes).
        let blocks: std::vec::Vec<u8> = (0..240_u8).collect();
        let sound = SoundId { handle: 1, generation: 1 };
        assert_eq!(
            store.put(&Data { handle: 1, generation: 1, first_block: 1, blocks: &blocks[100..] }),
            Stored::Blocks { first: 1, count: 2 }
        );
        assert_eq!(store.contiguous(sound), 0);
        assert_eq!(
            store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &blocks[..100] }),
            Stored::Blocks { first: 0, count: 1 }
        );
        assert!(store.entry(1).unwrap().complete());
        assert_eq!(store.block(sound, 2), Some(&blocks[200..]));
        // A repeat changes nothing; a different block is a conflict.
        assert_eq!(
            store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &blocks[..100] }),
            Stored::Repeat
        );
        let mut changed = blocks[..100].to_vec();
        changed[0] ^= 1;
        assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &changed }), Stored::Conflict);
        assert!(store.entry(1).unwrap().conflict);
    }

    #[test]
    fn wrong_lengths_and_generations_are_discarded() {
        let mut arena = [0_u8; 1000];
        let mut store = Store::<4>::new(&mut arena);
        store.define(&object(1, 1, 120), IDLE);
        assert_eq!(store.put(&Data { handle: 1, generation: 2, first_block: 0, blocks: &[0; 100] }), Stored::Unknown);
        assert_eq!(store.put(&Data { handle: 2, generation: 1, first_block: 0, blocks: &[0; 100] }), Stored::Unknown);
        assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 0, blocks: &[0; 99] }), Stored::Malformed);
        assert_eq!(store.put(&Data { handle: 1, generation: 1, first_block: 3, blocks: &[0; 40] }), Stored::Malformed);
    }

    #[test]
    fn generations_replace_and_conflicts_are_refused() {
        let mut arena = [0_u8; 1000];
        let mut store = Store::<4>::new(&mut arena);
        store.define(&object(1, 5, 100), IDLE);
        assert_eq!(store.define(&object(1, 5, 100), IDLE), Defined::Repeat);
        assert_eq!(store.define(&object(1, 5, 150), IDLE), Defined::Conflict, "same generation, different definition");
        assert_eq!(store.define(&object(1, 4, 100), IDLE), Defined::Conflict, "an older generation");
        assert!(matches!(
            store.define(&object(1, 6, 100), IDLE),
            Defined::New { replaced: Some(Entry { sound: SoundId { generation: 5, .. }, .. }), .. }
        ));
    }

    #[test]
    fn the_oldest_idle_sound_is_evicted_and_a_busy_one_never() {
        // Each 100-sample sound takes 200 bytes and a 1-byte bitmap.
        let mut arena = [0_u8; 450];
        let mut store = Store::<4>::new(&mut arena);
        store.define(&object(1, 1, 100), IDLE);
        store.define(&object(2, 1, 100), IDLE);
        let busy_one: &dyn Fn(SoundId) -> bool = &|sound| sound.handle == 1;
        assert!(matches!(store.define(&object(3, 1, 100), busy_one), Defined::New { evicted: 1, .. }));
        assert!(store.entry(1).is_some() && store.entry(2).is_none() && store.entry(3).is_some());
        let all_busy: &dyn Fn(SoundId) -> bool = &|_| true;
        assert_eq!(store.define(&object(4, 1, 100), all_busy), Defined::ResourceLimit);
        assert_eq!(store.define(&object(5, 1, 1000), IDLE), Defined::ResourceLimit, "larger than the arena");
    }

    #[test]
    fn a_reserved_codec_is_not_stored() {
        let mut arena = [0_u8; 100];
        let mut store = Store::<2>::new(&mut arena);
        assert_eq!(store.define(&Object { codec: None, ..object(1, 1, 10) }, IDLE), Defined::Ignored);
    }

    /// A DATA record of a live block at `index`.
    fn live(index: u16, block: &[u8]) -> Data<'_> {
        Data { handle: 1, generation: 1, first_block: index, blocks: block }
    }

    #[test]
    fn a_live_sound_keeps_its_newest_blocks_in_a_ring() {
        // A ring of 32 blocks of 100 bytes, its bitmap and its index table.
        let mut arena = [0_u8; 3_300];
        let mut store = Store::<2>::new(&mut arena);
        let sound = SoundId { handle: 1, generation: 1 };
        assert!(matches!(
            store.define(&Object { samples: 0, blocks: 0, ..object(1, 1, 0) }, IDLE),
            Defined::New { .. }
        ));
        let entry = *store.entry(1).unwrap();
        assert!(entry.live && entry.blocks == crate::LIVE_RING_BLOCKS && entry.newest.is_none());
        let block = |value: u8| [value; 100];
        assert_eq!(store.put(&live(5, &block(5))), Stored::Blocks { first: 5, count: 1 });
        assert_eq!(store.live_block(sound, 5), Some(&block(5)[..]));
        // Block 37 takes block 5's slot: 5 is gone and 37 is the newest.
        assert_eq!(store.put(&live(37, &block(37))), Stored::Blocks { first: 37, count: 1 });
        assert_eq!((store.live_block(sound, 5), store.live_block(sound, 37)), (None, Some(&block(37)[..])));
        assert_eq!(store.entry(1).unwrap().newest, Some(37));
        // A block older than the ring is dropped; an older one inside it is kept.
        assert_eq!(store.put(&live(5, &block(5))), Stored::Repeat);
        assert_eq!(store.live_block(sound, 37), Some(&block(37)[..]));
        assert_eq!(store.put(&live(30, &block(30))), Stored::Blocks { first: 30, count: 1 });
        assert_eq!(store.entry(1).unwrap().newest, Some(37), "an older block is not the newest");
        // The index wraps: 65 535 then 0.
        assert_eq!(store.put(&live(u16::MAX, &block(1))), Stored::Repeat, "far older by serial arithmetic");
        // A part block is malformed: live blocks are always whole.
        assert_eq!(store.put(&live(38, &[0; 40])), Stored::Malformed);
        // The same block with other bytes is a conflict.
        assert_eq!(store.put(&live(37, &block(9))), Stored::Conflict);
    }

    #[test]
    fn a_live_index_wraps_from_the_largest_to_zero() {
        let mut arena = [0_u8; 3_300];
        let mut store = Store::<2>::new(&mut arena);
        let sound = SoundId { handle: 1, generation: 1 };
        store.define(&Object { samples: 0, blocks: 0, ..object(1, 1, 0) }, IDLE);
        store.put(&live(u16::MAX, &[1; 100]));
        assert_eq!(store.put(&live(0, &[2; 100])), Stored::Blocks { first: 0, count: 1 });
        assert_eq!(store.entry(1).unwrap().newest, Some(0));
        assert_eq!(store.live_block(sound, u16::MAX), Some(&[1; 100][..]));
    }
}
