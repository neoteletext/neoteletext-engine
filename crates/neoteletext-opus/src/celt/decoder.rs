//! The CELT decoder (RFC 6716 §4.3): its state, and the decoding of a frame from its header
//! through its energies, allocation and band shapes to the synthesis, the post-filter and the
//! output. A lost frame goes to the concealment (`plc`).

use crate::celt::alloc::{band_width, compute_allocation, init_caps};
use crate::celt::bands::{BandsFrame, BandsScratch, EnergyHistory, anti_collapse, quant_all_bands};
use crate::celt::energy::{unquant_coarse_energy, unquant_energy_finalise, unquant_fine_energy};
use crate::celt::plc::lpc::LPC_ORDER;
use crate::celt::plc::{NARROW_SCRATCH, WIDE_SCRATCH, prefilter_and_fold};
use crate::celt::postfilter::{COMBFILTER_MINPERIOD, CombParameters, InPlace, comb_filter};
use crate::celt::synthesis::{FrameLayout, deemphasis, synthesise};
use crate::celt::tables::{SPREAD_ICDF, TAPSET_ICDF, TF_SELECT, TRIM_ICDF};
use crate::celt::{BITRES, FrameDuration, MAX_BAND, MAX_FRAME, NB_BANDS, OVERLAP, SHORT_BLOCK};
use crate::range::EcDec;
use crate::slices::discard_front;
use core::ops::Range;

/// `DECODE_BUFFER_SIZE`: the history of synthesised signal the decoder keeps, for the
/// post-filter and the concealment.
pub const HISTORY: usize = 2048;
// A frame and the longest pitch period the concealment reaches back fit the history.
const _: () = assert!(MAX_FRAME < HISTORY);

/// Where a frame of `n` samples starts in the history, which it ends before the overlap.
#[inline]
pub(super) const fn frame_start(n: usize) -> usize {
    HISTORY.saturating_sub(n)
}

/// The newest frame of `n` samples in `history`, before the overlap.
#[inline]
fn newest_frame(history: &[i32; HISTORY + OVERLAP], n: usize) -> &[i32] {
    history.get(frame_start(n)..HISTORY).unwrap_or_default()
}

/// The largest frame, in bytes (RFC 6716 §3.2.1).
const MAX_FRAME_BYTES: i32 = 1275;
/// `SPREAD_NORMAL`: the spreading when the frame has no bits to code it.
const SPREAD_NORMAL: i32 = 2;
/// `QCONST16(28.f, DB_SHIFT)`: the log energy of silence is its negation.
const LOG_28: i16 = 28672;
/// `QCONST16(20.f, DB_SHIFT)`.
const LOG_20: i32 = 20480;
/// `QCONST16(1.5f, DB_SHIFT)`, `QCONST16(.5f, DB_SHIFT)`: the energy safety margins after a
/// loss, for LM 0 and LM 1.
const SAFETY_LM0: i16 = 1536;
const SAFETY_LM1: i16 = 512;
/// `QCONST16(.09375f, 15)`: the post-filter's gain step.
const POSTFILTER_GAIN_STEP: i32 = 3072;
/// `QCONST16(0.001f, DB_SHIFT)`: the background energy's rise a 2.5 ms block.
const BACKGROUND_RISE: i32 = 1;
/// The largest band's folding source: every band but the last, in a 20 ms frame.
const NORM: usize = 8 * 78;

/// Why a frame could not be decoded.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CeltError {
    /// A duration the profile does not have, a frame over 1275 bytes, or an output too short.
    BadArgument,
    /// The frame read past its end: the stream is corrupt.
    Corrupt,
}

/// The band log energies the decoder keeps, Q`DB_SHIFT`.
#[derive(Clone)]
pub struct Energies {
    /// This frame's, or the last frame's until this one's are decoded (`oldBandE`).
    pub current: [i16; NB_BANDS],
    /// As the last decoded frame left them. Concealment lowers `current`; the next decoded
    /// frame starts from the larger of the two. (This is the second channel of the reference's
    /// state, which a mono decoder keeps as a copy of the first.)
    pub decoded: [i16; NB_BANDS],
    /// The last frame's (`oldLogE`), for the anti-collapse and the energy after a loss.
    pub previous: [i16; NB_BANDS],
    /// The frame's before the last (`oldLogE2`), for the same.
    pub before_previous: [i16; NB_BANDS],
    /// The background noise's, to which concealment decays.
    pub background: [i16; NB_BANDS],
}

