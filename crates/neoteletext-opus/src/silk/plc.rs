//! Packet loss concealment (RFC 6716 §4.4, which leaves it to the implementation; this is the
//! reference decoder's, so that the output is the same): the parameters kept from each decoded
//! frame, the extrapolation of a lost frame from them (pitch-periodic for voiced speech, noise
//! from the last excitation otherwise, both fading), and the energy matching of the first
//! frame received after a loss.
//!
//! From the reference decoder's `silk/PLC.c`, without deep PLC; `sum_sqr_shift.c`. Copyright (c)
//! 2006-2011, Skype Limited, BSD-3-Clause (the licence beside the crate).

use super::layout::{
    InternalRate, LTP_ORDER, Layout, MAX_FRAME_LENGTH, MAX_LPC_ORDER, MAX_PITCH_LAG_MS, MAX_SUBFRAMES, SignalType,
};
use super::lpc::{analysis_filter_of_order, bandwidth_expand, inverse_prediction_gain, synthesis_of_order};
use super::macros::{
    add_rshift_uint, add32, clz32, div32, inverse32_varq, lshift, positive_div, rand, rand_index, rshift, rshift_round,
    sat16, smlabb_ovflw, smlawb, smulbb, smulwb, smulww, sqrt_approx, sub32,
};
use super::parameters::FrameParameters;
use super::scratch::Scratch;

/// `BWE_COEF` 0.99: `SILK_FIX_CONST( BWE_COEF, 16 )` = (int)(0.99 * 65536 + 0.5).
const BWE_COEF_Q16: i32 = 64881;
/// `V_PITCH_GAIN_START_MIN_Q14`, `V_PITCH_GAIN_START_MAX_Q14`: 0.7 and 0.95 in Q14.
const V_PITCH_GAIN_START_MIN_Q14: i32 = 11469;
const V_PITCH_GAIN_START_MAX_Q14: i32 = 15565;
/// `RAND_BUF_SIZE`: the noise is drawn from the last this many samples of excitation.
const RAND_BUF_SIZE: usize = 128;
/// `LOG2_INV_LPC_GAIN_HIGH_THRES`, `LOG2_INV_LPC_GAIN_LOW_THRES`: 8 dB and 24 dB LPC gain.
const LOG2_INV_LPC_GAIN_HIGH_THRES: i32 = 3;
const LOG2_INV_LPC_GAIN_LOW_THRES: i32 = 8;
/// `PITCH_DRIFT_FAC_Q16`: 0.01 in Q16, the pitch lag's growth each subframe.
const PITCH_DRIFT_FAC_Q16: i32 = 655;
/// The attenuations of the first and later lost frames: harmonic 0.99, 0.95; random for
/// voiced 0.95, 0.8; random for unvoiced 0.99, 0.9; Q15.
const HARM_ATT_Q15: [i16; 2] = [32440, 31130];
const PLC_RAND_ATTENUATE_V_Q15: [i16; 2] = [31130, 26214];
const PLC_RAND_ATTENUATE_UV_Q15: [i16; 2] = [32440, 29491];

/// `silk_PLC_struct`.
#[derive(Clone)]
pub struct Concealment {
    /// Pitch lag to use for voiced concealment, Q8.
    pitch_l_q8: i32,
    /// LTP coefficients to use for voiced concealment.
    ltp_coef_q14: [i16; LTP_ORDER],
    prev_lpc_q12: [i16; MAX_LPC_ORDER],
    /// Was the previous frame lost.
    last_frame_lost: bool,
    /// Seed for unvoiced signal generation.
    rand_seed: i32,
    /// Scaling of the unvoiced random signal.
    pub rand_scale_q14: i16,
    conc_energy: i32,
    conc_energy_shift: i32,
    prev_ltp_scale_q14: i16,
    /// The last two subframe gains of the last decoded frame.
    pub prev_gain_q16: [i32; 2],
    /// The rate the state is for, `None` when it is to be reset.
    rate: Option<InternalRate>,
    /// The subframes and their length of the last decoded frame.
    subframes: u8,
    subframe_length: u8,
}

