//! Normalized line spectral frequencies (RFC 6716 §4.2.7.5): the codebooks, the decoding of the
//! two-stage vector quantiser (§4.2.7.5.1 to §4.2.7.5.3), stabilisation (§4.2.7.5.4), and the
//! conversion to LPC coefficients (§4.2.7.5.6 to §4.2.7.5.8).
//!
//! From the reference decoder's `silk/NLSF_decode.c`, `NLSF_unpack.c`, `NLSF_stabilize.c`,
//! `NLSF2A.c`, `sort.c`. Copyright (c) 2006-2011, Skype Limited, BSD-3-Clause (the licence beside
//! the crate).

use super::layout::{LpcOrder, MAX_LPC_ORDER};
use super::lpc::{bandwidth_expand_32, fit, inverse_prediction_gain};
use super::macros::{
    add_lshift32, add_sat16, add32, div32, lshift, mul, rshift, rshift_round, rshift_round64, smlawb, smulbb, smull,
    sub32,
};
use super::tables::LSFCOSTAB_FIX_Q12;
use crate::slices::nth_chunk;

/// `NLSF_QUANT_MAX_AMPLITUDE`: the largest stage-2 index before the extension.
pub const NLSF_QUANT_MAX_AMPLITUDE: u8 = 4;
/// The stage-1 vectors of each codebook.
pub const CB1_VECTORS: usize = 32;
/// The stage-2 index tables: 3 bits of a selection entry choose one.
const EC_TABLES: usize = 8;
/// A stage-2 index table's entries: an index from `-NLSF_QUANT_MAX_AMPLITUDE` to
/// `NLSF_QUANT_MAX_AMPLITUDE`, the ends extended (RFC 6716 §4.2.7.5.2).
const EC_ICDF_LENGTH: usize = 2 * NLSF_QUANT_MAX_AMPLITUDE as usize + 1;
/// `MAX_LPC_STABILIZE_ITERATIONS`.
const MAX_LPC_STABILIZE_ITERATIONS: i32 = 16;
/// `MAX_LOOPS` of `NLSF_stabilize.c`.
const MAX_STABILIZE_LOOPS: usize = 20;

/// `SILK_FIX_CONST( NLSF_QUANT_LEVEL_ADJ, 10 )`, `NLSF_QUANT_LEVEL_ADJ` 0.1: (int)(0.1 * 1024 + 0.5).
const NLSF_QUANT_LEVEL_ADJ_Q10: i32 = 102;

/// `silk_NLSF_CB_struct`, without the encoder's fields.
pub struct NlsfCodebook {
    /// The coefficients of each vector.
    pub order: LpcOrder,
    /// The stage-2 quantisation step, Q16.
    pub quant_step_size_q16: i16,
    /// The stage-1 vectors, Q8, `CB1_VECTORS` of `order` each.
    pub cb1_nlsf_q8: &'static [u8],
    /// The stage-1 vectors' weights for the stage-2 residual, Q9.
    pub cb1_wght_q9: &'static [i16],
    /// The stage-1 index's inverse CDFs, for inactive or unvoiced frames, then voiced ones.
    pub cb1_icdf: &'static [[u8; CB1_VECTORS]; 2],
    /// The backward prediction weights of the stage-2 residual, Q8.
    pub pred_q8: &'static [u8],
    /// Each stage-1 vector's selection of entropy tables and prediction weights.
    pub ec_sel: &'static [u8],
    /// The stage-2 indices' inverse CDFs, one for each of the tables `ec_sel` selects.
    pub ec_icdf: &'static [[u8; EC_ICDF_LENGTH]; EC_TABLES],
    /// The minimum distances between NLSFs and from the borders, Q15: `order + 1` of them.
    pub delta_min_q15: &'static [i16],
}

