//! The mixer: four voices mixed to mono at one output rate, with ducking
//! and a clamp after the sum (REQ-AUD-015, REQ-AUD-019).

use crate::ramp::{MIN_RAMP_MS, Ramp, UNITY, level, scale};
use crate::sound::{Sound, SoundKey};
use crate::voice::{Voice, advance, begin, interpolate};
use alloc::vec::Vec;

mod stream;

/// Voices, one for each channel of the audio profile.
pub const VOICES: usize = 4;

/// How a voice should start a sound.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Start {
    /// The sound.
    pub key: SoundKey,
    /// 0 to 100 per cent.
    pub gain: u8,
    /// The ramp in, and out for what it replaces.
    pub fade_ms: u16,
    /// Repeat until stopped.
    pub looped: bool,
    /// The other voices' level while this plays, in per cent.
    pub duck_percent: u8,
    /// Wait for the voice's current sound to end.
    pub after_current: bool,
}

/// Four voices mixed to mono at one output rate.
pub struct Mixer {
    rate_hz: u32,
    voices: [Voice; VOICES],
    sounds: Vec<Sound>,
    master: Ramp,
    master_percent: u8,
    muted: bool,
}

impl Mixer {
    /// A silent mixer rendering at `rate_hz`, at full master volume.
    #[must_use]
    pub fn new(rate_hz: u32) -> Self {
        Self {
            rate_hz: rate_hz.max(1),
            voices: [Voice::default(); VOICES],
            sounds: Vec::new(),
            master: Ramp::at(UNITY),
            master_percent: 100,
            muted: false,
        }
    }

    /// The output rate.
    #[must_use]
    pub fn rate(&self) -> u32 {
        self.rate_hz
    }

    /// Makes room for a sound's samples, silent until they arrive. A sound
    /// of the same key is replaced.
    pub fn define(&mut self, key: SoundKey, rate_hz: u32, samples: usize) {
        self.collect();
        self.sounds.retain(|sound| sound.key != key);
        self.sounds.push(Sound {
            key,
            rate_hz: rate_hz.max(1),
            samples: alloc::vec![0; samples],
            forgotten: false,
            stream: false,
            written: 0,
        });
    }

    /// Copies samples into a sound from sample `first`; samples beyond its
    /// end are dropped.
    pub fn samples(&mut self, key: SoundKey, first: usize, pcm: &[i16]) {
        let Some(sound) = self.sounds.iter_mut().find(|sound| sound.key == key) else { return };
        let end = first.saturating_add(pcm.len()).min(sound.samples.len());
        if let (Some(target), Some(source)) = (sound.samples.get_mut(first..end), pcm.get(..end.saturating_sub(first)))
        {
            target.copy_from_slice(source);
        }
    }

    /// Forgets a sound once no voice plays it.
    pub fn forget(&mut self, key: SoundKey) {
        for sound in self.sounds.iter_mut().filter(|sound| sound.key == key) {
            sound.forgotten = true;
        }
        self.collect();
    }

    /// Forgets every sound and silences every voice at once.
    pub fn clear(&mut self) {
        self.voices = [Voice::default(); VOICES];
        self.sounds.clear();
    }

    /// Starts a sound on a voice, ramping out what it played; or, with
    /// `after_current`, queues it for when the current sound ends.
    pub fn start(&mut self, voice: usize, start: Start) {
        let rate = self.rate_hz;
        let Some(sound) = self.sounds.iter().find(|sound| sound.key == start.key) else { return };
        let source = (sound.rate_hz, sound.stream.then_some(sound.written));
        let Some(slot) = self.voices.get_mut(voice) else { return };
        if start.after_current && slot.playing.is_some_and(|playing| !playing.stopping) {
            slot.pending = Some(start);
            return;
        }
        slot.pending = None;
        if let Some(mut old) = slot.playing.take() {
            old.gain.to(0, u32::from(start.fade_ms), rate);
            old.stopping = true;
            slot.outgoing = Some(old);
        }
        slot.playing = Some(begin(&start, source, rate));
    }

    /// Ramps a voice to silence and stops it, dropping its waiting sound.
    pub fn stop(&mut self, voice: usize, fade_ms: u16) {
        let rate = self.rate_hz;
        let Some(slot) = self.voices.get_mut(voice) else { return };
        slot.pending = None;
        if let Some(playing) = slot.playing.as_mut() {
            playing.gain.to(0, u32::from(fade_ms), rate);
            playing.stopping = true;
        }
    }

    /// Ramps a voice to a new gain.
    pub fn gain(&mut self, voice: usize, percent: u8, fade_ms: u16) {
        let rate = self.rate_hz;
        if let Some(playing) = self.voices.get_mut(voice).and_then(|slot| slot.playing.as_mut())
            && !playing.stopping
        {
            playing.gain.to(level(percent), u32::from(fade_ms), rate);
        }
    }

    /// The user's master volume, 0 to 100 per cent, and mute.
    pub fn set_master(&mut self, percent: u8, muted: bool) {
        self.master_percent = percent.min(100);
        self.muted = muted;
        let target = if muted { 0 } else { level(self.master_percent) };
        self.master.to(target, MIN_RAMP_MS, self.rate_hz);
    }