/// What concealment reads of the decoder state.
pub struct ConcealmentInput<'a> {
    pub layout: Layout,
    pub excitation_q14: &'a [i32; MAX_FRAME_LENGTH],
    pub history: &'a [i16],
    pub lpc_state_q14: &'a mut [i32; MAX_LPC_ORDER],
    pub first_frame_after_reset: bool,
    pub loss_count: u8,
    pub prev_signal_type: SignalType,
}

impl Concealment {
    /// The state as the C's zeroed memory and `silk_PLC_Reset` at no rate leave it.
    pub const NEW: Self = Self {
        pitch_l_q8: 0,
        ltp_coef_q14: [0; LTP_ORDER],
        prev_lpc_q12: [0; MAX_LPC_ORDER],
        last_frame_lost: false,
        rand_seed: 0,
        rand_scale_q14: 0,
        conc_energy: 0,
        conc_energy_shift: 0,
        prev_ltp_scale_q14: 0,
        prev_gain_q16: [1 << 16; 2],
        rate: None,
        subframes: 2,
        subframe_length: 20,
    };

    /// `silk_PLC_Reset`, when the rate is not the state's, for a frame of `layout`.
    pub fn follow_rate(&mut self, layout: Layout) {
        if self.rate != Some(layout.rate) {
            #[allow(clippy::cast_possible_wrap, reason = "a frame is at most 20 ms at 16 kHz, 320 samples")]
            let frame_length = layout.frame_length() as i32;
            self.pitch_l_q8 = lshift(frame_length, 8 - 1);
            self.prev_gain_q16 = [1 << 16; 2];
            self.subframe_length = 20;
            self.subframes = 2;
            self.rate = Some(layout.rate);
        }
    }

    /// `silk_PLC_update`: keeps the parameters of a decoded 20 ms frame of `signal_type` for
    /// concealing the next.
    pub fn update(&mut self, layout: Layout, signal_type: SignalType, parameters: &FrameParameters) {
        let subframes = layout.subframes();
        let mut ltp_gain_q14: i32 = 0;
        if signal_type == SignalType::Voiced {
            // Find the parameters for the last subframe which contains a pitch pulse: from the
            // last subframe back, while they lie within its lag
            let lags = parameters.pitch_l.get(..subframes).unwrap_or_default();
            let last_lag = lags.last().map_or(0, |&lag| usize::from(lag));
            let from_last = lags.iter().zip(&parameters.ltp_coef_q14).rev().enumerate();
            for (_, (&lag, coefficients)) in
                from_last.take_while(|&(j, _)| j.saturating_mul(layout.subframe_length()) < last_lag)
            {
                let temp_ltp_gain_q14: i32 = coefficients.iter().map(|&c| i32::from(c)).sum();
                if temp_ltp_gain_q14 > ltp_gain_q14 {
                    ltp_gain_q14 = temp_ltp_gain_q14;
                    self.pitch_l_q8 = lshift(i32::from(lag), 8);
                }
            }

            self.ltp_coef_q14 = [0; LTP_ORDER];
            self.ltp_coef_q14[LTP_ORDER / 2] = ltp_gain_q14 as i16;

            // Limit LT coefs
            if ltp_gain_q14 < V_PITCH_GAIN_START_MIN_Q14 {
                let scale_q10 = positive_div(lshift(V_PITCH_GAIN_START_MIN_Q14, 10), ltp_gain_q14.max(1));
                for c in &mut self.ltp_coef_q14 {
                    *c = rshift(smulbb(i32::from(*c), scale_q10), 10) as i16;
                }
            } else if ltp_gain_q14 > V_PITCH_GAIN_START_MAX_Q14 {
                let scale_q14 = positive_div(lshift(V_PITCH_GAIN_START_MAX_Q14, 14), ltp_gain_q14.max(1));
                for c in &mut self.ltp_coef_q14 {
                    *c = rshift(smulbb(i32::from(*c), scale_q14), 14) as i16;
                }
            }
        } else {
            self.pitch_l_q8 = lshift(smulbb(layout.rate.khz(), 18), 8);
            self.ltp_coef_q14 = [0; LTP_ORDER];
        }

        // Save LPC coeficients
        let order = layout.rate.lpc_order().coefficients();
        for (previous, &coefficient) in self.prev_lpc_q12.iter_mut().zip(&parameters.pred_coef_q12[1]).take(order) {
            *previous = coefficient;
        }
        self.prev_ltp_scale_q14 = parameters.ltp_scale_q14 as i16;

        // Save last two gains
        if let Some(&last_two) = parameters.gains_q16.get(..subframes).and_then(<[i32]>::last_chunk::<2>) {
            self.prev_gain_q16 = last_two;
        }

        self.subframe_length = layout.subframe_length() as u8;
        self.subframes = subframes as u8;
    }

