//! A voice and the sound on it: the source position in 32.32 fixed point,
//! its step, and the windowed-sinc interpolation between samples ([`sinc`](crate::sinc)).

use crate::mixer::Start;
use crate::ramp::{Ramp, level};
use crate::sinc::{FILTER, ONE, PHASES, TAPS_BEFORE};
use crate::sound::{STREAM_TARGET_MS, Sound, SoundKey};

/// Fraction bits of a source position.
pub(crate) const FRACTION_BITS: u32 = 32;
/// Shift from a position's fraction to a filter phase.
pub(crate) const PHASE_SHIFT: u32 = FRACTION_BITS - PHASES.trailing_zeros();

/// A sound on a voice.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Playing {
    pub(crate) key: SoundKey,
    /// The source position: whole samples and a 32-bit fraction.
    pub(crate) whole: i64,
    pub(crate) fraction: u32,
    /// The source step per output sample, in the same form.
    pub(crate) step_whole: i64,
    pub(crate) step_fraction: u32,
    /// The nominal step, whole and fraction in one: a live sound's step is
    /// trimmed from it to follow the sender's clock.
    pub(crate) nominal: u64,
    /// A live sound's voice waits, silent, until the target is buffered.
    pub(crate) primed: bool,
    pub(crate) looped: bool,
    pub(crate) gain: Ramp,
    /// Other voices' level while this plays, in per cent.
    pub(crate) duck_percent: u8,
    pub(crate) fade_ms: u16,
    /// Stopping: it ends when its gain reaches zero.
    pub(crate) stopping: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Voice {
    pub(crate) playing: Option<Playing>,
    /// The sound it replaced, fading out.
    pub(crate) outgoing: Option<Playing>,
    /// One sound waiting for the current one to end.
    pub(crate) pending: Option<Start>,
    pub(crate) duck: Option<Ramp>,
}

/// A sound starting on a voice, its gain ramping in. `source` is the
/// sound's rate and, for a live sound, the samples written so far: its
/// voice joins [`STREAM_TARGET_MS`] behind the newest.
pub(crate) fn begin(start: &Start, source: (u32, Option<i64>), output_rate: u32) -> Playing {
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
pub(crate) fn set_step(playing: &mut Playing, step: u64) {
    playing.step_whole = i64::try_from(step >> FRACTION_BITS).unwrap_or(0);
    playing.step_fraction = u32::try_from(step & u64::from(u32::MAX)).unwrap_or(0);
}

pub(crate) fn advance(playing: &mut Playing) {
    let (fraction, carry) = playing.fraction.overflowing_add(playing.step_fraction);
    playing.fraction = fraction;
    playing.whole = playing.whole.saturating_add(playing.step_whole).saturating_add(i64::from(carry));
}

/// The sound's value between samples `whole` and `whole + 1`, filtered by
/// the windowed sinc; samples outside the sound are silence.
pub(crate) fn interpolate(sound: &Sound, whole: i64, fraction: u32) -> i32 {
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
