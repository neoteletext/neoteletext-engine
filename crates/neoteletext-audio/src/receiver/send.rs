//! Sending samples to the sink: a stored sound's blocks, and a live sound's
//! from its cursor, each decoded by its codec (REQ-AUD-001, REQ-AUD-014).

use super::{Receiver, bump};
use crate::port::{SoundFormat, SoundId, VoiceSink};
use crate::store::Entry;
use crate::{Codec, MAX_BLOCK_SAMPLES};

impl<const OBJECTS: usize, const SLOTS: usize> Receiver<'_, OBJECTS, SLOTS> {
    /// Defines a sound at the sink and sends every block received so far.
    pub(super) fn send_sound(&mut self, entry: &Entry, sink: &mut dyn VoiceSink) {
        sink.define(entry.sound, SoundFormat { rate_hz: entry.rate_hz, samples: entry.samples });
        self.store.set_at_sink(entry.sound, true);
        // Sent whole: an Opus sound is decoded again from its first block.
        #[cfg(feature = "opus")]
        self.opus.release(entry.sound);
        self.send_blocks(entry, 0, entry.blocks, sink);
    }

    /// Sends `count` blocks from `first` that have just arrived: each decoded
    /// alone, or an Opus sound's in order from where its decoder stands.
    pub(super) fn send_blocks(&mut self, entry: &Entry, first: u16, count: u16, sink: &mut dyn VoiceSink) {
        #[cfg(feature = "opus")]
        if entry.codec == Codec::Opus {
            self.opus.send_stored(&self.store, entry, &mut self.statistics, sink);
            return;
        }
        for index in first..first.saturating_add(count) {
            self.send_block(entry, index, sink);
        }
    }

    /// Sends a live sound's blocks in index order from its cursor. A block
    /// still missing [`crate::LIVE_LOOKAHEAD_BLOCKS`] behind the newest is
    /// sent as silence, so a lost block never stops the sound.
    pub(super) fn send_live(&mut self, sound: SoundId, sink: &mut dyn VoiceSink) {
        let Some(entry) = self.store.entries().find(|entry| entry.sound == sound).copied() else { return };
        #[cfg(feature = "opus")]
        if entry.codec == Codec::Opus {
            let next = self.opus.send_live(&self.store, &entry, &mut self.statistics, sink);
            self.store.set_live_cursor(sound, next, entry.arrived_ms);
            return;
        }
        let Some(newest) = entry.newest else { return };
        let mut next = entry.next;
        let silence = [0_i16; MAX_BLOCK_SAMPLES];
        // At most a ring's worth a call: the cursor never laps the ring.
        for _ in 0..entry.blocks {
            if crate::serial_newer(next, newest) {
                break;
            }
            let mut pcm = [0_i16; MAX_BLOCK_SAMPLES];
            let decoded = self.store.live_block(sound, next).and_then(|block| decode(entry.codec, block, &mut pcm));
            if let Some(count) = decoded {
                sink.stream_samples(sound, pcm.get(..count).unwrap_or(&[]));
            } else if newest.wrapping_sub(next) >= crate::LIVE_LOOKAHEAD_BLOCKS {
                let count = usize::try_from(entry.samples_per_block()).unwrap_or(0);
                sink.stream_samples(sound, silence.get(..count).unwrap_or(&[]));
                bump(&mut self.statistics.concealed);
            } else {
                break;
            }
            next = next.wrapping_add(1);
        }
        self.store.set_live_cursor(sound, next, entry.arrived_ms);
    }

    /// Decodes block `index` and sends its samples, if it has arrived.
    pub(super) fn send_block(&mut self, entry: &Entry, index: u16, sink: &mut dyn VoiceSink) {
        let Some(block) = self.store.block(entry.sound, index) else { return };
        let mut pcm = [0_i16; MAX_BLOCK_SAMPLES];
        let decoded = decode(entry.codec, block, &mut pcm);
        let Some(count) = decoded else {
            bump(&mut self.statistics.decode_failures);
            return;
        };
        // The final block's last code may carry a padding sample.
        let samples = usize::try_from(entry.block_samples(index)).unwrap_or(0).min(count);
        let first = u32::from(index).saturating_mul(entry.samples_per_block());
        sink.samples(entry.sound, first, pcm.get(..samples).unwrap_or(&[]));
    }
}

/// Decodes one block into `pcm`; the samples it held, or `None` if it
/// could not be decoded.
fn decode(codec: Codec, block: &[u8], pcm: &mut [i16; MAX_BLOCK_SAMPLES]) -> Option<usize> {
    match codec {
        Codec::Pcm16 => {
            let mut count: usize = 0;
            for (sample, bytes) in pcm.iter_mut().zip(block.as_chunks::<2>().0) {
                *sample = i16::from_le_bytes(*bytes);
                count = count.saturating_add(1);
            }
            Some(count)
        }
        #[cfg(feature = "ima")]
        Codec::ImaAdpcm => crate::ima::decode_block(block, pcm).ok(),
        #[cfg(not(feature = "ima"))]
        Codec::ImaAdpcm => None,
        // An Opus packet cannot be decoded alone: the decoders of
        // `receiver::opus` take them in order.
        Codec::Opus => None,
    }
}