/// The scratch of a frame's decoding, shared by the stages that never run together: the band
/// shapes, the synthesis, and the concealment.
#[derive(Clone)]
pub struct Workspace {
    /// 16-bit values: the band shapes, then their folding source and the Hadamard scratch; or
    /// the concealment's.
    pub narrow: [i16; NARROW],
    /// 32-bit values: the coefficients and a codeword's pulses; or the concealment's.
    pub wide: [i32; WIDE],
}

/// The 16-bit and 32-bit scratch values: the most either the frame decoding or the concealment
/// needs.
const NARROW: usize = larger(MAX_FRAME + NORM + MAX_BAND, NARROW_SCRATCH);
const WIDE: usize = larger(MAX_FRAME + MAX_BAND, WIDE_SCRATCH);

const fn larger(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

/// The CELT decoder, mono, for the 48 kHz mode (`OpusCustomDecoder`).
#[derive(Clone)]
pub struct CeltDecoder {
    /// The bands decoded: from 17 in Hybrid frames, to the bandwidth's last.
    pub(super) start_band: u8,
    pub(super) end_band: u8,
    pub(super) rng: u32,
    /// The 2.5 ms blocks lost since the last decoded frame.
    pub(super) loss_duration: u16,
    /// Whether the concealment must use noise: until a frame is decoded after a reset.
    pub(super) skip_plc: bool,
    /// The pitch the concealment found at the start of the loss.
    pub(super) last_pitch_index: u16,
    /// The post-filter of this frame and of the last.
    pub(super) postfilter: CombParameters,
    pub(super) postfilter_old: CombParameters,
    /// Whether the last frame was a pitch concealment, whose tail the next frame folds.
    pub(super) prefilter_and_fold: bool,
    deemphasis_memory: i32,
    /// The synthesised signal, the newest frame last, with the overlap of the next after it.
    pub(super) history: [i32; HISTORY + OVERLAP],
    /// The concealment's prediction filter.
    pub(super) lpc: [i16; LPC_ORDER],
    pub(super) energy: Energies,
    pub(super) workspace: Workspace,
}

impl Default for CeltDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl CeltDecoder {
    /// `celt_decoder_init`: a decoder with its output at 48 kHz.
    #[must_use]
    pub fn new() -> Self {
        let mut decoder = Self {
            start_band: 0,
            end_band: NB_BANDS as u8,
            rng: 0,
            loss_duration: 0,
            skip_plc: true,
            last_pitch_index: 0,
            postfilter: CombParameters::default(),
            postfilter_old: CombParameters::default(),
            prefilter_and_fold: false,
            deemphasis_memory: 0,
            history: [0; HISTORY + OVERLAP],
            lpc: [0; LPC_ORDER],
            energy: Energies {
                current: [0; NB_BANDS],
                decoded: [0; NB_BANDS],
                previous: [0; NB_BANDS],
                before_previous: [0; NB_BANDS],
                background: [0; NB_BANDS],
            },
            workspace: Workspace { narrow: [0; NARROW], wide: [0; WIDE] },
        };
        decoder.reset();
        decoder
    }

    /// `OPUS_RESET_STATE`: everything but the band range.
    pub fn reset(&mut self) {
        self.rng = 0;
        self.loss_duration = 0;
        self.skip_plc = true;
        self.last_pitch_index = 0;
        self.postfilter = CombParameters::default();
        self.postfilter_old = CombParameters::default();
        self.prefilter_and_fold = false;
        self.deemphasis_memory = 0;
        self.history.fill(0);
        self.lpc.fill(0);
        self.energy = Energies {
            current: [0; NB_BANDS],
            decoded: [0; NB_BANDS],
            previous: [-LOG_28; NB_BANDS],
            before_previous: [-LOG_28; NB_BANDS],
            background: [0; NB_BANDS],
        };
    }

    /// `CELT_SET_START_BAND`: 0, or 17 for Hybrid frames.
    pub fn set_start_band(&mut self, band: usize) {
        self.start_band = band.min(NB_BANDS) as u8;
    }

    /// `CELT_SET_END_BAND`: the bandwidth's last band, one past.
    pub fn set_end_band(&mut self, band: usize) {
        self.end_band = band.min(NB_BANDS) as u8;
    }

    /// `OPUS_GET_FINAL_RANGE`.
    #[must_use]
    pub fn final_range(&self) -> u32 {
        self.rng
    }

    /// `celt_decode_with_ec`: one frame of `duration` at 48 kHz into `pcm`, or added to it
    /// when `accum`, from `data` (`None`, or a byte or less: lost, and concealed), with the
    /// Opus layer's range decoder `dec` when it shares one.
    pub fn decode_with_ec<'a>(
        &mut self,
        data: Option<&'a [u8]>,
        pcm: &mut [i16],
        duration: FrameDuration,
        dec: Option<&mut EcDec<'a>>,
        accum: bool,
    ) -> Result<(), CeltError> {
        let n = duration.samples();
        let pcm = pcm.get_mut(..n).ok_or(CeltError::BadArgument)?;
        let Some(len) = i32::try_from(data.map_or(0, <[u8]>::len)).ok().filter(|&len| len <= MAX_FRAME_BYTES) else {
            return Err(CeltError::BadArgument);
        };
        let Some(data) = data.filter(|d| d.len() > 1) else {
            self.conceal(duration);
            deemphasis(newest_frame(&self.history, n), pcm, &mut self.deemphasis_memory, accum);
            return Ok(());
        };
        let mut local;
        let dec: &mut EcDec<'a> = if let Some(dec) = dec {
            dec
        } else {
            local = EcDec::new(data);
            &mut local
        };
        self.decode_frame(duration, len, dec);
        deemphasis(newest_frame(&self.history, n), pcm, &mut self.deemphasis_memory, accum);
        self.loss_duration = 0;
        self.prefilter_and_fold = false;
        if dec.tell() > len.wrapping_mul(8) { Err(CeltError::Corrupt) } else { Ok(()) }
    }

    /// The decoding of a frame of `len` bytes (at most 1275) into the history (RFC 6716 §4.3),
    /// the de-emphasis aside. The C's `int` arithmetic wraps, as it compiles to; on these
    /// magnitudes it never does.
    fn decode_frame(&mut self, duration: FrameDuration, len: i32, dec: &mut EcDec) {
        let frame_bits = len.wrapping_mul(8);
        let lm = duration.lm();
        let n = duration.samples();
        let start = usize::from(self.start_band);
        let end = usize::from(self.end_band);
        if self.loss_duration == 0 {
            self.skip_plc = false;
        }
        let energy = &mut self.energy;
        for (current, &decoded) in energy.current.iter_mut().zip(&energy.decoded) {
            *current = (*current).max(decoded);
        }

        let header = decode_header(dec, frame_bits, start == 0, lm);
        let Header { silence, postfilter, transient, intra } = header;
        if !intra && self.loss_duration != 0 {
            limit_energy_after_loss(energy, start..end, self.loss_duration, duration);
        }
        unquant_coarse_energy(start, end, &mut energy.current, intra, dec, duration);

        let tf_res = tf_decode(start, end, transient, duration, dec);
        let spread =
            if dec.tell().wrapping_add(4) <= frame_bits { dec.dec_icdf(&SPREAD_ICDF, 5) } else { SPREAD_NORMAL };

        // The dynamic allocation's boosts and the allocation trim (RFC 6716 §4.3.3).
        let cap = init_caps(duration);
        let mut total_bits = frame_bits << BITRES;
        let offsets = decode_boosts(dec, start..end, lm, &cap, &mut total_bits);
        let alloc_trim =
            if dec.tell_frac().wrapping_add(6 << BITRES) <= total_bits { dec.dec_icdf(&TRIM_ICDF, 7) } else { 5 };
        let mut bits = (frame_bits << BITRES).wrapping_sub(dec.tell_frac()).wrapping_sub(1);
        let anti_collapse_rsv =
            if transient && lm >= 2 && bits >= (lm.wrapping_add(2) << BITRES) { 1 << BITRES } else { 0 };
        bits = bits.wrapping_sub(anti_collapse_rsv);
        let allocation = compute_allocation(start..end, &offsets, &cap, alloc_trim, bits, lm, dec);
        unquant_fine_energy(start, end, &mut energy.current, &allocation.fine_quant, dec);

        // The band shapes, into a frame's room at the history's end; and the last frame's
        // concealment folded into it, if it was one.
        discard_front(&mut self.history, n);
        if self.prefilter_and_fold {
            prefilter_and_fold(&mut self.history, self.postfilter_old, self.postfilter, n);
        }
        let (x, rest) = self.workspace.narrow.split_at_mut(MAX_FRAME);
        let (norm, hadamard) = rest.split_at_mut(NORM);
        let (freq, pulses) = self.workspace.wide.split_at_mut(MAX_FRAME);
        let Some(x) = x.get_mut(..n) else {
            return;
        };
        let mut collapse_masks = [0u8; NB_BANDS];
        let frame = BandsFrame {
            start,
            end,
            pulses: &allocation.pulses,
            transient,
            spread,
            tf_res: &tf_res,
            total_bits: len.wrapping_mul(8 << BITRES).wrapping_sub(anti_collapse_rsv),
            balance: allocation.balance,
            lm,
            coded_bands: allocation.coded_bands,
        };
        quant_all_bands(&frame, x, &mut collapse_masks, dec, &mut self.rng, BandsScratch { norm, hadamard, pulses });
        let anti_collapse_on = anti_collapse_rsv > 0 && dec.dec_bits(1) != 0;
        unquant_energy_finalise(
            start,
            end,
            &mut energy.current,
            &allocation.fine_quant,
            &allocation.fine_priority,
            frame_bits.wrapping_sub(dec.tell()),
            dec,
        );
        if anti_collapse_on {
            let history = EnergyHistory {
                current: &energy.current,
                previous: &energy.previous,
                before_previous: &energy.before_previous,
            };
            anti_collapse(x, &collapse_masks, lm, start..end, history, &allocation.pulses, self.rng);
        }
        if silence {
            energy.current = [-LOG_28; NB_BANDS];
        }

        let layout = FrameLayout { duration, transient, silence };
        let frame_and_overlap = self.history.get_mut(frame_start(n)..).unwrap_or_default();
        synthesise(x, freq, &self.energy.current, start..end, layout, frame_and_overlap);
        self.postfilter_history(duration, postfilter);
        self.keep_energies(start..end, transient, lm);
        self.rng = dec.rng();
    }

    /// The post-filter over the frame just synthesised (RFC 6716 §4.3.7.1): the last frame's
    /// filter cross-faded to this one's, `postfilter`, over the first block, this one's after.
    fn postfilter_history(&mut self, duration: FrameDuration, postfilter: CombParameters) {
        let n = duration.samples();
        let long = duration != FrameDuration::TwoAndAHalfMs;
        self.postfilter.period = self.postfilter.period.max(COMBFILTER_MINPERIOD);
        self.postfilter_old.period = self.postfilter_old.period.max(COMBFILTER_MINPERIOD);
        let at = frame_start(n);
        comb_filter(
            &mut InPlace { buffer: &mut self.history, at },
            self.postfilter_old,
            self.postfilter,
            SHORT_BLOCK,
            OVERLAP,
        );
        if long {
            comb_filter(
                &mut InPlace { buffer: &mut self.history, at: at.saturating_add(SHORT_BLOCK) },
                self.postfilter,
                postfilter,
                n.saturating_sub(SHORT_BLOCK),
                OVERLAP,
            );
        }
        self.postfilter_old = if long { postfilter } else { self.postfilter };
        self.postfilter = postfilter;
    }

    /// The energies kept for the next frame, after one of `lm` decoding `bands`.
    fn keep_energies(&mut self, bands: Range<usize>, transient: bool, lm: i32) {
        let energy = &mut self.energy;
        energy.decoded = energy.current;
        if transient {
            for (previous, &current) in energy.previous.iter_mut().zip(&energy.current) {
                *previous = (*previous).min(current);
            }
        } else {
            energy.before_previous = energy.previous;
            energy.previous = energy.current;
        }
        let rise = i32::from(self.loss_duration).wrapping_add(1 << lm).min(160).wrapping_mul(BACKGROUND_RISE);
        for (background, &current) in energy.background.iter_mut().zip(&energy.current) {
            *background = (i32::from(*background).wrapping_add(rise) as i16).min(current);
        }
        // Clear the bands outside this frame's, in case the next frame's range differs.
        let kept = energy
            .current
            .iter_mut()
            .zip(&mut energy.decoded)
            .zip(&mut energy.previous)
            .zip(&mut energy.before_previous)
            .enumerate();
        for (band, (((current, decoded), previous), before_previous)) in kept {
            if !bands.contains(&band) {
                *current = 0;
                *decoded = 0;
                *previous = -LOG_28;
                *before_previous = -LOG_28;
            }
        }
    }
}

