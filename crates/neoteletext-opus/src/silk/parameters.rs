//! The parameters of a decoded frame, from its side information (RFC 6716 §4.2.7.4 to
//! §4.2.7.6): the subframe gains, the LPC filters of each half (interpolated or not), the pitch
//! lags, and the LTP filters and scaling.
//!
//! From the reference decoder's `silk/decode_parameters.c`, `gain_quant.c`, `log2lin.c`,
//! `decode_pitch.c`. Copyright (c) 2006-2011, Skype Limited, BSD-3-Clause (the licence beside the
//! crate).

use super::layout::{InternalRate, LTP_ORDER, MAX_LPC_ORDER, MAX_PITCH_LAG_MS, MAX_SUBFRAMES, MIN_PITCH_LAG_MS};
use super::lpc::bandwidth_expand;
use super::macros::{add_rshift32, add32, mla, mul, rshift, smlawb, smulbb, smulwb, sub32};
use super::nlsf::{self, NlsfCodebook};
use super::side_info::SideInfo;
use super::tables::{CB_LAGS_STAGE2, CB_LAGS_STAGE3, LTP_VQ_PTRS_Q7, LTPSCALES_TABLE_Q14, NLSF_CB_NB_MB, NLSF_CB_WB};

/// `BWE_AFTER_LOSS_Q16`: the bandwidth expansion of the first frame after a loss.
const BWE_AFTER_LOSS_Q16: i32 = 63570;
/// `N_LEVELS_QGAIN`, `MIN_QGAIN_DB`, `MAX_QGAIN_DB`, `MAX_DELTA_GAIN_QUANT`,
/// `MIN_DELTA_GAIN_QUANT`.
const N_LEVELS_QGAIN: i32 = 64;
const MIN_QGAIN_DB: i32 = 2;
const MAX_QGAIN_DB: i32 = 88;
const MAX_DELTA_GAIN_QUANT: i32 = 36;
const MIN_DELTA_GAIN_QUANT: i32 = -4;
/// `OFFSET`: `( MIN_QGAIN_DB * 128 ) / 6 + 16 * 128`.
const GAIN_OFFSET: i32 = (MIN_QGAIN_DB * 128) / 6 + 16 * 128;
/// `INV_SCALE_Q16`: `( 65536 * ( ( ( MAX_QGAIN_DB - MIN_QGAIN_DB ) * 128 ) / 6 ) ) / ( N_LEVELS_QGAIN - 1 )`.
const GAIN_INV_SCALE_Q16: i32 = (65536 * (((MAX_QGAIN_DB - MIN_QGAIN_DB) * 128) / 6)) / (N_LEVELS_QGAIN - 1);
/// The largest log gain, 31 in Q7.
const MAX_LOG_GAIN_Q7: i32 = 3967;

/// The NLSF codebook of a rate: narrowband and mediumband share one.
#[inline]
#[must_use]
pub fn nlsf_codebook(rate: InternalRate) -> &'static NlsfCodebook {
    match rate {
        InternalRate::Narrowband | InternalRate::Mediumband => &NLSF_CB_NB_MB,
        InternalRate::Wideband => &NLSF_CB_WB,
    }
}

/// `silk_decoder_control`: the parameters of one frame.
#[derive(Clone, Copy)]
pub struct FrameParameters {
    /// The pitch lag of each subframe, in samples: 2 to 18 ms, so at most 288.
    pub pitch_l: [u16; MAX_SUBFRAMES],
    pub gains_q16: [i32; MAX_SUBFRAMES],
    /// The LPC filters of the first and second halves of the frame.
    pub pred_coef_q12: [[i16; MAX_LPC_ORDER]; 2],
    pub ltp_coef_q14: [[i16; LTP_ORDER]; MAX_SUBFRAMES],
    pub ltp_scale_q14: i32,
    /// Whether the first half's filter is interpolated from the previous frame's.
    pub nlsf_interpolated: bool,
}

