//! Frame reconstruction (RFC 6716 §4.2.7.8.6 and §4.2.7.9): the excitation from the pulses and
//! the LCG, then for each subframe the long-term (pitch) synthesis of voiced frames and the
//! short-term (LPC) synthesis, scaled by the subframe gain.
//!
//! From the reference decoder's `silk/decode_core.c`. Copyright (c) 2006-2011, Skype Limited,
//! BSD-3-Clause (the licence beside the crate).

use super::layout::{LTP_ORDER, Layout, MAX_FRAME_LENGTH, MAX_LPC_ORDER, MAX_SUBFRAMES, SignalType};
use super::lpc::{analysis_filter_of_order, synthesis_of_order};
use super::macros::{
    add_lshift32, add32, div32_varq, inverse32_varq, lshift, rand, rshift, rshift_round, sat16, smlawb, smulwb, smulww,
    sub32,
};
use super::parameters::FrameParameters;
use super::scratch::Scratch;
use super::side_info::SideInfo;
use super::tables::QUANTIZATION_OFFSETS_Q10;
use crate::slices::{chunks, copy, nth_chunk_mut};

/// `QUANT_LEVEL_ADJUST_Q10`: the pulses' magnitudes are reduced by this, before the offset.
const QUANT_LEVEL_ADJUST_Q10: i32 = 80;

/// The decoder state that reconstruction reads and updates.
pub struct SynthesisState<'a> {
    /// The excitation, Q14, kept for concealment and comfort noise.
    pub excitation_q14: &'a mut [i32; MAX_FRAME_LENGTH],
    /// The LPC filter state, Q14.
    pub lpc_state_q14: &'a mut [i32; MAX_LPC_ORDER],
    /// The output history the LTP reaches into, `outBuf`.
    pub history: &'a mut [i16],
    pub prev_gain_q16: &'a mut i32,
    pub after_loss: bool,
    pub prev_signal_type: SignalType,
    pub lag_prev: u16,
}

/// RFC 6716 §4.2.7.8.6: the excitation, from the pulses, the offset of the signal and
/// quantisation offset types, and the signs of the LCG.
fn excitation_from_pulses(excitation_q14: &mut [i32], pulses: &[i16], side: &SideInfo) {
    let offset_q10 =
        i32::from(*side.quant_offset_type.select(side.signal_type.select_by_voicing(&QUANTIZATION_OFFSETS_Q10)));
    let mut rand_seed = i32::from(side.seed);
    for (excitation, &pulse) in excitation_q14.iter_mut().zip(pulses) {
        rand_seed = rand(rand_seed);
        let mut e = lshift(i32::from(pulse), 14);
        if e > 0 {
            e = sub32(e, QUANT_LEVEL_ADJUST_Q10 << 4);
        } else if e < 0 {
            e = add32(e, QUANT_LEVEL_ADJUST_Q10 << 4);
        }
        e = add32(e, offset_q10 << 4);
        *excitation = if rand_seed < 0 { e.wrapping_neg() } else { e };
        rand_seed = add32(rand_seed, i32::from(pulse));
    }
}

/// RFC 6716 §4.2.7.9.1: the long-term prediction with the filter `b_q14` at `lag`, added to each
/// excitation sample to give the LPC excitation, and kept in the scaled LTP state from `ltp_at`.
fn predict_long_term<'a>(
    s_ltp_q15: &mut [i32],
    ltp_at: usize,
    lag: usize,
    b_q14: &[i16; LTP_ORDER],
    excitations: impl Iterator<Item = (&'a mut i32, &'a i32)>,
) {
    for (at, (lpc_excitation, &e)) in (ltp_at..).zip(excitations) {
        // Unrolled loop; avoids introducing a bias because silk_SMLAWB() always rounds to -inf
        let mut ltp_pred_q13: i32 = 2;
        // The taps centred on `lag` samples back
        let taps = at
            .checked_sub(lag)
            .and_then(|from| from.checked_sub(LTP_ORDER / 2))
            .and_then(|from| s_ltp_q15.get(from..))
            .and_then(<[i32]>::first_chunk::<LTP_ORDER>);
        for (&x, &b) in taps.into_iter().flatten().rev().zip(b_q14) {
            ltp_pred_q13 = smlawb(ltp_pred_q13, x, i32::from(b));
        }
        let res_q14 = add_lshift32(e, ltp_pred_q13, 1);
        *lpc_excitation = res_q14;
        if let Some(now) = s_ltp_q15.get_mut(at) {
            *now = lshift(res_q14, 1);
        }
    }
}

