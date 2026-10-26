//! The SILK decoder (RFC 6716 §4.2) for the profile: mono, one 20 ms frame a packet, output at
//! 48 kHz, concealment of 10 or 20 ms, and the in-band FEC (LBRR) data parsed, to stay in
//! step, and not decoded.
//!
//! From the reference decoder's `silk/dec_API.c`, `decode_frame.c`, `decoder_set_fs.c`,
//! `init_decoder.c`, without deep PLC and OSCE. Copyright (c) 2006-2011, Skype Limited,
//! BSD-3-Clause (the licence beside the crate).

use crate::range::EcDec;

use super::cng::{ComfortNoise, ComfortNoiseInput};
use super::excitation::decode_pulses;
use super::layout::{
    FrameDuration, InternalRate, Layout, MAX_FRAME_LENGTH, MAX_LPC_ORDER, MAX_SUBFRAME_LENGTH, SignalType,
};
use super::parameters::{self, FrameParameters, ParameterState};
use super::plc::{Concealment, ConcealmentInput};
use super::resampler::{RESAMPLER_BUFFER, Resampler};
use super::scratch::Scratch;
use super::side_info::SideInfo;
use super::synthesis::{SynthesisState, reconstruct};
use crate::slices::discard_front;

/// The output history the long-term predictor and the concealment reach into: the LTP memory
/// and, at a rewhitening in the middle of a frame, its first half.
const HISTORY: usize = MAX_FRAME_LENGTH + 2 * MAX_SUBFRAME_LENGTH;

/// What one call decodes: a received 20 ms frame, or a lost frame to conceal.
pub enum Frame<'a, 'b> {
    /// A received frame, read from the packet's range decoder.
    Received {
        /// The packet's range decoder, at the frame's first symbol.
        range: &'a mut EcDec<'b>,
        /// The internal rate the packet's bandwidth sets.
        rate: InternalRate,
    },
    /// A lost frame, concealed at the rate of the last received one.
    Lost {
        /// How much to conceal.
        duration: FrameDuration,
    },
}

/// Why a call produced no output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SilkError {
    /// A lost frame before any received one: there is no rate to conceal at.
    NoRate,
    /// The output is shorter than the frame.
    OutputTooSmall,
}

/// `silk_decoder_state` without what one frame a packet and mono make constant: everything
/// `silk_reset_decoder` clears.
#[derive(Clone)]
struct ChannelState {
    /// The internal rate of the last frame, `None` after a reset.
    rate: Option<InternalRate>,
    prev_gain_q16: i32,
    excitation_q14: [i32; MAX_FRAME_LENGTH],
    lpc_state_q14: [i32; MAX_LPC_ORDER],
    history: [i16; HISTORY],
    /// The pitch lag of the last subframe, at most 18 ms.
    lag_prev: u16,
    last_gain_index: i8,
    /// The NLSFs of the last frame, from which the next interpolates.
    prev_nlsf_q15: [i16; MAX_LPC_ORDER],
    /// Deactivates NLSF interpolation and clears the concealment's filter.
    first_frame_after_reset: bool,
    /// Lost frames in a row, saturating: only whether it is 0 or 1 matters.
    loss_count: u8,
    prev_signal_type: SignalType,
    resampler: Resampler,
    comfort_noise: ComfortNoise,
    concealment: Concealment,
}

impl ChannelState {
    /// `silk_reset_decoder`.
    const RESET: Self = Self {
        rate: None,
        prev_gain_q16: 1 << 16,
        excitation_q14: [0; MAX_FRAME_LENGTH],
        lpc_state_q14: [0; MAX_LPC_ORDER],
        history: [0; HISTORY],
        lag_prev: 0,
        last_gain_index: 0,
        prev_nlsf_q15: [0; MAX_LPC_ORDER],
        first_frame_after_reset: true,
        loss_count: 0,
        prev_signal_type: SignalType::Inactive,
        // Replaced by `set_rate` before it is used
        resampler: Resampler::new(InternalRate::Wideband),
        comfort_noise: ComfortNoise::NEW,
        concealment: Concealment::NEW,
    };

