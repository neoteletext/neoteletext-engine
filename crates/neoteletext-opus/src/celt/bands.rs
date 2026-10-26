//! The band shapes (RFC 6716 §4.3.4): each band's bits split between halves by a coded angle
//! until a codebook can take them, the time-frequency changes and Hadamard reordering around
//! that, the folding of bands without pulses from the bands below, and the anti-collapse noise
//! of transients. Mono: the decoder never meets a stereo frame.

use crate::celt::alloc::{band_width, bits2pulses, get_pulses, pulses2bits, splits};
use crate::celt::pvq::{Codeword, SPREAD_AGGRESSIVE, alg_unquant, renormalise_vector};
use crate::celt::tables::{BAND_WIDTHS, EBANDS, LOG_N};
use crate::celt::{BITRES, MAX_BAND, NB_BANDS};
use crate::fixed::{
    NORM_SCALING, Q15ONE, add32, ec_ilog, extract16, min32, mult16_16, mult16_16_p15, mult16_16_q14, mult16_16_q15,
    mult16_32_q15, pshr32, shl16, shl32, shr32, sub32,
};
use crate::math::{celt_exp2, celt_ilog2, celt_rsqrt_norm, celt_sqrt, isqrt32};
use crate::range::EcDec;

use crate::slices::{chunks_mut, copy, for_each_strided, strided};
use core::num::NonZeroU32;
use core::ops::Range;

/// `QTHETA_OFFSET`: the split angle's resolution offset.
const QTHETA_OFFSET: i32 = 4;
/// `QCONST16(.70710678f, 15)`.
const SQRT_HALF_Q15: i32 = 23170;
/// `QCONST16(1.0f/256, 10)`: the folding noise, about 48 dB below the normal folding level.
const FOLD_NOISE: i16 = 4;
/// `QCONST16(0.5f, 15)`.
const HALF_Q15: i32 = 16384;

/// `FRAC_MUL16`: the same rounded Q15 product of the operands truncated to 16 bits as
/// `MULT16_16_P15`.
#[inline]
const fn frac_mul16(a: i32, b: i32) -> i32 {
    mult16_16_p15(a, b)
}

/// `celt_lcg_rand`: the folding and concealment noise's generator.
#[inline]
#[must_use]
pub const fn celt_lcg_rand(seed: u32) -> u32 {
    1_664_525u32.wrapping_mul(seed).wrapping_add(1_013_904_223)
}

/// `bitexact_cos`: a cosine of `x` × π/32768 that is the same on every platform, since the bit
/// allocation depends on it.
#[must_use]
pub fn bitexact_cos(x: i16) -> i16 {
    // The C's `int` arithmetic, wrapping as it compiles to.
    let tmp: i32 = add32(4096, mult16_16(i32::from(x), i32::from(x))) >> 13;
    let x2 = i32::from(tmp as i16);
    let x2 = i32::from(add32(
        sub32(32767, x2),
        frac_mul16(x2, add32(-7651, frac_mul16(x2, add32(8277, frac_mul16(-626, x2))))),
    ) as i16);
    add32(1, x2) as i16
}

/// `bitexact_log2tan`: log2 of `isin / icos`, Q11, the same on every platform.
#[must_use]
pub fn bitexact_log2tan(mut isin: i32, mut icos: i32) -> i32 {
    // `EC_ILOG` of the C's `int`s, converted to unsigned.
    let lc = ec_ilog(icos.cast_unsigned());
    let ls = ec_ilog(isin.cast_unsigned());
    icos <<= 15i32.wrapping_sub(lc);
    isin <<= 15i32.wrapping_sub(ls);
    sub32(
        add32(sub32(ls, lc).wrapping_mul(1 << 11), frac_mul16(isin, add32(frac_mul16(isin, -2597), 7932))),
        frac_mul16(icos, add32(frac_mul16(icos, -2597), 7932)),
    )
}

/// `ordery_table`: natural Hadamard order to the bit-reversed Gray order with the DC at the
/// end, a row for each of N = 2, 4, 8 and 16.
static ORDERY_TABLE: [u8; 30] =
    [1, 0, 3, 0, 2, 1, 7, 0, 4, 3, 6, 1, 5, 2, 15, 0, 8, 7, 12, 3, 11, 4, 14, 1, 9, 6, 13, 2, 10, 5];

/// `deinterleave_hadamard`: the `stride` interleaved blocks of `n0` values of `x` one after
/// another, in the ordery order if `hadamard`; `tmp` is scratch.
fn deinterleave_hadamard(x: &mut [i16], tmp: &mut [i16], n0: usize, stride: usize, hadamard: bool) {
    let Some(n) = n0.checked_mul(stride) else {
        return;
    };
    let (Some(x), Some(tmp)) = (x.get_mut(..n), tmp.get_mut(..n)) else {
        return;
    };
    for i in 0..stride {
        let block_start = hadamard_row(i, stride, hadamard).checked_mul(n0);
        let Some(block) = block_start.and_then(|at| tmp.get_mut(at..)).and_then(|b| b.get_mut(..n0)) else {
            continue;
        };
        for (v, &value) in block.iter_mut().zip(strided(x, i, stride)) {
            *v = value;
        }
    }
    copy(x, tmp);
}