/// The stage-2 inverse CDF of each coefficient, from `silk_NLSF_unpack`.
pub type EntropyTables = [&'static [u8; EC_ICDF_LENGTH]; MAX_LPC_ORDER];

/// `silk_NLSF_unpack`: the stage-2 entropy tables, `ec_icdf`, and the prediction weights,
/// `pred_q8`, of each coefficient for the stage-1 index `cb1_index`. Each selection entry
/// serves two coefficients.
pub fn unpack(
    ec_icdf: &mut EntropyTables,
    pred_q8: &mut [u8; MAX_LPC_ORDER],
    codebook: &'static NlsfCodebook,
    cb1_index: usize,
) {
    let order = codebook.order.coefficients();
    let selects = nth_chunk(codebook.ec_sel, order / 2, cb1_index).unwrap_or_default();
    // The weights are two rows of `order - 1`, the selected row's entry for each coefficient
    // (the last coefficient's weight multiplies 0, the C reading past the row)
    let rows = codebook.pred_q8.split_at_checked(order.saturating_sub(1)).unwrap_or_default();
    let weight = |row: u8, i: usize| if row & 1 == 0 { rows.0 } else { rows.1 }.get(i).copied().unwrap_or(0);
    // Three bits of a selection entry choose one of the `EC_TABLES` tables
    let table = |bits: u8| codebook.ec_icdf.get(usize::from(bits & 7)).unwrap_or(&codebook.ec_icdf[0]);
    let (icdf_pairs, _) = ec_icdf.as_chunks_mut::<2>();
    let (pred_pairs, _) = pred_q8.as_chunks_mut::<2>();
    let coefficient_pairs = (0..).step_by(2).zip((1..).step_by(2));
    for ((even, odd), ((icdf, pred), &entry)) in
        coefficient_pairs.zip(icdf_pairs.iter_mut().zip(pred_pairs).zip(selects))
    {
        *icdf = [table(entry >> 1), table(entry >> 5)];
        *pred = [weight(entry, even), weight(entry >> 4, odd)];
    }
}

/// `silk_NLSF_decode`: the NLSF vector of the stage-1 index `nlsf_indices[0]` and the stage-2
/// residual indices `nlsf_indices[1..]`, stabilised, into `nlsf_q15[..order]`.
pub fn decode(
    nlsf_q15: &mut [i16; MAX_LPC_ORDER],
    nlsf_indices: &[i8; MAX_LPC_ORDER + 1],
    codebook: &'static NlsfCodebook,
) {
    let mut pred_q8 = [0u8; MAX_LPC_ORDER];
    let mut ec_icdf = [&codebook.ec_icdf[0]; MAX_LPC_ORDER];
    let order = codebook.order.coefficients();
    let [cb1_index, stage2_indices @ ..] = nlsf_indices;
    #[allow(
        clippy::cast_sign_loss,
        reason = "the stage-1 index is decoded from a table of the codebook's 32 vectors, so 0 to 31"
    )]
    let cb1_index = *cb1_index as usize;

    // Unpack entropy table indices and predictor for current CB1 index
    unpack(&mut ec_icdf, &mut pred_q8, codebook, cb1_index);

    // `silk_NLSF_residual_dequant`: the predictive dequantiser of the residual, backwards
    let mut res_q10 = [0i16; MAX_LPC_ORDER];
    let mut out_q10: i32 = 0;
    for ((res, &index), &pred) in res_q10.iter_mut().zip(stage2_indices).zip(&pred_q8).take(order).rev() {
        let pred_q10 = rshift(smulbb(out_q10, i32::from(pred)), 8);
        out_q10 = lshift(i32::from(index), 10);
        if out_q10 > 0 {
            out_q10 = sub32(out_q10, NLSF_QUANT_LEVEL_ADJ_Q10);
        } else if out_q10 < 0 {
            out_q10 = add32(out_q10, NLSF_QUANT_LEVEL_ADJ_Q10);
        }
        out_q10 = smlawb(pred_q10, out_q10, i32::from(codebook.quant_step_size_q16));
        *res = out_q10 as i16;
    }

    // Apply inverse square-rooted weights to first stage and add to output
    let elements = nth_chunk(codebook.cb1_nlsf_q8, order, cb1_index).unwrap_or_default();
    let weights = nth_chunk(codebook.cb1_wght_q9, order, cb1_index).unwrap_or_default();
    for (((nlsf, &res), &element), &weight) in nlsf_q15.iter_mut().zip(&res_q10).zip(elements).zip(weights) {
        // The weights are 1838 to 5227, which `max` says so that the division needs no check
        let weight = i32::from(weight).max(1);
        let nlsf_q15_tmp = add_lshift32(div32(lshift(i32::from(res), 14), weight), i32::from(element), 7);
        *nlsf = nlsf_q15_tmp.clamp(0, 32767) as i16;
    }

    if let Some(nlsf_q15) = nlsf_q15.get_mut(..order) {
        stabilize(nlsf_q15, codebook.delta_min_q15);
    }
}

