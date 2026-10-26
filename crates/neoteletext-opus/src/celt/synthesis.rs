//! Synthesis (RFC 6716 §4.3.6, §4.3.7): the band shapes scaled to their energies, the inverse
//! MDCT of each block into the decoder's history, and the de-emphasis from there to the output.

use crate::celt::tables::{E_MEANS, EBANDS};
use crate::celt::transform::{MDCT_240, MDCT_480, MDCT_1920, inverse_mdct, inverse_mdct_of_zeros};
use crate::celt::{FrameDuration, NB_BANDS, SHORT_BLOCK};
use crate::fixed::{
    DB_SHIFT, SIG_SAT, add32, mult16_16, mult16_32_q15, sat16, saturate, saturate16, shl32, shr32, sig2word16,
};
use crate::math::celt_exp2_frac;
use core::ops::Range;

/// The de-emphasis filter's coefficient, `QCONST16(0.85f, 15)` (RFC 6716 §4.3.7.2).
const PREEMPHASIS: i32 = 27853;

/// `denormalise_bands`: the unit-norm band shapes `x` scaled by the log energies `log_energy`
/// (Q`DB_SHIFT`) into the coefficients `freq`, for the bands `start..end` of a frame of
/// `120 << lm` coefficients. Answers whether any coefficient may be nonzero.
pub fn denormalise_bands(
    x: &[i16],
    freq: &mut [i32],
    log_energy: &[i16; NB_BANDS],
    start: usize,
    end: usize,
    lm: i32,
    silence: bool,
) -> bool {
    // An LM outside the mode's leaves nothing in reach, and every coefficient zero.
    let Some(m) = u32::try_from(lm).ok().and_then(|lm| 1usize.checked_shl(lm)) else {
        freq.fill(0);
        return false;
    };
    let Some(freq) = freq.get_mut(..m.saturating_mul(SHORT_BLOCK)) else {
        return false;
    };
    // A band edge, in this frame's coefficients.
    let edge = |band: usize| m.saturating_mul(usize::from(EBANDS.get(band).copied().unwrap_or_default()));
    // The mode's bands at most, so that each band's entries of the tables are in reach.
    let end = end.min(NB_BANDS);
    let start = start.min(end);
    let (start, end, bound) = if silence { (0, 0, 0) } else { (start, end, edge(end)) };
    let first = edge(start);
    freq.get_mut(..first).unwrap_or_default().fill(0);
    freq.get_mut(bound..).unwrap_or_default().fill(0);
    let edges = EBANDS.iter().zip(EBANDS.iter().skip(1));
    for (((&low, &high), &energy), &mean) in edges.zip(log_energy).zip(&E_MEANS).take(end).skip(start) {
        let (low, high) = (m.saturating_mul(usize::from(low)), m.saturating_mul(usize::from(high)));
        let lg = saturate16(add32(i32::from(energy), shl32(i32::from(mean), 6)));
        // The integer part of the log energy is a shift, the fractional part a gain.
        let mut shift: i32 = 16i32.wrapping_sub(i32::from(lg) >> DB_SHIFT);
        let mut gain: i16 = if shift > 31 {
            shift = 0;
            0
        } else {
            celt_exp2_frac((i32::from(lg) & ((1 << DB_SHIFT) - 1)) as i16) as i16
        };
        let samples = freq.get_mut(low..high).unwrap_or_default().iter_mut().zip(x.get(low..high).unwrap_or_default());
        if shift < 0 {
            // A gain this large is likely to overflow, and is capped (at a log energy of 18); it
            // means the stream is already corrupt.
            if shift <= -2 {
                gain = 16384;
                shift = -2;
            }
            // The shift's direction is the branch's; its magnitude the shift's.
            for (f, &v) in samples {
                *f = shl32(mult16_16(i32::from(v), i32::from(gain)), shift.unsigned_abs());
            }
        } else {
            for (f, &v) in samples {
                *f = shr32(mult16_16(i32::from(v), i32::from(gain)), shift.unsigned_abs());
            }
        }
    }
    first < bound
}

/// What the synthesis follows of a frame: its duration, and from its header (RFC 6716 §4.3,
/// Table 56) whether it is a transient, of short blocks, or silence.
#[derive(Clone, Copy)]
pub struct FrameLayout {
    /// The frame's duration.
    pub duration: FrameDuration,
    /// Whether the frame is a transient, synthesised a 2.5 ms block at a time.
    pub transient: bool,
    /// Whether the frame is silence, every coefficient zero.
    pub silence: bool,
}

/// The synthesis of a frame (`celt_synthesis`): the shapes `x` and energies `log_energy` of the
/// bands `bands` into the coefficients `freq`, then, through one long inverse MDCT or a short
/// one for each 2.5 ms block of a transient, into `out`, the frame's place in the history with
/// the previous frame's overlap before it, saturated.
pub fn synthesise(
    x: &[i16],
    freq: &mut [i32],
    log_energy: &[i16; NB_BANDS],
    bands: Range<usize>,
    layout: FrameLayout,
    out: &mut [i32],
) {
    let FrameLayout { duration, transient, silence } = layout;
    let n = duration.samples();
    let any = denormalise_bands(x, freq, log_energy, bands.start, bands.end, duration.lm(), silence);
    let (mdct, blocks) = match (transient, duration) {
        (true, _) => (&MDCT_240, duration.blocks()),
        (false, FrameDuration::TwentyMs) => (&MDCT_1920, 1),
        (false, FrameDuration::FiveMs) => (&MDCT_480, 1),
        (false, FrameDuration::TwoAndAHalfMs) => (&MDCT_240, 1),
    };
    for (block, offset) in (0..n).step_by(SHORT_BLOCK).take(blocks).enumerate() {
        let Some(block_out) = out.get_mut(offset..) else {
            break;
        };
        if any {
            inverse_mdct(mdct, freq.get(block..n).unwrap_or_default(), blocks, block_out);
        } else {
            inverse_mdct_of_zeros(mdct, block_out);
        }
    }
    for sample in out.iter_mut().take(n) {
        *sample = saturate(*sample, SIG_SAT);
    }
}

/// `deemphasis` (RFC 6716 §4.3.7.2): the synthesised `signal` through the de-emphasis filter,
/// whose state is `memory`, into `pcm`, or added to it when `accum`.
pub fn deemphasis(signal: &[i32], pcm: &mut [i16], memory: &mut i32, accum: bool) {
    let mut m = *memory;
    for (out, &x) in pcm.iter_mut().zip(signal) {
        let y = x.wrapping_add(m);
        m = mult16_32_q15(PREEMPHASIS, y);
        *out = if accum { sat16(add32(i32::from(*out), i32::from(sig2word16(y)))) } else { sig2word16(y) };
    }
    *memory = m;
}