/// Where block `i` of `stride` goes in the ordery order (when `hadamard`, for a stride of 2 to
/// 16), or in the natural one.
#[inline]
fn hadamard_row(i: usize, stride: usize, hadamard: bool) -> usize {
    if hadamard {
        ORDERY_TABLE.get(stride.wrapping_add(i).wrapping_sub(2)).map_or(i, |&row| usize::from(row))
    } else {
        i
    }
}

/// `interleave_hadamard`: the inverse of `deinterleave_hadamard`.
fn interleave_hadamard(x: &mut [i16], tmp: &mut [i16], n0: usize, stride: usize, hadamard: bool) {
    let Some(n) = n0.checked_mul(stride) else {
        return;
    };
    let (Some(x), Some(tmp)) = (x.get_mut(..n), tmp.get_mut(..n)) else {
        return;
    };
    for i in 0..stride {
        let block_start = hadamard_row(i, stride, hadamard).checked_mul(n0);
        let Some(block) = block_start.and_then(|at| x.get(at..)).and_then(|b| b.get(..n0)) else {
            continue;
        };
        let mut values = block.iter();
        for_each_strided(tmp, i, stride, |out| {
            if let Some(&v) = values.next() {
                *out = v;
            }
        });
    }
    copy(x, tmp);
}

/// `haar1`: one level of the Haar transform of the `stride` interleaved signals of `n0` values
/// in `x`.
pub fn haar1(x: &mut [i16], n0: i32, stride: i32) {
    #[allow(clippy::cast_sign_loss, reason = "a band's size and its blocks are positive")]
    let (n0, stride) = (n0 as usize, stride as usize);
    // The whole pairs of values, `stride` signals of each.
    let (Some(len), Some(pair_len)) = ((n0 & !1).checked_mul(stride), stride.checked_mul(2)) else {
        return;
    };
    let Some(x) = x.get_mut(..len).filter(|_| stride > 0) else {
        return;
    };
    for pair in chunks_mut(x, pair_len) {
        let Some((a, b)) = pair.split_at_mut_checked(stride) else {
            return;
        };
        for (a, b) in a.iter_mut().zip(b) {
            let tmp1 = mult16_16(SQRT_HALF_Q15, i32::from(*a));
            let tmp2 = mult16_16(SQRT_HALF_Q15, i32::from(*b));
            *a = extract16(pshr32(add32(tmp1, tmp2), 15));
            *b = extract16(pshr32(sub32(tmp1, tmp2), 15));
        }
    }
}

/// `compute_qn`: the resolution of a split's angle, from the band's `b` bits.
fn compute_qn(n: i32, b: i32, offset: i32, pulse_cap: i32) -> i32 {
    static EXP2_TABLE8: [i32; 8] = [16384, 17866, 19483, 21247, 23170, 25267, 27554, 30048];
    // The C's `int` arithmetic, wrapping as it compiles to.
    let n2 = n.wrapping_mul(2).wrapping_sub(1);
    // `celt_sudiv`: the band has a value at least, so `n2` is at least 1.
    let qb = b
        .wrapping_add(n2.wrapping_mul(offset))
        .checked_div(n2)
        .unwrap_or(0)
        .min(b.wrapping_sub(pulse_cap).wrapping_sub(4 << BITRES))
        .min(8 << BITRES);
    #[allow(clippy::cast_sign_loss, reason = "masked to 0..=7")]
    let fraction = (qb & 0x7) as usize;
    // The mask keeps the fraction within the table.
    let exp2 = EXP2_TABLE8.get(fraction).copied().unwrap_or_default();
    if qb < (1 << BITRES >> 1) { 1 } else { (exp2 >> 14i32.wrapping_sub(qb >> BITRES)).wrapping_add(1) >> 1 << 1 }
}

/// The state the band decoding carries from band to band and down the splits (`struct
/// band_ctx`, the decoder's fields).
struct BandCtx<'a, 'b> {
    band: usize,
    spread: i32,
    tf_change: i32,
    dec: &'a mut EcDec<'b>,
    remaining_bits: i32,
    seed: u32,
    /// Scratch for the Hadamard reordering.
    hadamard: &'a mut [i16],
    /// Scratch for a codeword's pulses.
    pulses: &'a mut [i32],
}

/// A split's angle and what follows from it (`struct split_ctx`, mono).
struct Split {
    imid: i32,
    iside: i32,
    delta: i32,
    itheta: i32,
    qalloc: i32,
}

