//! The band energies (RFC 6716 §4.3.2): the coarse energy, predicted in time and frequency, its
//! residual coded with a Laplace-like distribution, then the fine energy and its final bits. Log
//! energies are Q`DB_SHIFT` (Q10), one for each band.

use crate::celt::tables::{BETA_COEF, E_PROB_MODEL, PRED_COEF, SMALL_ENERGY_ICDF};
use crate::celt::{FrameDuration, NB_BANDS};
use crate::fixed::{DB_SHIFT, extract16, max32, mult16_16, pshr32, shl16, shl32, shr32, sub16};
use crate::range::EcDec;

/// The most fine-energy bits a band takes (`MAX_FINE_BITS`).
pub const MAX_FINE_BITS: u8 = 8;
/// `beta_intra`: the prediction from the band below in an intra frame, Q15.
const BETA_INTRA: i16 = 4915;
/// `-QCONST16(9.f, DB_SHIFT)`: the floor of a previous log energy before the prediction.
const MIN_PREDICTED_ENERGY: i16 = -9216;
/// `-QCONST32(28.f, DB_SHIFT+7)`: the floor of the predicted energy, Q(`DB_SHIFT`+7).
const MIN_ENERGY_Q17: i32 = -3_670_016;
/// `QCONST16(.5f, DB_SHIFT)`.
const HALF_DB: i32 = 512;
// Each band has a Laplace model of its own, a pair of entries.
const _: () = assert!(E_PROB_MODEL[0][0].len() == 2 * NB_BANDS);

/// The bits each coding of a coarse residual needs: the Laplace model from 15 on, the small
/// model from 2, one bit from 1 (RFC 6716 §4.3.2.1).
const LAPLACE_MIN_BUDGET: i32 = 15;
const SMALL_MIN_BUDGET: i32 = 2;

/// The Laplace distribution's total, 32768, as its log.
const LAPLACE_FTB: u32 = 15;
const LAPLACE_FT: u32 = 1 << LAPLACE_FTB;
/// `LAPLACE_MINP`: the least probability of a value, in 1/32768.
const LAPLACE_MINP: u32 = 1;
/// `LAPLACE_NMIN`: the values on each side of zero that are always representable.
const LAPLACE_NMIN: u32 = 16;

/// `ec_laplace_decode` (RFC 6716 §4.3.2.1): a value of the Laplace-like distribution where `fs`
/// is the probability of 0 in 1/32768, and `decay` that of ±1 relative to it, in 1/16384 (at
/// most 11456).
pub fn laplace_decode(dec: &mut EcDec, fs: u32, decay: i32) -> i32 {
    let mut fs = fs;
    let mut val: i32 = 0;
    let fm = dec.decode_bin(LAPLACE_FTB);
    let mut fl: u32 = 0;
    if fm >= fs {
        val = val.wrapping_add(1);
        fl = fs;
        // `ec_laplace_get_freq1`: the probability of ±1.
        // The C multiplies the unsigned total by the signed `16384 - decay`, converted.
        let ft = LAPLACE_FT.wrapping_sub(LAPLACE_MINP * (2 * LAPLACE_NMIN)).wrapping_sub(fs);
        fs = (ft.wrapping_mul(16384i32.wrapping_sub(decay).cast_unsigned()) >> 15).wrapping_add(LAPLACE_MINP);
        // Search the decaying part of the distribution.
        while fs > LAPLACE_MINP && fm >= fl.wrapping_add(fs.wrapping_mul(2)) {
            fs = fs.wrapping_mul(2);
            fl = fl.wrapping_add(fs);
            fs = (fs.wrapping_sub(2 * LAPLACE_MINP).wrapping_mul(decay.cast_unsigned()) >> 15)
                .wrapping_add(LAPLACE_MINP);
            val = val.wrapping_add(1);
        }
        // Everything beyond that has probability LAPLACE_MINP.
        if fs <= LAPLACE_MINP {
            // The C's `int di`: the same bits whether its products are signed or not.
            let di = fm.wrapping_sub(fl) >> 1;
            val = val.wrapping_add(di.cast_signed());
            fl = fl.wrapping_add(di.wrapping_mul(2 * LAPLACE_MINP));
        }
        if fm < fl.wrapping_add(fs) {
            val = val.wrapping_neg();
        } else {
            fl = fl.wrapping_add(fs);
        }
    }
    dec.update(fl, fl.wrapping_add(fs).min(LAPLACE_FT), LAPLACE_FT);
    val
}

