//! The generator's noise source: a deterministic pseudo-random generator,
//! with uniform and Gaussian draws.

use core::f32::consts::TAU;

use crate::math::{ln, sin_cos, sqrt};

/// A deterministic pseudo-random generator: `xorshift64*` (Marsaglia 2003,
/// Vigna 2016), seeded through `SplitMix64` so any seed, even 0, is usable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rng {
    state: u64,
}

/// `SplitMix64` increment (Steele, Lea and Flood 2014).
const SPLITMIX_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
/// `SplitMix64` first mixing multiplier.
const SPLITMIX_MULTIPLIER_1: u64 = 0xBF58_476D_1CE4_E5B9;
/// `SplitMix64` second mixing multiplier.
const SPLITMIX_MULTIPLIER_2: u64 = 0x94D0_49BB_1331_11EB;
/// `xorshift64*` output multiplier (Vigna 2016).
const XORSHIFT_MULTIPLIER: u64 = 0x2545_F491_4F6C_DD1D;
/// Bits of a uniform `f32` draw: the 24-bit significand.
const UNIFORM_BITS: u32 = 24;

impl Rng {
    /// A generator from a seed.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(SPLITMIX_GAMMA);
        z = (z ^ (z >> 30)).wrapping_mul(SPLITMIX_MULTIPLIER_1);
        z = (z ^ (z >> 27)).wrapping_mul(SPLITMIX_MULTIPLIER_2);
        z ^= z >> 31;
        // xorshift needs a non-zero state.
        Self { state: if z == 0 { SPLITMIX_GAMMA } else { z } }
    }

    /// The next 64 random bits.
    pub const fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(XORSHIFT_MULTIPLIER)
    }

    /// A uniform value in (0, 1]: never 0, so its logarithm is finite.
    pub fn uniform(&mut self) -> f32 {
        let bits = self.next_u64() >> (u64::BITS - UNIFORM_BITS);
        // 24 bits fit an f32 exactly.
        #[allow(clippy::cast_precision_loss)]
        let value = (bits as f32 + 1.0) / (1u32 << UNIFORM_BITS) as f32;
        value
    }

    /// A standard normal value, by the Box–Muller transform.
    pub fn gaussian(&mut self) -> f32 {
        let radius = sqrt(-2.0 * ln(self.uniform()));
        let (_, cosine) = sin_cos(TAU * self.uniform());
        radius * cosine
    }

    /// A random byte.
    pub fn byte(&mut self) -> u8 {
        (self.next_u64() >> (u64::BITS - u8::BITS)) as u8
    }
}