/// `compute_theta`, decoding mono (RFC 6716 §4.3.4.1): reads the split angle; gives the halves'
/// gains, the bit split and the bits the angle took.
fn compute_theta(ctx: &mut BandCtx, n: i32, b: &mut i32, big_b: i32, b0: i32, lm: i32, fill: &mut i32) -> Split {
    // The resolution to give the split parameter theta.
    let pulse_cap = LOG_N.get(ctx.band).map_or(0, |&log_n| i32::from(log_n)).wrapping_add(lm.wrapping_mul(1 << BITRES));
    let offset = (pulse_cap >> 1).wrapping_sub(QTHETA_OFFSET);
    let qn = compute_qn(n, *b, offset, pulse_cap);
    // The C converts between its `int`s and the range decoder's unsigned values as it reads
    // the angle; each conversion here is one of those. Its `int` arithmetic wraps, as it
    // compiles to.
    let tell = ctx.dec.tell_frac();
    let mut itheta: i32 = 0;
    if qn != 1 {
        // A uniform distribution for the time split, a triangular one for the rest.
        if b0 > 1 {
            itheta = ctx.dec.dec_uint(qn.wrapping_add(1).cast_unsigned()).cast_signed();
        } else {
            let half_qn = qn >> 1;
            let ft = half_qn.wrapping_add(1).wrapping_mul(half_qn.wrapping_add(1));
            let fm = ctx.dec.decode(ft.cast_unsigned()).cast_signed();
            let (fs, fl);
            if fm < (half_qn.wrapping_mul(half_qn.wrapping_add(1)) >> 1) {
                let root = isqrt32(fm.cast_unsigned().wrapping_mul(8).wrapping_add(1)).cast_signed();
                itheta = root.wrapping_sub(1) >> 1;
                fs = itheta.wrapping_add(1);
                fl = itheta.wrapping_mul(itheta.wrapping_add(1)) >> 1;
            } else {
                let root = isqrt32(ft.wrapping_sub(fm).wrapping_sub(1).cast_unsigned().wrapping_mul(8).wrapping_add(1))
                    .cast_signed();
                itheta = qn.wrapping_add(1).wrapping_mul(2).wrapping_sub(root) >> 1;
                // `qn + 1 - itheta`, and `qn + 2 - itheta` one more.
                fs = qn.wrapping_add(1).wrapping_sub(itheta);
                fl = ft.wrapping_sub(fs.wrapping_mul(fs.wrapping_add(1)) >> 1);
            }
            ctx.dec.update(fl.cast_unsigned(), fl.wrapping_add(fs).cast_unsigned(), ft.cast_unsigned());
        }
        // `celt_udiv`: past the angle of one value, `qn` is at least 2.
        itheta = NonZeroU32::new(qn.cast_unsigned())
            .map_or(0, |qn| (itheta.wrapping_mul(16384).cast_unsigned() / qn).cast_signed());
    }
    let qalloc = ctx.dec.tell_frac().wrapping_sub(tell);
    *b = b.wrapping_sub(qalloc);

    let block_mask = (1i32 << big_b).wrapping_sub(1);
    let (imid, iside, delta) = if itheta == 0 {
        *fill &= block_mask;
        (32767, 0, -16384)
    } else if itheta == 16384 {
        *fill &= block_mask << big_b;
        (0, 32767, 16384)
    } else {
        let imid = i32::from(bitexact_cos(itheta as i16));
        let iside = i32::from(bitexact_cos(16384i32.wrapping_sub(itheta) as i16));
        // The mid and side allocation that minimises the squared error in the band.
        (imid, iside, frac_mul16(n.wrapping_sub(1) << 7, bitexact_log2tan(iside, imid)))
    };
    Split { imid, iside, delta, itheta, qalloc }
}

/// `quant_band_n1`, mono: a band of one sample, its sign.
fn quant_band_n1(ctx: &mut BandCtx, x: &mut [i16], lowband_out: Option<&mut i16>) -> u32 {
    let mut sign: u32 = 0;
    if ctx.remaining_bits >= 1 << BITRES {
        sign = ctx.dec.dec_bits(1);
        ctx.remaining_bits = ctx.remaining_bits.wrapping_sub(1 << BITRES);
    }
    let value = if sign != 0 { (NORM_SCALING as i16).wrapping_neg() } else { NORM_SCALING as i16 };
    if let Some(first) = x.first_mut() {
        *first = value;
    }
    if let Some(out) = lowband_out {
        *out = value >> 4;
    }
    1
}

/// A partition of a band (`quant_partition`'s and `quant_band`'s arguments, as the C names
/// them).
#[derive(Clone, Copy)]
struct Partition {
    /// Its values (`N`).
    n: i32,
    /// Its bits, in 1/8 bits (`b`).
    bits: i32,
    /// Its interleaved blocks (`B`), a power of two.
    blocks: i32,
    /// Its LM, from -1 for a half of an LM 0 band.
    lm: i32,
    /// The norm it is decoded to.
    gain: i16,
    /// The blocks that may be folded into (`fill`).
    fill: i32,
}