    /// `silk_PLC_conceal`: extrapolates a lost frame of `input.layout` into `frame`, and sets
    /// the pitch lags of `parameters`.
    #[cold]
    #[inline(never)]
    pub fn conceal(
        &mut self,
        input: &mut ConcealmentInput<'_>,
        parameters: &mut FrameParameters,
        frame: &mut [i16],
        scratch: &mut Scratch,
    ) {
        let layout = input.layout;
        let ltp_memory_length = layout.ltp_memory_length();
        let subframe_length = layout.subframe_length();
        let frame_length = layout.frame_length();
        let order = layout.rate.lpc_order();
        let lpc_order = order.coefficients();
        // `sLTP`, the whitened history (and first the scaled excitation of the energies);
        // `sLTP_Q14`, the scaled whitened history, then the LTP output, then the LPC synthesis
        let Scratch { samples: s_ltp, wide: s_ltp_q14, .. } = scratch;

        let prev_gain_q10 = [rshift(self.prev_gain_q16[0], 6), rshift(self.prev_gain_q16[1], 6)];

        if input.first_frame_after_reset {
            self.prev_lpc_q12 = [0; MAX_LPC_ORDER];
        }

        let rand_from = self.noise_start(layout, input.excitation_q14, prev_gain_q10, s_ltp);
        // At most a frame less `RAND_BUF_SIZE`, so the noise is always there
        let noise = input
            .excitation_q14
            .get(rand_from..)
            .and_then(<[i32]>::first_chunk::<RAND_BUF_SIZE>)
            .unwrap_or(&[0; RAND_BUF_SIZE]);

        // Set up Gain to random noise component
        let mut rand_scale_q14 = self.rand_scale_q14;

        // Set up attenuation gains
        let att = |[first, later]: [i16; 2]| i32::from(if input.loss_count == 0 { first } else { later });
        let harm_gain_q15 = att(HARM_ATT_Q15);
        let mut rand_gain_q15 = if input.prev_signal_type == SignalType::Voiced {
            att(PLC_RAND_ATTENUATE_V_Q15)
        } else {
            att(PLC_RAND_ATTENUATE_UV_Q15)
        };

        // LPC concealment. Apply BWE to previous LPC
        bandwidth_expand(self.prev_lpc_q12.get_mut(..lpc_order).unwrap_or_default(), BWE_COEF_Q16);
        let mut a_q12 = [0i16; MAX_LPC_ORDER];
        for (a, &previous) in a_q12.iter_mut().zip(&self.prev_lpc_q12).take(lpc_order) {
            *a = previous;
        }

        if input.loss_count == 0 {
            (rand_scale_q14, rand_gain_q15) = self.first_loss_noise(input.prev_signal_type, lpc_order, rand_gain_q15);
        }

        let mut rand_seed = self.rand_seed;
        let mut lag = lag_of(self.pitch_l_q8);

        // Rewhiten LTP state. A lag is at most 18 ms and the LTP memory 20 ms, more than the lag
        // and the filters' reach, so every slice here exists; were one missing, its step would
        // be skipped rather than panic
        let start =
            ltp_memory_length.saturating_sub(usize::from(lag).saturating_add(lpc_order).saturating_add(LTP_ORDER / 2));
        if let (Some(whitened), Some(history)) =
            (s_ltp.get_mut(start..ltp_memory_length), input.history.get(start..ltp_memory_length))
        {
            analysis_filter_of_order(order, whitened, history, &a_q12);
        }

        // Scale LTP state
        let inv_gain_q30 = inverse32_varq(self.prev_gain_q16[1], 46).min(i32::MAX >> 1);
        let scaled = s_ltp_q14.get_mut(start.saturating_add(lpc_order)..ltp_memory_length).unwrap_or_default();
        let whitened = s_ltp.get(start.saturating_add(lpc_order)..ltp_memory_length).unwrap_or_default();
        for (state, &x) in scaled.iter_mut().zip(whitened) {
            *state = smulwb(inv_gain_q30, i32::from(x));
        }

        // LTP synthesis filtering
        let max_pitch_l_q8 = lshift(smulbb(MAX_PITCH_LAG_MS, layout.rate.khz()), 8);
        // Each sample's place in `s_ltp_q14`, after the LTP memory
        let mut positions = ltp_memory_length..;
        for _ in 0..layout.subframes() {
            let b_q14 = self.ltp_coef_q14;
            for ltp_at in positions.by_ref().take(subframe_length) {
                // Unrolled loop; avoids introducing a bias because silk_SMLAWB() always rounds to -inf
                let mut ltp_pred_q12: i32 = 2;
                // The taps centred on `lag` samples back
                let taps = ltp_at
                    .checked_sub(usize::from(lag))
                    .and_then(|from| from.checked_sub(LTP_ORDER / 2))
                    .and_then(|from| s_ltp_q14.get(from..))
                    .and_then(<[i32]>::first_chunk::<LTP_ORDER>);
                for (&x, &b) in taps.into_iter().flatten().rev().zip(&b_q14) {
                    ltp_pred_q12 = smlawb(ltp_pred_q12, x, i32::from(b));
                }

                // Generate LPC excitation
                rand_seed = rand(rand_seed);
                let idx = rand_index(rand_seed, RAND_BUF_SIZE.ilog2()) & (RAND_BUF_SIZE - 1);
                if let Some(now) = s_ltp_q14.get_mut(ltp_at) {
                    let noise = noise.get(idx).copied().unwrap_or(0);
                    *now = lshift(smlawb(ltp_pred_q12, noise, i32::from(rand_scale_q14)), 2);
                }
            }

            // Gradually reduce LTP gain
            for c in &mut self.ltp_coef_q14 {
                *c = rshift(smulbb(harm_gain_q15, i32::from(*c)), 15) as i16;
            }
            // Gradually reduce excitation gain
            rand_scale_q14 = rshift(smulbb(i32::from(rand_scale_q14), rand_gain_q15), 15) as i16;

            // Slowly increase pitch lag
            self.pitch_l_q8 = smlawb(self.pitch_l_q8, self.pitch_l_q8, PITCH_DRIFT_FAC_Q16).min(max_pitch_l_q8);
            lag = lag_of(self.pitch_l_q8);
        }

        // LPC synthesis filtering, over the LTP output, after the LPC state
        let lpc_signal = s_ltp_q14
            .get_mut(ltp_memory_length.saturating_sub(MAX_LPC_ORDER)..ltp_memory_length.saturating_add(frame_length))
            .unwrap_or_default();
        if let Some((state, _)) = lpc_signal.split_first_chunk_mut::<MAX_LPC_ORDER>() {
            *state = *input.lpc_state_q14;
        }
        synthesis_of_order(order, lpc_signal, &a_q12, |i, lpc_q14| {
            // Scale with Gain
            if let Some(sample) = frame.get_mut(i) {
                *sample = sat16(rshift_round(smulww(lpc_q14, prev_gain_q10[1]), 8)) as i16;
            }
        });

        // Save LPC state
        if let Some(state) = lpc_signal.get(frame_length..).and_then(<[i32]>::first_chunk::<MAX_LPC_ORDER>) {
            *input.lpc_state_q14 = *state;
        }

        // Update states
        self.rand_seed = rand_seed;
        self.rand_scale_q14 = rand_scale_q14;
        parameters.pitch_l = [lag; MAX_SUBFRAMES];
    }

