//! Comfort noise generation (outside RFC 6716's normative decoder; this is the reference
//! decoder's, so that the output is the same): while frames carry no voice activity, it tracks
//! their spectrum, gain and excitation; in a lost frame it adds noise shaped by them.
//!
//! From the reference decoder's `silk/CNG.c`. Copyright (c) 2006-2011, Skype Limited, BSD-3-Clause
//! (the licence beside the crate).

use super::layout::{InternalRate, Layout, LpcOrder, MAX_FRAME_LENGTH, MAX_LPC_ORDER, MAX_SUBFRAMES};
use super::lpc::synthesis_of_order;
use super::macros::{
    add_sat16, add32, lshift, rand, rand_index, rshift, rshift_round, sat16, smultt, smulwb, smulww, sqrt_approx,
    sub_lshift32, sub32,
};
use super::nlsf;
use crate::slices::{move_up, nth_chunk};

/// `CNG_BUF_MASK_MAX`: `2^floor(log2(MAX_FRAME_LENGTH)) - 1`.
const CNG_BUF_MASK_MAX: usize = 255;
/// `CNG_GAIN_SMTH_Q16`: 0.25^(1/4).
const CNG_GAIN_SMTH_Q16: i32 = 4634;
/// `CNG_GAIN_SMTH_THRESHOLD_Q16`: -3 dB.
const CNG_GAIN_SMTH_THRESHOLD_Q16: i32 = 46396;
/// `CNG_NLSF_SMTH_Q16`: 0.25.
const CNG_NLSF_SMTH_Q16: i32 = 16348;
/// `silk_CNG_Reset`'s NLSF step, `silk_int16_MAX / (order + 1)`, for each order.
const NLSF_STEP_10_Q15: i32 = i16::MAX as i32 / 11;
const NLSF_STEP_16_Q15: i32 = i16::MAX as i32 / 17;
/// The seed `silk_CNG_Reset` sets.
const RESET_SEED: i32 = 3_176_576;

/// `silk_CNG_struct`.
#[derive(Clone)]
pub struct ComfortNoise {
    exc_buf_q14: [i32; MAX_FRAME_LENGTH],
    smth_nlsf_q15: [i16; MAX_LPC_ORDER],
    synth_state: [i32; MAX_LPC_ORDER],
    smth_gain_q16: i32,
    rand_seed: i32,
    /// The rate the state is for, `None` when it is to be reset.
    rate: Option<InternalRate>,
}

/// What comfort noise reads of the decoder state.
pub struct ComfortNoiseInput<'a> {
    pub layout: Layout,
    /// Whether this frame was decoded, and its type no voice activity: the estimate is updated.
    pub inactive_decoded: bool,
    /// Whether this frame was concealed: noise is added.
    pub concealed: bool,
    pub prev_nlsf_q15: &'a [i16; MAX_LPC_ORDER],
    pub excitation_q14: &'a [i32; MAX_FRAME_LENGTH],
    pub gains_q16: &'a [i32; MAX_SUBFRAMES],
    /// The concealment's noise scale and last gain.
    pub rand_scale_q14: i16,
    pub prev_gain_q16: i32,
}

impl ComfortNoise {
    /// The state as the C's zeroed memory leaves it.
    pub const NEW: Self = Self {
        exc_buf_q14: [0; MAX_FRAME_LENGTH],
        smth_nlsf_q15: [0; MAX_LPC_ORDER],
        synth_state: [0; MAX_LPC_ORDER],
        smth_gain_q16: 0,
        rand_seed: RESET_SEED,
        rate: None,
    };

    /// `silk_CNG`: updates the estimate from a decoded frame without voice activity, and adds
    /// comfort noise to a concealed `frame`; `signal` is the working buffer.
    pub fn process(&mut self, input: &ComfortNoiseInput<'_>, frame: &mut [i16], signal: &mut [i32]) {
        let layout = input.layout;
        let order = layout.rate.lpc_order();
        let lpc_order = order.coefficients();
        if self.rate != Some(layout.rate) {
            // `silk_CNG_Reset`
            let nlsf_step_q15 = match order {
                LpcOrder::Ten => NLSF_STEP_10_Q15,
                LpcOrder::Sixteen => NLSF_STEP_16_Q15,
            };
            let mut nlsf_acc_q15: i32 = 0;
            for nlsf in self.smth_nlsf_q15.iter_mut().take(lpc_order) {
                // At most 16 steps of a 17th of 32767
                nlsf_acc_q15 = nlsf_acc_q15.wrapping_add(nlsf_step_q15);
                *nlsf = nlsf_acc_q15 as i16;
            }
            self.smth_gain_q16 = 0;
            self.rand_seed = RESET_SEED;
            self.rate = Some(layout.rate);
        }

        if input.inactive_decoded {
            self.update(input);
        }

        if input.concealed {
            self.add_noise(input, frame, signal);
        } else {
            for state in self.synth_state.iter_mut().take(lpc_order) {
                *state = 0;
            }
        }
    }

