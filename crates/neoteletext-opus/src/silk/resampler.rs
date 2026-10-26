//! The output resampler (RFC 6716 §4.2.9, which leaves it to the implementation; this is the
//! reference decoder's, so that the output is the same): from the internal 8, 12 or 16 kHz to
//! the output's 48 kHz: an allpass-based 2x upsampler followed by a 12-phase FIR interpolator,
//! with 16 to 48 kHz (every wideband and Hybrid frame) specialised.
//!
//! From the reference decoder's `silk/resampler.c`, `resampler_private_up2_HQ.c`,
//! `resampler_private_IIR_FIR.c`. Copyright (c) 2006-2011, Skype Limited, BSD-3-Clause (the licence
//! beside the crate).

use super::layout::{InternalRate, OUTPUT_KHZ};
use super::macros::{add32, lshift, rshift_round, sat16, smlabb, smlawb, smulwb, smulww, sub32};
use crate::slices::{chunks_up_to, chunks_up_to_mut, copy};

/// `silk_resampler_up2_hq_0` and `_1`: the allpass coefficients of the even and the odd output
/// samples, Q16, the third above 1 and stored less 65536.
const UP2_EVEN: [i32; 3] = [1746, 14986, 39083 - 65536];
const UP2_ODD: [i32; 3] = [6854, 25769, 55542 - 65536];

/// `silk_resampler_frac_FIR_12`: the first half of each phase of the symmetric interpolation
/// filter, Q15.
const FRAC_FIR_12: [[i16; 4]; 12] = [
    [189, -600, 617, 30567],
    [117, -159, -1070, 29704],
    [52, 221, -2392, 28276],
    [-4, 529, -3350, 26341],
    [-48, 758, -3956, 23973],
    [-80, 905, -4235, 21254],
    [-99, 972, -4222, 18278],
    [-107, 967, -3957, 15143],
    [-103, 896, -3487, 11950],
    [-91, 773, -2865, 8798],
    [-71, 611, -2143, 5784],
    [-46, 425, -1375, 2996],
];

/// The interpolator's taps, `RESAMPLER_ORDER_FIR_12`.
const FIR_ORDER: usize = 8;
/// The interpolator's phases.
const FIR_PHASES: usize = 12;
/// The 1 ms of input buffered for the delay compensation, at the highest internal rate (16 kHz).
const MAX_INPUT_DELAY_BUFFER: usize = 16;
/// `RESAMPLER_MAX_BATCH_SIZE_MS`: the interpolator works on 10 ms of input at a time.
const BATCH_MS: usize = 10;
/// The longest batch, 10 ms at 16 kHz.
const MAX_BATCH: usize = BATCH_MS * 16;
/// The working buffer `process` needs: the interpolator's history and a batch upsampled.
pub const RESAMPLER_BUFFER: usize = FIR_ORDER + 2 * MAX_BATCH;

/// The 8 taps of each phase of the interpolator: the 4 coefficients of `silk_resampler_frac_FIR_12`
/// for the phase, then the mirror phase's in reverse (the filter is symmetric).
const PHASE_TAPS: [[i16; FIR_ORDER]; FIR_PHASES] = {
    const fn phase(low: [i16; 4], high: [i16; 4]) -> [i16; FIR_ORDER] {
        let ([low0, low1, low2, low3], [high0, high1, high2, high3]) = (low, high);
        [low0, low1, low2, low3, high3, high2, high1, high0]
    }
    let [p0, p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11] = FRAC_FIR_12;
    [
        phase(p0, p11),
        phase(p1, p10),
        phase(p2, p9),
        phase(p3, p8),
        phase(p4, p7),
        phase(p5, p6),
        phase(p6, p5),
        phase(p7, p4),
        phase(p8, p3),
        phase(p9, p2),
        phase(p10, p1),
        phase(p11, p0),
    ]
};

/// The resampling method for an internal rate.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    /// 8 or 12 to 48 kHz: the 2x upsampler, then FIR interpolation that steps
    /// through the upsampled signal by `step_q16` (the input/output ratio, rounded up).
    Up2Interpolate { step_q16: usize },
    /// 16 to 48 kHz: the same as `Up2Interpolate`, whose phases there run 0, 8, 4 for each
    /// input sample (see `up3`).
    Up3,
}

/// `silk_resampler_init`'s `invRatio_Q16` for the upsamplers that interpolate (`up2x` 1): the
/// input/output ratio in Q16, rounded up. Evaluated when compiled, for the rates given.
struct Step<const INPUT_HZ: i32, const OUTPUT_HZ: i32>;

impl<const INPUT_HZ: i32, const OUTPUT_HZ: i32> Step<INPUT_HZ, OUTPUT_HZ> {
    #[allow(clippy::cast_sign_loss, reason = "the ratio of two positive rates is positive")]
    const Q16: usize = {
        let mut step = ((INPUT_HZ << 15) / OUTPUT_HZ) << 2;
        while smulww(step, OUTPUT_HZ) < INPUT_HZ << 1 {
            step += 1;
        }
        step as usize
    };
}

