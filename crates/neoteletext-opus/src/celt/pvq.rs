//! The pyramid vector quantiser (RFC 6716 §4.3.4): a band's codeword decoded to its pulses
//! (§4.3.4.2), scaled to unit norm, and spread by the rotation of §4.3.4.3; with the
//! renormalisation the folding and anti-collapse share.

use crate::celt::tables::{PVQ_U, PVQ_U_ROW};
use crate::fixed::{
    EPSILON, Q15ONE, add32, extract16, half16, mac16_16, mult16_16, mult16_16_p15, mult16_16_q15, pshr32, sub16, vshr32,
};
use crate::math::{celt_cos_norm, celt_div, celt_ilog2, celt_rsqrt_norm};
use crate::range::EcDec;
use crate::slices::{chunks, chunks_mut};

/// The spreading of the pulses (RFC 6716 §4.3.4.3), as the frame signals it.
pub const SPREAD_NONE: i32 = 0;
/// The most aggressive spreading, which needs no folding mask (`SPREAD_AGGRESSIVE`).
pub const SPREAD_AGGRESSIVE: i32 = 3;

/// The sum of the products of `x` and `y`, over the shorter (`celt_inner_prod`), wrapping as
/// 32-bit sums do.
#[inline]
#[must_use]
pub fn inner_product(x: &[i16], y: &[i16]) -> i32 {
    x.iter().zip(y).fold(0i32, |sum, (&a, &b)| mac16_16(sum, i32::from(a), i32::from(b)))
}

/// `CELT_PVQ_U_ROW[row][column]`, or 0 outside the table.
#[inline]
fn pvq_u(row: i32, column: i32) -> u32 {
    let (Ok(row), Ok(column)) = (usize::try_from(row), usize::try_from(column)) else {
        return 0;
    };
    PVQ_U_ROW.get(row).and_then(|&first| PVQ_U.get(usize::from(first)..)?.get(column)).copied().unwrap_or(0)
}

/// `CELT_PVQ_V(n, k)`: U(N,K) + U(N,K+1), the codewords of `k` pulses over `n` dimensions.
#[inline]
fn pvq_v(n: i32, k: i32) -> u32 {
    let k1 = k.wrapping_add(1);
    pvq_u(n.min(k), n.max(k)).wrapping_add(pvq_u(n.min(k1), n.max(k1)))
}

/// `cwrsi`: the pulses `y` (`n` of them, `n > 1`) of codeword `i` of `k > 0` pulses; answers
/// their squared norm. The C's signs are masks of all ones (`s`), applied to the unsigned
/// codeword as their conversions. The C's `int` arithmetic wraps, as it compiles to.
#[allow(clippy::many_single_char_names, reason = "the names of RFC 6716 §4.3.4.2: n, k, i, p, q, s, y")]
fn cwrsi(mut n: i32, mut k: i32, mut i: u32, y: &mut [i32]) -> i32 {
    let mut yy: i32 = 0;
    let mut out = y.iter_mut();
    let mut put = |val: i16, yy: &mut i32| {
        if let Some(slot) = out.next() {
            *slot = i32::from(val);
        }
        *yy = mac16_16(*yy, i32::from(val), i32::from(val));
    };
    while n > 2 {
        if k >= n {
            // Lots of pulses: are the pulses in this dimension negative?
            let mut p = pvq_u(n, k.wrapping_add(1));
            let s = i32::from(i >= p).wrapping_neg();
            i = i.wrapping_sub(p & s.cast_unsigned());
            // Count how many pulses were placed in this dimension.
            let k0 = k;
            let q = pvq_u(n, n);
            if q > i {
                k = n;
                loop {
                    k = k.wrapping_sub(1);
                    p = pvq_u(k, n);
                    if p <= i {
                        break;
                    }
                }
            } else {
                p = pvq_u(n, k);
                while p > i {
                    k = k.wrapping_sub(1);
                    p = pvq_u(n, k);
                }
            }
            i = i.wrapping_sub(p);
            put((k0.wrapping_sub(k).wrapping_add(s) ^ s) as i16, &mut yy);
        } else {
            // Lots of dimensions: are there any pulses in this dimension at all?
            let p = pvq_u(k, n);
            let q = pvq_u(k.wrapping_add(1), n);
            if p <= i && i < q {
                i = i.wrapping_sub(p);
                put(0, &mut yy);
            } else {
                // Are the pulses in this dimension negative?
                let s = i32::from(i >= q).wrapping_neg();
                i = i.wrapping_sub(q & s.cast_unsigned());
                // Count how many pulses were placed in this dimension.
                let k0 = k;
                let mut p;
                loop {
                    k = k.wrapping_sub(1);
                    p = pvq_u(k, n);
                    if p <= i {
                        break;
                    }
                }
                i = i.wrapping_sub(p);
                put((k0.wrapping_sub(k).wrapping_add(s) ^ s) as i16, &mut yy);
            }
        }
        n = n.wrapping_sub(1);
    }
    // n == 2
    let p = k.wrapping_mul(2).wrapping_add(1).cast_unsigned();
    let s = i32::from(i >= p).wrapping_neg();
    i = i.wrapping_sub(p & s.cast_unsigned());
    let k0 = k;
    k = (i.wrapping_add(1) >> 1).cast_signed();
    if k != 0 {
        i = i.wrapping_sub(k.wrapping_mul(2).wrapping_sub(1).cast_unsigned());
    }
    put((k0.wrapping_sub(k).wrapping_add(s) ^ s) as i16, &mut yy);
    // n == 1
    let s = i.cast_signed().wrapping_neg();
    put((k.wrapping_add(s) ^ s) as i16, &mut yy);
    yy
}