/// `unquant_coarse_energy` (RFC 6716 §4.3.2.1): the coarse energies of the bands `start..end`
/// into `log_energy`, which holds the last frame's, predicted from them (unless `intra`) and
/// from the band below.
pub fn unquant_coarse_energy(
    start: usize,
    end: usize,
    log_energy: &mut [i16; NB_BANDS],
    intra: bool,
    dec: &mut EcDec,
    duration: FrameDuration,
) {
    let [inter_model, intra_model] = duration.row(&E_PROB_MODEL);
    // Each band has a Laplace model of its own: its probability of 0 and its decay.
    let (model, _) = if intra { intra_model } else { inter_model }.as_chunks::<2>();
    let (coef, beta) = if intra { (0, BETA_INTRA) } else { (*duration.row(&PRED_COEF), *duration.row(&BETA_COEF)) };
    // `dec->storage*8`: unsigned, kept in an opus_int32.
    let budget = (dec.storage() as u32).wrapping_mul(8).cast_signed();
    let mut prev: i32 = 0;
    for (energy, &[probability, decay]) in log_energy.iter_mut().zip(model).take(end).skip(start) {
        let remaining = budget.wrapping_sub(dec.tell());
        let qi: i32 = if remaining >= LAPLACE_MIN_BUDGET {
            laplace_decode(dec, u32::from(probability) << 7, i32::from(decay) << 6)
        } else if remaining >= SMALL_MIN_BUDGET {
            let qi = dec.dec_icdf(&SMALL_ENERGY_ICDF, 2);
            (qi >> 1) ^ (qi & 1).wrapping_neg()
        } else if remaining >= 1 {
            if dec.dec_bit_logp(1) { -1 } else { 0 }
        } else {
            -1
        };
        let q = shl32(qi, DB_SHIFT);
        *energy = (*energy).max(MIN_PREDICTED_ENERGY);
        let tmp =
            pshr32(mult16_16(i32::from(coef), i32::from(*energy)), 8).wrapping_add(prev).wrapping_add(shl32(q, 7));
        *energy = extract16(pshr32(max32(MIN_ENERGY_Q17, tmp), 7));
        prev = prev.wrapping_add(shl32(q, 7)).wrapping_sub(mult16_16(i32::from(beta), pshr32(q, 8)));
    }
}

/// `unquant_fine_energy` (RFC 6716 §4.3.2.2): `fine_quant[i]` more bits of each band's energy.
pub fn unquant_fine_energy(
    start: usize,
    end: usize,
    log_energy: &mut [i16; NB_BANDS],
    fine_quant: &[u8; NB_BANDS],
    dec: &mut EcDec,
) {
    for (energy, &bits) in log_energy.iter_mut().zip(fine_quant).take(end).skip(start) {
        if bits == 0 {
            continue;
        }
        let bits = u32::from(bits);
        // At most `MAX_FINE_BITS` bits: the C's `int` holds them unchanged.
        let q2 = dec.dec_bits(bits).cast_signed();
        let offset = extract16(sub16(shr32(shl32(q2, DB_SHIFT).wrapping_add(HALF_DB), bits), HALF_DB));
        *energy = extract16(i32::from(*energy).wrapping_add(i32::from(offset)));
    }
}

/// `unquant_energy_finalise` (RFC 6716 §4.3.2.2): one more bit of energy for each band that can
/// take it, first those of `fine_priority` false, while `bits_left` lasts.
pub fn unquant_energy_finalise(
    start: usize,
    end: usize,
    log_energy: &mut [i16; NB_BANDS],
    fine_quant: &[u8; NB_BANDS],
    fine_priority: &[bool; NB_BANDS],
    bits_left: i32,
    dec: &mut EcDec,
) {
    let mut bits_left = bits_left;
    for priority in [false, true] {
        for ((energy, &bits), &band_priority) in
            log_energy.iter_mut().zip(fine_quant).zip(fine_priority).take(end).skip(start)
        {
            if bits_left < 1 {
                break;
            }
            if bits >= MAX_FINE_BITS || band_priority != priority {
                continue;
            }
            let q2 = dec.dec_bits(1).cast_signed();
            // `SHR16(SHL16(q2,DB_SHIFT)-QCONST16(.5f,DB_SHIFT),fine_quant[i]+1)`.
            // The shift by `fine_quant[i] + 1` taken in two steps, which equal it.
            let offset = extract16(i32::from(shl16(q2, DB_SHIFT)).wrapping_sub(HALF_DB) >> bits >> 1);
            *energy = extract16(i32::from(*energy).wrapping_add(i32::from(offset)));
            bits_left = bits_left.wrapping_sub(1);
        }
    }
}