/// `silk_resampler_state_struct`, from an internal rate to 48 kHz.
#[derive(Clone)]
pub struct Resampler {
    /// The 2x upsampler's allpass states of the even and the odd samples, Q10.
    iir: [[i32; 3]; 2],
    /// The last `FIR_ORDER` samples of the upsampled signal.
    fir: [i16; FIR_ORDER],
    /// The delay-compensation buffer: the end of the previous input.
    delay_buffer: [i16; MAX_INPUT_DELAY_BUFFER],
    method: Method,
    /// The input rate in kHz: 1 ms of input.
    input_khz: u8,
    /// The input delay, in input samples, that equalises the total delay of every pair.
    input_delay: u8,
}

impl Resampler {
    /// `silk_resampler_init` for the decoder (`forEnc` 0).
    #[must_use]
    pub const fn new(input: InternalRate) -> Self {
        // `delay_matrix_dec` of the rows 8, 12, 16 kHz, column 48 kHz
        let (input_delay, method) = match input {
            InternalRate::Narrowband => (0, Method::Up2Interpolate { step_q16: Step::<8000, 48000>::Q16 }),
            InternalRate::Mediumband => (4, Method::Up2Interpolate { step_q16: Step::<12000, 48000>::Q16 }),
            InternalRate::Wideband => (7, Method::Up3),
        };
        Self {
            iir: [[0; 3]; 2],
            fir: [0; FIR_ORDER],
            delay_buffer: [0; MAX_INPUT_DELAY_BUFFER],
            method,
            input_khz: input.samples_per_ms() as u8,
            input_delay,
        }
    }

    /// `silk_resampler`: resamples `input` (at least 1 ms) into `out`, which holds the same
    /// duration at 48 kHz, with `buffer` to work in.
    pub fn process(&mut self, out: &mut [i16], input: &[i16], buffer: &mut [i16; RESAMPLER_BUFFER]) {
        let (input_khz, delay) = (usize::from(self.input_khz), usize::from(self.input_delay));
        // The input is a frame, longer than the 1 ms the delay buffer holds and the delay; were
        // it not, nothing would be resampled
        let Some(kept) = input.len().checked_sub(delay) else { return };
        let fresh = input_khz.saturating_sub(delay);
        let rest = input.get(fresh..kept).unwrap_or_default();

        // The first 1 ms comes from the delay buffer: the end of the previous input, then the
        // start of this one
        let starts = self.delay_buffer.get_mut(delay..).unwrap_or_default();
        for (buffered, &sample) in starts.iter_mut().zip(input).take(fresh) {
            *buffered = sample;
        }
        let first = self.delay_buffer;
        let first = first.get(..input_khz).unwrap_or_default();
        let Some((out_first, out_rest)) = out.split_at_mut_checked(OUTPUT_KHZ) else { return };
        match self.method {
            Method::Up2Interpolate { step_q16 } => {
                self.up2_interpolate(out_first, first, step_q16, buffer);
                self.up2_interpolate(out_rest, rest, step_q16, buffer);
            }
            Method::Up3 => {
                self.up3(out_first, first, buffer);
                self.up3(out_rest, rest, buffer);
            }
        }
        for (buffered, &sample) in self.delay_buffer.iter_mut().zip(input.get(kept..).unwrap_or_default()) {
            *buffered = sample;
        }
    }

    /// `silk_resampler_private_IIR_FIR`: the 2x upsampler, then FIR interpolation, 10 ms of
    /// input at a time.
    fn up2_interpolate(
        &mut self,
        out: &mut [i16],
        input: &[i16],
        step_q16: usize,
        buffer: &mut [i16; RESAMPLER_BUFFER],
    ) {
        copy(&mut buffer[..FIR_ORDER], &self.fir);
        let mut outputs = out.iter_mut();
        // The input rate is 8 or 12 kHz here, which `max` says so that the chunks are not empty
        for batch in chunks_up_to(input, BATCH_MS.saturating_mul(usize::from(self.input_khz)).max(1)) {
            let upsampled = batch.len().saturating_mul(2);
            up2(
                &mut self.iir,
                buffer.get_mut(FIR_ORDER..FIR_ORDER.saturating_add(upsampled)).unwrap_or_default(),
                batch,
            );
            // `silk_resampler_private_IIR_FIR_INTERPOL`: the integer part of the index points
            // into the upsampled signal, the fractional part selects the phase. The index is
            // nonnegative and below 2^24 (a batch is 10 ms at 12 kHz at most, 240 upsampled
            // samples), so a `usize`, and the phase, `silk_SMULWB( fraction, FIR_PHASES )` of a
            // 16-bit fraction, is the plain product shifted
            let end_q16 = upsampled << 16;
            let mut index_q16 = 0;
            while index_q16 < end_q16 {
                let Some(out) = outputs.next() else { break };
                let phase = ((index_q16 & 0xFFFF).saturating_mul(FIR_PHASES)) >> 16;
                let at = index_q16 >> 16;
                let samples = buffer.get(at..).and_then(<[i16]>::first_chunk::<FIR_ORDER>);
                if let (Some(samples), Some(taps)) = (samples, PHASE_TAPS.get(phase)) {
                    *out = interpolate(samples, taps);
                }
                index_q16 = index_q16.saturating_add(step_q16);
            }
            keep_history(buffer, upsampled);
        }
        copy(&mut self.fir, &buffer[..FIR_ORDER]);
    }