/// The flags of a frame's header (RFC 6716 §4.3, Table 56).
struct Header {
    silence: bool,
    postfilter: CombParameters,
    transient: bool,
    intra: bool,
}

/// The header of a frame of `total_bits` (RFC 6716 §4.3, Table 56): silence, the post-filter
/// (only when the frame `codes_postfilter`, from band 0), transient and intra.
fn decode_header(dec: &mut EcDec, total_bits: i32, codes_postfilter: bool, lm: i32) -> Header {
    let mut tell = dec.tell();
    let silence = if tell >= total_bits { true } else { tell == 1 && dec.dec_bit_logp(15) };
    if silence {
        // Pretend every bit is read, so that what follows reads nothing.
        tell = total_bits;
        dec.add_nbits_total(tell.wrapping_sub(dec.tell()));
    }
    let mut postfilter = CombParameters::default();
    if codes_postfilter && tell.wrapping_add(16) <= total_bits {
        if dec.dec_bit_logp(1) {
            let octave = dec.dec_uint(6);
            // At most 16 << 5 plus 9 bits: the C's `int` holds the period unchanged.
            postfilter.period =
                (16u32 << octave).wrapping_add(dec.dec_bits(octave.wrapping_add(4))).wrapping_sub(1).cast_signed();
            let qg = dec.dec_bits(3).cast_signed();
            if dec.tell().wrapping_add(2) <= total_bits {
                // `dec_icdf` answers one of the three symbols.
                postfilter.tapset = u8::try_from(dec.dec_icdf(&TAPSET_ICDF, 2)).unwrap_or(0);
            }
            postfilter.gain = POSTFILTER_GAIN_STEP.wrapping_mul(qg.wrapping_add(1)) as i16;
        }
        tell = dec.tell();
    }
    let transient = if lm > 0 && tell.wrapping_add(3) <= total_bits {
        let transient = dec.dec_bit_logp(3);
        tell = dec.tell();
        transient
    } else {
        false
    };
    let intra = tell.wrapping_add(3) <= total_bits && dec.dec_bit_logp(3);
    Header { silence, postfilter, transient, intra }
}

