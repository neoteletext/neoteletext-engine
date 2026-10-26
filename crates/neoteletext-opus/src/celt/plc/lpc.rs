//! Linear prediction for the concealment: the autocorrelation, Levinson-Durbin, and the FIR and
//! IIR filters through the prediction. Every sum wraps at 32 bits, as two's complement does, so
//! that its order does not change it.

use crate::celt::OVERLAP;
use crate::celt::pvq::inner_product;
use crate::celt::tables::WINDOW;
use crate::fixed::{
    SIG_SHIFT, abs32, add32, ec_ilog, extract16, mac16_16, min32, mult16_16, mult16_16_q15, mult32_32_32,
    mult32_32_q16, mult32_32_q31, pshr32, shl32, shr32, sround16, sub32,
};
use crate::math::{celt_ilog2, frac_div32};
use crate::slices::copy;

/// `CELT_LPC_ORDER`: the concealment's prediction order.
pub const LPC_ORDER: usize = 24;

/// `_celt_lpc`: Levinson-Durbin from the autocorrelation `ac` (one more value than `lpc_out`)
/// to Q12 prediction coefficients in `lpc_out`. As in the reference, the fallback to `A(z) = 1`
/// writes only `lpc_out[0]`.
pub fn lpc(lpc_out: &mut [i16], ac: &[i32]) {
    // As many coefficients as the autocorrelation has lags past the first.
    let p = lpc_out.len().min(LPC_ORDER).min(ac.len().saturating_sub(1));
    let Some(&ac0) = ac.first() else {
        return;
    };
    let mut error = ac0;
    let mut lpc = [0i32; LPC_ORDER];
    if ac0 != 0 {
        for (i, &next_lag) in ac.iter().skip(1).enumerate().take(p) {
            // This iteration's reflection coefficient.
            // The coefficients so far against the lags `i` down to 1.
            let lags = ac.get(1..=i).unwrap_or_default();
            let mut rr: i32 = lpc.iter().zip(lags.iter().rev()).fold(0, |rr, (&c, &a)| add32(rr, mult32_32_q31(c, a)));
            rr = add32(rr, shr32(next_lag, 6));
            let r = frac_div32(shl32(rr, 6), error).wrapping_neg();
            // Update the coefficients and the error.
            if let Some(coefficient) = lpc.get_mut(i) {
                *coefficient = shr32(r, 6);
            }
            // Each coefficient `j` before `i` with its mirror `i - 1 - j`; the middle one, for an
            // odd `i`, with itself.
            let half = i / 2;
            let Some((front, rest)) = lpc.get_mut(..i).and_then(|c| c.split_at_mut_checked(half)) else {
                break;
            };
            let Some((middle, back)) = rest.split_at_mut_checked(i & 1) else {
                break;
            };
            for (a, b) in front.iter_mut().zip(back.iter_mut().rev()) {
                let (tmp1, tmp2) = (*a, *b);
                *a = add32(tmp1, mult32_32_q31(r, tmp2));
                *b = add32(tmp2, mult32_32_q31(r, tmp1));
            }
            for c in middle {
                *c = add32(*c, mult32_32_q31(r, *c));
            }
            error = sub32(error, mult32_32_q31(mult32_32_q31(r, r), error));
            // Bail out once we get 30 dB gain.
            if error <= shr32(ac0, 10) {
                break;
            }
        }
    }
    // Fit the 32-bit coefficients to 16 bits by bandwidth expansion, as SILK fits its own.
    let lpc = lpc.get_mut(..p).unwrap_or_default();
    let mut fitted = false;
    for _ in 0..10 {
        let (index, maxabs) = lpc
            .iter()
            .zip(0i32..)
            .fold((0, 0), |(index, max), (&v, i)| if abs32(v) > max { (i, abs32(v)) } else { (index, max) });
        // Q25 to Q12.
        let maxabs = pshr32(maxabs, 13);
        if maxabs <= 32767 {
            fitted = true;
            break;
        }
        let maxabs = min32(maxabs, 163_838);
        // QCONST32(0.999, 16), less the expansion that brings the largest within range.
        // `DIV32`: past 32767, the largest's product with its place is at least 8191.
        let mut chirp_q16 = sub32(
            65470,
            shl32(sub32(maxabs, 32767), 14)
                .checked_div(shr32(mult32_32_32(maxabs, index.wrapping_add(1)), 2))
                .unwrap_or(0),
        );
        let chirp_minus_one_q16 = sub32(chirp_q16, 65536);
        if let Some((last, rest)) = lpc.split_last_mut() {
            for v in rest {
                *v = mult32_32_q16(chirp_q16, *v);
                chirp_q16 = add32(chirp_q16, pshr32(mult32_32_32(chirp_q16, chirp_minus_one_q16), 16));
            }
            *last = mult32_32_q16(chirp_q16, *last);
        }
    }
    if fitted {
        for (out, &v) in lpc_out.iter_mut().zip(lpc.iter()) {
            // Q25 to Q12.
            *out = extract16(pshr32(v, 13));
        }
    } else if let Some(first) = lpc_out.first_mut() {
        // Still past 16 bits after 10 expansions: A(z) = 1. (The reference clears its 32-bit
        // copy, not the output.)
        *first = 4096;
    }
}

