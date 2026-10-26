//! The mixed-radix FFT under the inverse MDCT: radix 2, 3, 4 and 5 butterflies, decimation in
//! time, in place over input already in bit-reversed order, unscaled. Each size's plan (its
//! stages, their radices and twiddle strides) is a constant. The sums wrap, as two's complement
//! does: the transform's input may span the whole 32-bit range.

use crate::celt::tables::{FFT_BITREV_60, FFT_BITREV_120, FFT_BITREV_480, FFT_TWIDDLES};
use crate::fixed::{add32_ovflw, mult16_32_q15, neg32_ovflw, sub32_ovflw};
use crate::slices::chunks_mut;
use core::num::NonZeroUsize;

/// A complex value, `[real, imaginary]`.
pub type Complex = [i32; 2];
/// A twiddle, `(real, imaginary)`, Q15.
type Twiddle = (i16, i16);

/// The radix of one stage.
#[derive(Clone, Copy)]
enum Radix {
    Two,
    Three,
    Four,
    Five,
}

impl Radix {
    const fn value(self) -> usize {
        match self {
            Self::Two => 2,
            Self::Three => 3,
            Self::Four => 4,
            Self::Five => 5,
        }
    }
}

/// One stage: butterflies of `radix` sub-transforms of `m` points each, over blocks of
/// `radix * m`, with the twiddles of the k-th sub-transform `k * stride` apart in
/// `FFT_TWIDDLES`.
struct Stage {
    radix: Radix,
    m: usize,
    /// `radix * m`.
    block: NonZeroUsize,
    /// `stride`, `2 * stride`, `3 * stride` and `4 * stride`: none zero, so the butterflies'
    /// steps through the twiddles are never empty.
    strides: [NonZeroUsize; 4],
}

/// `a * b`, checked when the plans are compiled (only the plans' statics call it) to be nonzero
/// and to fit a `usize`.
const fn nonzero_product(a: usize, b: usize) -> NonZeroUsize {
    let product = match a.checked_mul(b) {
        Some(product) => NonZeroUsize::new(product),
        None => None,
    };
    assert!(product.is_some(), "a plan's sizes and strides are nonzero");
    match product {
        Some(product) => product,
        None => NonZeroUsize::MIN,
    }
}

/// The index of the last twiddle a stage's butterflies read: `(radix - 1) * (m - 1) * stride`,
/// or `None` if that does not fit a `usize`.
const fn last_twiddle(radix: Radix, m: usize, stride: usize) -> Option<usize> {
    let (Some(radix_less_one), Some(m_less_one)) = (radix.value().checked_sub(1), m.checked_sub(1)) else {
        return None;
    };
    match radix_less_one.checked_mul(m_less_one) {
        Some(steps) => steps.checked_mul(stride),
        None => None,
    }
}

impl Stage {
    /// A stage, checked when compiled: its twiddles must lie within `FFT_TWIDDLES`, and a radix-2
    /// stage only ever follows a radix-4 one of single points, which leaves it four points apart.
    const fn new(radix: Radix, m: usize, stride: usize) -> Self {
        assert!(matches!(last_twiddle(radix, m, stride), Some(last) if last < FFT_TWIDDLES.len()));
        assert!(!matches!(radix, Radix::Two) || m == 4);
        Self {
            radix,
            m,
            block: nonzero_product(radix.value(), m),
            strides: [
                nonzero_product(1, stride),
                nonzero_product(2, stride),
                nonzero_product(3, stride),
                nonzero_product(4, stride),
            ],
        }
    }
}

/// An FFT's plan: its size, its input order and its stages, in the order they run.
pub struct FftPlan {
    /// The points.
    pub size: usize,
    /// Where each input goes: the bit-reversed order of the radices.
    pub bitrev: &'static [u16],
    stages: &'static [Stage],
}

