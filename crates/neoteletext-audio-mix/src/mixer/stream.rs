//! Live sounds in the mixer (REQ-AUD-014): their rings filled as samples
//! arrive, and each voice's rate trimmed to keep the target buffered.

use super::Mixer;
use crate::sound::{PPM, STREAM_RING_MS, STREAM_TRIM_PPM, Sound, SoundKey};
use crate::voice::set_step;

impl Mixer {
    /// Makes room for a live sound (REQ-AUD-014): its samples arrive in
    /// order through [`Mixer::stream_samples`], without end. A voice playing
    /// it waits until [`STREAM_TARGET_MS`](crate::STREAM_TARGET_MS) are buffered, then trims its rate
    /// to keep that much buffered, following the sender's clock.
    pub fn stream(&mut self, key: SoundKey, rate_hz: u32) {
        self.collect();
        let rate_hz = rate_hz.max(1);
        let ring = usize::try_from(rate_hz.saturating_mul(STREAM_RING_MS) / 1000).unwrap_or(0).max(1);
        self.sounds.retain(|sound| sound.key != key);
        self.sounds.push(Sound {
            key,
            rate_hz,
            samples: alloc::vec![0; ring],
            forgotten: false,
            stream: true,
            written: 0,
        });
    }

    /// The next samples of a live sound. If a voice has fallen so far behind
    /// that the ring would overwrite what it has yet to play, it skips ahead
    /// to the target.
    pub fn stream_samples(&mut self, key: SoundKey, pcm: &[i16]) {
        let Some(sound) = self.sounds.iter_mut().find(|sound| sound.key == key && sound.stream) else { return };
        let length = i64::try_from(sound.samples.len()).unwrap_or(i64::MAX);
        for &value in pcm {
            let slot = usize::try_from(sound.written.checked_rem(length).unwrap_or(0)).unwrap_or(0);
            if let Some(place) = sound.samples.get_mut(slot) {
                *place = value;
            }
            sound.written = sound.written.saturating_add(1);
        }
        let (written, target) = (sound.written, sound.target());
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
                let Some(sound) = sounds.iter().find(|sound| sound.key == playing.key && sound.stream) else {
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
