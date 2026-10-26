//! The inverse MDCT with its windowed overlap (RFC 6716 §4.3.7): an N/4-point complex FFT between
//! a pre- and a post-rotation, then the time-domain aliasing cancellation of the overlap. The
//! pre-rotation stores its output in the FFT's bit-reversed input order, so the FFT needs no
//! reordering pass of its own.

use super::fft::{FFT_60, FFT_120, FFT_480, FftPlan, fft};
use crate::celt::OVERLAP;
use crate::celt::tables::{MDCT_TWIDDLES_240, MDCT_TWIDDLES_480, MDCT_TWIDDLES_1920, WINDOW};
use crate::fixed::{add32_ovflw, mult16_32_q15, sub32_ovflw};
use crate::slices::{strided, strided_down};

/// One inverse MDCT's size, twiddles and FFT.
pub struct Mdct {
    /// N: the transform's outputs, twice its inputs.
    n: usize,
    /// The pre- and post-rotations' twiddles, N/2 of them, Q15.
    trig: &'static [i16],
    fft: &'static FftPlan,
}

impl Mdct {
    /// A transform of `n` outputs, checked when compiled: its twiddles and its FFT fit `n`, and
    /// N/4 is even, so that the post-rotation's two ends never meet in one value.
    const fn new(n: usize, trig: &'static [i16], fft: &'static FftPlan) -> Self {
        assert!(trig.len() == n / 2 && n.is_multiple_of(8));
        Self { n, trig, fft }
    }
}

/// The 20 ms frame's long block: 960 coefficients.
pub static MDCT_1920: Mdct = Mdct::new(1920, &MDCT_TWIDDLES_1920, &FFT_480);
/// The 5 ms frame's long block: 240 coefficients.
pub static MDCT_480: Mdct = Mdct::new(480, &MDCT_TWIDDLES_480, &FFT_120);
/// A 2.5 ms block, short or a frame of its own: 120 coefficients.
pub static MDCT_240: Mdct = Mdct::new(240, &MDCT_TWIDDLES_240, &FFT_60);

/// `S_MUL`: a sample times a Q15 twiddle.
#[inline]
const fn s_mul(a: i32, b: i16) -> i32 {
    mult16_32_q15(b as i32, a)
}

/// `clt_mdct_backward`: the inverse MDCT of the N/2 coefficients `input[0]`, `input[stride]`,
/// ..., with the overlap's mirror. The transform writes `out[OVERLAP/2 .. OVERLAP/2 + N/2]`, then
/// the mirror folds `out[..OVERLAP]`, whose first half holds the previous block's overlap.
pub fn inverse_mdct(mdct: &Mdct, input: &[i32], stride: usize, out: &mut [i32]) {
    let n2 = mdct.n / 2;
    let n4 = mdct.n / 4;
    let half_overlap = OVERLAP / 2;
    let Some((twiddles_low, twiddles_high)) = mdct.trig.split_at_checked(n4) else {
        return;
    };
    let Some(window) = out.get_mut(half_overlap..).and_then(|rest| rest.get_mut(..n2)) else {
        return;
    };
    let (buffer, _) = window.as_chunks_mut::<2>();

    // Pre-rotate, from both ends of the input at once, into the FFT's input order. Real and
    // imaginary swap, since an FFT stands in for the inverse FFT.
    // The coefficients are every stride'th input value, n2 of them: the even ones upwards from
    // the first, the odd ones downwards from the last.
    let pair_step = stride.saturating_mul(2);
    let last = n2.checked_sub(1).and_then(|j| j.checked_mul(stride)).unwrap_or(usize::MAX);
    let from_low = strided(input, 0, pair_step);
    let from_high = strided_down(input, last, pair_step);
    for (((&x1, &x2), (&t0, &t1)), &rev) in
        from_low.zip(from_high).zip(twiddles_low.iter().zip(twiddles_high)).zip(mdct.fft.bitrev)
    {
        let yr = add32_ovflw(s_mul(x2, t0), s_mul(x1, t1));
        let yi = sub32_ovflw(s_mul(x1, t0), s_mul(x2, t1));
        if let Some(value) = buffer.get_mut(usize::from(rev)) {
            *value = [yi, yr];
        }
    }

    fft(mdct.fft, buffer);

    // Post-rotate and de-shuffle from both ends of the buffer at once, in place; real and
    // imaginary swap back. The scaling by 2 is left to the window.
    let Some((front, back)) = buffer.split_at_mut_checked(n4 / 2) else {
        return;
    };
    let front_twiddles = twiddles_low.iter().zip(twiddles_high);
    let back_twiddles = twiddles_low.iter().rev().zip(twiddles_high.iter().rev());
    for ((low, high), ((&t0, &t1), (&t2, &t3))) in
        front.iter_mut().zip(back.iter_mut().rev()).zip(front_twiddles.zip(back_twiddles))
    {
        let [im, re] = *low;
        low[0] = add32_ovflw(s_mul(re, t0), s_mul(im, t1));
        let yi = sub32_ovflw(s_mul(re, t1), s_mul(im, t0));
        let [im, re] = *high;
        high[1] = yi;
        high[0] = add32_ovflw(s_mul(re, t2), s_mul(im, t3));
        low[1] = sub32_ovflw(s_mul(re, t3), s_mul(im, t2));
    }

    mirror(out);
}

/// The inverse MDCT of coefficients that are all zero: the transform's output is zero (every
/// step of it is linear), and only the overlap's mirror remains.
pub fn inverse_mdct_of_zeros(mdct: &Mdct, out: &mut [i32]) {
    let half_overlap = OVERLAP / 2;
    out.get_mut(half_overlap..).and_then(|rest| rest.get_mut(..mdct.n / 2)).unwrap_or_default().fill(0);
    mirror(out);
}

/// The mirror on both sides of the overlap, for the time-domain aliasing cancellation.
#[inline]
fn mirror(out: &mut [i32]) {
    let half_overlap = OVERLAP / 2;
    let Some(overlap) = out.get_mut(..OVERLAP) else {
        return;
    };
    let (head, tail) = overlap.split_at_mut(half_overlap);
    let (window_low, window_high) = WINDOW.split_at(half_overlap);
    for ((x2, x1), (&w1, &w2)) in
        head.iter_mut().zip(tail.iter_mut().rev()).zip(window_low.iter().zip(window_high.iter().rev()))
    {
        let (w1, w2) = (i32::from(w1), i32::from(w2));
        let (a, b) = (*x2, *x1);
        *x2 = sub32_ovflw(mult16_32_q15(w2, a), mult16_32_q15(w1, b));
        *x1 = add32_ovflw(mult16_32_q15(w1, a), mult16_32_q15(w2, b));
    }
}
