//! The side information of a 20 ms frame (RFC 6716 §4.2.7.3 to §4.2.7.7): the frame type,
//! the gain indices, the NLSF indices and interpolation factor, the pitch lag, contour and LTP
//! indices of a voiced frame, and the LCG seed. Every frame is coded independently: the profile
//! has one frame a packet, and only later frames of a packet are coded conditionally.
//!
//! From the reference decoder's `silk/decode_indices.c`. Copyright (c) 2006-2011, Skype Limited,
//! BSD-3-Clause (the licence beside the crate).

use crate::range::EcDec;

use super::layout::{InternalRate, MAX_LPC_ORDER, MAX_SUBFRAMES, QuantOffsetType, SignalType};
use super::nlsf::{NLSF_QUANT_MAX_AMPLITUDE, unpack};
use super::parameters::nlsf_codebook;
use super::tables::{
    DELTA_GAIN_ICDF, GAIN_ICDF, LTP_GAIN_ICDF_PTRS, LTP_PER_INDEX_ICDF, LTPSCALE_ICDF, NLSF_EXT_ICDF,
    NLSF_INTERPOLATION_FACTOR_ICDF, PITCH_CONTOUR_ICDF, PITCH_CONTOUR_NB_ICDF, PITCH_LAG_ICDF, TYPE_OFFSET_NO_VAD_ICDF,
    TYPE_OFFSET_VAD_ICDF, UNIFORM4_ICDF, UNIFORM6_ICDF, UNIFORM8_ICDF,
};
use super::{decode_below, decode_index};

/// The last symbol of a stage-2 NLSF table, which an extension follows (RFC 6716 §4.2.7.5.2).
const HIGHEST_STAGE2_SYMBOL: i32 = 2 * NLSF_QUANT_MAX_AMPLITUDE as i32;

/// The long-term prediction indices of a voiced frame (RFC 6716 §4.2.7.6).
#[derive(Clone, Copy)]
pub struct LtpIndices {
    /// The absolute pitch lag index.
    pub lag: i16,
    /// The pitch contour index.
    pub contour: usize,
    /// The periodicity index, which selects the LTP codebook.
    pub periodicity: usize,
    /// The LTP filter of each subframe, in the codebook.
    pub filters: [usize; MAX_SUBFRAMES],
    /// The LTP scaling index.
    pub scale: usize,
}

/// `SideInfoIndices`, for one frame.
#[derive(Clone, Copy)]
pub struct SideInfo {
    pub signal_type: SignalType,
    pub quant_offset_type: QuantOffsetType,
    pub gains_indices: [i8; MAX_SUBFRAMES],
    /// The stage-1 index, then the stage-2 indices.
    pub nlsf_indices: [i8; MAX_LPC_ORDER + 1],
    /// The NLSF interpolation factor, Q2: 4 is none.
    pub nlsf_interp_coef_q2: i8,
    /// Present when the frame is voiced.
    pub ltp: Option<LtpIndices>,
    /// The LCG seed, 0 to 3.
    pub seed: i8,
}