/// `quant_partition`: a partition `part` into `x`, split in two (recursively, so a band can end
/// up split in 8 parts) while it has more bits than one codebook can use, folded from `lowband`
/// where it gets no pulse.
fn quant_partition(ctx: &mut BandCtx, x: &mut [i16], lowband: Option<&[i16]>, part: Partition) -> u32 {
    let Partition { mut n, bits: mut b, blocks: mut big_b, mut lm, gain, mut fill } = part;
    #[allow(clippy::cast_sign_loss, reason = "a partition's size is positive")]
    let len = n as usize;
    let b0 = big_b;
    let band = ctx.band;
    // If we need 1.5 more bit than we can produce, split the band in two.
    if lm != -1 && n > 2 && splits(band, lm, b) {
        n >>= 1;
        let half = len >> 1;
        let Some((x, y)) = x.split_at_mut_checked(half) else {
            return 0;
        };
        lm = lm.wrapping_sub(1);
        if big_b == 1 {
            fill = (fill & 1) | (fill << 1);
        }
        big_b = big_b.wrapping_add(1) >> 1;

        let split = compute_theta(ctx, n, &mut b, big_b, b0, lm, &mut fill);
        let mut delta = split.delta;
        let itheta = split.itheta;
        let mid = split.imid as i16;
        let side = split.iside as i16;

        // Give more bits to low-energy MDCTs than they would otherwise deserve.
        if b0 > 1 && (itheta & 0x3fff) != 0 {
            if itheta > 8192 {
                // Rough approximation for pre-echo masking.
                delta = delta.wrapping_sub(delta >> 4i32.wrapping_sub(lm));
            } else {
                // Corresponds to a forward-masking slope of 1.5 dB per 10 ms.
                delta = delta.wrapping_add(n << BITRES >> 5i32.wrapping_sub(lm)).min(0);
            }
        }
        let mut mbits = b.min(b.wrapping_sub(delta) / 2).max(0);
        let mut sbits = b.wrapping_sub(mbits);
        ctx.remaining_bits = ctx.remaining_bits.wrapping_sub(split.qalloc);

        let next_lowband = lowband.and_then(|l| l.get(half..));
        let half_part = |bits: i32, share: i32, fill: i32| Partition {
            n,
            bits,
            blocks: big_b,
            lm,
            gain: mult16_16_p15(i32::from(gain), share) as i16,
            fill,
        };
        let (mid_gain, side_gain) = (i32::from(mid), i32::from(side));

        let mut rebalance = ctx.remaining_bits;
        let mut cm;
        if mbits >= sbits {
            cm = quant_partition(ctx, x, lowband, half_part(mbits, mid_gain, fill));
            rebalance = mbits.wrapping_sub(rebalance.wrapping_sub(ctx.remaining_bits));
            if rebalance > 3 << BITRES && itheta != 0 {
                sbits = sbits.wrapping_add(rebalance.wrapping_sub(3 << BITRES));
            }
            cm |= quant_partition(ctx, y, next_lowband, half_part(sbits, side_gain, fill >> big_b)) << (b0 >> 1);
        } else {
            cm = quant_partition(ctx, y, next_lowband, half_part(sbits, side_gain, fill >> big_b)) << (b0 >> 1);
            rebalance = sbits.wrapping_sub(rebalance.wrapping_sub(ctx.remaining_bits));
            if rebalance > 3 << BITRES && itheta != 16384 {
                mbits = mbits.wrapping_add(rebalance.wrapping_sub(3 << BITRES));
            }
            cm |= quant_partition(ctx, x, lowband, half_part(mbits, mid_gain, fill));
        }
        return cm;
    }

    // The basic no-split case.
    let mut q = bits2pulses(band, lm, b);
    let mut curr_bits = pulses2bits(band, lm, q);
    ctx.remaining_bits = ctx.remaining_bits.wrapping_sub(curr_bits);
    // Never bust the budget.
    while ctx.remaining_bits < 0 && q > 0 {
        ctx.remaining_bits = ctx.remaining_bits.wrapping_add(curr_bits);
        q = q.wrapping_sub(1);
        curr_bits = pulses2bits(band, lm, q);
        ctx.remaining_bits = ctx.remaining_bits.wrapping_sub(curr_bits);
    }
    if q != 0 {
        return alg_unquant(
            x,
            Codeword { n, k: get_pulses(i32::from(q)) },
            ctx.spread,
            big_b,
            ctx.dec,
            gain,
            ctx.pulses,
        );
    }

    fill_without_pulses(ctx, x, lowband, len, big_b, fill, gain)
}