    /// The estimate's update from a decoded frame without voice activity.
    fn update(&mut self, input: &ComfortNoiseInput<'_>) {
        let layout = input.layout;
        let subframe_length = layout.subframe_length();
        let subframes = layout.subframes();

        // Smoothing of LSF's
        for (smoothed, &nlsf) in
            self.smth_nlsf_q15.iter_mut().zip(input.prev_nlsf_q15).take(layout.rate.lpc_order().coefficients())
        {
            *smoothed =
                add32(i32::from(*smoothed), smulwb(sub32(i32::from(nlsf), i32::from(*smoothed)), CNG_NLSF_SMTH_Q16))
                    as i16;
        }

        // Find the subframe with the highest gain
        let mut max_gain_q16: i32 = 0;
        let mut subfr = 0usize;
        for (i, &gain) in input.gains_q16.iter().enumerate().take(subframes) {
            if gain > max_gain_q16 {
                max_gain_q16 = gain;
                subfr = i;
            }
        }
        // Update CNG excitation buffer with excitation from this subframe: the others move up
        // one subframe, within the frame
        let frame = self.exc_buf_q14.get_mut(..layout.frame_length()).unwrap_or_default();
        if let Some(moved) = frame.len().checked_sub(subframe_length) {
            move_up(frame, moved, subframe_length);
        }
        let loudest = nth_chunk(input.excitation_q14, subframe_length, subfr).unwrap_or_default();
        for (buffered, &e) in frame.iter_mut().zip(loudest) {
            *buffered = e;
        }

        // Smooth gains
        for &gain in input.gains_q16.iter().take(subframes) {
            self.smth_gain_q16 = add32(self.smth_gain_q16, smulwb(sub32(gain, self.smth_gain_q16), CNG_GAIN_SMTH_Q16));
            // If the smoothed gain is 3 dB greater than this subframe's gain, use this
            // subframe's gain to adapt faster
            if smulww(self.smth_gain_q16, CNG_GAIN_SMTH_THRESHOLD_Q16) > gain {
                self.smth_gain_q16 = gain;
            }
        }
    }

    /// The noise added to a concealed frame.
    #[cold]
    #[inline(never)]
    fn add_noise(&mut self, input: &ComfortNoiseInput<'_>, frame: &mut [i16], signal: &mut [i32]) {
        let layout = input.layout;
        let length = layout.frame_length();
        let order = layout.rate.lpc_order();

        // Generate CNG excitation
        let mut gain_q16 = smulww(i32::from(input.rand_scale_q14), input.prev_gain_q16);
        if gain_q16 >= 1 << 21 || self.smth_gain_q16 > 1 << 23 {
            gain_q16 = smultt(gain_q16, gain_q16);
            gain_q16 = sub_lshift32(smultt(self.smth_gain_q16, self.smth_gain_q16), gain_q16, 5);
            gain_q16 = lshift(sqrt_approx(gain_q16), 16);
        } else {
            gain_q16 = smulww(gain_q16, gain_q16);
            gain_q16 = sub_lshift32(smulww(self.smth_gain_q16, self.smth_gain_q16), gain_q16, 5);
            gain_q16 = lshift(sqrt_approx(gain_q16), 8);
        }
        let gain_q10 = rshift(gain_q16, 6);

        // `silk_CNG_exc`: the excitation, random samples of the buffer
        let signal = signal.get_mut(..MAX_LPC_ORDER.saturating_add(length)).unwrap_or_default();
        let mut exc_mask = CNG_BUF_MASK_MAX;
        while exc_mask > length {
            exc_mask >>= 1;
        }
        let mut seed = self.rand_seed;
        for e in signal.iter_mut().skip(MAX_LPC_ORDER) {
            seed = rand(seed);
            // The mask is below the buffer's length
            *e = self.exc_buf_q14.get(rand_index(seed, 8) & exc_mask).copied().unwrap_or(0);
        }
        self.rand_seed = seed;

        // Convert CNG NLSF to filter representation
        let mut a_q12 = [0i16; MAX_LPC_ORDER];
        nlsf::to_lpc(&mut a_q12, &self.smth_nlsf_q15, order);

        // Generate CNG signal, by synthesis filtering, scaled and added to the frame
        if let Some(state) = signal.first_chunk_mut::<MAX_LPC_ORDER>() {
            *state = self.synth_state;
        }
        synthesis_of_order(order, signal, &a_q12, |i, lpc_q14| {
            if let Some(sample) = frame.get_mut(i) {
                *sample = add_sat16(i32::from(*sample), sat16(rshift_round(smulww(lpc_q14, gain_q10), 8)));
            }
        });
        // The signal is the state and `length` samples, so its last are the new state
        if let Some(state) = signal.last_chunk::<MAX_LPC_ORDER>() {
            self.synth_state = *state;
        }
    }
}