    /// The noise's scale, Q14, and attenuation, Q15, in the first lost frame: less noise after a
    /// voiced frame, the more its pitch predicts, and after an unvoiced one with a high LPC gain.
    fn first_loss_noise(&self, prev_signal_type: SignalType, lpc_order: usize, rand_gain_q15: i32) -> (i16, i32) {
        if prev_signal_type == SignalType::Voiced {
            // Reduce random noise Gain for voiced frames
            let mut rand_scale_q14: i16 = 1 << 14;
            for &c in &self.ltp_coef_q14 {
                rand_scale_q14 = i32::from(rand_scale_q14).wrapping_sub(i32::from(c)) as i16;
            }
            rand_scale_q14 = rand_scale_q14.max(3277); // 0.2
            rand_scale_q14 = rshift(smulbb(i32::from(rand_scale_q14), i32::from(self.prev_ltp_scale_q14)), 14) as i16;
            (rand_scale_q14, rand_gain_q15)
        } else {
            // Reduce random noise for unvoiced frames with high LPC gain
            let inv_gain_q30 = inverse_prediction_gain(self.prev_lpc_q12.get(..lpc_order).unwrap_or_default());
            let down_scale_q30 =
                inv_gain_q30.clamp((1 << 30) >> LOG2_INV_LPC_GAIN_LOW_THRES, (1 << 30) >> LOG2_INV_LPC_GAIN_HIGH_THRES);
            let down_scale_q30 = lshift(down_scale_q30, LOG2_INV_LPC_GAIN_HIGH_THRES);
            (1 << 14, rshift(smulwb(down_scale_q30, rand_gain_q15), 14))
        }
    }