/// The 480-point FFT, radices 5, 3, 4, 2, 4 (the 1920-point MDCT's).
pub static FFT_480: FftPlan = FftPlan {
    size: 480,
    bitrev: &FFT_BITREV_480,
    stages: &[
        Stage::new(Radix::Four, 1, 120),
        Stage::new(Radix::Two, 4, 60),
        Stage::new(Radix::Four, 8, 15),
        Stage::new(Radix::Three, 32, 5),
        Stage::new(Radix::Five, 96, 1),
    ],
};

/// The 120-point FFT, radices 5, 3, 2, 4 (the 480-point MDCT's): the 480-point FFT's twiddles,
/// every fourth.
pub static FFT_120: FftPlan = FftPlan {
    size: 120,
    bitrev: &FFT_BITREV_120,
    stages: &[
        Stage::new(Radix::Four, 1, 120),
        Stage::new(Radix::Two, 4, 60),
        Stage::new(Radix::Three, 8, 20),
        Stage::new(Radix::Five, 24, 4),
    ],
};

/// The 60-point FFT, radices 5, 3, 4 (the 240-point MDCT's): every eighth twiddle.
pub static FFT_60: FftPlan = FftPlan {
    size: 60,
    bitrev: &FFT_BITREV_60,
    stages: &[Stage::new(Radix::Four, 1, 120), Stage::new(Radix::Three, 4, 40), Stage::new(Radix::Five, 12, 8)],
};

/// `S_MUL`: a sample times a Q15 twiddle.
#[inline]
const fn s_mul(a: i32, b: i16) -> i32 {
    mult16_32_q15(b as i32, a)
}

/// `C_MUL`.
#[inline]
const fn c_mul(a: Complex, b: Twiddle) -> Complex {
    [sub32_ovflw(s_mul(a[0], b.0), s_mul(a[1], b.1)), add32_ovflw(s_mul(a[0], b.1), s_mul(a[1], b.0))]
}

/// `C_ADD`.
#[inline]
const fn c_add(a: Complex, b: Complex) -> Complex {
    [add32_ovflw(a[0], b[0]), add32_ovflw(a[1], b[1])]
}

/// `C_SUB`.
#[inline]
const fn c_sub(a: Complex, b: Complex) -> Complex {
    [sub32_ovflw(a[0], b[0]), sub32_ovflw(a[1], b[1])]
}

/// The FFT of `data` (`plan.size` values, in bit-reversed order), in place.
pub fn fft(plan: &FftPlan, data: &mut [Complex]) {
    let Some(data) = data.get_mut(..plan.size) else {
        return;
    };
    for stage in plan.stages {
        let block = stage.block.get();
        let twiddles = FFT_TWIDDLES.as_slice();
        match stage.radix {
            Radix::Two => chunks_mut(data, block).for_each(butterfly2),
            Radix::Three => chunks_mut(data, block).for_each(|b| butterfly3(b, stage, twiddles)),
            Radix::Four if stage.m == 1 => chunks_mut(data, block).for_each(butterfly4_single),
            Radix::Four => chunks_mut(data, block).for_each(|b| butterfly4(b, stage, twiddles)),
            Radix::Five => chunks_mut(data, block).for_each(|b| butterfly5(b, stage, twiddles)),
        }
    }
}

/// `kf_bfly2` over one block of eight: it always follows a radix-4 stage of single points, so
/// its twiddles are the eighth roots of unity.
#[inline]
fn butterfly2(block: &mut [Complex]) {
    // QCONST16(0.7071067812f, 15)
    const TW: i16 = 23170;
    let Some((a, b)) = block.split_at_mut_checked(4) else {
        return;
    };
    let (Ok(a), Ok(b)) = (<&mut [Complex; 4]>::try_from(a), <&mut [Complex; 4]>::try_from(b)) else { return };
    let t = b[0];
    b[0] = c_sub(a[0], t);
    a[0] = c_add(a[0], t);

    let t = [s_mul(add32_ovflw(b[1][0], b[1][1]), TW), s_mul(sub32_ovflw(b[1][1], b[1][0]), TW)];
    b[1] = c_sub(a[1], t);
    a[1] = c_add(a[1], t);

    // The C's plain negation of an int32, which wraps as compiled.
    let t = [b[2][1], b[2][0].wrapping_neg()];
    b[2] = c_sub(a[2], t);
    a[2] = c_add(a[2], t);

    let t = [s_mul(sub32_ovflw(b[3][1], b[3][0]), TW), s_mul(neg32_ovflw(add32_ovflw(b[3][1], b[3][0])), TW)];
    b[3] = c_sub(a[3], t);
    a[3] = c_add(a[3], t);
}