/// After a loss of `loss_duration` blocks, an inter frame predicts from energies no higher than
/// the history's trend, less a margin (RFC 6716 §4.4).
fn limit_energy_after_loss(energy: &mut Energies, bands: Range<usize>, loss_duration: u16, duration: FrameDuration) {
    let missing = i32::from(loss_duration >> duration.lm()).min(10);
    let safety = match duration {
        FrameDuration::TwoAndAHalfMs => SAFETY_LM0,
        FrameDuration::FiveMs => SAFETY_LM1,
        FrameDuration::TwentyMs => 0,
    };
    let kept = energy.current.iter_mut().zip(&energy.previous).zip(&energy.before_previous);
    for ((current, &previous), &before) in kept.take(bands.end).skip(bands.start) {
        if *current < previous.max(before) {
            let e0 = i32::from(*current);
            let slope = i32::from(previous).wrapping_sub(e0).max(i32::from(before).wrapping_sub(e0) >> 1);
            *current = (-LOG_20).max(e0.wrapping_sub(0.max(missing.wrapping_add(1).wrapping_mul(slope)))) as i16;
        } else {
            *current = (*current).min(previous).min(before);
        }
        *current = i32::from(*current).wrapping_sub(i32::from(safety)) as i16;
    }
}

/// The dynamic allocation's boosts of `bands` (RFC 6716 §4.3.3), each taken from `total_bits`
/// (in 1/8 bits) as it is read.
fn decode_boosts(
    dec: &mut EcDec,
    bands: Range<usize>,
    lm: i32,
    cap: &[i32; NB_BANDS],
    total_bits: &mut i32,
) -> [i32; NB_BANDS] {
    let mut offsets = [0i32; NB_BANDS];
    let mut dynalloc_logp: u8 = 6;
    let mut tell_frac = dec.tell_frac();
    for (band, (offset, &band_cap)) in offsets.iter_mut().zip(cap).enumerate().take(bands.end).skip(bands.start) {
        let width = band_width(band) << lm;
        // Each boost is a bit a sample, at least 6 bits and at most a byte a sample.
        let quanta = (width << BITRES).min((6 << BITRES).max(width));
        let mut dynalloc_loop_logp = dynalloc_logp;
        let mut boost = 0;
        while tell_frac.wrapping_add(i32::from(dynalloc_loop_logp) << BITRES) < *total_bits && boost < band_cap {
            let flag = dec.dec_bit_logp(u32::from(dynalloc_loop_logp));
            tell_frac = dec.tell_frac();
            if !flag {
                break;
            }
            boost = boost.wrapping_add(quanta);
            *total_bits = total_bits.wrapping_sub(quanta);
            dynalloc_loop_logp = 1;
        }
        *offset = boost;
        if boost > 0 {
            dynalloc_logp = dynalloc_logp.saturating_sub(1).max(2);
        }
    }
    offsets
}

