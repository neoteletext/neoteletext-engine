//! The mixer: four voices mixed to mono at one output rate, with ducking
//! and a clamp after the sum (REQ-AUD-015, REQ-AUD-019). A voice and the
//! sound on it: the source position in 32.32 fixed point, its step, and the
//! windowed-sinc interpolation between samples ([`sinc`](crate::sinc)).

use crate::ramp::{MIN_RAMP_MS, Ramp, UNITY, level, scale};
use crate::sinc::{FILTER, ONE, PHASES, TAPS_BEFORE};
use crate::sound::{STREAM_TARGET_MS, Sound, SoundKey};
use alloc::vec::Vec;

mod stream;

/// Voices, one for each channel of the audio profile.
pub const VOICES: usize = 4;

/// Fraction bits of a source position.
const FRACTION_BITS: u32 = 32;
/// Shift from a position's fraction to a filter phase.
const PHASE_SHIFT: u32 = FRACTION_BITS - PHASES.trailing_zeros();

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

/// A sound on a voice.
#[derive(Clone, Copy, Debug)]
struct Playing {
    key: SoundKey,
    /// The source position: whole samples and a 32-bit fraction.
    whole: i64,
    fraction: u32,
    /// The source step per output sample, in the same form.
    step_whole: i64,
    step_fraction: u32,
    /// The nominal step, whole and fraction in one: a live sound's step is
    /// trimmed from it to follow the sender's clock.
    nominal: u64,
    /// A live sound's voice waits, silent, until the target is buffered.
    primed: bool,
    looped: bool,
    gain: Ramp,
    /// Other voices' level while this plays, in per cent.
    duck_percent: u8,
    fade_ms: u16,
    /// Stopping: it ends when its gain reaches zero.
    stopping: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    playing: Option<Playing>,
    /// The sound it replaced, fading out.
    outgoing: Option<Playing>,
    /// One sound waiting for the current one to end.
    pending: Option<Start>,
    duck: Option<Ramp>,
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
        self.sounds.retain(|sound| sound.key() != key);
        self.sounds.push(Sound::new(key, rate_hz.max(1), samples));
    }

    /// Copies samples into a sound from sample `first`; samples beyond its
    /// end are dropped.
    pub fn samples(&mut self, key: SoundKey, first: usize, pcm: &[i16]) {
        let Some(sound) = self.sounds.iter_mut().find(|sound| sound.key() == key) else { return };
        sound.copy_in(first, pcm);
    }

    /// Forgets a sound once no voice plays it.
    pub fn forget(&mut self, key: SoundKey) {
        for sound in self.sounds.iter_mut().filter(|sound| sound.key() == key) {
            sound.forget();
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
        let Some(sound) = self.sounds.iter().find(|sound| sound.key() == start.key) else { return };
        let source = sound.source();
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
            let Some(sound) = self.sounds.iter().find(|sound| sound.key() == playing.key) else {
                *slot = None;
                continue;
            };
            if sound.is_live() {
                if !playing.primed {
                    playing.primed = sound.ahead(playing.whole) >= sound.target();
                    if sound.is_forgotten() {
                        // It will get no more samples: what it has plays out.
                        playing.primed = true;
                    }
                }
                if playing.primed {
                    let value = interpolate(sound, playing.whole, playing.fraction);
                    total = total.saturating_add(scale(scale(value, gain), duck));
                    advance(&mut playing);
                    if sound.ahead(playing.whole) <= 0 {
                        if sound.is_forgotten() {
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
            let length = sound.length();
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
            && let Some(source) = self.sounds.iter().find(|sound| sound.key() == next.key).map(Sound::source)
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
            !sound.is_forgotten()
                || voices.iter().any(|voice| {
                    [voice.playing, voice.outgoing].iter().flatten().any(|playing| playing.key == sound.key())
                        || voice.pending.is_some_and(|start| start.key == sound.key())
                })
        });
    }
}

/// A sound starting on a voice, its gain ramping in. `source` is the
/// sound's rate and, for a live sound, the samples written so far: its
/// voice joins [`STREAM_TARGET_MS`] behind the newest.
fn begin(start: &Start, source: (u32, Option<i64>), output_rate: u32) -> Playing {
    let (source_rate, written) = source;
    // The step is source rate / output rate, in 32.32 fixed point.
    let nominal = (u64::from(source_rate) << FRACTION_BITS).checked_div(u64::from(output_rate)).unwrap_or(0);
    let mut gain = Ramp::at(0);
    gain.to(level(start.gain), u32::from(start.fade_ms), output_rate);
    let target = i64::from(source_rate.saturating_mul(STREAM_TARGET_MS) / 1000);
    let mut playing = Playing {
        key: start.key,
        whole: written.map_or(0, |written| written.saturating_sub(target).max(0)),
        fraction: 0,
        step_whole: 0,
        step_fraction: 0,
        nominal,
        primed: written.is_none(),
        looped: start.looped && written.is_none(),
        gain,
        duck_percent: start.duck_percent,
        fade_ms: start.fade_ms,
        stopping: false,
    };
    set_step(&mut playing, nominal);
    playing
}

/// Sets a voice's step from 32.32 fixed point.
fn set_step(playing: &mut Playing, step: u64) {
    playing.step_whole = i64::try_from(step >> FRACTION_BITS).unwrap_or(0);
    playing.step_fraction = u32::try_from(step & u64::from(u32::MAX)).unwrap_or(0);
}

fn advance(playing: &mut Playing) {
    let (fraction, carry) = playing.fraction.overflowing_add(playing.step_fraction);
    playing.fraction = fraction;
    playing.whole = playing.whole.saturating_add(playing.step_whole).saturating_add(i64::from(carry));
}

/// The sound's value between samples `whole` and `whole + 1`, filtered by
/// the windowed sinc; samples outside the sound are silence.
fn interpolate(sound: &Sound, whole: i64, fraction: u32) -> i32 {
    let phase = usize::try_from(fraction >> PHASE_SHIFT).unwrap_or(0);
    let Some(taps) = FILTER.get(phase) else { return 0 };
    let first = whole.saturating_sub(TAPS_BEFORE);
    let mut sum: i32 = 0;
    for (offset, &tap) in (0_i64..).zip(taps.iter()) {
        let at = first.saturating_add(offset);
        let value = sound.at(at);
        sum = sum.saturating_add(i32::from(value).saturating_mul(i32::from(tap)));
    }
    sum / ONE
}

#[cfg(test)]
mod tests;