/// `kf_bfly4` of single points, where every twiddle is 1.
#[inline]
fn butterfly4_single(block: &mut [Complex]) {
    let Ok(f) = <&mut [Complex; 4]>::try_from(block) else { return };
    let scratch0 = c_sub(f[0], f[2]);
    f[0] = c_add(f[0], f[2]);
    let scratch1 = c_add(f[1], f[3]);
    f[2] = c_sub(f[0], scratch1);
    f[0] = c_add(f[0], scratch1);
    let scratch1 = c_sub(f[1], f[3]);
    f[1] = [add32_ovflw(scratch0[0], scratch1[1]), sub32_ovflw(scratch0[1], scratch1[0])];
    f[3] = [sub32_ovflw(scratch0[0], scratch1[1]), add32_ovflw(scratch0[1], scratch1[0])];
}

/// `kf_bfly4` over one block of four sub-transforms of `m` points.
#[inline]
fn butterfly4(block: &mut [Complex], stage: &Stage, twiddles: &[Twiddle]) {
    let Some((f0, rest)) = block.split_at_mut_checked(stage.m) else {
        return;
    };
    let Some((f1, rest)) = rest.split_at_mut_checked(stage.m) else {
        return;
    };
    let Some((f2, f3)) = rest.split_at_mut_checked(stage.m) else {
        return;
    };
    let [s1, s2, s3, _] = stage.strides;
    let tw1 = twiddles.iter().step_by(s1.get());
    let tw2 = twiddles.iter().step_by(s2.get());
    let tw3 = twiddles.iter().step_by(s3.get());
    for (((((a, b), c), d), &w1), (&w2, &w3)) in
        f0.iter_mut().zip(f1.iter_mut()).zip(f2.iter_mut()).zip(f3.iter_mut()).zip(tw1).zip(tw2.zip(tw3))
    {
        let scratch0 = c_mul(*b, w1);
        let scratch1 = c_mul(*c, w2);
        let scratch2 = c_mul(*d, w3);

        let scratch5 = c_sub(*a, scratch1);
        *a = c_add(*a, scratch1);
        let scratch3 = c_add(scratch0, scratch2);
        let scratch4 = c_sub(scratch0, scratch2);
        *c = c_sub(*a, scratch3);
        *a = c_add(*a, scratch3);

        *b = [add32_ovflw(scratch5[0], scratch4[1]), sub32_ovflw(scratch5[1], scratch4[0])];
        *d = [sub32_ovflw(scratch5[0], scratch4[1]), add32_ovflw(scratch5[1], scratch4[0])];
    }
}

/// `kf_bfly3` over one block of three sub-transforms of `m` points.
#[inline]
fn butterfly3(block: &mut [Complex], stage: &Stage, twiddles: &[Twiddle]) {
    // The imaginary part of exp(-2 pi i / 3), Q15 (its real part, -1/2, is a halving).
    const EPI3_I: i16 = -28378;
    let Some((f0, rest)) = block.split_at_mut_checked(stage.m) else {
        return;
    };
    let Some((f1, f2)) = rest.split_at_mut_checked(stage.m) else {
        return;
    };
    let [s1, s2, _, _] = stage.strides;
    let tw1 = twiddles.iter().step_by(s1.get());
    let tw2 = twiddles.iter().step_by(s2.get());
    for ((((a, b), c), &w1), &w2) in f0.iter_mut().zip(f1.iter_mut()).zip(f2.iter_mut()).zip(tw1).zip(tw2) {
        let scratch1 = c_mul(*b, w1);
        let scratch2 = c_mul(*c, w2);
        let scratch3 = c_add(scratch1, scratch2);
        let scratch0 = c_sub(scratch1, scratch2);

        let half = [sub32_ovflw(a[0], scratch3[0] >> 1), sub32_ovflw(a[1], scratch3[1] >> 1)];
        let scratch0 = [s_mul(scratch0[0], EPI3_I), s_mul(scratch0[1], EPI3_I)];
        *a = c_add(*a, scratch3);
        *c = [add32_ovflw(half[0], scratch0[1]), sub32_ovflw(half[1], scratch0[0])];
        *b = [sub32_ovflw(half[0], scratch0[1]), add32_ovflw(half[1], scratch0[0])];
    }
}

