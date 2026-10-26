//! The reference's fixed-point mathematical approximations (`celt/mathops.h` and
//! `celt/mathops.c`, built `FIXED_POINT`). Each C assignment to an `opus_val16` truncates, and
//! is written so.
//!
//! Ported from libopus 1.5.2: Copyright (c) 2002-2008 Jean-Marc Valin, 2007-2008 CSIRO,
//! 2007-2009 Xiph.Org Foundation, BSD-3-Clause (see `LICENSE-libopus`).

use crate::fixed::{
    add16, add32, ec_ilog, extract16, mult16_16_p15, mult16_16_q15, mult16_32_q15, mult32_32_q31, pshr32, round16,
    shl16, shl32, sub16, sub32, vshr32,
};

/// `isqrt32`: the integer square root, rounded down.
#[must_use]
pub fn isqrt32(mut val: u32) -> u32 {
    let mut g: u32 = 0;
    let mut bshift: i32 = ec_ilog(val).wrapping_sub(1) >> 1;
    let mut b: u32 = 1u32 << bshift;
    loop {
        let t: u32 = ((g << 1).wrapping_add(b)) << bshift;
        if t <= val {
            g = g.wrapping_add(b);
            val = val.wrapping_sub(t);
        }
        b >>= 1;
        bshift = bshift.wrapping_sub(1);
        if bshift < 0 {
            break;
        }
    }
    g
}

/// `celt_ilog2`: undefined for zero and negative numbers, as in the C.
#[inline]
#[must_use]
pub fn celt_ilog2(x: i32) -> i16 {
    ec_ilog(x.cast_unsigned()).wrapping_sub(1) as i16
}

/// `celt_zlog2`: zero for zero.
#[inline]
#[must_use]
pub fn celt_zlog2(x: i32) -> i16 {
    if x <= 0 { 0 } else { celt_ilog2(x) }
}

/// `celt_maxabs16`.
#[must_use]
pub fn celt_maxabs16(x: &[i16]) -> i32 {
    let mut maxval: i16 = 0;
    let mut minval: i16 = 0;
    for &v in x {
        maxval = maxval.max(v);
        minval = minval.min(v);
    }
    i32::from(maxval).max(i32::from(minval).wrapping_neg())
}

/// `celt_maxabs32`.
#[must_use]
pub fn celt_maxabs32(x: &[i32]) -> i32 {
    let mut maxval: i32 = 0;
    let mut minval: i32 = 0;
    for &v in x {
        maxval = maxval.max(v);
        minval = minval.min(v);
    }
    maxval.max(minval.wrapping_neg())
}

/// `celt_exp2_frac`.
#[inline]
#[must_use]
pub fn celt_exp2_frac(x: i16) -> i32 {
    const D0: i32 = 16383;
    const D1: i32 = 22804;
    const D2: i32 = 14819;
    const D3: i32 = 10204;
    let frac = i32::from(shl16(i32::from(x), 4));
    let t2 = i32::from(add16(D2, mult16_16_q15(D3, frac)));
    let t1 = i32::from(add16(D1, mult16_16_q15(frac, t2)));
    i32::from(add16(D0, mult16_16_q15(frac, t1)))
}

/// `celt_exp2`: Q10 in, Q16 out.
#[must_use]
pub fn celt_exp2(x: i16) -> i32 {
    let integer: i32 = i32::from(x) >> 10;
    if integer > 14 {
        return 0x7f00_0000;
    } else if integer < -15 {
        return 0;
    }
    let frac: i16 = celt_exp2_frac(i32::from(x).wrapping_sub(i32::from(shl16(integer, 10))) as i16) as i16;
    vshr32(i32::from(frac), integer.wrapping_neg().wrapping_sub(2))
}

/// `celt_rcp`: the reciprocal, for `x > 0`.
#[must_use]
pub fn celt_rcp(x: i32) -> i32 {
    let i = i32::from(celt_ilog2(x));
    let n: i32 = i32::from(vshr32(x, i.wrapping_sub(15)).wrapping_sub(32768) as i16);
    let r: i32 = i32::from(add16(30840, mult16_16_q15(-15420, n)));
    let r: i32 = i32::from(sub16(
        r,
        mult16_16_q15(r, i32::from(add16(mult16_16_q15(r, n), i32::from(add16(r, -32768))))),
    ) as i16);
    let r: i32 = i32::from(sub16(
        r,
        i32::from(add16(1, mult16_16_q15(r, i32::from(add16(mult16_16_q15(r, n), i32::from(add16(r, -32768))))))),
    ) as i16);
    vshr32(r, i.wrapping_sub(16))
}