/// `silk_NLSF_stabilize`: moves the NLSFs further apart where they are closer than
/// `delta_min_q15` (one more entry than `nlsf_q15`, the last the distance from the top), and
/// away from the borders; the output is sorted (RFC 6716 §4.2.7.5.4).
pub fn stabilize(nlsf_q15: &mut [i16], delta_min_q15: &[i16]) {
    let l = nlsf_q15.len();
    let Some(delta_min_q15) = delta_min_q15.get(..=l) else { return };
    // The distances are from a codebook's table, `l + 1` of them; the first is from the bottom,
    // the last from the top
    let (Some(&first_delta), Some(&last_delta)) = (delta_min_q15.first(), delta_min_q15.last()) else { return };
    let (first_delta, last_delta) = (i32::from(first_delta), i32::from(last_delta));
    // The distances between neighbours, the `l - 1` in the middle
    let inner_deltas = || delta_min_q15.iter().skip(1).map(|&d| i32::from(d));
    // NLSFs and distances are 16-bit, so no sum or difference of them wraps an `i32`
    for _ in 0..MAX_STABILIZE_LOOPS {
        let (Some(&first), Some(&last)) = (nlsf_q15.first(), nlsf_q15.last()) else { return };
        // Find smallest distance: first element
        let mut min_diff_q15 = i32::from(first).wrapping_sub(first_delta);
        let mut ii = 0usize;
        // Middle elements
        for (i, (&[low, high], delta)) in (1..).zip(nlsf_q15.array_windows::<2>().zip(inner_deltas())) {
            let diff_q15 = i32::from(high).wrapping_sub(i32::from(low).wrapping_add(delta));
            if diff_q15 < min_diff_q15 {
                min_diff_q15 = diff_q15;
                ii = i;
            }
        }
        // Last element
        let diff_q15 = (1i32 << 15).wrapping_sub(i32::from(last).wrapping_add(last_delta));
        if diff_q15 < min_diff_q15 {
            min_diff_q15 = diff_q15;
            ii = l;
        }

        // Now check if the smallest distance non-negative
        if min_diff_q15 >= 0 {
            return;
        }

        if ii == 0 {
            // Move away from lower limit
            if let Some(first) = nlsf_q15.first_mut() {
                *first = first_delta as i16;
            }
        } else if ii == l {
            // Move away from higher limit
            if let Some(last) = nlsf_q15.last_mut() {
                *last = (1i32 << 15).wrapping_sub(last_delta) as i16;
            }
        } else {
            // `ii` is between 1 and `l - 1` here: the distance between NLSFs `ii - 1` and `ii`
            let delta = delta_min_q15.get(ii).map_or(0, |&d| i32::from(d));
            // Find the lower extreme for the location of the current center frequency
            let below: i32 = delta_min_q15.iter().take(ii).map(|&d| i32::from(d)).sum();
            let min_center_q15 = below.wrapping_add(rshift(delta, 1));

            // Find the upper extreme for the location of the current center frequency
            let above: i32 = delta_min_q15.iter().skip(ii).skip(1).map(|&d| i32::from(d)).sum();
            let max_center_q15 = (1i32 << 15).wrapping_sub(above).wrapping_sub(rshift(delta, 1));

            // Move apart, sorted by value, keeping the same center frequency; `silk_LIMIT_32`
            // takes its bounds in either order
            if let Some([low, high]) = nlsf_q15.get_mut(ii.saturating_sub(1)..=ii) {
                let center = rshift_round(i32::from(*low).wrapping_add(i32::from(*high)), 1);
                let center_freq_q15 = limit(center, min_center_q15, max_center_q15) as i16;
                *low = i32::from(center_freq_q15).wrapping_sub(rshift(delta, 1)) as i16;
                *high = i32::from(*low).wrapping_add(delta) as i16;
            }
        }
    }

    // Safe and simple fall back method, which is less ideal than the above: insertion sort
    // (fast for already almost sorted arrays)
    sort_increasing(nlsf_q15);

    // First NLSF should be no less than NDeltaMin[0]; keep delta_min distance between the
    // NLSFs, each from the one before as it now is
    let mut previous = first_delta;
    for (i, nlsf) in nlsf_q15.iter_mut().enumerate() {
        let floor = if i == 0 {
            first_delta
        } else {
            i32::from(add_sat16(previous, delta_min_q15.get(i).map_or(0, |&d| i32::from(d))))
        };
        *nlsf = i32::from(*nlsf).max(floor) as i16;
        previous = i32::from(*nlsf);
    }

    // Last NLSF should be no higher than 1 - NDeltaMin[L]; keep NDeltaMin distance between the
    // NLSFs, each from the one after as it now is: NLSF `i` and the distance `i + 1` above it
    let mut next = 1i32 << 15;
    for (nlsf, &delta) in nlsf_q15.iter_mut().rev().zip(delta_min_q15.iter().skip(1).rev()) {
        *nlsf = i32::from(*nlsf).min(next.wrapping_sub(i32::from(delta))) as i16;
        next = i32::from(*nlsf);
    }
}

