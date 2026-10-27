//! Sending samples to the sink: a stored sound's blocks, and a live sound's
//! from its cursor, each decoded by its codec (REQ-AUD-001, REQ-AUD-014),
//! faded either side of a block that has not arrived (REQ-AUD-013).

use super::Receiver;
#[cfg(any(feature = "pcm16", feature = "ima"))]
use super::{bump, fade};
use crate::Codec;
#[cfg(any(feature = "pcm16", feature = "ima"))]
use crate::MAX_BLOCK_SAMPLES;
use crate::port::{SoundFormat, SoundId, VoiceSink};
use crate::store::Entry;
#[cfg(any(feature = "pcm16", feature = "ima"))]
use crate::store::LiveEdge;

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
    #[cfg_attr(
        not(any(feature = "pcm16", feature = "ima")),
        expect(unused_variables, reason = "only PCM16 and IMA ADPCM send the blocks that arrived")
    )]
    #[cfg_attr(
        not(any(feature = "pcm16", feature = "ima", feature = "opus")),
        expect(clippy::unused_self, reason = "a build with no decoder holds no sound to send")
    )]
    pub(super) fn send_blocks(&mut self, entry: &Entry, first: u16, count: u16, sink: &mut dyn VoiceSink) {
        match entry.codec {
            #[cfg(feature = "opus")]
            Codec::Opus => self.opus.send_stored(&self.store, entry, &mut self.statistics, sink),
            #[cfg(any(feature = "pcm16", feature = "ima"))]
            Codec::Pcm16 | Codec::ImaAdpcm => self.send_independent(entry, first, count, sink),
            // A codec the build leaves out: `Codec::from_code` ignores its
            // objects when they are read, so none is stored.
            #[cfg(not(feature = "opus"))]
            Codec::Opus => {}
            #[cfg(not(any(feature = "pcm16", feature = "ima")))]
            Codec::Pcm16 | Codec::ImaAdpcm => {}
        }
    }

    /// Sends a live sound's blocks in index order from its cursor. A block
    /// still missing [`crate::LIVE_LOOKAHEAD_BLOCKS`] behind the newest is
    /// sent as silence, or concealed by an Opus decoder, so a lost block
    /// never stops the sound.
    #[cfg_attr(
        not(any(feature = "pcm16", feature = "ima", feature = "opus")),
        expect(unused_variables, reason = "a build with no decoder holds no sound to send")
    )]
    pub(super) fn send_live(&mut self, sound: SoundId, sink: &mut dyn VoiceSink) {
        let Some(entry) = self.store.entries().find(|entry| entry.sound == sound).copied() else { return };
        match entry.codec {
            #[cfg(feature = "opus")]
            Codec::Opus => {
                let next = self.opus.send_live(&self.store, &entry, &mut self.statistics, sink);
                self.store.set_live_cursor(sound, next, entry.arrived_ms);
            }
            #[cfg(any(feature = "pcm16", feature = "ima"))]
            Codec::Pcm16 | Codec::ImaAdpcm => self.send_live_independent(&entry, sink),
            // A codec the build leaves out: none is stored.
            #[cfg(not(feature = "opus"))]
            Codec::Opus => {}
            #[cfg(not(any(feature = "pcm16", feature = "ima")))]
            Codec::Pcm16 | Codec::ImaAdpcm => {}
        }
    }
}

