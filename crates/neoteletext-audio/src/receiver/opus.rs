//! Opus sounds decoded in order (decision E.124). A PCM or IMA ADPCM block
//! decodes on its own; an Opus packet's frame depends on the frames before
//! it. So the receiver keeps [`crate::OPUS_DECODERS`] decoders, each
//! following one sound from a block on: a stored sound's blocks are decoded
//! from the first, as far as they have arrived without a gap, and its
//! decoder is freed when the last is decoded; a live sound keeps its
//! decoder, which conceals a block that never arrived.

use neoteletext_opus::{Decoder, FRAME_SAMPLES};

use crate::port::{SoundId, VoiceSink};
use crate::store::{Entry, Store};

use super::Statistics;

/// One decoder and the sound it follows.
struct Lane {
    /// The sound, or `None` while free.
    sound: Option<SoundId>,
    /// The next block it decodes.
    next: u16,
    decoder: Decoder,
}

/// The receiver's Opus decoders.
pub(super) struct Lanes {
    lanes: [Lane; crate::OPUS_DECODERS],
}

impl Lanes {
    pub(super) fn new() -> Self {
        Self { lanes: core::array::from_fn(|_| Lane { sound: None, next: 0, decoder: Decoder::new() }) }
    }

    /// Frees every decoder: the sink has dropped every sound.
    pub(super) fn clear(&mut self) {
        for lane in &mut self.lanes {
            lane.sound = None;
        }
    }

    /// Frees the decoder that follows `sound`, if one does.
    pub(super) fn release(&mut self, sound: SoundId) {
        for lane in self.lanes.iter_mut().filter(|lane| lane.sound == Some(sound)) {
            lane.sound = None;
        }
    }

    /// The decoder that follows `sound`; or a free one, or one whose sound
    /// the store no longer holds, started at block `first`; or none, while
    /// every decoder is busy.
    fn lane<const OBJECTS: usize>(
        &mut self,
        store: &Store<'_, OBJECTS>,
        sound: SoundId,
        first: u16,
    ) -> Option<&mut Lane> {
        let found = self.lanes.iter().position(|lane| lane.sound == Some(sound)).or_else(|| {
            self.lanes
                .iter()
                .position(|lane| lane.sound.is_none_or(|held| store.entries().all(|entry| entry.sound != held)))
        })?;
        let lane = self.lanes.get_mut(found)?;
        if lane.sound != Some(sound) {
            lane.sound = Some(sound);
            lane.next = first;
            lane.decoder.reset();
        }
        Some(lane)
    }

    /// Decodes a stored sound's blocks from where its decoder stands, as far
    /// as they have arrived without a gap, and gives the sink their samples.
    pub(super) fn send_stored<const OBJECTS: usize>(
        &mut self,
        store: &Store<'_, OBJECTS>,
        entry: &Entry,
        statistics: &mut Statistics,
        sink: &mut dyn VoiceSink,
    ) {
        let Some(lane) = self.lane(store, entry.sound, 0) else { return };
        let mut pcm = [0_i16; FRAME_SAMPLES];
        while lane.next < entry.blocks {
            let Some(packet) = store.block(entry.sound, lane.next) else { break };
            if lane.decoder.decode(packet, &mut pcm).is_err() {
                // A refused packet keeps the timeline: its frame is concealed.
                statistics.decode_failures = statistics.decode_failures.saturating_add(1);
                lane.decoder.conceal(&mut pcm);
            }
            let samples = usize::try_from(entry.block_samples(lane.next)).unwrap_or(0);
            let first = u32::from(lane.next).saturating_mul(entry.samples_per_block());
            sink.samples(entry.sound, first, pcm.get(..samples).unwrap_or(&[]));
            lane.next = lane.next.saturating_add(1);
        }
        if lane.next >= entry.blocks {
            lane.sound = None;
        }
    }

    /// Decodes a live sound's blocks in index order from its cursor: a block
    /// still missing [`crate::LIVE_LOOKAHEAD_BLOCKS`] behind the newest is
    /// concealed, so a lost packet never stops the sound. Answers the next
    /// block, for the store's cursor.
    pub(super) fn send_live<const OBJECTS: usize>(
        &mut self,
        store: &Store<'_, OBJECTS>,
        entry: &Entry,
        statistics: &mut Statistics,
        sink: &mut dyn VoiceSink,
    ) -> u16 {
        let mut next = entry.next;
        let Some(newest) = entry.newest else { return next };
        let Some(lane) = self.lane(store, entry.sound, next) else { return next };
        let mut pcm = [0_i16; FRAME_SAMPLES];
        // At most a ring's worth a call: the cursor never laps the ring.
        for _ in 0..entry.blocks {
            if crate::serial_newer(next, newest) {
                break;
            }
            match store.live_block(entry.sound, next) {
                Some(packet) => {
                    if lane.decoder.decode(packet, &mut pcm).is_err() {
                        statistics.decode_failures = statistics.decode_failures.saturating_add(1);
                        lane.decoder.conceal(&mut pcm);
                    }
                }
                None if newest.wrapping_sub(next) >= crate::LIVE_LOOKAHEAD_BLOCKS => {
                    lane.decoder.conceal(&mut pcm);
                    statistics.concealed = statistics.concealed.saturating_add(1);
                }
                None => break,
            }
            sink.stream_samples(entry.sound, &pcm);
            next = next.wrapping_add(1);
        }
        lane.next = next;
        next
    }
}