/// `silk_LIMIT`: `a` limited to the range between `limit1` and `limit2`, in either order.
#[inline]
const fn limit(a: i32, limit1: i32, limit2: i32) -> i32 {
    if limit1 > limit2 {
        if a > limit1 {
            limit1
        } else if a < limit2 {
            limit2
        } else {
            a
        }
    } else if a > limit2 {
        limit2
    } else if a < limit1 {
        limit1
    } else {
        a
    }
}

/// `silk_insertion_sort_increasing_all_values_int16`.
pub fn sort_increasing(a: &mut [i16]) {
    for i in 2..=a.len() {
        // The value at the end of the sorted part moves down past each larger one, as in the C,
        // which shifts them up one at a time
        let mut from_end = a.iter_mut().take(i).rev();
        let Some(mut value) = from_end.next() else { return };
        for previous in from_end {
            if *value >= *previous {
                break;
            }
            core::mem::swap(value, previous);
            value = previous;
        }
    }
}

/// `QA` of `NLSF2A.c`.
const QA: i32 = 16;

/// `silk_NLSF2A_find_poly`: the polynomial of the `dd` 2*cos(LSF)s, Q16, interleaved in `c_lsf`
/// from `offset`, every other entry.
fn find_polynomial(out: &mut [i32; MAX_LPC_ORDER / 2 + 1], c_lsf: &[i32; MAX_LPC_ORDER], offset: usize, dd: usize) {
    let mut cos_lsf = c_lsf.iter().skip(offset).step_by(2).take(dd);
    out[0] = lshift(1, QA);
    out[1] = cos_lsf.next().map_or(0, |c| c.wrapping_neg());
    // Step `k` (from 1) sets `out[k + 1]` and updates `out[2..=k]`, each from the entries below
    // it as they were before the step, which the loop carries as it goes up
    for (updated, &ftmp) in (0..).zip(cos_lsf) {
        let (mut two_below, mut one_below) = (out[0], out[1]);
        let mut higher = out.iter_mut().skip(2);
        for value in higher.by_ref().take(updated) {
            let before = *value;
            *value = before.wrapping_add(two_below.wrapping_sub(rshift_round64(smull(ftmp, one_below), QA) as i32));
            (two_below, one_below) = (one_below, before);
        }
        if let Some(next) = higher.next() {
            *next = lshift(two_below, 1).wrapping_sub(rshift_round64(smull(ftmp, one_below), QA) as i32);
        }
        out[1] = out[1].wrapping_sub(ftmp);
    }
}

/// This ordering was found to maximize quality. It improves numerical accuracy of
/// `find_polynomial` compared to "standard" ordering.
const ORDERING16: [u8; 16] = [0, 15, 8, 7, 4, 11, 12, 3, 2, 13, 10, 5, 6, 9, 14, 1];
const ORDERING10: [u8; 10] = [0, 9, 6, 3, 4, 5, 8, 1, 2, 7];

