//! Small floating-point helpers for a `no_std` crate.
//!
//! `core` has no `floor`, `sqrt`, `sin`, `ln` or `atan2` for `f32`, so the
//! few the slicer and the signal generator need are written here. Floating
//! point is permitted in the slicer by REQ-GEN-016; the fixed-point form of
//! REQ-SLC-015 (Phase 2) would replace this module and the signal-processing
//! stage of `slicer`, not the state machine.
//!
//! Each function is accurate to a few units in the last place of `f32` over
//! the ranges used, which is far finer than any signal measurement needs.

use core::f32::consts::{FRAC_PI_2, FRAC_PI_6, LN_2, PI, TAU};

/// The square root of 3, to `f32` precision.
const SQRT_3: f32 = 1.732_050_8;
/// tan(π/12) = 2 − √3: above it, [`atan`] reduces its argument by π/6.
const TAN_PI_12: f32 = 2.0 - SQRT_3;
/// Mantissa bits of an IEEE 754 single (binary32).
const F32_MANTISSA_BITS: u32 = 23;
/// Mask of the mantissa field of an IEEE 754 single.
const F32_MANTISSA_MASK: u32 = (1 << F32_MANTISSA_BITS) - 1;
/// Biased exponent of 1.0 in an IEEE 754 single.
const F32_EXPONENT_BIAS: i32 = 127;
/// Mask of the 8-bit biased exponent field after shifting.
const F32_EXPONENT_MASK: u32 = 0xFF;

/// Rounds towards negative infinity and converts, saturating at the `i32`
/// range; NaN gives 0.
#[must_use]
// The cast saturates by definition (Rust reference, "numeric cast"); the
// correction below only moves negative non-integers down by one.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
pub fn floor_i32(x: f32) -> i32 {
    let truncated = x as i32;
    if (truncated as f32) > x { truncated.saturating_sub(1) } else { truncated }
}

/// Rounds to the nearest integer, halves away from zero, as `i32`.
#[must_use]
pub fn round_i32(x: f32) -> i32 {
    if x >= 0.0 { floor_i32(x + 0.5) } else { floor_i32(0.5 - x).saturating_neg() }
}

/// Converts an `i32` to `f32`. Values used here are sample positions and
/// counts below 2^24, which `f32` holds exactly.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub const fn f32_from_i32(value: i32) -> f32 {
    value as f32
}

/// Converts a `usize` to `f32`. Values used here are sample positions and
/// counts below 2^24, which `f32` holds exactly.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub const fn f32_from_usize(value: usize) -> f32 {
    value as f32
}

/// Converts a `u32` to `f32`, rounding to the nearest representable value
/// (sample rates such as 35,468,950 Hz lose under one part in 10^7).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub const fn f32_from_u32(value: u32) -> f32 {
    value as f32
}

/// The floor of a non-negative position as an index; negative values and
/// NaN give 0.
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub fn floor_index(x: f32) -> usize {
    floor_i32(x).max(0) as usize
}

/// The ceiling of a non-negative position as an index; negative values and
/// NaN give 0.
#[must_use]
pub fn ceil_index(x: f32) -> usize {
    let floor = floor_index(x);
    if f32_from_usize(floor) < x { floor.saturating_add(1) } else { floor }
}

/// Clamps a value to 0 to `u16::MAX` and rounds it.
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub fn to_u16_saturating(x: f32) -> u16 {
    let rounded = round_i32(x).clamp(0, i32::from(u16::MAX));
    rounded as u16
}

/// Clamps a value to 0 to `u8::MAX` and rounds it.
#[must_use]
#[allow(clippy::cast_sign_loss)]
pub fn to_u8_saturating(x: f32) -> u8 {
    let rounded = round_i32(x).clamp(0, i32::from(u8::MAX));
    rounded as u8
}

/// Reduces an angle to −π to π.
fn reduce_angle(x: f32) -> f32 {
    x - TAU * f32_from_i32(round_i32(x / TAU))
}