/// A partition that got no pulse, filled anyway: with noise, or folded from `lowband` with a
/// little noise, then normalised to `gain`; its collapse mask.
fn fill_without_pulses(
    ctx: &mut BandCtx,
    x: &mut [i16],
    lowband: Option<&[i16]>,
    len: usize,
    big_b: i32,
    mut fill: i32,
    gain: i16,
) -> u32 {
    let Some(x) = x.get_mut(..len) else {
        return 0;
    };
    // B can be as large as 16, so the C shifts in an unsigned long.
    let cm_mask: u32 = (1u64 << big_b).wrapping_sub(1) as u32;
    fill &= cm_mask.cast_signed();
    if fill == 0 {
        x.fill(0);
        return 0;
    }
    let cm = match lowband {
        None => {
            // Noise: the generator's top bits, signed.
            for v in x.iter_mut() {
                ctx.seed = celt_lcg_rand(ctx.seed);
                *v = (ctx.seed.cast_signed() >> 20) as i16;
            }
            cm_mask
        }
        Some(lowband) => {
            // The folded spectrum, with noise about 48 dB below the normal folding level.
            for (v, &l) in x.iter_mut().zip(lowband) {
                ctx.seed = celt_lcg_rand(ctx.seed);
                let noise = if ctx.seed & 0x8000 != 0 { FOLD_NOISE } else { -FOLD_NOISE };
                *v = add32(i32::from(l), i32::from(noise)) as i16;
            }
            // The C answers the `int` fill as its unsigned collapse mask.
            fill.cast_unsigned()
        }
    };
    renormalise_vector(x, gain);
    cm
}

/// The folding source of `quant_band`: the copy in `lowband_scratch` once one is made, else
/// `norm` at `lowband`.
fn lowband_slice<'s>(
    norm: &'s mut [i16],
    lowband_scratch: &'s mut Option<&mut [i16]>,
    lowband: Option<usize>,
    in_scratch: bool,
    n: usize,
) -> Option<&'s mut [i16]> {
    let offset = lowband?;
    if in_scratch {
        lowband_scratch.as_deref_mut().and_then(|s| s.get_mut(..n))
    } else {
        norm.get_mut(offset..).and_then(|s| s.get_mut(..n))
    }
}

/// Where `quant_band` folds from and to: offsets into `norm`, which may overlap (the folding
/// source is read before the folding output is written), and the scratch for a copy of the
/// source, where there is room.
struct Folding<'n, 's> {
    norm: &'n mut [i16],
    lowband: Option<usize>,
    lowband_out: Option<usize>,
    scratch: Option<&'s mut [i16]>,
}

/// `quant_band`, mono: a band `part` into `x`, folding as `fold` says.
fn quant_band(ctx: &mut BandCtx, x: &mut [i16], part: Partition, fold: Folding) -> u32 {
    static BIT_INTERLEAVE_TABLE: [i32; 16] = [0, 1, 1, 1, 2, 3, 3, 3, 2, 3, 3, 3, 2, 3, 3, 3];
    static BIT_DEINTERLEAVE_TABLE: [u32; 16] =
        [0x00, 0x03, 0x0C, 0x0F, 0x30, 0x33, 0x3C, 0x3F, 0xC0, 0xC3, 0xCC, 0xCF, 0xF0, 0xF3, 0xFC, 0xFF];
    let Partition { n, blocks: mut big_b, mut fill, .. } = part;
    let Folding { norm, lowband, lowband_out, scratch: mut lowband_scratch } = fold;
    let n0 = n;
    #[allow(clippy::cast_sign_loss, reason = "a band's size is positive")]
    let nu = n as usize;
    let mut b0 = big_b;
    let mut time_divide: i32 = 0;
    let mut tf_change = ctx.tf_change;
    let long_blocks = b0 == 1;
    debug_assert!(big_b > 0 && big_b.count_ones() == 1, "the blocks are a power of two");
    let mut n_b = n >> big_b.trailing_zeros();

    // A band of one sample.
    if n == 1 {
        return quant_band_n1(ctx, x, lowband_out.and_then(|o| norm.get_mut(o)));
    }

    let recombine = tf_change.max(0);
    // Band recombining to increase frequency resolution.
    let mut in_scratch = false;
    if let Some(scratch) = lowband_scratch.as_deref_mut()
        && let Some(offset) = lowband
        && (recombine != 0 || ((n_b & 1) == 0 && tf_change < 0) || b0 > 1)
    {
        for (copy, &v) in scratch.iter_mut().zip(norm.iter().skip(offset)).take(nu) {
            *copy = v;
        }
        in_scratch = true;
    }

    // The fill's two nibbles index the table: the C's `fill&0xF` and `fill>>4`. The mask keeps
    // each within it.
    let interleave = |fill: i32, shift: i32| {
        BIT_INTERLEAVE_TABLE.get(((fill >> shift).cast_unsigned() & 0xF) as usize).copied().unwrap_or_default()
    };
    for k in 0..recombine {
        if let Some(lb) = lowband_slice(norm, &mut lowband_scratch, lowband, in_scratch, nu) {
            haar1(lb, n >> k, 1 << k);
        }
        fill = interleave(fill, 0) | interleave(fill, 4) << 2;
    }
    big_b >>= recombine;
    n_b <<= recombine;

    // Increasing the time resolution.
    while (n_b & 1) == 0 && tf_change < 0 {
        if let Some(lb) = lowband_slice(norm, &mut lowband_scratch, lowband, in_scratch, nu) {
            haar1(lb, n_b, big_b);
        }
        fill |= fill << big_b;
        big_b <<= 1;
        n_b >>= 1;
        time_divide = time_divide.wrapping_add(1);
        tf_change = tf_change.wrapping_add(1);
    }
    b0 = big_b;
    let n_b0 = n_b;

    // Reorganise the samples in time order instead of frequency order.
    #[allow(clippy::cast_sign_loss, reason = "a band's blocks and their size are positive")]
    let (hadamard_len, hadamard_stride) = ((n_b >> recombine) as usize, (b0 << recombine) as usize);
    if b0 > 1
        && let Some(lb) = lowband_slice(norm, &mut lowband_scratch, lowband, in_scratch, nu)
    {
        deinterleave_hadamard(lb, ctx.hadamard, hadamard_len, hadamard_stride, long_blocks);
    }

    let mut cm = {
        let lb = lowband_slice(norm, &mut lowband_scratch, lowband, in_scratch, nu);
        quant_partition(ctx, x, lb.as_deref(), Partition { blocks: big_b, fill, ..part })
    };

    // Undo the sample reorganisation, from time order back to frequency order.
    if b0 > 1 {
        interleave_hadamard(x, ctx.hadamard, hadamard_len, hadamard_stride, long_blocks);
    }

    // Undo the time-frequency changes made above.
    n_b = n_b0;
    big_b = b0;
    for _ in 0..time_divide {
        big_b >>= 1;
        n_b <<= 1;
        cm |= cm >> big_b;
        haar1(x, n_b, big_b);
    }
    for k in 0..recombine {
        // The mask of at most four blocks, as the C's table assumes; the mask keeps it within it.
        cm = BIT_DEINTERLEAVE_TABLE.get((cm & 0xF) as usize).copied().unwrap_or_default();
        haar1(x, n0 >> k, 1 << k);
    }
    big_b <<= recombine;

    // Scale the output for later folding.
    if let Some(offset) = lowband_out {
        let scale = i32::from(celt_sqrt(shl32(n0, 22)) as i16);
        for (out, &v) in norm.iter_mut().skip(offset).zip(x.iter()).take(nu) {
            *out = mult16_16_q15(scale, i32::from(v)) as i16;
        }
    }
    cm & (1u32 << big_b).wrapping_sub(1)
}