/// `tf_decode` (RFC 6716 §4.3.4.5): each band's time-frequency change.
fn tf_decode(start: usize, end: usize, transient: bool, duration: FrameDuration, dec: &mut EcDec) -> [i8; NB_BANDS] {
    let mut budget = (dec.storage() as u32).wrapping_mul(8);
    // The C keeps the tell in an `opus_uint32`.
    let mut tell = dec.tell().cast_unsigned();
    let mut logp: u32 = if transient { 2 } else { 4 };
    let tf_select_rsv = u32::from(duration.lm() > 0 && tell.wrapping_add(logp) < budget);
    budget = budget.wrapping_sub(tf_select_rsv);
    // Whether each band's change differs from the band below's, read as a running parity.
    let mut changed_bands = [false; NB_BANDS];
    let mut tf_changed = false;
    let mut curr = false;
    for changed in changed_bands.iter_mut().take(end).skip(start) {
        if tell.wrapping_add(logp) <= budget {
            curr ^= dec.dec_bit_logp(logp);
            tell = dec.tell().cast_unsigned();
            tf_changed |= curr;
        }
        *changed = curr;
        logp = if transient { 4 } else { 5 };
    }
    // The changes of a frame transient or not, by `tf_select`, then by whether the band's
    // differs from the band below's.
    let &[a0, a1, a2, a3, b0, b1, b2, b3] = duration.row(&TF_SELECT);
    let [select0, select1] = if transient { [[b0, b1], [b2, b3]] } else { [[a0, a1], [a2, a3]] };
    let pick = |[unchanged, changed]: [i8; 2], band_changed: bool| if band_changed { changed } else { unchanged };
    let tf_select = tf_select_rsv != 0 && pick(select0, tf_changed) != pick(select1, tf_changed) && dec.dec_bit_logp(1);
    let selected = if tf_select { select1 } else { select0 };
    let mut tf_res = [0i8; NB_BANDS];
    for (res, &changed) in tf_res.iter_mut().zip(&changed_bands).take(end).skip(start) {
        *res = pick(selected, changed);
    }
    tf_res
}
