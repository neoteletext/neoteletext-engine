//! The concealment's pitch search: the history low-passed, whitened and decimated by two, then
//! searched at a further decimation by two, refined at the first, and interpolated. Every sum
//! wraps at 32 bits, as two's complement does.

use super::lpc::{autocorr, lpc};
use crate::celt::pvq::inner_product;
use crate::fixed::{
    Q15ONE, SIG_SHIFT, add32, extract16, mac16_16, max32, mult16_16, mult16_16_q15, mult16_32_q15, round16, shl32,
    shr32, sub32, vshr32,
};
use crate::math::{celt_ilog2, celt_maxabs16, celt_maxabs32};

/// `QCONST16(.7f,15)`.
const Q15_0_7: i32 = 22938;

/// `celt_pitch_xcorr`: `xcorr[i]`, the correlation of `x` with `y` from lag `i`, for each lag
/// `xcorr` holds (over the values of `x`, which the product takes as the shorter); answers the
/// largest, at least 1.
pub fn pitch_xcorr(x: &[i16], y: &[i16], xcorr: &mut [i32]) -> i32 {
    xcorr.iter_mut().enumerate().fold(1, |max, (lag, out)| {
        *out = inner_product(x, y.get(lag..).unwrap_or_default());
        max32(max, *out)
    })
}

/// `find_best_pitch`: the two lags of `xcorr` with the best normalised correlation against `y`
/// (`len` values from each lag), each energy shifted down by `yshift`.
#[must_use]
pub fn find_best_pitch(xcorr: &[i32], y: &[i16], len: usize, yshift: u32, maxcorr: i32) -> [i32; 2] {
    let mut best_pitch = [0, 1];
    let mut best_num: [i16; 2] = [-1, -1];
    let mut best_den: [i32; 2] = [0, 0];
    let xshift = i32::from(celt_ilog2(maxcorr)).wrapping_sub(14);
    let energy = |v: i16| shr32(mult16_16(i32::from(v), i32::from(v)), yshift);
    let mut syy: i32 = y.iter().take(len).fold(1, |sum, &v| add32(sum, energy(v)));
    // Each lag's window of `y`: the value leaving it and the one entering.
    let window_ends = y.iter().zip(y.iter().skip(len));
    for ((&corr, lag), (&leaving, &entering)) in xcorr.iter().zip(0i32..).zip(window_ends) {
        if corr > 0 {
            let xcorr16 = i32::from(extract16(vshr32(corr, xshift)));
            let num = mult16_16_q15(xcorr16, xcorr16) as i16;
            if mult16_32_q15(i32::from(num), best_den[1]) > mult16_32_q15(i32::from(best_num[1]), syy) {
                if mult16_32_q15(i32::from(num), best_den[0]) > mult16_32_q15(i32::from(best_num[0]), syy) {
                    best_num[1] = best_num[0];
                    best_den[1] = best_den[0];
                    best_pitch[1] = best_pitch[0];
                    best_num[0] = num;
                    best_den[0] = syy;
                    best_pitch[0] = lag;
                } else {
                    best_num[1] = num;
                    best_den[1] = syy;
                    best_pitch[1] = lag;
                }
            }
        }
        syy = max32(1, add32(syy, sub32(energy(entering), energy(leaving))));
    }
    best_pitch
}

/// `celt_fir5`: the five-tap FIR of `pitch_downsample`, in place.
pub fn fir5(x: &mut [i16], num: &[i16; 5]) {
    let num = num.map(i32::from);
    let mut mem = [0i32; 5];
    for v in x {
        let input = i32::from(*v);
        let sum = num.iter().zip(&mem).fold(shl32(input, SIG_SHIFT), |sum, (&a, &m)| mac16_16(sum, a, m));
        mem = [input, mem[0], mem[1], mem[2], mem[3]];
        *v = round16(sum, SIG_SHIFT);
    }
}

