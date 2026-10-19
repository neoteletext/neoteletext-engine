//! The software mixer of the audio profile (REQ-AUD-019), for ports with an
//! operating system's audio output: desktops and phones. It behaves as the
//! tellyboy driver does on a microcontroller, so a page sounds alike
//! everywhere: four voices, gains in whole per cent, every change ramped,
//! ducking in EN 300 708 Table 19's steps, and a clamp after the sum.
//!
//! It holds sounds as 16-bit PCM and renders mono 16-bit samples at the
//! output's rate, resampling each voice with a windowed-sinc filter
//! ([`sinc`]). It is pure: no clock, no thread and no device. The port
//! calls [`Mixer::render`] from its audio callback, which never allocates
//! or frees; everything that allocates is a call the port makes elsewhere.
#![no_std]
// REQ-SEC-015: only the C interface crate may contain unsafe code.
#![forbid(unsafe_code)]
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod sinc;

use alloc::vec::Vec;

use sinc::{FILTER, ONE, PHASES, TAPS_BEFORE};

/// Voices, one for each channel of the audio profile.
pub const VOICES: usize = 4;
/// Full level in the mixer's fixed point: 1.0 is 2¹⁶.
const UNITY: i32 = 1 << 16;
/// The shortest ramp, in milliseconds: tellyboy's `TB_AUDIO_DECLICK_MS`.
const MIN_RAMP_MS: u32 = 5;
/// A sound's identity: the audio receiver's handle and generation.
pub type SoundKey = (u8, u16);
/// Fraction bits of a source position.
const FRACTION_BITS: u32 = 32;
/// Shift from a position's fraction to a filter phase.
const PHASE_SHIFT: u32 = FRACTION_BITS - PHASES.trailing_zeros();
/// How much of a live sound a voice keeps buffered ahead of it, in
/// milliseconds: enough to ride out blocks arriving in bursts. A port may
/// hand over packets in pieces (the browser, every 200 ms), and a 256-byte
/// IMA ADPCM block at 8 kHz holds 63 ms, so samples can come 263 ms apart.
pub const STREAM_TARGET_MS: u32 = 400;
/// A live sound's ring of samples, in milliseconds: four times the target,
/// so a burst never overwrites what the voice has yet to play.
pub const STREAM_RING_MS: u32 = 4 * STREAM_TARGET_MS;
/// The furthest a live sound's playback rate moves from its nominal rate
/// to follow the sender's clock, in parts per million: ±5 per cent, where
/// REQ-AUD-014 asks for at least ±1.
pub const STREAM_TRIM_PPM: i64 = 50_000;
/// Parts in a million.
const PPM: i64 = 1_000_000;

/// A ramp from the current level to a target, in steps of one output
/// sample.
#[derive(Clone, Copy, Debug)]
struct Ramp {
    level: i32,
    target: i32,
    step: i32,
}

impl Ramp {
    const fn at(level: i32) -> Self {
        Self { level, target: level, step: 0 }
    }

    /// Ramps to `target` over `milliseconds` at `rate` samples a second.
    fn to(&mut self, target: i32, milliseconds: u32, rate: u32) {
        let samples =
            i32::try_from(milliseconds.max(MIN_RAMP_MS).saturating_mul(rate) / 1000).unwrap_or(i32::MAX).max(1);
        self.target = target;
        self.step = target.saturating_sub(self.level).saturating_abs().checked_div(samples).unwrap_or(0).max(1);
    }

    fn advance(&mut self) -> i32 {
        if self.level < self.target {
            self.level = self.level.saturating_add(self.step).min(self.target);
        } else if self.level > self.target {
            self.level = self.level.saturating_sub(self.step).max(self.target);
        }
        self.level
    }

    fn settled_at_zero(&self) -> bool {
        self.level == 0 && self.target == 0
    }
}

/// A sound held for playing.
struct Sound {
    key: SoundKey,
    rate_hz: u32,
    samples: Vec<i16>,
    /// The receiver forgot it: it goes once no voice plays it.
    forgotten: bool,
    /// A live sound: `samples` is a ring, and `written` counts every
    /// sample ever given it.
    stream: bool,
    written: i64,
}

impl Sound {
    /// Sample `index` of the sound; silence outside it, or outside the part
    /// of a live sound its ring still holds.
    fn at(&self, index: i64) -> i16 {
        let length = i64::try_from(self.samples.len()).unwrap_or(i64::MAX);
        let index = if self.stream {
            if index >= self.written || index < self.written.saturating_sub(length) {
                return 0;
            }
            index.checked_rem(length).unwrap_or(0)
        } else {
            index
        };
        usize::try_from(index).ok().and_then(|index| self.samples.get(index)).copied().unwrap_or(0)
    }

    /// Samples of a live sound a voice at `whole` has yet to play.
    fn ahead(&self, whole: i64) -> i64 {
        self.written.saturating_sub(whole)
    }