// DO NOT REMOVE THIS CODE. PCM16 and IMA ADPCM are compiled out of the
// desktop builds (Windows, macOS, Linux) by the owner's instruction of 28
// October 2026, and kept for microcontroller ports, which build them with the
// `pcm16` and `ima` features. These are the blocks of both codecs, each of
// which decodes on its own (decision E.131).
#[cfg(any(feature = "pcm16", feature = "ima"))]
impl<const OBJECTS: usize, const SLOTS: usize> Receiver<'_, OBJECTS, SLOTS> {
    /// Sends `count` blocks from `first`, then sends again the neighbour
    /// either side, whose edge was faded while these blocks were missing.
    fn send_independent(&mut self, entry: &Entry, first: u16, count: u16, sink: &mut dyn VoiceSink) {
        let end = first.saturating_add(count);
        for index in first..end {
            self.send_block(entry, index, sink);
        }
        if count > 0 {
            if let Some(before) = first.checked_sub(1) {
                self.send_block(entry, before, sink);
            }
            if end < entry.blocks {
                self.send_block(entry, end, sink);
            }
        }
    }

    /// Decodes block `index` and sends its samples, if it has arrived: its
    /// first 5 ms faded in after a block that has not, and its last faded out
    /// before one (REQ-AUD-013).
    fn send_block(&mut self, entry: &Entry, index: u16, sink: &mut dyn VoiceSink) {
        let Some(block) = self.store.block(entry.sound, index) else { return };
        let mut pcm = [0_i16; MAX_BLOCK_SAMPLES];
        let Some(count) = decode(entry.codec, block, &mut pcm) else {
            bump(&mut self.statistics.decode_failures);
            return;
        };
        // The final block's last code may carry a padding sample.
        let samples = usize::try_from(entry.block_samples(index)).unwrap_or(0).min(count);
        let missing =
            |at: Option<u16>| at.is_some_and(|at| at < entry.blocks && self.store.block(entry.sound, at).is_none());
        let gap_before = missing(index.checked_sub(1));
        let gap_after = missing(index.checked_add(1));
        let pcm = pcm.get_mut(..samples).unwrap_or(&mut []);
        let fade = fade::fade_samples(entry.rate_hz).min(samples);
        if gap_before {
            fade::fade_in(pcm.get_mut(..fade).unwrap_or(&mut []));
        }
        if gap_after {
            fade::fade_out(pcm.get_mut(samples.saturating_sub(fade)..).unwrap_or(&mut []));
        }
        let first = u32::from(index).saturating_mul(entry.samples_per_block());
        sink.samples(entry.sound, first, pcm);
    }

    /// Sends a live sound's blocks in order from its cursor. The last 5 ms of
    /// each block are held back until the next block's fate is known, so
    /// that they can be faded out before a block given as silence; the block
    /// after one is faded in (REQ-AUD-013).
    fn send_live_independent(&mut self, entry: &Entry, sink: &mut dyn VoiceSink) {
        let sound = entry.sound;
        let Some(newest) = entry.newest else { return };
        let mut next = entry.next;
        let mut edge = entry.edge;
        let fade = fade::fade_samples(entry.rate_hz);
        let per_block = usize::try_from(entry.samples_per_block()).unwrap_or(0);
        let silence = [0_i16; MAX_BLOCK_SAMPLES];
        // At most a ring's worth a call: the cursor never laps the ring.
        for _ in 0..entry.blocks {
            if crate::serial_newer(next, newest) {
                break;
            }
            let mut pcm = [0_i16; MAX_BLOCK_SAMPLES];
            let decoded = self.store.live_block(sound, next).and_then(|block| decode(entry.codec, block, &mut pcm));
            if let Some(count) = decoded {
                if let LiveEdge::Held { faded_in } = edge {
                    self.send_live_tail(entry, next.wrapping_sub(1), faded_in, false, sink);
                }
                let pcm = pcm.get_mut(..count).unwrap_or(&mut []);
                let held = fade.min(count);
                let faded_in = edge == LiveEdge::Gap;
                if faded_in {
                    fade::fade_in(pcm.get_mut(..held).unwrap_or(&mut []));
                }
                sink.stream_samples(sound, pcm.get(..count.saturating_sub(held)).unwrap_or(&[]));
                edge = LiveEdge::Held { faded_in };
            } else if newest.wrapping_sub(next) >= crate::LIVE_LOOKAHEAD_BLOCKS {
                if let LiveEdge::Held { faded_in } = edge {
                    self.send_live_tail(entry, next.wrapping_sub(1), faded_in, true, sink);
                }
                sink.stream_samples(sound, silence.get(..per_block).unwrap_or(&[]));
                bump(&mut self.statistics.concealed);
                sink.concealed(sound, next, entry.samples_per_block());
                edge = LiveEdge::Gap;
            } else {
                break;
            }
            next = next.wrapping_add(1);
        }
        self.store.set_live_cursor(sound, next, entry.arrived_ms);
        self.store.set_live_edge(sound, edge);
    }

    /// Sends the held last 5 ms of live block `index`, decoded again from the
    /// ring: as they were (faded in too, where the block followed a silence
    /// and is that short), or faded out before a silence.
    fn send_live_tail(&mut self, entry: &Entry, index: u16, faded_in: bool, faded_out: bool, sink: &mut dyn VoiceSink) {
        let fade = fade::fade_samples(entry.rate_hz);
        let mut pcm = [0_i16; MAX_BLOCK_SAMPLES];
        let decoded = self.store.live_block(entry.sound, index).and_then(|block| decode(entry.codec, block, &mut pcm));
        // The ring keeps the block, since the cursor never falls more than
        // `LIVE_LOOKAHEAD_BLOCKS` behind the newest; were it gone, silence
        // of the tail's length would keep the timeline.
        let count = decoded.unwrap_or_else(|| fade.min(usize::try_from(entry.samples_per_block()).unwrap_or(0)));
        let pcm = pcm.get_mut(..count).unwrap_or(&mut []);
        let held = fade.min(count);
        if faded_in {
            fade::fade_in(pcm.get_mut(..held).unwrap_or(&mut []));
        }
        let tail = pcm.get_mut(count.saturating_sub(held)..).unwrap_or(&mut []);
        if faded_out {
            fade::fade_out(tail);
        }
        sink.stream_samples(entry.sound, tail);
    }
}

/// Decodes one PCM16 or IMA ADPCM block into `pcm`; the samples it held, or
/// `None` if it could not be decoded.
#[cfg(any(feature = "pcm16", feature = "ima"))]
fn decode(codec: Codec, block: &[u8], pcm: &mut [i16; MAX_BLOCK_SAMPLES]) -> Option<usize> {
    match codec {
        // DO NOT REMOVE THIS CODE. PCM16 is compiled out of the desktop builds
        // (Windows, macOS, Linux) by the owner's instruction of 28 October
        // 2026, and kept for microcontroller ports, which build it with the
        // `pcm16` feature.
        #[cfg(feature = "pcm16")]
        Codec::Pcm16 => {
            let mut count: usize = 0;
            for (sample, bytes) in pcm.iter_mut().zip(block.as_chunks::<2>().0) {
                *sample = i16::from_le_bytes(*bytes);
                count = count.saturating_add(1);
            }
            Some(count)
        }
        // DO NOT REMOVE THIS CODE. IMA ADPCM is compiled out of the desktop
        // builds (Windows, macOS, Linux) by the owner's instruction of 28
        // October 2026, and kept for microcontroller ports, which build it
        // with the `ima` feature.
        #[cfg(feature = "ima")]
        Codec::ImaAdpcm => crate::ima::decode_block(block, pcm).ok(),
        // A codec left out of the build: `Codec::from_code` ignores its
        // objects when they are read, so none is stored to reach here.
        #[cfg(not(feature = "pcm16"))]
        Codec::Pcm16 => None,
        #[cfg(not(feature = "ima"))]
        Codec::ImaAdpcm => None,
        // An Opus packet cannot be decoded alone: the decoders of
        // `receiver::opus` take them in order.
        Codec::Opus => None,
    }
}