/// `special_hybrid_folding`, mono: duplicates enough of the first band's folding data to fold
/// the second (nothing for a start of 0, where both are one bin a block).
fn special_hybrid_folding(norm: &mut [i16], start: usize, m: usize) {
    let Some(&[first, second, ..]) = BAND_WIDTHS.get(start..) else {
        return;
    };
    let n1 = m.saturating_mul(usize::from(first));
    let n2 = m.saturating_mul(usize::from(second));
    // The last `n2 - n1` of the first band's `n1` values, copied after them.
    if let Some(from) = n1.checked_mul(2).and_then(|twice| twice.checked_sub(n2))
        && let Some(source_len) = n1.checked_sub(from)
        && let Some((source, copy)) = norm.get_mut(from..n2).and_then(|region| region.split_at_mut_checked(source_len))
    {
        for (to, &value) in copy.iter_mut().zip(source.iter()) {
            *to = value;
        }
    }
}

/// The decoder's inputs to the band shapes of a frame.
pub struct BandsFrame<'a> {
    /// The first band decoded.
    pub start: usize,
    /// One past the last band decoded.
    pub end: usize,
    /// Each band's shape bits, in 1/8 bits.
    pub pulses: &'a [i32; NB_BANDS],
    /// Whether the frame is a transient, of short blocks.
    pub transient: bool,
    /// The spreading of the pulses (RFC 6716 §4.3.4.3).
    pub spread: i32,
    /// Each band's time-frequency change.
    pub tf_res: &'a [i8; NB_BANDS],
    /// The frame's bits, in 1/8 bits.
    pub total_bits: i32,
    /// The bits left over the bands' caps, in 1/8 bits.
    pub balance: i32,
    /// The frame's LM.
    pub lm: i32,
    /// The bands coded; those above were skipped.
    pub coded_bands: usize,
}

/// The scratch the band shapes use.
pub struct BandsScratch<'a> {
    /// The folding source, `8 * 78` values at least: every band but the last.
    pub norm: &'a mut [i16],
    /// The Hadamard reordering's scratch, `MAX_BAND` values at least.
    pub hadamard: &'a mut [i16],
    /// A codeword's pulses, `MAX_BAND` values at least.
    pub pulses: &'a mut [i32],
}