/// `silk_decode_core`: reconstructs a 20 ms frame into `out[..frame_length]`.
pub fn reconstruct(
    state: &mut SynthesisState<'_>,
    layout: Layout,
    side: &SideInfo,
    parameters: &mut FrameParameters,
    out: &mut [i16],
    scratch: &mut Scratch,
) {
    let frame_length = layout.frame_length();
    let subframe_length = layout.subframe_length();
    let ltp_memory_length = layout.ltp_memory_length();
    let order = layout.rate.lpc_order();
    let lpc_order = order.coefficients();
    // `sLTP`, the whitened history; `sLTP_Q15`, the scaled whitened history, then this frame's
    // LPC excitation; `sLPC_Q14`, the LPC state, then one subframe
    let Scratch { pulses, samples: s_ltp, wide: s_ltp_q15, lpc_q14: s_lpc_q14 } = scratch;
    let pulses = &*pulses;

    excitation_from_pulses(state.excitation_q14.get_mut(..frame_length).unwrap_or_default(), pulses, side);

    // Copy LPC state
    copy(&mut s_lpc_q14[..MAX_LPC_ORDER], state.lpc_state_q14);

    let mut ltp_at = ltp_memory_length; // into s_ltp_q15
    let mut lag;
    let mut prev_gain_q16 = *state.prev_gain_q16;
    let voiced_frame = side.signal_type == SignalType::Voiced;
    let out = out.get_mut(..frame_length).unwrap_or_default();
    let subframes = chunks(&state.excitation_q14[..], subframe_length).zip(&parameters.gains_q16);
    let ltp = parameters.ltp_coef_q14.iter_mut().zip(parameters.pitch_l.iter_mut());
    for (k, ((excitation, &gain_q16), (ltp_coef_q14, pitch_l))) in
        (0..MAX_SUBFRAMES).zip(subframes.zip(ltp)).take(layout.subframes())
    {
        // The first two subframes have the first half's filter
        let a_q12 = if k < 2 { parameters.pred_coef_q12[0] } else { parameters.pred_coef_q12[1] };

        let gain_q10 = rshift(gain_q16, 6);
        let mut inv_gain_q31 = inverse32_varq(gain_q16, 47);

        // Calculate gain adjustment factor, and scale the short-term state with it
        let gain_adj_q16 = if gain_q16 == prev_gain_q16 {
            1 << 16
        } else {
            let gain_adj_q16 = div32_varq(prev_gain_q16, gain_q16, 16);
            for v in &mut s_lpc_q14[..MAX_LPC_ORDER] {
                *v = smulww(gain_adj_q16, *v);
            }
            gain_adj_q16
        };
        prev_gain_q16 = gain_q16;

        // Avoid abrupt transition from voiced PLC to unvoiced normal decoding
        let mut voiced = voiced_frame;
        if state.after_loss && state.prev_signal_type == SignalType::Voiced && !voiced_frame && k < 2 {
            *ltp_coef_q14 = [0; LTP_ORDER];
            // SILK_FIX_CONST( 0.25, 14 )
            ltp_coef_q14[LTP_ORDER / 2] = 4096;
            voiced = true;
            *pitch_l = state.lag_prev;
        }

        // The slices below cannot fail: a lag is at most 18 ms, and the LTP memory 20 ms, more
        // than the lag and the filters' reach; a subframe is at most 80 samples. Where one could,
        // it is skipped rather than panic
        let (_, lpc_excitation) = s_lpc_q14.split_at_mut(MAX_LPC_ORDER);
        let lpc_excitation = lpc_excitation.iter_mut().take(subframe_length);
        if voiced {
            lag = usize::from(*pitch_l);

            // Re-whitening
            if k == 0 || (k == 2 && parameters.nlsf_interpolated) {
                // Rewhiten with new A coefs
                let start =
                    ltp_memory_length.saturating_sub(lag.saturating_add(lpc_order).saturating_add(LTP_ORDER / 2));
                if k == 2
                    && let Some(history) = state.history.get_mut(ltp_memory_length..)
                {
                    for (h, &o) in
                        history.iter_mut().zip(out.get(..subframe_length.saturating_mul(2)).unwrap_or_default())
                    {
                        *h = o;
                    }
                }
                // The history from `start`, moved on by the subframes already in it
                let offset = k.saturating_mul(subframe_length);
                if let (Some(whitened), Some(input)) = (
                    s_ltp.get_mut(start..ltp_memory_length),
                    state.history.get(start.saturating_add(offset)..ltp_memory_length.saturating_add(offset)),
                ) {
                    analysis_filter_of_order(order, whitened, input, &a_q12);
                }

                // After rewhitening the LTP state is unscaled
                if k == 0 {
                    // Do LTP downscaling to reduce inter-packet dependency
                    inv_gain_q31 = lshift(smulwb(inv_gain_q31, parameters.ltp_scale_q14), 2);
                }
                let span = lag.saturating_add(LTP_ORDER / 2);
                let scaled = s_ltp_q15.get_mut(ltp_at.saturating_sub(span)..ltp_at).unwrap_or_default();
                let whitened = s_ltp.get(ltp_memory_length.saturating_sub(span)..ltp_memory_length).unwrap_or_default();
                for (state, &x) in scaled.iter_mut().zip(whitened) {
                    *state = smulwb(inv_gain_q31, i32::from(x));
                }
            } else if gain_adj_q16 != 1 << 16 {
                // Update LTP state when Gain changes
                let span = lag.saturating_add(LTP_ORDER / 2);
                let scaled = s_ltp_q15.get_mut(ltp_at.saturating_sub(span)..ltp_at).unwrap_or_default();
                for state in scaled {
                    *state = smulww(gain_adj_q16, *state);
                }
            }

            predict_long_term(s_ltp_q15, ltp_at, lag, ltp_coef_q14, lpc_excitation.zip(excitation));
            ltp_at = ltp_at.saturating_add(subframe_length);
        } else {
            for (lpc_excitation, &e) in lpc_excitation.zip(excitation) {
                *lpc_excitation = e;
            }
        }

        // RFC 6716 §4.2.7.9.2: the short-term prediction, scaled with the gain
        let out = nth_chunk_mut(out, subframe_length, k).unwrap_or_default();
        let signal = s_lpc_q14.get_mut(..MAX_LPC_ORDER.saturating_add(subframe_length)).unwrap_or_default();
        synthesis_of_order(order, signal, &a_q12, |i, lpc_q14| {
            if let Some(sample) = out.get_mut(i) {
                *sample = sat16(rshift_round(smulww(lpc_q14, gain_q10), 8)) as i16;
            }
        });

        // Update LPC filter state: the last `MAX_LPC_ORDER` samples become the state
        let (lpc_state, rest) = s_lpc_q14.split_at_mut(MAX_LPC_ORDER);
        if let Some(last) = rest.get(..subframe_length).and_then(<[i32]>::last_chunk::<MAX_LPC_ORDER>) {
            copy(lpc_state, last);
        }
    }
    *state.prev_gain_q16 = prev_gain_q16;

    // Save LPC state
    copy(state.lpc_state_q14, &s_lpc_q14[..MAX_LPC_ORDER]);
}