    /// `up2_interpolate` from 16 to 48 kHz. Its step, 43691 in Q16, is 2/3 plus 1/3 of a Q16
    /// unit, so the k-th output of a batch of n inputs (k < 3n, n at most 160) is at
    /// `k * 43691 = m * 131073 + j * 43691` for `k = 3m + j`: upsampled sample 2m in phase 0
    /// (fraction m), 2m in phase 8 (fraction 43691 + m) and 2m + 1 in phase 4 (fraction
    /// 21846 + m), the phase being `fraction * 12 >> 16`; and a batch yields exactly 3n
    /// outputs. Each input sample then gives three outputs from a fixed pattern, whatever the
    /// batches, so the whole input goes in one batch per 10 ms as before.
    fn up3(&mut self, out: &mut [i16], input: &[i16], buffer: &mut [i16; RESAMPLER_BUFFER]) {
        const _: () = assert!(Step::<16000, 48000>::Q16 == 43691);
        copy(&mut buffer[..FIR_ORDER], &self.fir);
        for (batch, out) in chunks_up_to(input, MAX_BATCH).zip(chunks_up_to_mut(out, 3 * MAX_BATCH)) {
            let upsampled = batch.len().saturating_mul(2);
            let signal = buffer.get_mut(..FIR_ORDER.saturating_add(upsampled)).unwrap_or_default();
            up2(&mut self.iir, signal.get_mut(FIR_ORDER..).unwrap_or_default(), batch);
            for (window, out) in signal.array_windows::<{ FIR_ORDER + 1 }>().step_by(2).zip(out.as_chunks_mut::<3>().0)
            {
                let (Some(head), Some(tail)) = (window.first_chunk::<FIR_ORDER>(), window.last_chunk::<FIR_ORDER>())
                else {
                    continue;
                };
                *out = [
                    interpolate(head, &PHASE_TAPS[0]),
                    interpolate(head, &PHASE_TAPS[8]),
                    interpolate(tail, &PHASE_TAPS[4]),
                ];
            }
            keep_history(buffer, upsampled);
        }
        copy(&mut self.fir, &buffer[..FIR_ORDER]);
    }
}

/// Moves the last `FIR_ORDER` samples of the `upsampled` after the history to the front: the
/// history of the next batch.
#[inline]
fn keep_history(buffer: &mut [i16; RESAMPLER_BUFFER], upsampled: usize) {
    if let Some(&history) = buffer.get(upsampled..).and_then(<[i16]>::first_chunk::<FIR_ORDER>) {
        copy(&mut buffer[..FIR_ORDER], &history);
    }
}

/// One output sample of the interpolator: 8 upsampled samples through one phase's taps, Q15.
#[inline]
fn interpolate(samples: &[i16; FIR_ORDER], taps: &[i16; FIR_ORDER]) -> i16 {
    // The first product starts the sum, as in the C: adding it to 0 is the same
    let sum_q15 =
        samples.iter().zip(taps).fold(0, |sum, (&sample, &tap)| smlabb(sum, i32::from(sample), i32::from(tap)));
    sat16(rshift_round(sum_q15, 15)) as i16
}

/// `silk_resampler_private_up2_HQ`: upsamples by 2, high quality, through three 2nd-order
/// allpass sections for each of the even and the odd output samples. The state is Q10.
fn up2(state: &mut [[i32; 3]; 2], out: &mut [i16], input: &[i16]) {
    let [even_state, odd_state] = state;
    for (&sample, pair) in input.iter().zip(out.as_chunks_mut::<2>().0) {
        let in32 = lshift(i32::from(sample), 10);
        *pair = [allpass_cascade(even_state, in32, UP2_EVEN), allpass_cascade(odd_state, in32, UP2_ODD)];
    }
}

/// Three allpass sections of `silk_resampler_private_up2_HQ`, the third with its coefficient
/// above 1 (`c2 - 65536`, applied as `y + y * c2`); the output converted back to 16 bits.
#[inline]
fn allpass_cascade(state: &mut [i32; 3], in32: i32, [c0, c1, c2]: [i32; 3]) -> i16 {
    let [s0, s1, s2] = state;
    let y = sub32(in32, *s0);
    let x = smulwb(y, c0);
    let out1 = add32(*s0, x);
    *s0 = add32(in32, x);

    let y = sub32(out1, *s1);
    let x = smulwb(y, c1);
    let out2 = add32(*s1, x);
    *s1 = add32(out1, x);

    let y = sub32(out2, *s2);
    let x = smlawb(y, y, c2);
    let out3 = add32(*s2, x);
    *s2 = add32(out2, x);

    sat16(rshift_round(out3, 10)) as i16
}