/// `kf_bfly5` over one block of five sub-transforms of `m` points.
#[inline]
fn butterfly5(block: &mut [Complex], stage: &Stage, twiddles: &[Twiddle]) {
    // exp(-2 pi i / 5) and exp(-4 pi i / 5), Q15.
    const YA: Twiddle = (10126, -31164);
    const YB: Twiddle = (-26510, -19261);
    let Some((f0, rest)) = block.split_at_mut_checked(stage.m) else {
        return;
    };
    let Some((f1, rest)) = rest.split_at_mut_checked(stage.m) else {
        return;
    };
    let Some((f2, rest)) = rest.split_at_mut_checked(stage.m) else {
        return;
    };
    let Some((f3, f4)) = rest.split_at_mut_checked(stage.m) else {
        return;
    };
    let [s1, s2, s3, s4] = stage.strides;
    let tw1 = twiddles.iter().step_by(s1.get());
    let tw2 = twiddles.iter().step_by(s2.get());
    let tw3 = twiddles.iter().step_by(s3.get());
    let tw4 = twiddles.iter().step_by(s4.get());
    let points = f0.iter_mut().zip(f1.iter_mut()).zip(f2.iter_mut()).zip(f3.iter_mut()).zip(f4.iter_mut());
    for (((((p0, p1), p2), p3), p4), (((&w1, &w2), &w3), &w4)) in points.zip(tw1.zip(tw2).zip(tw3).zip(tw4)) {
        let s0 = *p0;
        let s1 = c_mul(*p1, w1);
        let s2 = c_mul(*p2, w2);
        let s3 = c_mul(*p3, w3);
        let s4 = c_mul(*p4, w4);

        let s7 = c_add(s1, s4);
        let s10 = c_sub(s1, s4);
        let s8 = c_add(s2, s3);
        let s9 = c_sub(s2, s3);

        *p0 = [add32_ovflw(s0[0], add32_ovflw(s7[0], s8[0])), add32_ovflw(s0[1], add32_ovflw(s7[1], s8[1]))];

        let s5 = [
            add32_ovflw(s0[0], add32_ovflw(s_mul(s7[0], YA.0), s_mul(s8[0], YB.0))),
            add32_ovflw(s0[1], add32_ovflw(s_mul(s7[1], YA.0), s_mul(s8[1], YB.0))),
        ];
        let s6 = [
            add32_ovflw(s_mul(s10[1], YA.1), s_mul(s9[1], YB.1)),
            neg32_ovflw(add32_ovflw(s_mul(s10[0], YA.1), s_mul(s9[0], YB.1))),
        ];
        *p1 = c_sub(s5, s6);
        *p4 = c_add(s5, s6);

        let s11 = [
            add32_ovflw(s0[0], add32_ovflw(s_mul(s7[0], YB.0), s_mul(s8[0], YA.0))),
            add32_ovflw(s0[1], add32_ovflw(s_mul(s7[1], YB.0), s_mul(s8[1], YA.0))),
        ];
        let s12 = [
            sub32_ovflw(s_mul(s9[1], YA.1), s_mul(s10[1], YB.1)),
            sub32_ovflw(s_mul(s10[0], YB.1), s_mul(s9[0], YA.1)),
        ];
        *p2 = c_add(s11, s12);
        *p3 = c_sub(s11, s12);
    }
}