/// Sine and cosine of `x` radians.
#[must_use]
pub fn sin_cos(x: f32) -> (f32, f32) {
    let reduced = reduce_angle(x);
    // Fold into −π/2 to π/2, where the series below converges fast:
    // sin(π − a) = sin a and cos(π − a) = −cos a.
    let (folded, cos_sign) = if reduced > FRAC_PI_2 {
        (PI - reduced, -1.0)
    } else if reduced < -FRAC_PI_2 {
        (-PI - reduced, -1.0)
    } else {
        (reduced, 1.0)
    };
    let square = folded * folded;
    // Taylor series to the x^11 and x^12 terms: the truncation error at π/2
    // is below (π/2)^13 / 13! ≈ 6e-8.
    let sine = folded
        * (1.0
            - square / 6.0
                * (1.0 - square / 20.0 * (1.0 - square / 42.0 * (1.0 - square / 72.0 * (1.0 - square / 110.0)))));
    let cosine = 1.0
        - square / 2.0
            * (1.0
                - square / 12.0
                    * (1.0 - square / 30.0 * (1.0 - square / 56.0 * (1.0 - square / 90.0 * (1.0 - square / 132.0)))));
    (sine, cos_sign * cosine)
}

/// Arctangent of `x` for `x` in 0 to 1.
fn atan_unit(x: f32) -> f32 {
    // atan x = π/6 + atan((√3·x − 1)/(√3 + x)) brings x below tan(π/12).
    let (base, reduced) = if x > TAN_PI_12 { (FRAC_PI_6, (SQRT_3 * x - 1.0) / (SQRT_3 + x)) } else { (0.0, x) };
    let square = reduced * reduced;
    // Taylor series to x^13: with |x| ≤ 0.268 the next term is below 1e-9.
    let series = reduced
        * (1.0
            - square
                * (1.0 / 3.0
                    - square
                        * (1.0 / 5.0
                            - square * (1.0 / 7.0 - square * (1.0 / 9.0 - square * (1.0 / 11.0 - square / 13.0))))));
    base + series
}

/// The four-quadrant arctangent of `y / x`, in −π to π.
#[must_use]
pub fn atan2(y: f32, x: f32) -> f32 {
    let (ax, ay) = (x.abs(), y.abs());
    if ax <= 0.0 && ay <= 0.0 {
        return 0.0;
    }
    let first_octant = if ay <= ax { atan_unit(ay / ax) } else { FRAC_PI_2 - atan_unit(ax / ay) };
    let quadrant = if x < 0.0 { PI - first_octant } else { first_octant };
    if y < 0.0 { -quadrant } else { quadrant }
}

/// Splits a positive normal `x` into a mantissa in 1 to 2 and a power of two.
fn frexp(x: f32) -> (f32, i32) {
    let bits = x.to_bits();
    let exponent_bits = (bits >> F32_MANTISSA_BITS) & F32_EXPONENT_MASK;
    // The biased exponent is 8 bits, so it always fits an i32.
    let exponent = i32::try_from(exponent_bits).unwrap_or(F32_EXPONENT_BIAS).saturating_sub(F32_EXPONENT_BIAS);
    let one_bits = 1.0_f32.to_bits() & !F32_MANTISSA_MASK;
    let mantissa = f32::from_bits((bits & F32_MANTISSA_MASK) | one_bits);
    (mantissa, exponent)
}

/// Natural logarithm of a positive normal number; zero, negative or NaN
/// input gives negative infinity.
#[must_use]
pub fn ln(x: f32) -> f32 {
    if x.is_nan() || x <= f32::MIN_POSITIVE {
        return f32::NEG_INFINITY;
    }
    let (mantissa, exponent) = frexp(x);
    // ln m = 2·atanh s with s = (m − 1)/(m + 1) ≤ 1/3: series to s^13.
    let s = (mantissa - 1.0) / (mantissa + 1.0);
    let square = s * s;
    let series = 2.0
        * s
        * (1.0
            + square
                * (1.0 / 3.0
                    + square
                        * (1.0 / 5.0
                            + square * (1.0 / 7.0 + square * (1.0 / 9.0 + square * (1.0 / 11.0 + square / 13.0))))));
    series + f32_from_i32(exponent) * LN_2
}

