//! LPC filters and their coefficients: the prediction-gain limit, the range limit and the
//! bandwidth expansion of RFC 6716 §4.2.7.5.7 and §4.2.7.5.8, the analysis (whitening) filter
//! that rebuilds the LTP state, and the synthesis filter of §4.2.7.9.2, the last two
//! specialised for each order.
//!
//! From the reference decoder's `silk/LPC_inv_pred_gain.c`, `LPC_fit.c`, `bwexpander.c`,
//! `bwexpander_32.c`, `LPC_analysis_filter.c`, and the synthesis loops of `decode_core.c`, `PLC.c`
//! and `CNG.c`. Copyright (c) 2006-2011, Skype Limited, BSD-3-Clause (the licence beside the
//! crate).

use super::layout::{LpcOrder, MAX_LPC_ORDER};
use super::macros::{
    abs, add_sat32, add32, clz32, inverse32_varq, lshift, lshift_sat32, mul, positive_div, rshift, rshift_round,
    rshift_round64, sat16, smlabb_ovflw, smlawb, smmul, smull, smulww, sub_sat32, sub32,
};

/// `QA` of `LPC_inv_pred_gain.c`.
const QA: i32 = 24;
/// `A_LIMIT`: `SILK_FIX_CONST( 0.99975, QA )` = (int)(0.99975 * 2^24 + 0.5).
const A_LIMIT: i32 = 16_773_022;
/// `SILK_FIX_CONST( 1, 30 )`.
const ONE_Q30: i32 = 1 << 30;
/// `SILK_FIX_CONST( 1.0f / MAX_PREDICTION_POWER_GAIN, 30 )`, `MAX_PREDICTION_POWER_GAIN` 1e4f:
/// (int)((float)(1.0f / 1e4f) * 2^30 + 0.5).
const MIN_INV_GAIN_Q30: i32 = 107_374;

/// `MUL32_FRAC_Q`.
#[inline]
const fn mul32_frac_q(a32: i32, b32: i32, q: i32) -> i32 {
    rshift_round64(smull(a32, b32), q) as i32
}

/// `silk_LPC_inverse_pred_gain_c`: the inverse prediction gain of the Q12 coefficients `a_q12`
/// in the energy domain, Q30, or 0 if the filter is unstable (a pole on or outside the unit
/// circle, or too close to it), by the step-down recursion of RFC 6716 §4.2.7.5.8.
#[must_use]
pub fn inverse_prediction_gain(a_q12: &[i16]) -> i32 {
    let mut a_qa = [0i32; MAX_LPC_ORDER];
    let a_qa = a_qa.get_mut(..a_q12.len().min(MAX_LPC_ORDER)).unwrap_or_default();

    // Increase Q domain of the AR coefficients; if the DC is unstable, we don't even need to do
    // the full calculations
    let mut dc_resp: i32 = 0;
    for (q, &a) in a_qa.iter_mut().zip(a_q12) {
        dc_resp = dc_resp.wrapping_add(i32::from(a));
        *q = lshift(i32::from(a), QA - 12);
    }
    if dc_resp >= 4096 {
        return 0;
    }

    let mut inv_gain_q30 = ONE_Q30;
    // From the last coefficient down: each step takes the last of `rest` and updates the others
    let mut rest = a_qa;
    while let Some((&mut last, head)) = rest.split_last_mut() {
        // Check for stability
        if !(-A_LIMIT..=A_LIMIT).contains(&last) {
            return 0;
        }

        // Set RC equal to negated AR coef
        let rc_q31 = lshift(last, 31 - QA).wrapping_neg();

        // rc_mult1_Q30 range: [ 1 : 2^30 ]
        let rc_mult1_q30 = sub32(ONE_Q30, smmul(rc_q31, rc_q31));

        // Update inverse gain; invGain_Q30 range: [ 0 : 2^30 ]
        inv_gain_q30 = lshift(smmul(inv_gain_q30, rc_mult1_q30), 2);
        if inv_gain_q30 < MIN_INV_GAIN_Q30 {
            return 0;
        }
        if head.is_empty() {
            break;
        }

        // rc_mult2 range: [ 2^30 : silk_int32_MAX ]
        let mult2q = 32i32.wrapping_sub(clz32(abs(rc_mult1_q30)));
        let rc_mult2 = inverse32_varq(rc_mult1_q30, mult2q.wrapping_add(30));

        // Update AR coefficient: each from the front with its mirror from the back, and the
        // middle one of an odd count with itself
        let k = head.len();
        let (front, back) = head.split_at_mut(k / 2);
        for (low, high) in front.iter_mut().zip(back.iter_mut().rev()) {
            let Some(new_low) = step_down(*low, *high, rc_q31, rc_mult2, mult2q) else { return 0 };
            let Some(new_high) = step_down(*high, *low, rc_q31, rc_mult2, mult2q) else { return 0 };
            (*low, *high) = (new_low, new_high);
        }
        if k % 2 == 1
            && let Some(middle) = back.first_mut()
        {
            let Some(new_middle) = step_down(*middle, *middle, rc_q31, rc_mult2, mult2q) else { return 0 };
            *middle = new_middle;
        }
        rest = head;
    }
    inv_gain_q30
}