    /// `silk_decoder_set_fs`: a change of internal rate (or the first frame after a reset)
    /// starts the resampler and the LTP and LPC state afresh. The output rate is the decoder's
    /// and does not change.
    #[inline]
    fn set_rate(&mut self, rate: InternalRate) {
        if self.rate != Some(rate) {
            self.change_rate(rate);
        }
    }

    #[cold]
    #[inline(never)]
    fn change_rate(&mut self, rate: InternalRate) {
        self.resampler = Resampler::new(rate);
        self.first_frame_after_reset = true;
        self.lag_prev = 100;
        self.last_gain_index = 10;
        self.prev_signal_type = SignalType::Inactive;
        self.history = [0; HISTORY];
        self.lpc_state_q14 = [0; MAX_LPC_ORDER];
        self.rate = Some(rate);
    }

    /// `silk_decode_frame`: decodes a 20 ms frame from `range`, or conceals a frame when there
    /// is none, into `out[..frame_length]`.
    fn decode_frame(
        &mut self,
        layout: Layout,
        range: Option<(&mut EcDec<'_>, bool)>,
        out: &mut [i16],
        scratch: &mut Scratch,
    ) {
        let frame_length = layout.frame_length();
        let mut parameters = FrameParameters::ZERO;
        let decoded_signal_type = if let Some((range, vad_flag)) = range {
            let side = SideInfo::decode(range, layout.rate, vad_flag);
            decode_pulses(range, &mut scratch.pulses, side.signal_type, side.quant_offset_type, frame_length);
            parameters = parameters::decode(
                &side,
                layout.rate,
                &mut ParameterState {
                    last_gain_index: &mut self.last_gain_index,
                    prev_nlsf_q15: &mut self.prev_nlsf_q15,
                    first_frame_after_reset: self.first_frame_after_reset,
                    after_loss: self.loss_count != 0,
                },
            );
            reconstruct(
                &mut SynthesisState {
                    excitation_q14: &mut self.excitation_q14,
                    lpc_state_q14: &mut self.lpc_state_q14,
                    history: &mut self.history,
                    prev_gain_q16: &mut self.prev_gain_q16,
                    after_loss: self.loss_count != 0,
                    prev_signal_type: self.prev_signal_type,
                    lag_prev: self.lag_prev,
                },
                layout,
                &side,
                &mut parameters,
                out,
                scratch,
            );
            self.update_history(layout, out);
            self.concealment.follow_rate(layout);
            self.concealment.update(layout, side.signal_type, &parameters);
            self.loss_count = 0;
            self.prev_signal_type = side.signal_type;
            self.first_frame_after_reset = false;
            Some(side.signal_type)
        } else {
            // Handle packet loss by extrapolation
            self.concealment.follow_rate(layout);
            self.concealment.conceal(
                &mut ConcealmentInput {
                    layout,
                    excitation_q14: &self.excitation_q14,
                    history: &self.history,
                    lpc_state_q14: &mut self.lpc_state_q14,
                    first_frame_after_reset: self.first_frame_after_reset,
                    loss_count: self.loss_count,
                    prev_signal_type: self.prev_signal_type,
                },
                &mut parameters,
                out,
                scratch,
            );
            self.loss_count = self.loss_count.saturating_add(1);
            self.update_history(layout, out);
            None
        };

        // Comfort noise generation / estimation
        let out = out.get_mut(..frame_length).unwrap_or_default();
        self.comfort_noise.process(
            &ComfortNoiseInput {
                layout,
                inactive_decoded: decoded_signal_type.is_some() && self.prev_signal_type == SignalType::Inactive,
                concealed: self.loss_count != 0,
                prev_nlsf_q15: &self.prev_nlsf_q15,
                excitation_q14: &self.excitation_q14,
                gains_q16: &parameters.gains_q16,
                rand_scale_q14: self.concealment.rand_scale_q14,
                prev_gain_q16: self.concealment.prev_gain_q16[1],
            },
            out,
            &mut scratch.wide,
        );

        // Ensure smooth connection of extrapolated and good frames
        self.concealment.glue(out, self.loss_count);

        // Update some decoder state variables
        if let Some(&lag) = parameters.pitch_l.get(..layout.subframes()).and_then(<[u16]>::last) {
            self.lag_prev = lag;
        }
    }

    /// Shifts the frame into the output history.
    fn update_history(&mut self, layout: Layout, out: &[i16]) {
        let frame_length = layout.frame_length();
        // The LTP memory is 20 ms, a frame 10 or 20 ms
        let history = self.history.get_mut(..layout.ltp_memory_length()).unwrap_or_default();
        let Some(kept) = history.len().checked_sub(frame_length) else { return };
        discard_front(history, frame_length);
        for (h, &o) in history.iter_mut().skip(kept).zip(out) {
            *h = o;
        }
    }
}

/// The SILK decoder: the channel state, the rate of the last received frame (the Opus layer's,
/// which a reset keeps), the decoded frame after the previous frame's last sample (the mono
/// path of the C's mid/side buffering delays the output by one sample), and the working memory.
#[derive(Clone)]
pub struct SilkDecoder {
    channel: ChannelState,
    received_rate: Option<InternalRate>,
    frame: [i16; 1 + MAX_FRAME_LENGTH],
    scratch: Scratch,
}

impl SilkDecoder {
    /// A decoder as `silk_InitDecoder` leaves it.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            channel: ChannelState::RESET,
            received_rate: None,
            frame: [0; 1 + MAX_FRAME_LENGTH],
            scratch: Scratch::ZERO,
        }
    }

    /// `silk_ResetDecoder`.
    pub fn reset(&mut self) {
        self.channel = ChannelState::RESET;
        self.frame[0] = 0;
    }

    /// `silk_Decode` for one frame: decodes or conceals it into `out` at the output rate and
    /// answers the number of samples written.
    ///
    /// # Errors
    ///
    /// `NoRate` for a lost frame before any received one, `OutputTooSmall` when `out` cannot hold
    /// the frame; nothing is decoded then.
    pub fn decode(&mut self, frame: Frame<'_, '_>, out: &mut [i16]) -> Result<usize, SilkError> {
        let (rate, duration, mut range) = match frame {
            Frame::Received { range, rate } => (rate, FrameDuration::Ms20, Some(range)),
            Frame::Lost { duration } => (self.received_rate.ok_or(SilkError::NoRate)?, duration, None),
        };
        let n_out = duration.output_samples();
        let out = out.get_mut(..n_out).ok_or(SilkError::OutputTooSmall)?;
        if range.is_some() {
            self.received_rate = Some(rate);
        }
        let layout = Layout { rate, duration };
        self.channel.set_rate(rate);

        let mut vad_flag = false;
        if let Some(range) = range.as_deref_mut() {
            // RFC 6716 §4.2.3: the VAD flag and the LBRR flag of the one frame
            vad_flag = range.dec_bit_logp(1);
            let lbrr_flag = range.dec_bit_logp(1);
            // RFC 6716 §4.2.4, §4.2.5: the LBRR frame is parsed, to stay in step, and not
            // decoded
            if lbrr_flag {
                let side = SideInfo::decode(range, rate, true);
                decode_pulses(
                    range,
                    &mut self.scratch.pulses,
                    side.signal_type,
                    side.quant_offset_type,
                    layout.frame_length(),
                );
            }
        }

        let frame_length = layout.frame_length();
        let [_, frame @ ..] = &mut self.frame;
        self.channel.decode_frame(
            layout,
            range.map(|range| (range, vad_flag)),
            frame.get_mut(..frame_length).unwrap_or_default(),
            &mut self.scratch,
        );

        // RFC 6716 §4.2.9: resampled from the previous frame's last sample on
        let (buffer, _) =
            self.scratch.samples.split_first_chunk_mut::<RESAMPLER_BUFFER>().ok_or(SilkError::OutputTooSmall)?;
        self.channel.resampler.process(out, self.frame.get(..frame_length).unwrap_or_default(), buffer);
        if let Some(&last) = self.frame.get(frame_length) {
            self.frame[0] = last;
        }

        if self.channel.loss_count != 0 {
            // On packet loss, remove the gain clamping to prevent having the energy "bounce
            // back" if we lose packets when the energy is going down
            self.channel.last_gain_index = 10;
        }
        Ok(n_out)
    }
}

impl Default for SilkDecoder {
    fn default() -> Self {
        Self::new()
    }
}