impl SideInfo {
    /// `silk_decode_indices` for a 20 ms frame coded independently. `active` is the frame's VAD
    /// flag, or true for an LBRR frame.
    pub fn decode(range: &mut EcDec, rate: InternalRate, active: bool) -> Self {
        // RFC 6716 §4.2.7.3: the signal type and the quantisation offset type
        // The C's `ix` is the symbol, plus 2 for an active frame: its high bit the signal type
        // (voiced or not, of an active frame) and its low bit the offset type
        let (signal_type, ix) = if active {
            let ix = decode_below(range, &TYPE_OFFSET_VAD_ICDF);
            (if ix >> 1 == 0 { SignalType::Unvoiced } else { SignalType::Voiced }, ix)
        } else {
            (SignalType::Inactive, decode_below(range, &TYPE_OFFSET_NO_VAD_ICDF))
        };

        // RFC 6716 §4.2.7.4: the first gain independently, in two stages: MSB bits followed by
        // 3 LSBs; the others as deltas
        let mut gains_indices = [0i8; MAX_SUBFRAMES];
        gains_indices[0] = (range.dec_icdf(signal_type.select(&GAIN_ICDF).as_slice(), 8) << 3)
            .wrapping_add(range.dec_icdf(&UNIFORM8_ICDF, 8)) as i8;
        for gain in &mut gains_indices[1..] {
            *gain = range.dec_icdf(&DELTA_GAIN_ICDF, 8) as i8;
        }

        // RFC 6716 §4.2.7.5.1 and §4.2.7.5.2: the NLSF indices
        let codebook = nlsf_codebook(rate);
        let mut nlsf_indices = [0i8; MAX_LPC_ORDER + 1];
        let cb1_index = decode_index(range, signal_type.select_by_voicing(codebook.cb1_icdf).as_slice());
        nlsf_indices[0] = cb1_index as i8;
        let mut ec_icdf = [&codebook.ec_icdf[0]; MAX_LPC_ORDER];
        let mut pred_q8 = [0u8; MAX_LPC_ORDER];
        unpack(&mut ec_icdf, &mut pred_q8, codebook, cb1_index);
        let [_, stage2_indices @ ..] = &mut nlsf_indices;
        for (index, &icdf) in stage2_indices.iter_mut().zip(&ec_icdf).take(codebook.order.coefficients()) {
            // Symbols and their extensions are below 10 each, so these cannot wrap
            let mut ix = range.dec_icdf(icdf, 8);
            if ix == 0 {
                ix = ix.wrapping_sub(range.dec_icdf(&NLSF_EXT_ICDF, 8));
            } else if ix == HIGHEST_STAGE2_SYMBOL {
                ix = ix.wrapping_add(range.dec_icdf(&NLSF_EXT_ICDF, 8));
            }
            *index = ix.wrapping_sub(i32::from(NLSF_QUANT_MAX_AMPLITUDE)) as i8;
        }

        // RFC 6716 §4.2.7.5.5: the NLSF interpolation factor
        let nlsf_interp_coef_q2 = range.dec_icdf(&NLSF_INTERPOLATION_FACTOR_ICDF, 8) as i8;

        let ltp = (signal_type == SignalType::Voiced).then(|| {
            // RFC 6716 §4.2.7.6.1: the absolute pitch lag index, its high part in steps of
            // half the rate in kHz, then the low part; the contour
            let (low_bits_icdf, contour_icdf): (&[u8], &[u8]) = match rate {
                InternalRate::Narrowband => (&UNIFORM4_ICDF, &PITCH_CONTOUR_NB_ICDF),
                InternalRate::Mediumband => (&UNIFORM6_ICDF, &PITCH_CONTOUR_ICDF),
                InternalRate::Wideband => (&UNIFORM8_ICDF, &PITCH_CONTOUR_ICDF),
            };
            let lag_index = range
                .dec_icdf(&PITCH_LAG_ICDF, 8)
                .wrapping_mul(rate.khz() >> 1)
                .wrapping_add(range.dec_icdf(low_bits_icdf, 8)) as i16;
            let contour_index = decode_index(range, contour_icdf);

            // RFC 6716 §4.2.7.6.2: the periodicity index, then each subframe's filter
            let per_index = decode_below(range, &LTP_PER_INDEX_ICDF);
            let [.., last_codebook_icdf] = LTP_GAIN_ICDF_PTRS;
            let filter_icdf = LTP_GAIN_ICDF_PTRS.get(per_index).copied().unwrap_or(last_codebook_icdf);
            let mut filter_index = [0usize; MAX_SUBFRAMES];
            for filter in &mut filter_index {
                *filter = decode_index(range, filter_icdf);
            }

            // RFC 6716 §4.2.7.6.3: the LTP scaling
            let scale_index = decode_below(range, &LTPSCALE_ICDF);
            LtpIndices {
                lag: lag_index,
                contour: contour_index,
                periodicity: per_index,
                filters: filter_index,
                scale: scale_index,
            }
        });

        // RFC 6716 §4.2.7.7: the LCG seed
        let seed = range.dec_icdf(&UNIFORM4_ICDF, 8) as i8;

        Self {
            signal_type,
            quant_offset_type: if ix & 1 == 0 { QuantOffsetType::Low } else { QuantOffsetType::High },
            gains_indices,
            nlsf_indices,
            nlsf_interp_coef_q2,
            ltp,
            seed,
        }
    }
}