/// One coefficient of the step-down recursion, or `None` when it no longer fits 32 bits.
#[inline]
fn step_down(a: i32, b: i32, rc_q31: i32, rc_mult2: i32, mult2q: i32) -> Option<i32> {
    let tmp64 = rshift_round64(smull(sub_sat32(a, mul32_frac_q(b, rc_q31, 31)), rc_mult2), mult2q);
    i32::try_from(tmp64).ok()
}

/// `silk_LPC_fit`: converts the `qin` coefficients `a_qin` to `qout` 16-bit ones in `a_qout`,
/// bandwidth-expanding them until they fit and clipping them if they still do not after 10
/// rounds (RFC 6716 §4.2.7.5.7).
pub fn fit(a_qout: &mut [i16], a_qin: &mut [i32], qout: i32, qin: i32) {
    let shift = qin.wrapping_sub(qout);
    let mut fitted = false;
    for _ in 0..10 {
        // Find maximum absolute value and its index
        let mut maxabs: i32 = 0;
        let mut idx: i32 = 0;
        for (k, &a) in (0..).zip(a_qin.iter()) {
            let absval = abs(a);
            if absval > maxabs {
                maxabs = absval;
                idx = k;
            }
        }
        maxabs = rshift_round(maxabs, shift);

        if maxabs > i32::from(i16::MAX) {
            // Reduce magnitude of prediction coefficients
            maxabs = maxabs.min(163_838); // ( silk_int32_MAX >> 14 ) + silk_int16_MAX = 163838
            // SILK_FIX_CONST( 0.999, 16 ) = (int)(0.999 * 65536 + 0.5) = 65470
            // The divisor is at least 32768 / 4, which `max` says so that it needs no check
            let chirp_q16 = sub32(
                65470,
                positive_div(
                    lshift(sub32(maxabs, i32::from(i16::MAX)), 14),
                    rshift(mul(maxabs, idx.wrapping_add(1)), 2).max(1),
                ),
            );
            bandwidth_expand_32(a_qin, chirp_q16);
        } else {
            fitted = true;
            break;
        }
    }

    if fitted {
        for (out, &a) in a_qout.iter_mut().zip(a_qin.iter()) {
            *out = rshift_round(a, shift) as i16;
        }
    } else {
        // Reached the last iteration, clip the coefficients
        for (out, a) in a_qout.iter_mut().zip(a_qin.iter_mut()) {
            *out = sat16(rshift_round(*a, shift)) as i16;
            *a = lshift(i32::from(*out), shift);
        }
    }
}

/// `silk_bwexpander`: chirps (bandwidth-expands) the LP filter `ar` by `chirp_q16`.
pub fn bandwidth_expand(ar: &mut [i16], mut chirp_q16: i32) {
    let chirp_minus_one_q16 = sub32(chirp_q16, 65536);
    // NB: Dont use silk_SMULWB, instead of silk_RSHIFT_ROUND( silk_MUL(), 16 ), below.
    // Bias in silk_SMULWB can lead to unstable filters
    let Some((last, rest)) = ar.split_last_mut() else { return };
    for a in rest {
        *a = rshift_round(mul(chirp_q16, i32::from(*a)), 16) as i16;
        chirp_q16 = add32(chirp_q16, rshift_round(mul(chirp_q16, chirp_minus_one_q16), 16));
    }
    *last = rshift_round(mul(chirp_q16, i32::from(*last)), 16) as i16;
}