/// `silk_NLSF2A`: the monic whitening filter, Q12, of the normalised line spectral frequencies
/// `nlsf[..order]`, Q15, into `a_q12[..order]` (RFC 6716 §4.2.7.5.6 to §4.2.7.5.8).
pub fn to_lpc(a_q12: &mut [i16; MAX_LPC_ORDER], nlsf: &[i16; MAX_LPC_ORDER], order: LpcOrder) {
    let mut cos_lsf_qa = [0i32; MAX_LPC_ORDER];
    let mut p = [0i32; MAX_LPC_ORDER / 2 + 1];
    let mut q = [0i32; MAX_LPC_ORDER / 2 + 1];
    let mut a32_qa1 = [0i32; MAX_LPC_ORDER];
    let d = order.coefficients();

    // convert LSFs to 2*cos(LSF), using piecewise linear curve from table
    let ordering: &[u8] = match order {
        LpcOrder::Ten => &ORDERING10,
        LpcOrder::Sixteen => &ORDERING16,
    };
    for (&f, &at) in nlsf.iter().zip(ordering) {
        // NLSFs are 0 to 32767 (decoding clamps them; interpolation and smoothing stay between
        // such), which `max` says, so that the table needs no bounds check
        let f = f.max(0).unsigned_abs();
        // f_int on a scale 0-127 (rounded down), f_frac 0..255
        let f_int = usize::from(f >> (15 - 7));
        let f_frac = i32::from(f & 0xFF);

        // Read start and end value from table; delta is Q12, with a range of 0..200
        let Some(&[cos_val, next_val]) = LSFCOSTAB_FIX_Q12.get(f_int..).and_then(<[i16]>::first_chunk) else {
            continue;
        };
        let cos_val = i32::from(cos_val);
        let delta = i32::from(next_val).wrapping_sub(cos_val);

        // Linear interpolation, into the place the ordering (a permutation of the coefficients) gives
        if let Some(cos_lsf) = cos_lsf_qa.get_mut(usize::from(at)) {
            *cos_lsf = rshift_round(add32(lshift(cos_val, 8), mul(delta, f_frac)), 20 - QA);
        }
    }

    // generate even and odd polynomials using convolution
    let dd = d / 2;
    find_polynomial(&mut p, &cos_lsf_qa, 0, dd);
    find_polynomial(&mut q, &cos_lsf_qa, 1, dd);

    // convert even and odd polynomials to opus_int32 Q12 filter coefs; the Ptmp and Qtmp
    // values at this stage need to fit in int32
    let a32_qa1 = a32_qa1.get_mut(..d).unwrap_or_default();
    let (low, high) = a32_qa1.split_at_mut_checked(dd).unwrap_or_default();
    for (((low, high), &[p0, p1]), &[q0, q1]) in
        low.iter_mut().zip(high.iter_mut().rev()).zip(p.array_windows::<2>()).zip(q.array_windows::<2>())
    {
        let ptmp = p1.wrapping_add(p0);
        let qtmp = q1.wrapping_sub(q0);
        *low = qtmp.wrapping_neg().wrapping_sub(ptmp);
        *high = qtmp.wrapping_sub(ptmp);
    }

    // Convert int32 coefficients to Q12 int16 coefs
    let a_q12 = a_q12.get_mut(..d).unwrap_or_default();
    fit(a_q12, a32_qa1, 12, QA + 1);

    for i in 0..MAX_LPC_STABILIZE_ITERATIONS {
        if inverse_prediction_gain(a_q12) != 0 {
            break;
        }
        // Prediction coefficients are (too close to) unstable; apply bandwidth expansion on the
        // unscaled coefficients, convert to Q12 and measure again
        bandwidth_expand_32(a32_qa1, 65536i32.wrapping_sub(lshift(2, i)));
        for (a, &a32) in a_q12.iter_mut().zip(a32_qa1.iter()) {
            *a = rshift_round(a32, QA + 1 - 12) as i16;
        }
    }
}
