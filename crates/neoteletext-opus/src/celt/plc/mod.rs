//! Concealment of a lost frame (RFC 6716 §4.4): for the first frames of a loss, the history
//! extended at its pitch through its own prediction filter, fading; after that, or in Hybrid
//! frames, noise shaped by the band energies, decaying to the background. The frame after a
//! pitch concealment folds the concealment's tail back through the post-filter.

pub mod lpc;
pub mod pitch;

use crate::celt::bands::celt_lcg_rand;
use crate::celt::decoder::{CeltDecoder, HISTORY, frame_start};
use crate::celt::postfilter::{Apart, CombParameters, comb_filter};
use crate::celt::pvq::renormalise_vector;
use crate::celt::synthesis::{FrameLayout, synthesise};
use crate::celt::tables::{BAND_WIDTHS, EBANDS, WINDOW};
use crate::celt::{FrameDuration, MAX_FRAME, OVERLAP};
use crate::fixed::{
    Q15ONE, SIG_SAT, SIG_SHIFT, abs16, add32, mult16_16, mult16_16_q15, mult16_32_q15, saturate, shl32, shr32,
    sround16, sub32,
};
use crate::math::{celt_maxabs16, celt_sqrt, celt_zlog2, frac_div32};

use crate::slices::{chunks_up_to_mut, discard_front};
use lpc::{LPC_ORDER, autocorr, fir_in_place, iir_in_place};
use pitch::{pitch_downsample, pitch_search};

/// `MAX_PERIOD`: the excitation the pitch concealment keeps.
const MAX_PERIOD: usize = 1024;
/// `PLC_PITCH_LAG_MAX`, `PLC_PITCH_LAG_MIN`: the pitch periods searched.
const PLC_PITCH_LAG_MAX: usize = 720;
const PLC_PITCH_LAG_MIN: usize = 100;
/// The lost 2.5 ms blocks after which the concealment turns from the pitch to noise.
const PITCH_CONCEALMENT_BLOCKS: u16 = 40;
/// The most lost blocks counted.
const MAX_LOSS_DURATION: u16 = 10000;
/// `QCONST16(1.5f, DB_SHIFT)`, `QCONST16(.5f, DB_SHIFT)`: the noise concealment's energy decay
/// in its first frame and after.
const FIRST_DECAY: i32 = 1536;
const LATER_DECAY: i32 = 512;
/// `QCONST16(.8f, 15)`: the pitch concealment's fade in each frame after its first.
const LATER_FADE: i16 = 26214;
/// `QCONST16(.99f, 15)`: the bandwidth expansion that keeps the prediction filter stable.
const LPC_CHIRP: i32 = 32440;
/// `QCONST16(1., SIG_SHIFT)`.
const SIG_ONE: i32 = 1 << SIG_SHIFT;

/// The workspace's 16-bit values the concealment uses: the pitch search's half-rate history
/// and its scratch, or the excitation and the prediction's scratch.
pub const NARROW_SCRATCH: usize = {
    let search = (HISTORY >> 1) + (HISTORY >> 1);
    let prediction = MAX_PERIOD + LPC_ORDER + MAX_PERIOD;
    let extrapolation = MAX_FRAME + OVERLAP + LPC_ORDER;
    let most = if search > prediction { search } else { prediction };
    if most > extrapolation { most } else { extrapolation }
};
/// The workspace's 32-bit values the concealment uses: the pitch search's correlations.
pub const WIDE_SCRATCH: usize = (PLC_PITCH_LAG_MAX - PLC_PITCH_LAG_MIN) >> 1;

impl CeltDecoder {
    /// `celt_decode_lost`: conceals a lost frame of `duration` in the history.
    #[cold]
    #[inline(never)]
    pub(super) fn conceal(&mut self, duration: FrameDuration) {
        let start = usize::from(self.start_band);
        if self.loss_duration >= PITCH_CONCEALMENT_BLOCKS || start != 0 || self.skip_plc {
            self.conceal_with_noise(duration);
        } else {
            self.conceal_with_pitch(duration.samples());
        }
        self.loss_duration = self.loss_duration.saturating_add(1 << duration.lm()).min(MAX_LOSS_DURATION);
    }

    /// Noise in each band at its energy, decayed towards the background.
    fn conceal_with_noise(&mut self, duration: FrameDuration) {
        let n = duration.samples();
        let lm = duration.lm();
        let start = usize::from(self.start_band);
        let end = usize::from(self.end_band);
        let coded_end = start.max(end);
        discard_front(&mut self.history, n);
        if self.prefilter_and_fold {
            prefilter_and_fold(&mut self.history, self.postfilter_old, self.postfilter, n);
        }
        let decay = if self.loss_duration == 0 { FIRST_DECAY } else { LATER_DECAY };
        let energy = &mut self.energy;
        for (current, &background) in energy.current.iter_mut().zip(&energy.background).take(end).skip(start) {
            *current = background.max(i32::from(*current).wrapping_sub(decay) as i16);
        }
        let (x, _) = self.workspace.narrow.split_at_mut(MAX_FRAME);
        let mut seed = self.rng;
        for (&edge, &width) in EBANDS.iter().zip(&BAND_WIDTHS).take(coded_end).skip(start) {
            let Some(band_x) = x.get_mut(usize::from(edge) << lm..).and_then(|x| x.get_mut(..usize::from(width) << lm))
            else {
                break;
            };
            for v in band_x.iter_mut() {
                // The generator's top bits, signed.
                seed = celt_lcg_rand(seed);
                *v = (seed.cast_signed() >> 20) as i16;
            }
            renormalise_vector(band_x, Q15ONE);
        }
        self.rng = seed;
        let layout = FrameLayout { duration, transient: false, silence: false };
        synthesise(
            x,
            &mut self.workspace.wide,
            &energy.current,
            start..coded_end,
            layout,
            self.history.get_mut(frame_start(n)..).unwrap_or_default(),
        );
        self.prefilter_and_fold = false;
        self.skip_plc = true;
    }