/// `silk_bwexpander_32`: `bandwidth_expand` of 32-bit coefficients.
pub fn bandwidth_expand_32(ar: &mut [i32], mut chirp_q16: i32) {
    let chirp_minus_one_q16 = sub32(chirp_q16, 65536);
    let Some((last, rest)) = ar.split_last_mut() else { return };
    for a in rest {
        *a = smulww(chirp_q16, *a);
        chirp_q16 = add32(chirp_q16, rshift_round(mul(chirp_q16, chirp_minus_one_q16), 16));
    }
    *last = smulww(chirp_q16, *last);
}

/// `silk_LPC_analysis_filter`: whitens `input` with the order-`ORDER` filter `b` into `out`
/// (the same length), starting from a zero state; the first `ORDER` outputs are zero.
pub fn analysis_filter<const ORDER: usize>(out: &mut [i16], input: &[i16], b: &[i16; ORDER]) {
    let (head, tail) = out.split_at_mut(ORDER.min(out.len()));
    head.fill(0);
    for ((past, &current), out) in input.array_windows::<ORDER>().zip(input.iter().skip(ORDER)).zip(tail) {
        // Allowing wrap around so that two wraps can cancel each other. The rare cases where the
        // result wraps around can only be triggered by invalid streams
        let mut prediction_q12: i32 = 0;
        for (&x, &coefficient) in past.iter().rev().zip(b) {
            prediction_q12 = smlabb_ovflw(prediction_q12, i32::from(x), i32::from(coefficient));
        }
        // Subtract prediction, scale to Q0 and saturate
        let residual_q12 = sub32(lshift(i32::from(current), 12), prediction_q12);
        *out = sat16(rshift_round(residual_q12, 12)) as i16;
    }
}

/// `analysis_filter` for a runtime order, the coefficients `b[..order]`.
pub fn analysis_filter_of_order(order: LpcOrder, out: &mut [i16], input: &[i16], b: &[i16; MAX_LPC_ORDER]) {
    match order {
        LpcOrder::Ten => {
            if let Some(b) = b.first_chunk::<10>() {
                analysis_filter::<10>(out, input, b);
            }
        }
        LpcOrder::Sixteen => analysis_filter::<16>(out, input, b),
    }
}

/// The short-term synthesis of RFC 6716 §4.2.7.9.2 with the order-`ORDER` filter `a_q12`, in
/// place: `signal` holds `MAX_LPC_ORDER` samples of filter state, Q14, then the excitation,
/// which each step replaces with the output, Q14, and hands to `emit` with its index.
#[inline]
fn synthesis<const ORDER: usize>(signal: &mut [i32], a_q12: &[i16; ORDER], mut emit: impl FnMut(usize, i32)) {
    // Sample `i` of the output is at `end`, after the state and the `i` outputs before it
    for (i, end) in (MAX_LPC_ORDER..signal.len()).enumerate() {
        let Some((past, current)) = signal.split_at_mut_checked(end) else { return };
        let (Some(past), Some(current)) = (past.last_chunk::<ORDER>(), current.first_mut()) else { return };
        // Avoids introducing a bias because silk_SMLAWB() always rounds to -inf
        #[allow(clippy::cast_possible_wrap, reason = "`ORDER` is an LPC order, 10 or 16")]
        let mut prediction_q10 = (ORDER >> 1) as i32;
        for (&x, &coefficient) in past.iter().rev().zip(a_q12) {
            prediction_q10 = smlawb(prediction_q10, x, i32::from(coefficient));
        }
        *current = add_sat32(*current, lshift_sat32(prediction_q10, 4));
        emit(i, *current);
    }
}

/// `synthesis` for a runtime order, the coefficients `a_q12[..order]`.
#[inline]
pub fn synthesis_of_order(
    order: LpcOrder,
    signal: &mut [i32],
    a_q12: &[i16; MAX_LPC_ORDER],
    emit: impl FnMut(usize, i32),
) {
    match order {
        LpcOrder::Ten => {
            if let Some(a_q12) = a_q12.first_chunk::<10>() {
                synthesis::<10>(signal, a_q12, emit);
            }
        }
        LpcOrder::Sixteen => synthesis::<16>(signal, a_q12, emit),
    }
}