    /// Whether any voice is sounding or about to.
    #[must_use]
    pub fn active(&self) -> bool {
        self.voices.iter().any(|voice| voice.playing.is_some() || voice.outgoing.is_some() || voice.pending.is_some())
    }

    /// Renders mono samples at the output rate. It never allocates or frees.
    pub fn render(&mut self, out: &mut [i16]) {
        self.trim_streams();
        for sample in out.iter_mut() {
            self.update_ducks();
            let mut sum: i32 = 0;
            for index in 0..VOICES {
                sum = sum.saturating_add(self.voice_sample(index));
            }
            let master = self.master.advance();
            let mixed = scale(sum, master);
            *sample = i16::try_from(mixed.clamp(i32::from(i16::MIN), i32::from(i16::MAX))).unwrap_or(0);
        }
    }

    /// One output sample of one voice, advancing it; a finished sound
    /// gives way to the waiting one.
    fn voice_sample(&mut self, index: usize) -> i32 {
        let rate = self.rate_hz;
        let Some(voice) = self.voices.get(index).copied() else { return 0 };
        let duck = voice.duck.map_or(UNITY, |ramp| ramp.level());
        let mut total: i32 = 0;
        let mut voice = voice;
        for slot in [&mut voice.playing, &mut voice.outgoing] {
            let Some(mut playing) = *slot else { continue };
            let gain = playing.gain.advance();
            let Some(sound) = self.sounds.iter().find(|sound| sound.key == playing.key) else {
                *slot = None;
                continue;
            };
            if sound.stream {
                if !playing.primed {
                    playing.primed = sound.ahead(playing.whole) >= sound.target();
                    if sound.forgotten {
                        // It will get no more samples: what it has plays out.
                        playing.primed = true;
                    }
                }
                if playing.primed {
                    let value = interpolate(sound, playing.whole, playing.fraction);
                    total = total.saturating_add(scale(scale(value, gain), duck));
                    advance(&mut playing);
                    if sound.ahead(playing.whole) <= 0 {
                        if sound.forgotten {
                            playing.stopping = true;
                            playing.gain = Ramp::at(0);
                        } else {
                            // Starved: wait, silent, for the target again.
                            playing.primed = false;
                        }
                    }
                }
                *slot = if playing.stopping && playing.gain.settled_at_zero() { None } else { Some(playing) };
                continue;
            }
            let value = interpolate(sound, playing.whole, playing.fraction);
            total = total.saturating_add(scale(scale(value, gain), duck));
            let length = i64::try_from(sound.samples.len()).unwrap_or(i64::MAX);
            advance(&mut playing);
            if playing.whole >= length {
                if playing.looped && length > 0 {
                    playing.whole = playing.whole.checked_rem(length).unwrap_or(0);
                } else {
                    playing.stopping = true;
                    playing.gain = Ramp::at(0);
                }
            }
            *slot = if playing.stopping && playing.gain.settled_at_zero() { None } else { Some(playing) };
        }
        if voice.playing.is_none()
            && let Some(next) = voice.pending.take()
            && let Some(source) = self
                .sounds
                .iter()
                .find(|sound| sound.key == next.key)
                .map(|sound| (sound.rate_hz, sound.stream.then_some(sound.written)))
        {
            voice.playing = Some(begin(&next, source, rate));
        }
        if let Some(place) = self.voices.get_mut(index) {
            *place = voice;
        }
        total
    }

    /// Sets each voice's duck ramp toward the lowest level any other
    /// sounding voice asks for.
    fn update_ducks(&mut self) {
        let rate = self.rate_hz;
        let asks: [Option<(u8, u16)>; VOICES] = core::array::from_fn(|index| {
            self.voices
                .get(index)
                .and_then(|voice| voice.playing)
                .filter(|playing| !playing.stopping && playing.duck_percent < 100)
                .map(|playing| (playing.duck_percent, playing.fade_ms))
        });
        for (index, voice) in self.voices.iter_mut().enumerate() {
            let strongest = asks
                .iter()
                .enumerate()
                .filter(|&(other, _)| other != index)
                .filter_map(|(_, ask)| *ask)
                .min_by_key(|&(percent, _)| percent);
            let (target, fade) =
                strongest.map_or((UNITY, MIN_RAMP_MS), |(percent, fade)| (level(percent), u32::from(fade)));
            let mut ramp = voice.duck.unwrap_or(Ramp::at(UNITY));
            if ramp.target() != target {
                ramp.to(target, fade, rate);
            }
            ramp.advance();
            voice.duck = Some(ramp);
        }
    }

    /// Drops forgotten sounds no voice plays.
    fn collect(&mut self) {
        let voices = self.voices;
        self.sounds.retain(|sound| {
            !sound.forgotten
                || voices.iter().any(|voice| {
                    [voice.playing, voice.outgoing].iter().flatten().any(|playing| playing.key == sound.key)
                        || voice.pending.is_some_and(|start| start.key == sound.key)
                })
        });
    }
}

#[cfg(test)]
mod tests;