/// Where a band folds from in `norm` (never repeating spectral content within one band), and a
/// conservative estimate of the collapse masks of the bands it folds from.
fn fold_source(
    collapse_masks: &[u8; NB_BANDS],
    bin: &impl Fn(usize) -> usize,
    lowband_offset: usize,
    band: usize,
    norm_offset: usize,
    bins: usize,
) -> (usize, u32) {
    let lowband = bin(lowband_offset).saturating_sub(norm_offset.saturating_add(bins));
    let (source_low, source_high) =
        (lowband.saturating_add(norm_offset), lowband.saturating_add(norm_offset).saturating_add(bins));
    // The last band below `lowband_offset` that starts at or below the source (band 0 always
    // does), and the first from `lowband_offset` that reaches past it, or `band`.
    let fold_start = (0..lowband_offset).rev().find(|&b| bin(b) <= source_low).unwrap_or(0);
    let fold_end = (lowband_offset..band).find(|&b| bin(b) >= source_high).unwrap_or(band.max(lowband_offset));
    let mask = collapse_masks
        .get(fold_start..fold_end.max(fold_start.saturating_add(1)))
        .unwrap_or_default()
        .iter()
        .fold(0u32, |mask, &c| mask | u32::from(c));
    (lowband, mask)
}

/// `quant_all_bands`, decoding mono (RFC 6716 §4.3.4): the shapes of the bands of `frame` into
/// `x` (`120 << lm` values), each band's collapse mask into `collapse_masks`, advancing the
/// folding noise's `seed`.
pub fn quant_all_bands(
    frame: &BandsFrame,
    x: &mut [i16],
    collapse_masks: &mut [u8; NB_BANDS],
    dec: &mut EcDec,
    seed: &mut u32,
    scratch: BandsScratch,
) {
    let BandsFrame { start, end, pulses, transient, spread, tf_res, total_bits, mut balance, lm, coded_bands } = *frame;
    let m = 1usize << lm;
    let big_b: i32 = if transient { 1 << lm } else { 1 };
    let bin = |band: usize| EBANDS.get(band).map_or(0, |&edge| usize::from(edge) << lm);
    // The mode's bands at most, so that each band's entries of the arrays are in reach.
    let end = end.min(NB_BANDS);
    let norm_offset = bin(start);
    // The last band needs no folding output, so `norm` stops short of it; and until the last
    // band is decoded, its place in `x` serves as the folding source's scratch.
    let scratch_offset = bin(NB_BANDS - 1);
    let mut update_lowband = true;
    let mut lowband_offset: usize = 0;
    let mut ctx = BandCtx {
        band: 0,
        spread,
        tf_change: 0,
        dec,
        remaining_bits: 0,
        seed: *seed,
        hadamard: scratch.hadamard,
        pulses: scratch.pulses,
    };
    let norm = scratch.norm;
    // Band counts are at most the mode's 21: their sums never saturate.
    for (band, (&band_pulses, &tf_res)) in pulses.iter().zip(tf_res).enumerate().take(end).skip(start) {
        ctx.band = band;
        let next_band = band.saturating_add(1);
        let last = next_band == end;
        let band_low = bin(band);
        let band_high = bin(next_band);
        let Some((head, tail)) = x.split_at_mut_checked(scratch_offset) else {
            break;
        };
        let x_band = if band_high <= scratch_offset {
            head.get_mut(band_low..band_high).map(|x_band| (x_band, Some(tail)))
        } else {
            tail.get_mut(band_low.saturating_sub(scratch_offset)..band_high.saturating_sub(scratch_offset))
                .map(|x_band| (x_band, None))
        };
        let Some((x_band, mut lowband_scratch)) = x_band else {
            break;
        };
        // The band's size, as bins and as the C's `int`.
        let bins = band_high.saturating_sub(band_low);
        let n = band_width(band) << lm;
        let tell = ctx.dec.tell_frac();

        // The bits this band wants.
        // The C's `int` arithmetic, wrapping as it compiles to.
        if band != start {
            balance = balance.wrapping_sub(tell);
        }
        let remaining_bits = total_bits.wrapping_sub(tell).wrapping_sub(1);
        ctx.remaining_bits = remaining_bits;
        let b = if band < coded_bands {
            // Shared over the next three bands, or those left.
            let shared_over = i32::try_from(coded_bands.saturating_sub(band).min(3)).unwrap_or(3);
            let curr_balance = balance.checked_div(shared_over).unwrap_or(0);
            remaining_bits.wrapping_add(1).min(band_pulses.wrapping_add(curr_balance)).clamp(0, 16383)
        } else {
            0
        };

        let second = band == start.saturating_add(1);
        if (band_low >= bin(start).saturating_add(bins) || second) && (update_lowband || lowband_offset == 0) {
            lowband_offset = band;
        }
        if second {
            special_hybrid_folding(norm, start, m);
        }

        let tf_change = i32::from(tf_res);
        ctx.tf_change = tf_change;
        if last {
            lowband_scratch = None;
        }

        // A conservative estimate of the collapse masks of the bands folded from.
        let mut effective_lowband = None;
        let fold_mask = if lowband_offset != 0 && (spread != SPREAD_AGGRESSIVE || big_b > 1 || tf_change < 0) {
            let (lowband, mask) = fold_source(collapse_masks, &bin, lowband_offset, band, norm_offset, bins);
            effective_lowband = Some(lowband);
            mask
        } else {
            // The noise fills every block (almost always).
            (1u32 << big_b).wrapping_sub(1)
        };

        let fold = Folding {
            norm: &mut *norm,
            lowband: effective_lowband,
            lowband_out: (!last).then_some(band_low.saturating_sub(norm_offset)),
            scratch: lowband_scratch,
        };
        let part = Partition { n, bits: b, blocks: big_b, lm, gain: Q15ONE, fill: fold_mask.cast_signed() };
        let mask = quant_band(&mut ctx, x_band, part, fold);
        if let Some(collapse_mask) = collapse_masks.get_mut(band) {
            *collapse_mask = mask as u8;
        }
        balance = balance.wrapping_add(band_pulses.wrapping_add(tell));

        // Update the folding position only as long as we have 1 bit/sample depth.
        update_lowband = b > (n << BITRES);
    }
    *seed = ctx.seed;
}