/// `celt_div`.
#[inline]
#[must_use]
pub fn celt_div(a: i32, b: i32) -> i32 {
    mult32_32_q31(a, celt_rcp(b))
}

/// `frac_div32`.
#[must_use]
pub fn frac_div32(a: i32, b: i32) -> i32 {
    let shift = i32::from(celt_ilog2(b)).wrapping_sub(29);
    let a = vshr32(a, shift);
    let b = vshr32(b, shift);
    let rcp: i32 = i32::from(round16(celt_rcp(i32::from(round16(b, 16))), 3));
    let mut result: i32 = mult16_32_q15(rcp, a);
    let rem: i32 = sub32(pshr32(a, 2), mult32_32_q31(result, b));
    result = add32(result, shl32(mult16_32_q15(rcp, rem), 2));
    if result >= 536_870_912 {
        2_147_483_647
    } else if result <= -536_870_912 {
        -2_147_483_647
    } else {
        shl32(result, 2)
    }
}

/// `celt_rsqrt_norm`: the Q14 reciprocal square root of a Q16 `x` in [0.25, 1).
#[must_use]
pub fn celt_rsqrt_norm(x: i32) -> i16 {
    let n: i32 = i32::from(x.wrapping_sub(32768) as i16);
    let r: i32 = i32::from(add16(23557, mult16_16_q15(n, i32::from(add16(-13490, mult16_16_q15(n, 6713))))));
    let r2: i32 = i32::from(mult16_16_q15(r, r) as i16);
    let y: i32 = i32::from(shl16(sub16(i32::from(add16(mult16_16_q15(r2, n), r2)), 16384), 1));
    add16(r, mult16_16_q15(r, mult16_16_q15(y, sub16(mult16_16_q15(y, 12288), 16384))))
}

/// `celt_sqrt`.
#[must_use]
pub fn celt_sqrt(x: i32) -> i32 {
    const C: [i32; 5] = [23175, 11561, -3011, 1699, -664];
    if x == 0 {
        return 0;
    } else if x >= 1_073_741_824 {
        return 32767;
    }
    let k = (i32::from(celt_ilog2(x)) >> 1).wrapping_sub(7);
    let x = vshr32(x, k.wrapping_mul(2));
    let n: i32 = i32::from(x.wrapping_sub(32768) as i16);
    let t4 = i32::from(add16(C[3], mult16_16_q15(n, C[4])));
    let t3 = i32::from(add16(C[2], mult16_16_q15(n, t4)));
    let t2 = i32::from(add16(C[1], mult16_16_q15(n, t3)));
    let rt: i32 = i32::from(add16(C[0], mult16_16_q15(n, t2)));
    vshr32(rt, 7i32.wrapping_sub(k))
}

/// `_celt_cos_pi_2`.
fn celt_cos_pi_2(x: i16) -> i16 {
    const L1: i32 = 32767;
    const L2: i32 = -7651;
    const L3: i32 = 8277;
    const L4: i32 = -626;
    let x = i32::from(x);
    let x2: i32 = i32::from(mult16_16_p15(x, x) as i16);
    let inner = add32(sub16(L1, x2), mult16_16_p15(x2, add32(L2, mult16_16_p15(x2, add32(L3, mult16_16_p15(L4, x2))))));
    add16(1, inner.min(32766))
}

/// `celt_cos_norm`: the cosine of `x` × π/2, `x` Q16 normalised.
#[must_use]
pub fn celt_cos_norm(x: i32) -> i16 {
    let mut x = x & 0x0001_ffff;
    if x > shl32(1, 16) {
        x = sub32(shl32(1, 17), x);
    }
    if x & 0x0000_7fff != 0 {
        if x < shl32(1, 15) {
            celt_cos_pi_2(extract16(x))
        } else {
            celt_cos_pi_2(extract16(65536i32.wrapping_sub(x))).wrapping_neg()
        }
    } else if x & 0x0000_ffff != 0 {
        0
    } else if x & 0x0001_ffff != 0 {
        -32767
    } else {
        32767
    }
}
