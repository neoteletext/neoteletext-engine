//! The arena: defining a sound in the space the port gave, evicting the
//! oldest sounds nothing needs to make room (REQ-AUD-009).

use super::{Defined, Entry, Store};
use crate::port::SoundId;
use crate::record::Object;

impl<const OBJECTS: usize> Store<'_, OBJECTS> {
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
        let Some((offset, evicted)) = self.make_space(size, busy) else { return Defined::ResourceLimit };
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

    /// Finds `size` free bytes, evicting the oldest sounds nothing needs
    /// until they are found. Returns their offset and how many sounds were
    /// evicted, or `None` when every sound left is needed.
    fn make_space(&mut self, size: usize, busy: &dyn Fn(SoundId) -> bool) -> Option<(usize, usize)> {
        let mut evicted: usize = 0;
        loop {
            if let Some(offset) = self.find_space(size) {
                return Some((offset, evicted));
            }
            // Evict the oldest sound nothing needs.
            let victim = self
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| entry.filter(|entry| !busy(entry.sound)).map(|entry| (index, entry.stamp)))
                .min_by_key(|&(_, stamp)| stamp)
                .map(|(index, _)| index)?;
            if let Some(slot) = self.entries.get_mut(victim) {
                *slot = None;
            }
            evicted = evicted.saturating_add(1);
        }
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
}