    /// The history extended at its pitch through its prediction filter, fading.
    fn conceal_with_pitch(&mut self, n: usize) {
        let (pitch_index, fade) = if self.loss_duration == 0 {
            let pitch_index = self.pitch_search();
            self.last_pitch_index = pitch_index as u16;
            (pitch_index, Q15ONE)
        } else {
            (usize::from(self.last_pitch_index), LATER_FADE)
        };
        let exc_length = pitch_index.saturating_mul(2).min(MAX_PERIOD);
        // The excitation: the last MAX_PERIOD samples, after the prediction order's of history.
        let Some((exc, scratch)) = self.workspace.narrow.split_first_chunk_mut::<{ LPC_ORDER + MAX_PERIOD }>() else {
            return;
        };
        for (e, &h) in exc.iter_mut().zip(&self.history[HISTORY - MAX_PERIOD - LPC_ORDER..HISTORY]) {
            *e = sround16(h, SIG_SHIFT);
        }
        if self.loss_duration == 0 {
            // The prediction filter of the history, from its windowed autocorrelation with a
            // noise floor at -40 dB and lag windowing.
            let mut ac = [0i32; LPC_ORDER + 1];
            autocorr(&exc[LPC_ORDER..], &mut ac, true, scratch);
            ac[0] = ac[0].wrapping_add(shr32(ac[0], 13));
            for (i, v) in (1i32..).zip(ac.iter_mut().skip(1)) {
                *v = v.wrapping_sub(mult16_32_q15(i.wrapping_mul(i).wrapping_mul(2), *v));
            }
            lpc::lpc(&mut self.lpc, &ac);
            // Expand the bandwidth until the filter is safely stable: its gain under 16.
            while self.lpc.iter().fold(SIG_ONE, |sum, &c| add32(sum, abs16(i32::from(c)))) >= 65535 {
                let mut tmp: i16 = Q15ONE;
                for c in &mut self.lpc {
                    tmp = mult16_16_q15(LPC_CHIRP, i32::from(tmp)) as i16;
                    *c = mult16_16_q15(i32::from(*c), i32::from(tmp)) as i16;
                }
            }
        }
        // The excitation: the last `exc_length` samples through the prediction's inverse, the
        // prediction order's before them its history.
        fir_in_place(last_mut(exc, exc_length.saturating_add(LPC_ORDER)), &self.lpc);

        // The decay over a pitch period: the energy of the excitation's last half-period
        // against the one before.
        let decay = {
            let tail = last(exc, exc_length);
            let shift =
                i32::from(celt_zlog2(celt_maxabs16(tail))).wrapping_mul(2).wrapping_sub(20).max(0).cast_unsigned();
            let decay_length = exc_length >> 1;
            let energy = |samples: &[i16]| {
                samples
                    .iter()
                    .fold(1i32, |sum, &e| sum.wrapping_add(shr32(mult16_16(i32::from(e), i32::from(e)), shift)))
            };
            // The last two half-periods, the earlier first.
            let (earlier, later) = last(tail, exc_length & !1).split_at_checked(decay_length).unwrap_or_default();
            let e1 = energy(later);
            let e2 = energy(earlier);
            celt_sqrt(frac_div32(shr32(e1.min(e2), 1), e2)) as i16
        };

        // Extend the excitation at the pitch, fading by the decay each period, into the frame and
        // the overlap after it.
        discard_front(self.history.get_mut(..HISTORY).unwrap_or_default(), n);
        let Some((before, extended)) = self.history.split_at_mut_checked(frame_start(n)) else {
            return;
        };
        // The last period of the excitation, and of the history it came from.
        let excitation_period = last(exc, pitch_index);
        let source_period = last(before, pitch_index);
        let mut attenuation = mult16_16_q15(i32::from(fade), i32::from(decay)) as i16;
        let mut s1: i32 = 0;
        for (period, chunk) in chunks_up_to_mut(extended, pitch_index.max(1)).enumerate() {
            if period > 0 {
                attenuation = mult16_16_q15(i32::from(attenuation), i32::from(decay)) as i16;
            }
            let sources = excitation_period.iter().zip(source_period);
            for (sample, (&excitation, &source)) in chunk.iter_mut().zip(sources) {
                *sample =
                    shl32(i32::from(mult16_16_q15(i32::from(attenuation), i32::from(excitation)) as i16), SIG_SHIFT);
                // The energy of the period the excitation came from.
                let tmp = i32::from(sround16(source, SIG_SHIFT));
                s1 = s1.wrapping_add(shr32(mult16_16(tmp, tmp), 10));
            }
        }

        // The excitation through the prediction filter, from the history before it; the
        // excitation's place serves as the filter's scratch.
        let mut lpc_mem = [0i16; LPC_ORDER];
        for (m, &h) in lpc_mem.iter_mut().zip(before.iter().rev()) {
            *m = sround16(h, SIG_SHIFT);
        }
        iir_in_place(extended, &self.lpc, &mut lpc_mem, &mut self.workspace.narrow);
        for sample in extended.iter_mut() {
            *sample = saturate(*sample, SIG_SAT);
        }

        // No more energy than the history had: none at all if the filter blew up, else scaled
        // down to the history's.
        let s2 = extended.iter().fold(0i32, |sum, &h| {
            let tmp = i32::from(sround16(h, SIG_SHIFT));
            sum.wrapping_add(shr32(mult16_16(tmp, tmp), 10))
        });
        if s1 <= shr32(s2, 2) {
            extended.fill(0);
        } else if s1 < s2 {
            let ratio = celt_sqrt(frac_div32(add32(shr32(s1, 1), 1), add32(s2, 1))) as i16;
            let (faded, rest) = extended.split_at_mut_checked(OVERLAP).unwrap_or_default();
            for (sample, &w) in faded.iter_mut().zip(WINDOW.iter()) {
                let gain =
                    sub32(i32::from(Q15ONE), mult16_16_q15(i32::from(w), sub32(i32::from(Q15ONE), i32::from(ratio))))
                        as i16;
                *sample = mult16_32_q15(i32::from(gain), *sample);
            }
            for sample in rest {
                *sample = mult16_32_q15(i32::from(ratio), *sample);
            }
        }
        self.prefilter_and_fold = true;
    }