/// `decode_pulses`: the codeword of `k` pulses over `n` dimensions, read into `y`; answers its
/// squared norm.
pub fn decode_pulses(y: &mut [i32], n: i32, k: i32, dec: &mut EcDec) -> i32 {
    cwrsi(n, k, dec.dec_uint(pvq_v(n, k)), y)
}

/// `exp_rotation1`: one pass of the rotation over `x`, `stride` apart.
fn exp_rotation1(x: &mut [i16], stride: usize, c: i16, s: i16) {
    let (c, s) = (i32::from(c), i32::from(s));
    let ms = i32::from((s as i16).wrapping_neg());
    // The pair `stride` apart from `p`; every `p` the passes take has one.
    let rotate = |x: &mut [i16], p: usize| {
        if let Some([first, .., last]) = x.get_mut(p..).and_then(|rest| rest.get_mut(..=stride)) {
            let x1 = i32::from(*first);
            let x2 = i32::from(*last);
            *last = extract16(pshr32(mac16_16(mult16_16(c, x2), s, x1), 15));
            *first = extract16(pshr32(mac16_16(mult16_16(c, x1), ms, x2), 15));
        }
    };
    let len = x.len();
    for p in 0..len.saturating_sub(stride) {
        rotate(x, p);
    }
    for p in (0..len.saturating_sub(stride.saturating_mul(2))).rev() {
        rotate(x, p);
    }
}

/// `exp_rotation` (RFC 6716 §4.3.4.3): spreads (`dir` 1) or gathers (`dir` -1) the energy of
/// `k` pulses over the `len` values of `x`, in `stride` interleaved blocks (a power of two).
pub fn exp_rotation(x: &mut [i16], len: i32, dir: i32, stride: i32, k: i32, spread: i32) {
    static SPREAD_FACTOR: [i32; 3] = [15, 10, 5];
    if k.wrapping_mul(2) >= len || spread == SPREAD_NONE {
        return;
    }
    // The factor of each spreading but `SPREAD_NONE`, the only ones the frame can signal.
    let Some(&factor) = usize::try_from(spread.wrapping_sub(1)).ok().and_then(|i| SPREAD_FACTOR.get(i)) else {
        return;
    };
    // A band of no blocks has nothing to rotate.
    let (Ok(len_usize), Some(stride_usize)) =
        (usize::try_from(len), usize::try_from(stride).ok().filter(|&stride| stride > 0))
    else {
        return;
    };
    let gain = celt_div(mult16_16(i32::from(Q15ONE), len), len.wrapping_add(factor.wrapping_mul(k))) as i16;
    let theta = half16(mult16_16_q15(i32::from(gain), i32::from(gain))) as i16;
    let c = celt_cos_norm(i32::from(theta));
    let s = celt_cos_norm(sub16(i32::from(Q15ONE), i32::from(theta))); // sin(theta)

    let (len, stride) = (len_usize, stride_usize);
    let stride2: usize = if len >= stride.saturating_mul(8) {
        // sqrt(len/stride) with rounding: the least stride2 with (stride2+0.5)^2 >= len/stride,
        // which is at most len.
        let reaches =
            |s: usize| s.saturating_mul(s).saturating_add(s).saturating_mul(stride).saturating_add(stride >> 2) >= len;
        (1..len).find(|&s| reaches(s)).unwrap_or(len)
    } else {
        0
    };
    debug_assert!(stride.is_power_of_two(), "the blocks are a power of two");
    let block_len = len >> stride.trailing_zeros();
    let Some(x) = x.get_mut(..len).filter(|_| block_len > 0) else {
        return;
    };
    for block in chunks_mut(x, block_len) {
        if dir < 0 {
            if stride2 != 0 {
                exp_rotation1(block, stride2, s, c);
            }
            exp_rotation1(block, 1, c, s);
        } else {
            exp_rotation1(block, 1, c, s.wrapping_neg());
            if stride2 != 0 {
                exp_rotation1(block, stride2, s, c.wrapping_neg());
            }
        }
    }
}