/// `pitch_downsample`, mono: `x` to `x_lp` (half as many values), low-passed and decimated by
/// two, then whitened by a fourth-order prediction with an added zero. `scratch` holds
/// `x_lp.len()` values.
pub fn pitch_downsample(x: &[i32], x_lp: &mut [i16], scratch: &mut [i16]) {
    let half = x_lp.len();
    let x = x.get(..half.saturating_mul(2)).unwrap_or_default();
    let maxabs = celt_maxabs32(x).max(1);
    let shift = i32::from(celt_ilog2(maxabs)).wrapping_sub(10).max(0);
    let (s1, s2) = (shift.wrapping_add(1).cast_unsigned(), shift.wrapping_add(2).cast_unsigned());
    let (Some((first, rest)), [x0, after_x0 @ ..]) = (x_lp.split_first_mut(), x) else {
        return;
    };
    let Some(x1) = after_x0.first() else {
        return;
    };
    *first = add32(shr32(*x1, s2), shr32(*x0, s1)) as i16;
    // Output `i` from inputs `2i - 1`, `2i` and `2i + 1`.
    for (out, triple) in rest.iter_mut().zip(after_x0.windows(3).step_by(2)) {
        if let [before, centre, after] = *triple {
            *out = add32(add32(shr32(before, s2), shr32(after, s2)), shr32(centre, s1)) as i16;
        }
    }

    let mut ac = [0i32; 5];
    autocorr(x_lp, &mut ac, false, scratch);
    // A noise floor at -40 dB.
    ac[0] = add32(ac[0], shr32(ac[0], 13));
    // Lag windowing: ac[i] *= exp(-.5*(2*pi*.002*i)^2).
    for (i, v) in (1i32..).zip(ac.iter_mut().skip(1)) {
        *v = sub32(*v, mult16_32_q15(i.wrapping_mul(i).wrapping_mul(2), *v));
    }
    let mut coefficients = [0i16; 4];
    lpc(&mut coefficients, &ac);
    let mut tmp: i16 = Q15ONE;
    for c in &mut coefficients {
        // QCONST16(.9f,15)
        tmp = mult16_16_q15(29491, i32::from(tmp)) as i16;
        *c = mult16_16_q15(i32::from(*c), i32::from(tmp)) as i16;
    }
    // Add a zero: QCONST16(.8f,15), and .8 in Q12 for the first tap.
    let c1: i32 = 26214;
    let [l0, l1, l2, l3] = coefficients.map(i32::from);
    let with_zero = [
        l0.wrapping_add(3277) as i16,
        l1.wrapping_add(mult16_16_q15(c1, l0)) as i16,
        l2.wrapping_add(mult16_16_q15(c1, l1)) as i16,
        l3.wrapping_add(mult16_16_q15(c1, l2)) as i16,
        mult16_16_q15(c1, l3) as i16,
    ];
    fir5(x_lp, &with_zero);
}

/// `pitch_search`: the lag, at the full rate up to `max_pitch`, at which `y` best matches
/// `x_lp`; both are `pitch_downsample`'s output, at half the rate, `x_lp` holding `len >> 1`
/// values and `y` `(len + max_pitch) >> 1`. `scratch` holds
/// `(len >> 2) + ((len + max_pitch) >> 2)` values and `xcorr` `max_pitch >> 1`.
pub fn pitch_search(
    x_lp: &[i16],
    y: &[i16],
    len: usize,
    max_pitch: usize,
    scratch: &mut [i16],
    xcorr: &mut [i32],
) -> i32 {
    let lag = len.saturating_add(max_pitch);
    // Buffers short of their stated lengths leave nothing to search.
    let (Some((x_lp4, rest)), Some(xcorr), Some(x_lp)) =
        (scratch.split_at_mut_checked(len >> 2), xcorr.get_mut(..max_pitch >> 1), x_lp.get(..len >> 1))
    else {
        return 0;
    };
    let Some(y_lp4) = rest.get_mut(..lag >> 2) else {
        return 0;
    };

    // Decimate by two again.
    for (out, &v) in x_lp4.iter_mut().zip(x_lp.iter().step_by(2)) {
        *out = v;
    }
    for (out, &v) in y_lp4.iter_mut().zip(y.iter().step_by(2)) {
        *out = v;
    }
    let xmax = celt_maxabs16(x_lp4);
    let ymax = celt_maxabs16(y_lp4);
    let mut shift = i32::from(celt_ilog2(max32(1, max32(xmax, ymax)))).wrapping_sub(11);
    if shift > 0 {
        for v in x_lp4.iter_mut().chain(y_lp4.iter_mut()) {
            *v = (i32::from(*v) >> shift) as i16;
        }
        // Double the shift for a product.
        shift = shift.wrapping_mul(2);
    } else {
        shift = 0;
    }

    // The coarse search at a decimation by four.
    let coarse = xcorr.get_mut(..max_pitch >> 2).unwrap_or_default();
    let maxcorr = pitch_xcorr(x_lp4, y_lp4, coarse);
    let [best0, best1] = find_best_pitch(coarse, y_lp4, len >> 2, 0, maxcorr);

    // The finer search at a decimation by two, near the coarse search's two best.
    let mut maxcorr = 1;
    // Doubled for a product, the shift is at least 0.
    let shift = shift.cast_unsigned();
    for ((i, out), lag) in xcorr.iter_mut().enumerate().zip(0i32..) {
        *out = 0;
        let near = |best: i32| lag.wrapping_sub(best.wrapping_mul(2)).unsigned_abs() <= 2;
        if !near(best0) && !near(best1) {
            continue;
        }
        let sum = x_lp
            .iter()
            .zip(y.get(i..).unwrap_or_default())
            .fold(0i32, |sum, (&a, &b)| add32(sum, shr32(mult16_16(i32::from(a), i32::from(b)), shift)));
        *out = max32(-1, sum);
        maxcorr = max32(maxcorr, sum);
    }
    let [b0, _] = find_best_pitch(xcorr, y, len >> 1, shift.wrapping_add(1), maxcorr);

    // Refine by pseudo-interpolation, where the best lag has a lag on each side.
    let neighbours = usize::try_from(b0).ok().and_then(|lag| lag.checked_sub(1)).and_then(|before| xcorr.get(before..));
    let offset = if let Some(&[a, b, c, ..]) = neighbours {
        if sub32(c, a) > mult16_32_q15(Q15_0_7, sub32(b, a)) {
            1
        } else if sub32(a, c) > mult16_32_q15(Q15_0_7, sub32(b, c)) {
            -1
        } else {
            0
        }
    } else {
        0
    };
    b0.wrapping_mul(2).wrapping_sub(offset)
}