    /// `celt_plc_pitch_search`: the history's pitch period.
    fn pitch_search(&mut self) -> usize {
        let Some((lp, scratch)) = self.workspace.narrow.split_first_chunk_mut::<{ HISTORY >> 1 }>() else {
            return PLC_PITCH_LAG_MAX;
        };
        pitch_downsample(&self.history[..HISTORY], lp, scratch);
        let pitch = pitch_search(
            &lp[PLC_PITCH_LAG_MAX >> 1..],
            lp,
            HISTORY - PLC_PITCH_LAG_MAX,
            PLC_PITCH_LAG_MAX - PLC_PITCH_LAG_MIN,
            scratch,
            &mut self.workspace.wide,
        );
        // The lag found, kept within those searched.
        PLC_PITCH_LAG_MAX
            .saturating_sub(usize::try_from(pitch).map_or(0, |pitch| pitch.min(PLC_PITCH_LAG_MAX - PLC_PITCH_LAG_MIN)))
    }
}

/// `prefilter_and_fold`: the tail of a pitch concealment in `history`, which the next frame's
/// overlap meets, through the inverse of the post-filter `old` to `new`, and folded as the
/// inverse MDCT folds its overlap, `n` samples from the history's end.
#[cold]
#[inline(never)]
pub(super) fn prefilter_and_fold(history: &mut [i32], old: CombParameters, new: CombParameters, n: usize) {
    let at = HISTORY.saturating_sub(n);
    let mut filtered = [0i32; OVERLAP];
    let negate = |p: CombParameters| CombParameters { gain: p.gain.wrapping_neg(), ..p };
    comb_filter(&mut Apart { y: &mut filtered, x: history, x_at: at }, negate(old), negate(new), OVERLAP, 0);
    let Some(folded) = history.get_mut(at..).and_then(|tail| tail.get_mut(..OVERLAP / 2)) else {
        return;
    };
    // Sample `i` meets the filtered `i` and its mirror, `OVERLAP - 1 - i`.
    let (window_low, window_high) = WINDOW.split_at(OVERLAP / 2);
    let (filtered_low, filtered_high) = filtered.split_at(OVERLAP / 2);
    let windows = window_low.iter().zip(window_high.iter().rev());
    for ((sample, (&w1, &w2)), (&direct, &mirror)) in
        folded.iter_mut().zip(windows).zip(filtered_low.iter().zip(filtered_high.iter().rev()))
    {
        *sample = add32(mult16_32_q15(i32::from(w1), mirror), mult16_32_q15(i32::from(w2), direct));
    }
}

/// The last `n` of `values`, or all of them if there are fewer.
#[inline]
fn last<T>(values: &[T], n: usize) -> &[T] {
    values.get(values.len().saturating_sub(n)..).unwrap_or_default()
}

/// The last `n` of `values`, mutably, or all of them if there are fewer.
#[inline]
fn last_mut<T>(values: &mut [T], n: usize) -> &mut [T] {
    let from = values.len().saturating_sub(n);
    values.get_mut(from..).unwrap_or_default()
}