/// `normalise_residual`: the pulses `iy`, of squared norm `ryy`, scaled to `gain` into `x`.
fn normalise_residual(iy: &[i32], x: &mut [i16], ryy: i32, gain: i16) {
    let (scale, shift) = unit_scale(ryy, gain);
    for (v, &pulse) in x.iter_mut().zip(iy) {
        *v = extract16(pshr32(mult16_16(scale, pulse), shift));
    }
}

/// The scale and shift that bring a vector of squared norm `energy` (positive) to the norm
/// `gain`: `gain / sqrt(energy)` as a Q15 scale and a rounding shift right.
fn unit_scale(energy: i32, gain: i16) -> (i32, u32) {
    // The energy is positive, so its half log is at least 0.
    let half_log = i32::from(celt_ilog2(energy)) >> 1;
    let normalised = vshr32(energy, half_log.wrapping_sub(7).wrapping_mul(2));
    let scale = i32::from(mult16_16_p15(i32::from(celt_rsqrt_norm(normalised)), i32::from(gain)) as i16);
    (scale, half_log.wrapping_add(1).cast_unsigned())
}

/// `extract_collapse_mask`: which of the `b` blocks (a power of two) of `iy` hold a pulse.
fn extract_collapse_mask(iy: &[i32], b: i32) -> u32 {
    if b <= 1 {
        return 1;
    }
    let block_len = iy.len() >> b.trailing_zeros();
    if block_len == 0 {
        // More blocks than values: none a codeword can fill.
        return 0;
    }
    chunks(iy, block_len)
        .zip(0..u32::BITS)
        .fold(0, |mask, (block, i)| mask | u32::from(block.iter().any(|&v| v != 0)) << i)
}

/// A band's codeword (RFC 6716 §4.3.4.2): its `k` pulses over `n` dimensions.
#[derive(Clone, Copy)]
pub struct Codeword {
    /// The dimensions.
    pub n: i32,
    /// The pulses.
    pub k: i32,
}

/// `alg_unquant` (RFC 6716 §4.3.4): the band `x` from its `codeword`, at norm `gain`, spread
/// over `b` blocks; answers which blocks hold a pulse. `pulses` is scratch of `n` values at
/// least.
pub fn alg_unquant(
    x: &mut [i16],
    codeword: Codeword,
    spread: i32,
    b: i32,
    dec: &mut EcDec,
    gain: i16,
    pulses: &mut [i32],
) -> u32 {
    let Codeword { n, k } = codeword;
    debug_assert!(k > 0 && n > 1, "a codeword needs a pulse and two dimensions");
    #[allow(clippy::cast_sign_loss, reason = "a codeword has two dimensions or more")]
    let n_usize = n as usize;
    let (Some(iy), Some(band)) = (pulses.get_mut(..n_usize), x.get_mut(..n_usize)) else {
        return 0;
    };
    let ryy = decode_pulses(iy, n, k, dec);
    normalise_residual(iy, band, ryy, gain);
    exp_rotation(x, n, -1, b, k, spread);
    extract_collapse_mask(iy, b)
}

/// `renormalise_vector`: `x` scaled to the norm `gain`.
pub fn renormalise_vector(x: &mut [i16], gain: i16) {
    let (scale, shift) = unit_scale(add32(EPSILON, inner_product(x, x)), gain);
    for v in x {
        *v = extract16(pshr32(mult16_16(scale, i32::from(*v)), shift));
    }
}