/// e raised to `x`, for `x` in about −80 to 80.
#[must_use]
pub fn exp(x: f32) -> f32 {
    // x = k·ln 2 + r with |r| ≤ ln 2 / 2, e^x = 2^k · e^r.
    let k = round_i32(x / LN_2).clamp(F32_EXPONENT_BIAS.saturating_sub(1).saturating_neg(), F32_EXPONENT_BIAS);
    let r = x - f32_from_i32(k) * LN_2;
    let series = 1.0
        + r * (1.0 + r / 2.0 * (1.0 + r / 3.0 * (1.0 + r / 4.0 * (1.0 + r / 5.0 * (1.0 + r / 6.0 * (1.0 + r / 7.0))))));
    let biased = u32::try_from(k.saturating_add(F32_EXPONENT_BIAS)).unwrap_or(0);
    series * f32::from_bits(biased << F32_MANTISSA_BITS)
}

/// Square root of a non-negative number; negative input gives 0.
#[must_use]
pub fn sqrt(x: f32) -> f32 {
    if x.is_nan() || x <= 0.0 {
        return 0.0;
    }
    // Halving the exponent gives a first guess within a factor of √2;
    // five Newton steps then reach full precision.
    let (mantissa, exponent) = frexp(x);
    let half = exponent.div_euclid(2);
    let odd = exponent.rem_euclid(2);
    let scaled = if odd == 1 { mantissa * 2.0 } else { mantissa };
    let biased = u32::try_from(half.saturating_add(F32_EXPONENT_BIAS)).unwrap_or(0);
    let power = f32::from_bits(biased << F32_MANTISSA_BITS);
    let mut root = 1.2_f32;
    for _ in 0..5 {
        root = f32::midpoint(root, scaled / root);
    }
    root * power
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn floor_and_round() {
        assert_eq!(floor_i32(2.7), 2);
        assert_eq!(floor_i32(-2.2), -3);
        assert_eq!(floor_i32(-3.0), -3);
        assert_eq!(floor_i32(f32::NAN), 0);
        assert_eq!(round_i32(2.5), 3);
        assert_eq!(round_i32(-2.5), -3);
        assert_eq!(round_i32(-2.4), -2);
        assert_eq!(ceil_index(2.1), 3);
        assert_eq!(ceil_index(2.0), 2);
        assert_eq!(floor_index(-1.0), 0);
    }

    #[test]
    fn trigonometry_matches_std() {
        let mut x = -20.0_f32;
        while x < 20.0 {
            let (s, c) = sin_cos(x);
            assert!(close(s, f64::from(x).sin() as f32, 2e-6), "sin {x}");
            assert!(close(c, f64::from(x).cos() as f32, 2e-6), "cos {x}");
            x += 0.013;
        }
    }

    #[test]
    fn atan2_matches_std() {
        for i in 0..720 {
            let angle = f64::from(i) * core::f64::consts::PI / 360.0 - core::f64::consts::PI;
            let (y, x) = (angle.sin() * 3.0, angle.cos() * 3.0);
            let got = atan2(y as f32, x as f32);
            let want = y.atan2(x) as f32;
            let difference = reduce_angle(got - want).abs();
            assert!(difference < 2e-6, "atan2 at {angle}: {got} vs {want}");
        }
        assert!(close(atan2(0.0, 0.0), 0.0, 0.0));
    }

    #[test]
    fn logarithm_exponential_and_root_match_std() {
        for x in [1e-6_f32, 0.001, 0.5, 1.0, 1.5, 2.0, 10.0, 1000.0, 65535.0] {
            assert!(close(ln(x), f64::from(x).ln() as f32, 2e-6 * f64::from(x).ln().abs().max(1.0) as f32), "ln {x}");
            assert!(close(sqrt(x), f64::from(x).sqrt() as f32, 2e-6 * f64::from(x).sqrt() as f32), "sqrt {x}");
        }
        for x in [-20.0_f32, -1.0, -0.1, 0.0, 0.3, 1.0, 5.0, 20.0] {
            let want = f64::from(x).exp() as f32;
            assert!(close(exp(x), want, 3e-6 * want), "exp {x}");
        }
        assert!(ln(0.0).is_infinite());
        assert!(close(sqrt(-1.0), 0.0, 0.0));
    }
}
