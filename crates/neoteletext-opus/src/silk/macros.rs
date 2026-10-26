//! SILK's fixed-point operations: the macros of the reference decoder's `silk/macros.h`,
//! `SigProc_FIX.h` and `Inlines.h` that the decoder uses, one function each, named as the macro
//! in lower case without the `silk_` prefix, so that the ported code reads as the reference
//! does and computes what it computes.
//!
//! Operands are `i32` (C's `int`, to which every narrower operand is promoted). Where the C
//! would overflow a signed `int` (undefined, but two's complement on every target), these wrap,
//! as the compiled C does; shift counts are taken modulo the width, as the hardware's are.
//!
//! The 32×16 multiplies (`smulwb`, `smlawb`) use their 32-bit forms, which no 64-bit multiply
//! needs (a Cortex-M0 has none) and which are exact: with `a = ah * 2^16 + al` (`al` in
//! 0..65536) and `c` 16-bit, `(a * c) >> 16 = ah * c + ((al * c) >> 16)`, as `ah * c * 2^16` is a
//! multiple of 2^16, and both products fit 32 bits. The 32×32 multiplies keep 64 bits.
//!
//! From the reference decoder. Copyright (c) 2006-2011, Skype Limited, BSD-3-Clause (the licence
//! beside the crate).

/// `RAND_MULTIPLIER`, `RAND_INCREMENT`: the linear congruential generator of RFC 6716 §4.2.7.7.
const RAND_MULTIPLIER: i32 = 196_314_165;
const RAND_INCREMENT: i32 = 907_633_515;

