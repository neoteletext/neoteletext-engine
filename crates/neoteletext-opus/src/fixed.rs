//! The reference's fixed-point arithmetic (`celt/fixed_generic.h` and `celt/arch.h`, built
//! `FIXED_POINT` with `OPUS_FAST_INT64` as on every 64-bit machine), one function per macro, so
//! that ported code reads as the C does and computes what it computes.
//!
//! Types follow `arch.h`: `opus_val16` is `i16`, `opus_val32` `i32`, `celt_sig` `i32` in
//! Q(`SIG_SHIFT`), `celt_norm` `i16` in Q14. C's conversions are written out: a macro that casts
//! to `opus_val16` truncates (`as i16`), and 32-bit sums wrap, as the C compiles to on
//! two's-complement machines, so results are bit-exact with the reference.
//!
//! Portions ported from libopus 1.5.2: Copyright (C) 2003-2008 Jean-Marc Valin,
//! Copyright (C) 2007-2009 Xiph.Org Foundation, BSD-3-Clause (see `LICENSE-libopus`).

/// `Q15ONE`.
pub const Q15ONE: i16 = 32767;
/// `SIG_SHIFT`: `celt_sig` is Q12.
pub const SIG_SHIFT: u32 = 12;
/// `SIG_SAT`: a safe saturation for 32-bit signals, below 2^31 × (1 − 0.85).
pub const SIG_SAT: i32 = 300_000_000;
/// `NORM_SCALING`: `celt_norm` is Q14.
pub const NORM_SCALING: i32 = 16384;
/// `DB_SHIFT`: energies in dB are Q10 (`opus_val16` log energies, `celt_glog` in 1.5's fixed build).
pub const DB_SHIFT: u32 = 10;
/// `EPSILON`.
pub const EPSILON: i32 = 1;

/// `ABS16`: of a 16-bit value, which cannot overflow; wrapping all the same, as the C does.
#[inline]
#[must_use]
pub const fn abs16(x: i32) -> i32 {
    if x < 0 { x.wrapping_neg() } else { x }
}
/// `ABS32`: wrapping at the most negative value, as the C does.
#[inline]
#[must_use]
pub const fn abs32(x: i32) -> i32 {
    if x < 0 { x.wrapping_neg() } else { x }
}
/// `SAT16`.
#[inline]
#[must_use]
pub const fn sat16(x: i32) -> i16 {
    if x > 32767 {
        32767
    } else if x < -32768 {
        -32768
    } else {
        x as i16
    }
}
/// `MULT16_16SU`: a signed 16-bit times an unsigned 16-bit.
#[inline]
#[must_use]
pub const fn mult16_16su(a: i32, b: i32) -> i32 {
    (a as i16 as i32).wrapping_mul((b as i16).cast_unsigned() as i32)
}
// The 16×32 products below are the C's 32-bit forms: a 16-bit by 16-bit product of the high
// half, shifted, plus one of the low half. They equal the exact 64-bit product shifted, modulo
// 2^32, because the low half's floor is the whole product's floor less an integer; so they are
// bit-exact with the 64-bit forms the reference builds on 64-bit machines, and need no 64-bit
// multiply on a microcontroller without one (Cortex-M0).