/// The band log energies the anti-collapse compares: this frame's, the last frame's and the
/// one before.
#[derive(Clone, Copy)]
pub struct EnergyHistory<'a> {
    /// This frame's.
    pub current: &'a [i16; NB_BANDS],
    /// The last frame's.
    pub previous: &'a [i16; NB_BANDS],
    /// The frame's before that.
    pub before_previous: &'a [i16; NB_BANDS],
}

/// `anti_collapse`, mono (RFC 6716 §4.3.5): noise in the blocks of a transient's bands that
/// received no pulse (those clear in `collapse_masks`), at a level from the band's depth and
/// its energy's rise over the last two frames.
pub fn anti_collapse(
    x: &mut [i16],
    collapse_masks: &[u8; NB_BANDS],
    lm: i32,
    bands: Range<usize>,
    energies: EnergyHistory,
    pulses: &[i32; NB_BANDS],
    mut seed: u32,
) {
    let EnergyHistory { current: log_energy, previous: prev1, before_previous: prev2 } = energies;
    let band_terms = pulses
        .iter()
        .zip(collapse_masks)
        .zip(log_energy.iter().zip(prev1.iter().zip(prev2)))
        .zip(EBANDS.iter().zip(&BAND_WIDTHS))
        .take(bands.end)
        .skip(bands.start);
    for (((&band_pulses, &collapse_mask), (&energy, (&prev1, &prev2))), (&edge, &width)) in band_terms {
        let n0 = i32::from(width);
        // The depth, in 1/8 bits, divided unsigned as the C does; its `int` arithmetic wraps.
        let depth = NonZeroU32::new(n0.cast_unsigned())
            .map_or(0, |n0| ((band_pulses.wrapping_add(1).cast_unsigned() / n0) >> lm).cast_signed());
        let thresh32 = shr32(celt_exp2(i32::from(shl16(depth, 10 - BITRES)).wrapping_neg() as i16), 1);
        let thresh = mult16_32_q15(HALF_Q15, min32(32767, thresh32)) as i16;
        let t = n0 << lm;
        // A band is at most 176 bins, so the shift is at most 3 and both shifts below are
        // positive.
        let shift = (i32::from(celt_ilog2(t)) >> 1).cast_unsigned();
        let sqrt_1 = celt_rsqrt_norm(shl32(t, 7u32.wrapping_sub(shift) << 1));

        let ediff = i32::from(energy).wrapping_sub(i32::from(prev1.min(prev2))).max(0);
        let mut r: i16 = if ediff < 16384 {
            min32(16383, shr32(celt_exp2(i32::from(extract16(ediff)).wrapping_neg() as i16), 1)).wrapping_mul(2) as i16
        } else {
            0
        };
        if lm == 3 {
            r = mult16_16_q14(23170, min32(23169, i32::from(r))) as i16;
        }
        r = thresh.min(r) >> 1;
        r = shr32(mult16_16_q15(i32::from(sqrt_1), i32::from(r)), shift) as i16;

        let blocks = 1usize << lm;
        let Some(x_band) = x.get_mut(usize::from(edge) << lm..).and_then(|x| x.get_mut(..usize::from(width) << lm))
        else {
            continue;
        };
        let mut renormalise = false;
        for k in 0..blocks {
            // A block with no pulse has collapsed: fill it with noise.
            if u32::from(collapse_mask) & (1 << k) == 0 {
                for_each_strided(x_band, k, blocks, |v| {
                    seed = celt_lcg_rand(seed);
                    *v = if seed & 0x8000 != 0 { r } else { r.wrapping_neg() };
                });
                renormalise = true;
            }
        }
        // The noise added energy: renormalise.
        if renormalise {
            renormalise_vector(x_band, Q15ONE);
        }
    }
}

/// The largest band in `x` the shape decoder writes, as a check on `MAX_BAND`.
const _: () = assert!(MAX_BAND >= 8 * 22);