    /// `silk_PLC_energy`: where in the excitation the noise is drawn from, the end of the
    /// subframe with the lower energy of the last two, scaled by their gains; `scaled` is the
    /// working buffer.
    fn noise_start(
        &self,
        layout: Layout,
        excitation_q14: &[i32; MAX_FRAME_LENGTH],
        prev_gain_q10: [i32; 2],
        scaled: &mut [i16],
    ) -> usize {
        let subframe_length = layout.subframe_length();
        let scaled = scaled.get_mut(..subframe_length.saturating_mul(2)).unwrap_or_default();
        let excitation = excitation_q14.get(..layout.frame_length()).unwrap_or_default();
        // The last two subframes, each scaled by its gain into its half of `scaled`.
        let tail =
            excitation.get(excitation.len().saturating_sub(subframe_length.saturating_mul(2))..).unwrap_or_default();
        let (excitation_first, excitation_second) = tail.split_at_checked(subframe_length).unwrap_or_default();
        let (scaled_first, scaled_second) = scaled.split_at_mut_checked(subframe_length).unwrap_or_default();
        let [gain_first, gain_second] = prev_gain_q10;
        for (half, excitation, gain_q10) in
            [(scaled_first, excitation_first, gain_first), (scaled_second, excitation_second, gain_second)]
        {
            for (s, &e) in half.iter_mut().zip(excitation) {
                *s = sat16(rshift(smulww(e, gain_q10), 8)) as i16;
            }
        }
        let (first, second) = scaled.split_at(subframe_length.min(scaled.len()));
        let (energy1, shift1) = energy_with_shift(first);
        let (energy2, shift2) = energy_with_shift(second);
        if rshift(energy1, shift2) < rshift(energy2, shift1) {
            // First sub-frame has lowest energy
            usize::from(self.subframes)
                .saturating_sub(1)
                .saturating_mul(usize::from(self.subframe_length))
                .saturating_sub(RAND_BUF_SIZE)
        } else {
            // Second sub-frame has lowest energy
            usize::from(self.subframes).saturating_mul(usize::from(self.subframe_length)).saturating_sub(RAND_BUF_SIZE)
        }
    }

    /// `silk_PLC_glue_frames`: after a concealed frame, keeps its energy; in the first decoded
    /// frame after a loss, fades in from that energy if the frame is louder.
    pub fn glue(&mut self, frame: &mut [i16], loss_count: u8) {
        if loss_count != 0 {
            // Calculate energy in concealed residual
            (self.conc_energy, self.conc_energy_shift) = energy_with_shift(frame);
            self.last_frame_lost = true;
            return;
        }
        if self.last_frame_lost {
            self.fade_in(frame);
        }
        self.last_frame_lost = false;
    }