/// `celt_fir`, in place: `signal[num.len()..]` through the FIR `num` (Q12), the first
/// `num.len()` values its history. Each output depends only on inputs before it, so the filter
/// runs from the end back, overwriting only what it no longer reads.
pub fn fir_in_place(signal: &mut [i16], num: &[i16]) {
    let order = num.len();
    for i in (order..signal.len()).rev() {
        // The value and, before it, its history, newest first: as many as the filter has taps.
        let Some((history, [value, ..])) = signal.split_at_mut_checked(i) else {
            continue;
        };
        let sum = num
            .iter()
            .zip(history.iter().rev())
            .fold(shl32(i32::from(*value), SIG_SHIFT), |sum, (&a, &x)| mac16_16(sum, i32::from(a), i32::from(x)));
        *value = sround16(sum, SIG_SHIFT);
    }
}

/// `celt_iir`, in place: `y` through the all-pole filter `den` (Q12), whose state, the last
/// outputs rounded to 16 bits newest first, is `mem` (`den.len()` values). `history` is scratch
/// of `y.len() + den.len()` values. The length of `y` is a multiple of four, as the
/// concealment's always is: past that the reference's unrolled loop and its remainder disagree.
pub fn iir_in_place(y: &mut [i32], den: &[i16], mem: &mut [i16], history: &mut [i16]) {
    let order = den.len();
    debug_assert!(
        y.len().is_multiple_of(4) && mem.len() == order,
        "the concealment's filter runs over multiples of four"
    );
    for (h, &m) in history.iter_mut().take(order).rev().zip(mem.iter()) {
        *h = m;
    }
    for (i, sample) in y.iter_mut().enumerate() {
        // The last `order` outputs, and the place of this one after them.
        let Some((past, [next, ..])) = history.get_mut(i..).and_then(|h| h.split_at_mut_checked(order)) else {
            break;
        };
        let sum = den
            .iter()
            .zip(past.iter().rev())
            .fold(*sample, |sum, (&a, &h)| sum.wrapping_sub(mult16_16(i32::from(a), i32::from(h))));
        *next = sround16(sum, SIG_SHIFT);
        *sample = sum;
    }
    for (m, &v) in mem.iter_mut().zip(y.iter().rev()) {
        *m = v as i16;
    }
}

/// `_celt_autocorr`: the autocorrelation `ac` (lags `0..ac.len()`) of `x`, windowed at both ends
/// by the overlap window when `windowed`, normalised; answers the normalisation's shift.
/// `scratch` holds `x.len()` values.
pub fn autocorr(x: &[i16], ac: &mut [i32], windowed: bool, scratch: &mut [i16]) -> i32 {
    let n = x.len();
    let Some(xx) = scratch.get_mut(..n) else {
        return 0;
    };
    copy(xx, x);
    if windowed {
        // Each window value at a place from the front and its mirror from the back, in turn, as
        // the reference writes them where the two ends meet.
        let places = WINDOW.iter().enumerate().zip((0..n).rev()).take(OVERLAP.min(n));
        for ((front, &w), back) in places {
            for at in [front, back] {
                if let (Some(out), Some(&v)) = (xx.get_mut(at), x.get(at)) {
                    *out = mult16_16_q15(i32::from(v), i32::from(w)) as i16;
                }
            }
        }
    }
    let mut shift: i32;
    {
        let square = |v: i16| shr32(mult16_16(i32::from(v), i32::from(v)), 9);
        // As the reference sums: the first value alone for an odd length, then in pairs.
        #[allow(clippy::cast_possible_wrap, reason = "the concealment's buffers are at most 1024 samples")]
        let mut ac0: i32 = ((n as i32) << 7).wrapping_add(1);
        let (odd, pairs) = xx.split_at(n & 1);
        for &v in odd {
            ac0 = add32(ac0, square(v));
        }
        for &[a, b] in pairs.as_chunks::<2>().0 {
            ac0 = add32(ac0, square(a));
            ac0 = add32(ac0, square(b));
        }
        // Halved as C divides, towards zero.
        #[allow(clippy::manual_midpoint, reason = "midpoint rounds down, the reference towards zero")]
        {
            shift = i32::from(celt_ilog2(ac0)).wrapping_sub(30).wrapping_add(10) / 2;
        }
    }
    if shift > 0 {
        for v in xx.iter_mut() {
            *v = pshr32(i32::from(*v), shift.cast_unsigned()) as i16;
        }
    } else {
        shift = 0;
    }
    // The product over the shorter of the two, the values `lag` apart.
    for (lag, out) in ac.iter_mut().enumerate() {
        *out = inner_product(xx, xx.get(lag..).unwrap_or_default());
    }
    shift = shift.wrapping_mul(2);
    let Some(ac0) = ac.first_mut() else {
        return shift;
    };
    if shift <= 0 {
        *ac0 = add32(*ac0, shl32(1, shift.unsigned_abs()));
    }
    let ac0 = *ac0;
    if (1..268_435_456).contains(&ac0) {
        // Under 2^28, the value has at most 28 bits: the shift is at least 1.
        let shift2 = 29i32.wrapping_sub(ec_ilog(ac0.cast_unsigned()));
        for v in ac.iter_mut() {
            *v = shl32(*v, shift2.cast_unsigned());
        }
        shift = shift.wrapping_sub(shift2);
    } else if ac0 <= 0 || ac0 >= 536_870_912 {
        // A lag-0 sum that wrapped past 2^31 reads as negative or zero: it is the largest, and is
        // scaled down as one at 2^30 or more. Only a history louder than any stream has been
        // found to make reaches this; the C's arithmetic is undefined there.
        let shift2: u8 = if ac0 <= 0 || ac0 >= 1_073_741_824 { 2 } else { 1 };
        for v in ac.iter_mut() {
            *v = shr32(*v, u32::from(shift2));
        }
        shift = shift.wrapping_add(i32::from(shift2));
    }
    shift
}
