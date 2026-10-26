//! The pitch post-filter (RFC 6716 §4.3.7.1): a comb filter at the pitch period, its gain and
//! taps cross-faded over the overlap from the last frame's to this frame's.

use crate::celt::tables::WINDOW;
use crate::fixed::{Q15ONE, SIG_SAT, add32, mult16_16_p15, mult16_16_q15, mult16_32_q15, saturate};

/// `COMBFILTER_MINPERIOD`: the shortest period, to which a zero period is raised.
pub const COMBFILTER_MINPERIOD: i32 = 15;

/// The taps of each tapset, Q15.
static TAPSETS: [[i16; 3]; 3] = [
    // QCONST16(0.3066406250f, 15), QCONST16(0.2170410156f, 15), QCONST16(0.1296386719f, 15)
    [10048, 7112, 4248],
    // QCONST16(0.4638671875f, 15), QCONST16(0.2680664062f, 15), QCONST16(0.f, 15)
    [15200, 8784, 0],
    // QCONST16(0.7998046875f, 15), QCONST16(0.1000976562f, 15), QCONST16(0.f, 15)
    [26208, 3280, 0],
];

/// A post-filter's period, gain (Q15) and tapset.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct CombParameters {
    /// The pitch period, in samples.
    pub period: i32,
    /// The gain, Q15.
    pub gain: i16,
    /// The row of `TAPSETS`.
    pub tapset: u8,
}

/// Where a comb filter reads and writes: in place (`y == x`), as the decoder post-filters, so
/// that each sample reads those already filtered; or apart, as the concealment folds.
pub trait CombSignal {
    /// The input `x[i]`, which reaches back before the output's start.
    fn x(&self, i: isize) -> i32;
    /// `y[i] = value`.
    fn set_y(&mut self, i: usize, value: i32);
    /// `y[from..from + n] = x[from..from + n]`, when the two are apart.
    fn copy_x_to_y(&mut self, from: usize, n: usize);
}

/// A comb filter in place on `buffer`, from `at`.
pub struct InPlace<'a> {
    /// The signal, filtered in place.
    pub buffer: &'a mut [i32],
    /// Where the output starts; the input reaches back before it.
    pub at: usize,
}

// The decoder's history always reaches back the longest period and two samples more, and on
// past the output: outside it, the input reads as silence and the output goes nowhere.
impl CombSignal for InPlace<'_> {
    #[inline]
    fn x(&self, i: isize) -> i32 {
        self.buffer.get(self.at.wrapping_add_signed(i)).copied().unwrap_or(0)
    }
    #[inline]
    fn set_y(&mut self, i: usize, value: i32) {
        if let Some(y) = self.at.checked_add(i).and_then(|at| self.buffer.get_mut(at)) {
            *y = value;
        }
    }
    #[inline]
    fn copy_x_to_y(&mut self, _from: usize, _n: usize) {}
}

/// A comb filter from `x`, at `x_at`, into `y`.
pub struct Apart<'a> {
    /// The output.
    pub y: &'a mut [i32],
    /// The input.
    pub x: &'a [i32],
    /// Where in `x` the output's first sample is; the input reaches back before it.
    pub x_at: usize,
}

// As in place: outside the input, silence; outside the output, nowhere.
impl CombSignal for Apart<'_> {
    #[inline]
    fn x(&self, i: isize) -> i32 {
        self.x.get(self.x_at.wrapping_add_signed(i)).copied().unwrap_or(0)
    }
    #[inline]
    fn set_y(&mut self, i: usize, value: i32) {
        if let Some(y) = self.y.get_mut(i) {
            *y = value;
        }
    }
    fn copy_x_to_y(&mut self, from: usize, n: usize) {
        let input = self.x.iter().skip(self.x_at).skip(from);
        for (y, &x) in self.y.iter_mut().skip(from).take(n).zip(input) {
            *y = x;
        }
    }
}

/// The input's index `period - offset` samples before `i`. It wraps: an index outside the input
/// reads as silence.
#[inline]
fn back(i: usize, period: i32, offset: i32) -> isize {
    i.cast_signed().wrapping_sub(period as isize).wrapping_add(offset as isize)
}