impl FrameParameters {
    /// The parameters of a concealed frame, before concealment sets the pitch lags.
    pub const ZERO: Self = Self {
        pitch_l: [0; MAX_SUBFRAMES],
        gains_q16: [0; MAX_SUBFRAMES],
        pred_coef_q12: [[0; MAX_LPC_ORDER]; 2],
        ltp_coef_q14: [[0; LTP_ORDER]; MAX_SUBFRAMES],
        ltp_scale_q14: 0,
        nlsf_interpolated: false,
    };
}

/// The state that decoding the parameters reads and updates.
pub struct ParameterState<'a> {
    pub last_gain_index: &'a mut i8,
    pub prev_nlsf_q15: &'a mut [i16; MAX_LPC_ORDER],
    pub first_frame_after_reset: bool,
    pub after_loss: bool,
}

/// `silk_decode_parameters`: the parameters of a 20 ms frame from its side information.
pub fn decode(side: &SideInfo, rate: InternalRate, state: &mut ParameterState<'_>) -> FrameParameters {
    let mut parameters = FrameParameters::ZERO;
    let order = rate.lpc_order();
    let codebook = nlsf_codebook(rate);

    // RFC 6716 §4.2.7.4: the gains
    dequantise_gains(&mut parameters.gains_q16, side.gains_indices, state.last_gain_index);

    // RFC 6716 §4.2.7.5: the NLSFs and the second half's LPC filter
    let mut nlsf_q15 = [0i16; MAX_LPC_ORDER];
    nlsf::decode(&mut nlsf_q15, &side.nlsf_indices, codebook);
    nlsf::to_lpc(&mut parameters.pred_coef_q12[1], &nlsf_q15, order);

    // If just reset, e.g., because internal Fs changed, do not allow interpolation; improves
    // the case of packet loss in the first frame after a switch
    let interp_coef_q2 = if state.first_frame_after_reset { 4 } else { i32::from(side.nlsf_interp_coef_q2) };
    parameters.nlsf_interpolated = interp_coef_q2 < 4;
    if parameters.nlsf_interpolated {
        // RFC 6716 §4.2.7.5.5: the first half's NLSFs interpolated between the previous frame's
        // and these
        let mut first_half_q15 = [0i16; MAX_LPC_ORDER];
        for ((first_half, &previous), &current) in
            first_half_q15.iter_mut().zip(state.prev_nlsf_q15.iter()).zip(&nlsf_q15).take(order.coefficients())
        {
            *first_half = add32(
                i32::from(previous),
                rshift(mul(interp_coef_q2, sub32(i32::from(current), i32::from(previous))), 2),
            ) as i16;
        }
        nlsf::to_lpc(&mut parameters.pred_coef_q12[0], &first_half_q15, order);
    } else {
        // Copy LPC coefficients for first half from second half
        parameters.pred_coef_q12[0] = parameters.pred_coef_q12[1];
    }
    for (previous, &current) in state.prev_nlsf_q15.iter_mut().zip(&nlsf_q15).take(order.coefficients()) {
        *previous = current;
    }

    // After a packet loss do BWE of LPC coefs
    if state.after_loss {
        for filter in &mut parameters.pred_coef_q12 {
            bandwidth_expand(filter.get_mut(..order.coefficients()).unwrap_or_default(), BWE_AFTER_LOSS_Q16);
        }
    }

    if let Some(ltp) = &side.ltp {
        // RFC 6716 §4.2.7.6: the pitch lags, the LTP filters and the scaling
        parameters.pitch_l = pitch_lags(ltp.lag, ltp.contour, rate);
        // The indices were decoded with these tables' sizes, so every entry exists
        let codebook_q7 = LTP_VQ_PTRS_Q7.get(ltp.periodicity).copied().unwrap_or_default();
        for (coefficients, &index) in parameters.ltp_coef_q14.iter_mut().zip(&ltp.filters) {
            for (coefficient, &tap_q7) in coefficients.iter_mut().zip(codebook_q7.get(index).into_iter().flatten()) {
                *coefficient = i16::from(tap_q7) << 7;
            }
        }
        parameters.ltp_scale_q14 = LTPSCALES_TABLE_Q14.get(ltp.scale).map_or(0, |&scale| i32::from(scale));
    }
    parameters
}

