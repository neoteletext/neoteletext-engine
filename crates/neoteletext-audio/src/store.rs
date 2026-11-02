//! The object store (REQ-AUD-009): sounds kept compressed in an arena the
//! port provides, each with a bitmap of the blocks received. An object is
//! admitted only if it fits once complete; the oldest objects nothing needs
//! are evicted to make room.

use crate::Codec;
use crate::port::SoundId;
use crate::record::Data;

mod arena;
mod live;

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
    /// Where a live sound's samples stand at the sink, for the fades either
    /// side of a block given as silence (REQ-AUD-013).
    pub edge: LiveEdge,
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
        usize::try_from(self.codec.block_bytes_for(self.block_bytes, self.block_samples(index))).unwrap_or(usize::MAX)
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

/// Where a live sound's samples stand at the sink (REQ-AUD-013). A block
/// given up as silence is faded into and out of over 5 ms; the fade into it
/// falls on the block before, which was sent before the loss was known, so
/// the last 5 ms of each block sent are held back until the next block's
/// fate is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LiveEdge {
    /// Nothing sent since the sound joined: the voice's own start ramps it in.
    #[default]
    Joined,
    /// The last 5 ms of block `next − 1` are held back; `faded_in` when that
    /// block followed a silence and its first 5 ms were faded in.
    Held {
        /// The held block followed a silence.
        faded_in: bool,
    },
    /// The last block sent was silence: the next block fades in.
    Gap,
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

    /// Records where a live sound's samples stand at the sink.
    pub fn set_live_edge(&mut self, sound: SoundId, edge: LiveEdge) {
        if let Some(entry) = self.entries.iter_mut().flatten().find(|entry| entry.sound == sound) {
            entry.edge = edge;
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

    /// Stores a DATA record's blocks. The record is judged whole before any
    /// of it is written, first each block's place and length and then each
    /// block already stored against the record's, so a record refused or in
    /// conflict stores nothing and the bitmap and `received` keep agreeing
    /// (REQ-AUD-003, REQ-AUD-005, REQ-AUD-009).
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
        if record_blocks(&entry, data).any(|block| block.is_none()) {
            return Stored::Malformed;
        }
        let differs = record_blocks(&entry, data)
            .flatten()
            .any(|(block, bytes)| self.block(entry.sound, block).is_some_and(|stored| stored != bytes));
        if differs {
            entry.conflict = true;
            if let Some(slot) = self.entries.get_mut(index) {
                *slot = Some(entry);
            }
            return Stored::Conflict;
        }
        let mut count: u16 = 0;
        let mut fresh = false;
        for (block, bytes) in record_blocks(&entry, data).flatten() {
            count = count.saturating_add(1);
            if self.received(&entry, block) {
                continue;
            }
            let start = entry.offset.saturating_add(usize::from(block).saturating_mul(usize::from(entry.block_bytes)));
            if let Some(target) = self.arena.get_mut(start..start.saturating_add(bytes.len())) {
                target.copy_from_slice(bytes);
            }
            let bit = entry.offset.saturating_add(entry.data_bytes()).saturating_add(usize::from(block / 8));
            if let Some(byte) = self.arena.get_mut(bit) {
                *byte |= 1 << (block % 8);
            }
            entry.received = entry.received.saturating_add(1);
            fresh = true;
        }
        if let Some(slot) = self.entries.get_mut(index) {
            *slot = Some(entry);
        }
        if fresh { Stored::Blocks { first: data.first_block, count } } else { Stored::Repeat }
    }
}

/// A DATA record's blocks in order, whole ones then possibly the final,
/// shorter one: each block's index and bytes, or `None` for a block past the
/// sound's end or cut short, which ends the walk.
fn record_blocks<'d>(entry: &Entry, data: &Data<'d>) -> impl Iterator<Item = Option<(u16, &'d [u8])>> + use<'d> {
    let entry = *entry;
    let mut rest = data.blocks;
    let mut block = data.first_block;
    core::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let parts = (block < entry.blocks).then(|| rest.split_at_checked(entry.block_length(block))).flatten();
        let Some((bytes, after)) = parts else {
            rest = &[];
            return Some(None);
        };
        rest = after;
        let index = block;
        block = block.saturating_add(1);
        Some(Some((index, bytes)))
    })
}

#[cfg(test)]
mod tests;