/// `silk_SMULWB`: `(a32 * (opus_int16)b32) >> 16`, in 32 bits.
#[inline]
#[must_use]
pub const fn smulwb(a32: i32, b32: i32) -> i32 {
    let c = b32 as i16 as i32;
    (a32 >> 16).wrapping_mul(c).wrapping_add((a32 & 0xFFFF).wrapping_mul(c) >> 16)
}
/// `silk_SMLAWB`: `a32 + ((b32 * (opus_int16)c32) >> 16)`, in 32 bits.
#[inline]
#[must_use]
pub const fn smlawb(a32: i32, b32: i32, c32: i32) -> i32 {
    a32.wrapping_add(smulwb(b32, c32))
}
/// `silk_SMULBB`: `(opus_int16)a32 * (opus_int16)b32`.
#[inline]
#[must_use]
pub const fn smulbb(a32: i32, b32: i32) -> i32 {
    (a32 as i16 as i32).wrapping_mul(b32 as i16 as i32)
}
/// `silk_SMLABB`: `a32 + (opus_int16)b32 * (opus_int16)c32`.
#[inline]
#[must_use]
pub const fn smlabb(a32: i32, b32: i32, c32: i32) -> i32 {
    a32.wrapping_add(smulbb(b32, c32))
}
/// `silk_SMLABB_ovflw`: the same, wrapping by intent.
#[inline]
#[must_use]
pub const fn smlabb_ovflw(a32: i32, b32: i32, c32: i32) -> i32 {
    smlabb(a32, b32, c32)
}
/// `silk_SMULTT`: `(a32 >> 16) * (b32 >> 16)`.
#[inline]
#[must_use]
pub const fn smultt(a32: i32, b32: i32) -> i32 {
    (a32 >> 16).wrapping_mul(b32 >> 16)
}
/// `silk_SMULWW`: `(a32 * b32) >> 16`.
#[inline]
#[must_use]
pub const fn smulww(a32: i32, b32: i32) -> i32 {
    ((a32 as i64).wrapping_mul(b32 as i64) >> 16) as i32
}
/// `silk_SMLAWW`: `a32 + ((b32 * c32) >> 16)`.
#[inline]
#[must_use]
pub const fn smlaww(a32: i32, b32: i32, c32: i32) -> i32 {
    (a32 as i64).wrapping_add((b32 as i64).wrapping_mul(c32 as i64) >> 16) as i32
}
/// `silk_SMULL`: the 64-bit product.
#[inline]
#[must_use]
pub const fn smull(a32: i32, b32: i32) -> i64 {
    (a32 as i64).wrapping_mul(b32 as i64)
}
/// `silk_SMMUL`: the top word of the 64-bit product.
#[inline]
#[must_use]
pub const fn smmul(a32: i32, b32: i32) -> i32 {
    (smull(a32, b32) >> 32) as i32
}
/// `silk_MUL`.
#[inline]
#[must_use]
pub const fn mul(a32: i32, b32: i32) -> i32 {
    a32.wrapping_mul(b32)
}
/// `silk_MLA`: a32 + b32 * c32.
#[inline]
#[must_use]
pub const fn mla(a32: i32, b32: i32, c32: i32) -> i32 {
    a32.wrapping_add(b32.wrapping_mul(c32))
}
/// `silk_ADD32`, `silk_ADD16` and `silk_ADD32_ovflw`.
#[inline]
#[must_use]
pub const fn add32(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}
/// `silk_SUB32`, `silk_SUB16` and `silk_SUB32_ovflw`.
#[inline]
#[must_use]
pub const fn sub32(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}
/// `silk_ADD_SAT32`.
#[inline]
#[must_use]
pub const fn add_sat32(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}
/// `silk_SUB_SAT32`.
#[inline]
#[must_use]
pub const fn sub_sat32(a: i32, b: i32) -> i32 {
    a.saturating_sub(b)
}
/// `silk_SAT16`: the value limited to the `opus_int16` range (still an `int`, as in the C).
#[inline]
#[must_use]
pub const fn sat16(a: i32) -> i32 {
    if a > i16::MAX as i32 {
        i16::MAX as i32
    } else if a < i16::MIN as i32 {
        i16::MIN as i32
    } else {
        a
    }
}
/// `silk_ADD_SAT16`.
#[inline]
#[must_use]
pub const fn add_sat16(a: i32, b: i32) -> i16 {
    sat16(add32(a, b)) as i16
}
/// `silk_CLZ32`.
#[inline]
#[must_use]
pub const fn clz32(in32: i32) -> i32 {
    // At most 32, so the conversion is exact
    in32.leading_zeros().cast_signed()
}
/// `silk_LSHIFT` and `silk_LSHIFT32`.
#[inline]
#[must_use]
pub const fn lshift(a: i32, shift: i32) -> i32 {
    // The C shifts the bits, so the shift is of the unsigned reinterpretation
    a.cast_unsigned().wrapping_shl(shift.cast_unsigned()).cast_signed()
}
/// `silk_RSHIFT` and `silk_RSHIFT32`: arithmetic.
#[inline]
#[must_use]
pub const fn rshift(a: i32, shift: i32) -> i32 {
    a.wrapping_shr(shift.cast_unsigned())
}
/// `silk_LSHIFT_SAT32`: saturates before shifting.
#[inline]
#[must_use]
pub const fn lshift_sat32(a: i32, shift: i32) -> i32 {
    let (low, high) = (rshift(i32::MIN, shift), rshift(i32::MAX, shift));
    lshift(
        if a > high {
            high
        } else if a < low {
            low
        } else {
            a
        },
        shift,
    )
}
/// `silk_ADD_LSHIFT32`.
#[inline]
#[must_use]
pub const fn add_lshift32(a: i32, b: i32, shift: i32) -> i32 {
    add32(a, lshift(b, shift))
}
/// `silk_ADD_RSHIFT32`.
#[inline]
#[must_use]
pub const fn add_rshift32(a: i32, b: i32, shift: i32) -> i32 {
    add32(a, rshift(b, shift))
}
/// `silk_ADD_RSHIFT_uint`.
#[inline]
#[must_use]
pub const fn add_rshift_uint(a: u32, b: u32, shift: i32) -> u32 {
    a.wrapping_add(b.wrapping_shr(shift.cast_unsigned()))
}
/// `silk_SUB_LSHIFT32`.
#[inline]
#[must_use]
pub const fn sub_lshift32(a: i32, b: i32, shift: i32) -> i32 {
    sub32(a, lshift(b, shift))
}
/// `silk_RSHIFT_ROUND` (requires `shift > 0`).
#[inline]
#[must_use]
pub const fn rshift_round(a: i32, shift: i32) -> i32 {
    if shift == 1 {
        (a >> 1).wrapping_add(a & 1)
    } else {
        (a.wrapping_shr(shift.wrapping_sub(1).cast_unsigned()).wrapping_add(1)) >> 1
    }
}
/// `silk_RSHIFT_ROUND64`.
#[inline]
#[must_use]
pub const fn rshift_round64(a: i64, shift: i32) -> i64 {
    if shift == 1 {
        (a >> 1).wrapping_add(a & 1)
    } else {
        (a.wrapping_shr(shift.wrapping_sub(1).cast_unsigned()).wrapping_add(1)) >> 1
    }
}
/// `silk_DIV32_16`, `silk_DIV32`: C division, truncating toward zero.
#[inline]
#[must_use]
pub const fn div32(a32: i32, b32: i32) -> i32 {
    match a32.checked_div(b32) {
        Some(quotient) => quotient,
        // The C divides by no zero on any stream; 0 stands in for its undefined result, so that
        // a hostile one cannot stop the decoder
        None if b32 == 0 => 0,
        // The most negative value by -1, which two's complement wraps back to itself
        None => a32,
    }
}
/// `silk_DIV32` of a nonnegative dividend by a positive divisor, which unsigned division
/// answers the same, more cheaply where the division is in software.
#[inline]
#[must_use]
pub const fn positive_div(a32: i32, b32: i32) -> i32 {
    // Every caller's divisor is at least 1; 0 stands in for a division by zero none makes
    match a32.cast_unsigned().checked_div(b32.cast_unsigned()) {
        Some(quotient) => quotient.cast_signed(),
        None => 0,
    }
}
/// `silk_abs`: wrong (as the C says) for the most negative value, which it returns unchanged.
#[inline]
#[must_use]
pub const fn abs(a: i32) -> i32 {
    a.wrapping_abs()
}
/// `silk_RAND`: the next value of the linear congruential generator.
#[inline]
#[must_use]
pub const fn rand(seed: i32) -> i32 {
    RAND_INCREMENT.wrapping_add(seed.wrapping_mul(RAND_MULTIPLIER))
}
/// The top `bits` bits of a `silk_RAND` seed (`bits` from 1 to 31), as an index: the same as
/// the C's `silk_RSHIFT( seed, 32 - bits )` masked to `bits` bits, whatever the seed's sign.
#[inline]
#[must_use]
pub const fn rand_index(seed: i32, bits: u32) -> usize {
    seed.cast_unsigned().wrapping_shr(32u32.wrapping_sub(bits)) as usize
}
/// `silk_CLZ_FRAC`: the number of leading zeros and the 7 bits after the leading one.
#[inline]
#[must_use]
pub const fn clz_frac(input: i32) -> (i32, i32) {
    let lzeros = clz32(input);
    (lzeros, input.cast_unsigned().rotate_right(24i32.wrapping_sub(lzeros).cast_unsigned()).cast_signed() & 0x7f)
}
/// `silk_SQRT_APPROX`.
#[inline]
#[must_use]
pub const fn sqrt_approx(x: i32) -> i32 {
    if x <= 0 {
        return 0;
    }
    let (lz, frac_q7) = clz_frac(x);
    let mut y: i32 = if lz & 1 != 0 {
        32768
    } else {
        46214 /* sqrt(2) * 32768 */
    };
    y >>= lz >> 1;
    smlawb(y, y, smulbb(213, frac_q7))
}
/// The reciprocal of `silk_DIV32_varQ` and `silk_INVERSE32_varQ`, `(silk_int32_MAX >> 2) /
/// (b32_nrm >> 16)`. The C requires `b32` nonzero, so normalised it is at least 2^30 in
/// magnitude and the divisor at least 2^14; a zero `b32`, which no caller passes, gives 0 here
/// rather than a division by zero.
#[inline]
const fn normalised_reciprocal(b32_nrm: i32) -> i32 {
    match (i32::MAX >> 2).checked_div(rshift(b32_nrm, 16)) {
        Some(reciprocal) => reciprocal,
        None => 0,
    }
}
/// `silk_DIV32_varQ`: a good approximation of (a32 << Qres) / b32.
#[inline]
#[must_use]
pub const fn div32_varq(a32: i32, b32: i32, qres: i32) -> i32 {
    let a_headrm = clz32(abs(a32)).wrapping_sub(1);
    let mut a32_nrm = lshift(a32, a_headrm);
    let b_headrm = clz32(abs(b32)).wrapping_sub(1);
    let b32_nrm = lshift(b32, b_headrm);
    let b32_inv = normalised_reciprocal(b32_nrm);
    let mut result = smulwb(a32_nrm, b32_inv);
    a32_nrm = sub32(a32_nrm, lshift(smmul(b32_nrm, result), 3));
    result = smlawb(result, a32_nrm, b32_inv);
    let lshift_by = 29i32.wrapping_add(a_headrm).wrapping_sub(b_headrm).wrapping_sub(qres);
    if lshift_by < 0 {
        lshift_sat32(result, lshift_by.wrapping_neg())
    } else if lshift_by < 32 {
        rshift(result, lshift_by)
    } else {
        0
    }
}
/// `silk_INVERSE32_varQ`: a good approximation of (1 << Qres) / b32.
#[inline]
#[must_use]
pub const fn inverse32_varq(b32: i32, qres: i32) -> i32 {
    let b_headrm = clz32(abs(b32)).wrapping_sub(1);
    let b32_nrm = lshift(b32, b_headrm);
    let b32_inv = normalised_reciprocal(b32_nrm);
    let mut result = lshift(b32_inv, 16);
    let err_q32 = lshift((1i32 << 29).wrapping_sub(smulwb(b32_nrm, b32_inv)), 3);
    result = smlaww(result, err_q32, b32_inv);
    let lshift_by = 61i32.wrapping_sub(b_headrm).wrapping_sub(qres);
    if lshift_by <= 0 {
        lshift_sat32(result, lshift_by.wrapping_neg())
    } else if lshift_by < 32 {
        rshift(result, lshift_by)
    } else {
        0
    }
}