/// `silk_gains_dequant`: the subframe gains, Q16, uniform on the log scale (RFC 6716
/// §4.2.7.4). The first index is independent (every frame is coded so), the others deltas;
/// `prev_ind` is the previous frame's last index and becomes this one's.
pub fn dequantise_gains(gain_q16: &mut [i32; MAX_SUBFRAMES], ind: [i8; MAX_SUBFRAMES], prev_ind: &mut i8) {
    let mut previous = i32::from(*prev_ind);
    for (k, (gain, index)) in gain_q16.iter_mut().zip(ind).enumerate() {
        let index = i32::from(index);
        if k == 0 {
            // Gain index is not allowed to go down more than 16 steps (~21.8 dB)
            previous = index.max(previous.wrapping_sub(16));
        } else {
            // Delta index, accumulated; the indices are 8-bit and `previous` is clamped below, so
            // none of this wraps
            let ind_tmp = index.wrapping_add(MIN_DELTA_GAIN_QUANT);
            let double_step_size_threshold = (2 * MAX_DELTA_GAIN_QUANT - N_LEVELS_QGAIN).wrapping_add(previous);
            if ind_tmp > double_step_size_threshold {
                previous = previous.wrapping_add((ind_tmp << 1).wrapping_sub(double_step_size_threshold));
            } else {
                previous = previous.wrapping_add(ind_tmp);
            }
        }
        previous = previous.clamp(0, N_LEVELS_QGAIN - 1);

        // Scale and convert to linear scale
        *gain = log2lin(add32(smulwb(GAIN_INV_SCALE_Q16, previous), GAIN_OFFSET).min(MAX_LOG_GAIN_Q7));
    }
    *prev_ind = previous as i8;
}

/// `silk_log2lin`: an approximation of 2^(x / 128), a very close inverse of `silk_lin2log`.
#[must_use]
pub fn log2lin(in_log_q7: i32) -> i32 {
    if in_log_q7 < 0 {
        return 0;
    } else if in_log_q7 >= MAX_LOG_GAIN_Q7 {
        return i32::MAX;
    }
    let out = 1 << (in_log_q7 >> 7);
    let frac_q7 = in_log_q7 & 0x7F;
    // Piece-wise parabolic approximation
    let parabola = smlawb(frac_q7, smulbb(frac_q7, sub32(128, frac_q7)), -174);
    if in_log_q7 < 2048 { add_rshift32(out, mul(out, parabola), 7) } else { mla(out, out >> 7, parabola) }
}

/// `silk_decode_pitch` for a 20 ms frame: the pitch lag of each subframe from the lag index and
/// the contour (RFC 6716 §4.2.7.6.1), limited to 2 to 18 ms.
#[must_use]
pub fn pitch_lags(lag_index: i16, contour_index: usize, rate: InternalRate) -> [u16; MAX_SUBFRAMES] {
    let fs_khz = rate.khz();
    let min_lag = smulbb(MIN_PITCH_LAG_MS, fs_khz);
    let max_lag = smulbb(MAX_PITCH_LAG_MS, fs_khz);
    let lag = add32(min_lag, i32::from(lag_index));
    // Each subframe's offset from the contour's row of its codebook
    let subframe_lag = |offset: Option<&i8>| {
        #[allow(
            clippy::cast_sign_loss,
            reason = "clamped to 2 to 18 ms at 8 to 16 kHz, 16 to 288 samples, which u16 holds"
        )]
        // silk_LIMIT, which needs no order of its bounds; min_lag is below max_lag.
        let lag = add32(lag, i32::from(offset.copied().unwrap_or(0))).max(min_lag).min(max_lag) as u16;
        lag
    };
    match rate {
        InternalRate::Narrowband => CB_LAGS_STAGE2.each_ref().map(|row| subframe_lag(row.get(contour_index))),
        InternalRate::Mediumband | InternalRate::Wideband => {
            CB_LAGS_STAGE3.each_ref().map(|row| subframe_lag(row.get(contour_index)))
        }
    }
}