    /// A live sound's target and ring, in samples.
    fn target(&self) -> i64 {
        i64::from(self.rate_hz.saturating_mul(STREAM_TARGET_MS) / 1000)
    }
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

#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    playing: Option<Playing>,
    /// The sound it replaced, fading out.
    outgoing: Option<Playing>,
    /// One sound waiting for the current one to end.
    pending: Option<Start>,
    duck: Option<Ramp>,
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

    /// Makes room for a live sound (REQ-AUD-014): its samples arrive in
    /// order through [`Mixer::stream_samples`], without end. A voice playing
    /// it waits until [`STREAM_TARGET_MS`] are buffered, then trims its rate
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
        let duck = voice.duck.map_or(UNITY, |ramp| ramp.level);
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
            if ramp.target != target {
                ramp.to(target, fade, rate);
            }
            ramp.advance();
            voice.duck = Some(ramp);
        }
    }

    /// Trims each live sound's step from its nominal one in proportion to
    /// how far its buffer is from the target: the full [`STREAM_TRIM_PPM`]
    /// when it is empty or twice full. A sender whose clock runs fast fills
    /// the buffer and is played faster, and the reverse.
    fn trim_streams(&mut self) {
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

/// A whole percentage as a level.
fn level(percent: u8) -> i32 {
    i32::from(percent.min(100)).saturating_mul(UNITY) / 100
}

/// `value` at `level` (1.0 = [`UNITY`]).
fn scale(value: i32, level: i32) -> i32 {
    i32::try_from(i64::from(value).saturating_mul(i64::from(level)) >> 16).unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-011, REQ-AUD-014, REQ-AUD-015 and REQ-AUD-019.
    use super::*;
    use std::vec;
    use std::vec::Vec;

    const RATE: u32 = 8_000;

    fn start(key: SoundKey, gain: u8) -> Start {
        Start { key, gain, fade_ms: 5, looped: false, duck_percent: 100, after_current: false }
    }

    fn constant(mixer: &mut Mixer, key: SoundKey, value: i16, samples: usize) {
        mixer.define(key, RATE, samples);
        mixer.samples(key, 0, &vec![value; samples]);
    }

    fn render(mixer: &mut Mixer, samples: usize) -> Vec<i16> {
        let mut out = vec![0; samples];
        mixer.render(&mut out);
        out
    }

    #[test]
    fn a_sound_ramps_in_over_its_fade_and_plays_at_its_gain() {
        let mut mixer = Mixer::new(RATE);
        constant(&mut mixer, (1, 1), 10_000, 4000);
        mixer.start(0, start((1, 1), 50));
        let out = render(&mut mixer, 400);
        // 5 ms at 8 kHz is 40 samples of ramp: no step at the start.
        assert!(out[0].abs() < 1000, "starts near silence: {}", out[0]);
        assert!(out[20] > out[5] && out[20] < 5000, "rising");
        // Settled: half of 10 000, give or take the filter's ripple.
        assert!((4_900..=5_100).contains(&out[300]), "{}", out[300]);
    }

    #[test]
    fn a_stop_fades_out_and_ends_the_voice() {
        let mut mixer = Mixer::new(RATE);
        constant(&mut mixer, (1, 1), 10_000, 8000);
        mixer.start(0, start((1, 1), 100));
        render(&mut mixer, 200);
        mixer.stop(0, 10);
        let out = render(&mut mixer, 200);
        assert!(out[0] > 9_000, "still sounding as the fade begins");
        assert_eq!(out[100], 0, "silent once 10 ms have passed");
        assert!(!mixer.active());
    }

    #[test]
    fn voices_sum_and_the_sum_is_clamped() {
        let mut mixer = Mixer::new(RATE);
        constant(&mut mixer, (1, 1), 20_000, 2000);
        constant(&mut mixer, (2, 1), 20_000, 2000);
        mixer.start(0, start((1, 1), 100));
        mixer.start(1, start((2, 1), 100));
        assert_eq!(render(&mut mixer, 500)[400], i16::MAX);
    }

    #[test]
    fn ducking_lowers_the_other_voices_by_table_19s_step() {
        let mut mixer = Mixer::new(RATE);
        constant(&mut mixer, (1, 1), 10_000, 8000);
        constant(&mut mixer, (2, 1), 0, 8000);
        mixer.start(0, start((1, 1), 100));
        render(&mut mixer, 200);
        // Voice 1 plays silence but asks the others to duck by 12 dB (25 %).
        mixer.start(1, Start { duck_percent: 25, ..start((2, 1), 100) });
        let out = render(&mut mixer, 400);
        assert!((2_400..=2_600).contains(&out[399]), "{}", out[399]);
    }

    #[test]
    fn a_queued_sound_follows_the_current_one() {
        let mut mixer = Mixer::new(RATE);
        constant(&mut mixer, (1, 1), 1_000, 100);
        constant(&mut mixer, (2, 1), 8_000, 2000);
        mixer.start(0, start((1, 1), 100));
        mixer.start(0, Start { after_current: true, ..start((2, 1), 100) });
        let out = render(&mut mixer, 600);
        assert!((900..=1_100).contains(&out[60]), "the first sound");
        assert!((7_800..=8_200).contains(&out[500]), "then the queued one: {}", out[500]);
    }

    #[test]
    fn a_sound_is_resampled_to_the_output_rate() {
        // A 1 kHz tone at 8 kHz played at 48 kHz keeps its pitch: zero
        // crossings every 24 output samples.
        let mut mixer = Mixer::new(48_000);
        let tone: Vec<i16> = (0..8000).map(|n| [0, 7071, 10_000, 7071, 0, -7071, -10_000, -7071][n % 8]).collect();
        mixer.define((1, 1), 8_000, tone.len());
        mixer.samples((1, 1), 0, &tone);
        mixer.start(0, start((1, 1), 100));
        let out = render(&mut mixer, 4800);
        let crossings: Vec<usize> = (2000..4700).filter(|&n| out[n] < 0 && out[n + 1] >= 0).collect();
        let spacings: Vec<usize> = crossings.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert!(!spacings.is_empty() && spacings.iter().all(|&gap| gap == 48), "{spacings:?}");
        let peak = out[2000..4700].iter().map(|value| value.unsigned_abs()).max().unwrap();
        assert!((9_500..=10_500).contains(&peak), "amplitude kept: {peak}");
    }

    #[test]
    fn mute_silences_and_a_forgotten_sound_waits_for_its_voice() {
        let mut mixer = Mixer::new(RATE);
        constant(&mut mixer, (1, 1), 10_000, 8000);
        mixer.start(0, start((1, 1), 100));
        mixer.forget((1, 1));
        assert!(render(&mut mixer, 100)[99] > 9_000, "still playing after being forgotten");
        mixer.set_master(100, true);
        assert_eq!(render(&mut mixer, 100)[99], 0);
    }

    /// Samples voice 0's live sound has buffered ahead of it.
    fn buffered(mixer: &Mixer) -> i64 {
        let playing = mixer.voices[0].playing.unwrap();
        let sound = mixer.sounds.iter().find(|sound| sound.key == playing.key).unwrap();
        sound.ahead(playing.whole)
    }

    #[test]
    fn a_live_sound_waits_for_its_target_then_plays() {
        let mut mixer = Mixer::new(RATE);
        mixer.stream((1, 1), RATE);
        mixer.start(0, start((1, 1), 100));
        // 400 ms at 8 kHz is 3 200 samples: 3 100 are not enough.
        mixer.stream_samples((1, 1), &vec![10_000; 3_100]);
        assert!(render(&mut mixer, 200).iter().all(|&value| value == 0), "silent until the target is buffered");
        mixer.stream_samples((1, 1), &vec![10_000; 200]);
        let out = render(&mut mixer, 200);
        assert!((9_800..=10_200).contains(&out[150]), "{}", out[150]);
    }

    #[test]
    fn a_live_sound_follows_a_sender_whose_clock_runs_one_per_cent_fast() {
        let mut mixer = Mixer::new(RATE);
        mixer.stream((1, 1), RATE);
        mixer.start(0, start((1, 1), 100));
        // 101 samples arrive for every 100 played; 200 000 samples (25 s)
        // would put 2 000 extra in the buffer were the rate not trimmed.
        for _ in 0..2_000 {
            mixer.stream_samples((1, 1), &[5_000; 101]);
            render(&mut mixer, 100);
        }
        let ahead = buffered(&mixer);
        // Steady state: 1 % of trim is a fifth of the target, 640 samples, over.
        assert!((3_200..=4_000).contains(&ahead), "buffer held near the target: {ahead}");
        // And one running slow drains less than the target's worth.
        for _ in 0..4_000 {
            mixer.stream_samples((1, 1), &[5_000; 99]);
            render(&mut mixer, 100);
        }
        let ahead = buffered(&mixer);
        assert!((2_400..=3_200).contains(&ahead), "buffer held near the target: {ahead}");
    }

    #[test]
    fn a_forgotten_live_sound_plays_what_it_holds_then_ends() {
        let mut mixer = Mixer::new(RATE);
        mixer.stream((1, 1), RATE);
        mixer.start(0, start((1, 1), 100));
        mixer.stream_samples((1, 1), &vec![10_000; 3_600]);
        render(&mut mixer, 1_000);
        mixer.forget((1, 1));
        let out = render(&mut mixer, 3_000);
        assert!(out[2_000] > 9_000, "still playing what it holds");
        assert!(!mixer.active() && out[2_999] == 0, "then ended");
        // Rendering never frees: the next call that may allocate collects it.
        mixer.stream((2, 1), RATE);
        assert_eq!(mixer.sounds.iter().map(|sound| sound.key).collect::<Vec<_>>(), [(2, 1)], "and gone");
    }

    #[test]
    fn a_voice_that_falls_a_ring_behind_skips_to_the_target() {
        let mut mixer = Mixer::new(RATE);
        mixer.stream((1, 1), RATE);
        mixer.start(0, start((1, 1), 100));
        // Two seconds of samples at once: more than the ring keeps ahead.
        mixer.stream_samples((1, 1), &vec![10_000; 16_000]);
        assert_eq!(buffered(&mixer), 3_200);
    }
}
