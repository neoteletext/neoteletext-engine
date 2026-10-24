//! Live sounds in the mixer (REQ-AUD-014): their rings filled as samples
//! arrive, and each voice's rate trimmed to keep the target buffered.

use super::{Mixer, set_step};
use crate::sound::{PPM, STREAM_RING_MS, STREAM_TRIM_PPM, Sound, SoundKey};
use alloc::vec::Vec;

impl Mixer {
    /// Makes room for a live sound (REQ-AUD-014): its samples arrive in
    /// order through [`Mixer::stream_samples`], without end. A voice playing
    /// it waits until [`STREAM_TARGET_MS`](crate::STREAM_TARGET_MS) are buffered, then trims its rate
    /// to keep that much buffered, following the sender's clock. It
    /// allocates the ring; a caller that holds a lock the audio callback
    /// takes allocates it beforehand and calls [`Mixer::stream_with`].
    pub fn stream(&mut self, key: SoundKey, rate_hz: u32) {
        let ring = alloc::vec![0; Self::stream_ring(rate_hz)];
        // The ring has the length it must have, so it is never refused.
        let made = self.stream_with(key, rate_hz, ring);
        debug_assert!(made.is_ok(), "a ring of stream_ring's length is taken");
    }

    /// Makes room for a live sound, as [`Mixer::stream`] does, whose ring is
    /// `ring`, the buffer as given and never copied; what it holds before
    /// samples arrive is never played. A ring whose length is not
    /// [`Mixer::stream_ring`] of `rate_hz` is refused and handed back, and
    /// the mixer is unchanged.
    ///
    /// It exists so that a port whose audio callback takes a lock around the
    /// mixer can allocate the ring before taking that lock: allocating under
    /// it would hold the callback up for as long as the allocator takes.
    /// What it may still allocate is a slot in the mixer's small table of
    /// sounds, and only when the table grows.
    ///
    /// # Errors
    ///
    /// The ring, when its length is not [`Mixer::stream_ring`] of `rate_hz`.
    pub fn stream_with(&mut self, key: SoundKey, rate_hz: u32, ring: Vec<i16>) -> Result<(), Vec<i16>> {
        if ring.len() != Self::stream_ring(rate_hz) {
            return Err(ring);
        }
        self.collect();
        self.sounds.retain(|sound| sound.key() != key);
        self.sounds.push(Sound::live(key, rate_hz.max(1), ring));
        Ok(())
    }

    /// The samples a live sound's ring holds at `rate_hz`: the samples of
    /// [`STREAM_RING_MS`](crate::STREAM_RING_MS), and at least one. A rate of
    /// zero is taken as one, as [`Mixer::stream`] takes it.
    #[must_use]
    pub fn stream_ring(rate_hz: u32) -> usize {
        usize::try_from(rate_hz.max(1).saturating_mul(STREAM_RING_MS) / 1000).unwrap_or(0).max(1)
    }

    /// The next samples of a live sound. If a voice has fallen so far behind
    /// that the ring would overwrite what it has yet to play, it skips ahead
    /// to the target.
    pub fn stream_samples(&mut self, key: SoundKey, pcm: &[i16]) {
        let Some(sound) = self.sounds.iter_mut().find(|sound| sound.key() == key && sound.is_live()) else { return };
        let length = sound.length();
        sound.write_live(pcm);
        let (written, target) = (sound.written(), sound.target());
        for voice in &mut self.voices {
            for playing in [&mut voice.playing, &mut voice.outgoing].into_iter().flatten() {
                if playing.key == key && written.saturating_sub(playing.whole) > length.saturating_sub(target) {
                    playing.whole = written.saturating_sub(target);
                }
            }
        }
    }

    /// Trims each live sound's step from its nominal one in proportion to
    /// how far its buffer is from the target: the full [`STREAM_TRIM_PPM`]
    /// when it is empty or twice full. A sender whose clock runs fast fills
    /// the buffer and is played faster, and the reverse.
    pub(super) fn trim_streams(&mut self) {
        let sounds = &self.sounds;
        for voice in &mut self.voices {
            for playing in [&mut voice.playing, &mut voice.outgoing].into_iter().flatten() {
                let Some(sound) = sounds.iter().find(|sound| sound.key() == playing.key && sound.is_live()) else {
                    continue;
                };
                let target = sound.target().max(1);
                let error = sound.ahead(playing.whole).saturating_sub(target);
                let ppm = error
                    .saturating_mul(STREAM_TRIM_PPM)
                    .checked_div(target)
                    .unwrap_or(0)
                    .clamp(-STREAM_TRIM_PPM, STREAM_TRIM_PPM);
                let nominal = i128::from(playing.nominal);
                let trimmed = nominal
                    .saturating_add(nominal.saturating_mul(i128::from(ppm)).checked_div(i128::from(PPM)).unwrap_or(0));
                set_step(playing, u64::try_from(trimmed).unwrap_or(playing.nominal));
            }
        }
    }
}