/// `MULT16_32_Q15`: (a × b) >> 15.
#[inline]
#[must_use]
pub const fn mult16_32_q15(a: i32, b: i32) -> i32 {
    shl32(mult16_16(a, b >> 16), 1).wrapping_add(mult16_16su(a, b & 0x0000_ffff) >> 15)
}
/// `MULT32_32_Q16`.
#[inline]
#[must_use]
pub const fn mult32_32_q16(a: i32, b: i32) -> i32 {
    ((a as i64).wrapping_mul(b as i64) >> 16) as i32
}
/// `MULT32_32_Q31`.
#[inline]
#[must_use]
pub const fn mult32_32_q31(a: i32, b: i32) -> i32 {
    ((a as i64).wrapping_mul(b as i64) >> 31) as i32
}
/// `EXTRACT16`: truncation to 16 bits.
#[inline]
#[must_use]
pub const fn extract16(x: i32) -> i16 {
    x as i16
}
/// `SHR16`, `SHR32`, `SHR`: arithmetic.
#[inline]
#[must_use]
pub const fn shr32(a: i32, shift: u32) -> i32 {
    a >> shift
}
/// `SHL16`: through `opus_uint16`.
#[inline]
#[must_use]
pub const fn shl16(a: i32, shift: u32) -> i16 {
    (a as i16) << shift
}
/// `SHL32`, `SHL`: through `opus_uint32`.
#[inline]
#[must_use]
pub const fn shl32(a: i32, shift: u32) -> i32 {
    a << shift
}
/// `PSHR32`, `PSHR`: shift with rounding.
#[inline]
#[must_use]
pub const fn pshr32(a: i32, shift: u32) -> i32 {
    a.wrapping_add((1i32 << shift) >> 1) >> shift
}
/// `VSHR32`: right for a positive shift, left for a negative.
#[inline]
#[must_use]
pub const fn vshr32(a: i32, shift: i32) -> i32 {
    if shift > 0 { shr32(a, shift.unsigned_abs()) } else { shl32(a, shift.unsigned_abs()) }
}
/// `SATURATE`.
#[inline]
#[must_use]
pub const fn saturate(x: i32, a: i32) -> i32 {
    if x > a {
        a
    } else if x < a.wrapping_neg() {
        a.wrapping_neg()
    } else {
        x
    }
}
/// `SATURATE16`.
#[inline]
#[must_use]
pub const fn saturate16(x: i32) -> i16 {
    sat16(x)
}
/// `ROUND16`.
#[inline]
#[must_use]
pub const fn round16(x: i32, a: u32) -> i16 {
    extract16(pshr32(x, a))
}
/// `SROUND16`.
#[inline]
#[must_use]
pub const fn sround16(x: i32, a: u32) -> i16 {
    extract16(saturate(pshr32(x, a), 32767))
}
/// `HALF16`.
#[inline]
#[must_use]
pub const fn half16(x: i32) -> i32 {
    x >> 1
}
/// `ADD16`: the sum truncated to 16 bits.
#[inline]
#[must_use]
pub const fn add16(a: i32, b: i32) -> i16 {
    (a as i16).wrapping_add(b as i16)
}
/// `SUB16`: an `int`, not truncated (the C casts each operand only).
#[inline]
#[must_use]
pub const fn sub16(a: i32, b: i32) -> i32 {
    (a as i16 as i32).wrapping_sub(b as i16 as i32)
}
/// `ADD32`.
#[inline]
#[must_use]
pub const fn add32(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}
/// `SUB32`.
#[inline]
#[must_use]
pub const fn sub32(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}
/// `ADD32_ovflw`.
#[inline]
#[must_use]
pub const fn add32_ovflw(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}
/// `SUB32_ovflw`.
#[inline]
#[must_use]
pub const fn sub32_ovflw(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}
/// `NEG32_ovflw`.
#[inline]
#[must_use]
pub const fn neg32_ovflw(a: i32) -> i32 {
    a.wrapping_neg()
}
/// `MULT32_32_32`.
#[inline]
#[must_use]
pub const fn mult32_32_32(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}
/// `MULT16_16`: the operands truncated to 16 bits, multiplied to 32.
#[inline]
#[must_use]
pub const fn mult16_16(a: i32, b: i32) -> i32 {
    (a as i16 as i32).wrapping_mul(b as i16 as i32)
}
/// `MAC16_16`.
#[inline]
#[must_use]
pub const fn mac16_16(c: i32, a: i32, b: i32) -> i32 {
    add32(c, mult16_16(a, b))
}
/// `MULT16_16_Q14`.
#[inline]
#[must_use]
pub const fn mult16_16_q14(a: i32, b: i32) -> i32 {
    mult16_16(a, b) >> 14
}
/// `MULT16_16_Q15`.
#[inline]
#[must_use]
pub const fn mult16_16_q15(a: i32, b: i32) -> i32 {
    mult16_16(a, b) >> 15
}
/// `MULT16_16_P15`.
#[inline]
#[must_use]
pub const fn mult16_16_p15(a: i32, b: i32) -> i32 {
    add32(16384, mult16_16(a, b)) >> 15
}
/// `SIG2WORD16`.
#[inline]
#[must_use]
pub const fn sig2word16(x: i32) -> i16 {
    let x = pshr32(x, SIG_SHIFT);
    let x = if x < -32768 { -32768 } else { x };
    let x = if x > 32767 { 32767 } else { x };
    x as i16
}
/// `MAX16`, `MAX32` (on the promoted values, as the C macros compare).
#[inline]
#[must_use]
pub const fn max32(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}
/// `MIN16`, `MIN32` (on the promoted values, as the C macros compare).
#[inline]
#[must_use]
pub const fn min32(a: i32, b: i32) -> i32 {
    if a < b { a } else { b }
}
/// The number of bits in `x`, 0 for 0: `u32::bit_width`, which the engine's minimum Rust
/// version lacks.
#[inline]
#[must_use]
pub const fn bit_count(x: u32) -> u32 {
    u32::BITS.wrapping_sub(x.leading_zeros())
}
/// `EC_ILOG` / `celt_ilog2` plus one for a nonzero value: the number of bits in `x`.
#[inline]
#[must_use]
pub const fn ec_ilog(x: u32) -> i32 {
    bit_count(x).cast_signed()
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "tests may panic"
)]
mod tests {
    use super::*;

    /// The 32-bit forms equal the exact products, over edges and a dense sweep.
    #[test]
    fn the_32_bit_forms_equal_the_exact_products() {
        let edges = [
            i32::MIN,
            i32::MIN + 1,
            -65537,
            -65536,
            -65535,
            -32769,
            -32768,
            -1,
            0,
            1,
            32767,
            32768,
            65535,
            65536,
            i32::MAX - 1,
            i32::MAX,
        ];
        let mut values: [i32; 4096] = [0; 4096];
        let mut x: u32 = 0x1234_5678;
        for v in &mut values {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *v = x.cast_signed();
        }
        for &a16 in edges.iter().chain(values.iter()) {
            let a = i32::from(a16 as i16);
            for &b in edges.iter().chain(values.iter().take(512)) {
                let exact = i64::from(a) * i64::from(b);
                assert_eq!(mult16_32_q15(a, b), (exact >> 15) as i32, "q15 {a} {b}");
            }
        }
    }
}