    #[cold]
    #[inline(never)]
    fn fade_in(&mut self, frame: &mut [i16]) {
        // Calculate residual in decoded signal if last frame was lost
        let (mut energy, energy_shift) = energy_with_shift(frame);

        // Normalize energies
        if energy_shift > self.conc_energy_shift {
            self.conc_energy = rshift(self.conc_energy, energy_shift.wrapping_sub(self.conc_energy_shift));
        } else if energy_shift < self.conc_energy_shift {
            energy = rshift(energy, self.conc_energy_shift.wrapping_sub(energy_shift));
        }

        // Fade in the energy difference
        if energy > self.conc_energy {
            let lz = clz32(self.conc_energy).wrapping_sub(1);
            self.conc_energy = lshift(self.conc_energy, lz);
            energy = rshift(energy, 24i32.wrapping_sub(lz).max(0));

            // Both energies are nonnegative (`energy_with_shift` leaves headroom; the shift keeps the sign)
            let frac_q24 = positive_div(self.conc_energy, energy.max(1));

            let mut gain_q16 = lshift(sqrt_approx(frac_q24), 4);
            // Make slope 4x steeper to avoid missing onsets after DTX
            #[allow(clippy::cast_possible_wrap, reason = "a frame is at most 20 ms at 16 kHz, 320 samples")]
            let frame_length = frame.len() as i32;
            let slope_q16 = lshift(div32(sub32(1 << 16, gain_q16), frame_length), 2);

            for v in frame.iter_mut() {
                *v = smulwb(gain_q16, i32::from(*v)) as i16;
                gain_q16 = add32(gain_q16, slope_q16);
                if gain_q16 > 1 << 16 {
                    break;
                }
            }
        }
    }
}

/// The pitch lag in samples of a Q8 lag, rounded.
#[inline]
#[allow(
    clippy::cast_sign_loss,
    reason = "the Q8 lag is set from a decoded lag or 18 ms, positive, and grows only up to 18 ms: \
              at most 288 samples, which u16 holds"
)]
fn lag_of(pitch_l_q8: i32) -> u16 {
    rshift_round(pitch_l_q8, 8) as u16
}

/// `silk_sum_sqr_shift`: the energy of `x`, right-shifted to fit an `opus_int32` with two bits
/// of headroom, and the shift applied. Returns `(energy, shift)`.
#[must_use]
pub fn energy_with_shift(x: &[i16]) -> (i32, i32) {
    #[allow(clippy::cast_possible_wrap, reason = "the longest input is a frame, at most 20 ms at 16 kHz, 320 samples")]
    let len = x.len() as i32;
    let (pairs, last) = x.as_chunks::<2>();
    // The C sums in `silk_ADD_RSHIFT_uint`, unsigned, the squares reinterpreted (a pair's may
    // wrap a signed 32 bits), and keeps the sum as an `opus_int32`
    let sum = |shift: i32, start: i32| {
        let mut nrg = start.cast_unsigned();
        for &[a, b] in pairs {
            let nrg_tmp = smlabb_ovflw(smulbb(i32::from(a), i32::from(a)), i32::from(b), i32::from(b));
            nrg = add_rshift_uint(nrg, nrg_tmp.cast_unsigned(), shift);
        }
        if let [a] = *last {
            // One sample left to process
            nrg = add_rshift_uint(nrg, smulbb(i32::from(a), i32::from(a)).cast_unsigned(), shift);
        }
        nrg.cast_signed()
    };
    // Do a first run with the maximum shift we could have; let's be conservative with
    // rounding and start with nrg=len
    let shift = 31i32.wrapping_sub(clz32(len));
    let nrg = sum(shift, len);
    // Make sure the result will fit in a 32-bit signed integer with two bits of headroom
    let shift = shift.wrapping_add(3).wrapping_sub(clz32(nrg)).max(0);
    (sum(shift, 0), shift)
}