/// `comb_filter_const`: the comb filter at a constant period `t` and taps `g10`, `g11`, `g12`
/// over the `n` samples from `from`.
pub fn comb_filter_const<S: CombSignal>(s: &mut S, from: usize, t: i32, n: usize, g10: i16, g11: i16, g12: i16) {
    let (g10, g11, g12) = (i32::from(g10), i32::from(g11), i32::from(g12));
    let mut x4 = s.x(back(from, t, -2));
    let mut x3 = s.x(back(from, t, -1));
    let mut x2 = s.x(back(from, t, 0));
    let mut x1 = s.x(back(from, t, 1));
    for i in from..from.saturating_add(n) {
        let x0 = s.x(back(i, t, 2));
        let y = add32(
            add32(add32(s.x(i.cast_signed()), mult16_32_q15(g10, x2)), mult16_32_q15(g11, add32(x1, x3))),
            mult16_32_q15(g12, add32(x0, x4)),
        );
        s.set_y(i, saturate(y, SIG_SAT));
        x4 = x3;
        x3 = x2;
        x2 = x1;
        x1 = x0;
    }
}

/// `comb_filter`: the post-filter over `n` samples, cross-faded over the first `overlap` (0 or
/// up to the window's 120) from the filter `old` to the filter `new`. The input reaches back
/// the longer period and two samples more.
#[allow(
    clippy::many_single_char_names,
    reason = "the filter's taps and samples, named as RFC 6716 §4.3.7.1 names them"
)]
pub fn comb_filter<S: CombSignal>(s: &mut S, old: CombParameters, new: CombParameters, n: usize, overlap: usize) {
    if old.gain == 0 && new.gain == 0 {
        s.copy_x_to_y(0, n);
        return;
    }
    // A zero gain comes with a zero period: raise both to the least, so as never to read past
    // the history.
    let t0 = old.period.max(COMBFILTER_MINPERIOD);
    let t1 = new.period.max(COMBFILTER_MINPERIOD);
    // A tapset is one of the three a frame can code.
    let taps0 = TAPSETS.get(usize::from(old.tapset)).unwrap_or(&TAPSETS[0]);
    let taps1 = TAPSETS.get(usize::from(new.tapset)).unwrap_or(&TAPSETS[0]);
    let tap = |gain: i16, tap: i16| mult16_16_p15(i32::from(gain), i32::from(tap)) as i16;
    let [g00, g01, g02] = taps0.map(|t| i32::from(tap(old.gain, t)));
    let [g10, g11, g12] = taps1.map(|t| tap(new.gain, t));
    // An unchanged filter needs no cross-fade.
    let overlap = if old.gain == new.gain && t0 == t1 && old.tapset == new.tapset { 0 } else { overlap.min(n) };
    let (f10, f11, f12) = (i32::from(g10), i32::from(g11), i32::from(g12));
    let mut x1 = s.x(back(0, t1, 1));
    let mut x2 = s.x(back(0, t1, 0));
    let mut x3 = s.x(back(0, t1, -1));
    let mut x4 = s.x(back(0, t1, -2));
    for (i, &w) in WINDOW.iter().enumerate().take(overlap) {
        let x0 = s.x(back(i, t1, 2));
        let w = i32::from(w);
        let f = i32::from(mult16_16_q15(w, w) as i16);
        let one_minus_f = i32::from(Q15ONE).wrapping_sub(f);
        let mut y = s.x(i.cast_signed());
        y = add32(y, mult16_32_q15(mult16_16_q15(one_minus_f, g00), s.x(back(i, t0, 0))));
        y = add32(y, mult16_32_q15(mult16_16_q15(one_minus_f, g01), add32(s.x(back(i, t0, 1)), s.x(back(i, t0, -1)))));
        y = add32(y, mult16_32_q15(mult16_16_q15(one_minus_f, g02), add32(s.x(back(i, t0, 2)), s.x(back(i, t0, -2)))));
        y = add32(y, mult16_32_q15(mult16_16_q15(f, f10), x2));
        y = add32(y, mult16_32_q15(mult16_16_q15(f, f11), add32(x1, x3)));
        y = add32(y, mult16_32_q15(mult16_16_q15(f, f12), add32(x0, x4)));
        s.set_y(i, saturate(y, SIG_SAT));
        x4 = x3;
        x3 = x2;
        x2 = x1;
        x1 = x0;
    }
    if new.gain == 0 {
        s.copy_x_to_y(overlap, n.saturating_sub(overlap));
        return;
    }
    // The rest, at the constant new filter.
    comb_filter_const(s, overlap, t1, n.saturating_sub(overlap), g10, g11, g12);
}
