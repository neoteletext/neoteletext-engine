//! The mixer's fixed-point levels: 1.0 is [`UNITY`], and every change of
//! level is a ramp of at least [`MIN_RAMP_MS`].

/// Full level in the mixer's fixed point: 1.0 is 2¹⁶.
pub(crate) const UNITY: i32 = 1 << 16;
/// The shortest ramp, in milliseconds: tellyboy's `TB_AUDIO_DECLICK_MS`.
pub(crate) const MIN_RAMP_MS: u32 = 5;

/// A ramp from the current level to a target, in steps of one output
/// sample.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ramp {
    level: i32,
    target: i32,
    step: i32,
}

impl Ramp {
    pub(crate) const fn at(level: i32) -> Self {
        Self { level, target: level, step: 0 }
    }

    /// Ramps to `target` over `milliseconds` at `rate` samples a second.
    pub(crate) fn to(&mut self, target: i32, milliseconds: u32, rate: u32) {
        let samples =
            i32::try_from(milliseconds.max(MIN_RAMP_MS).saturating_mul(rate) / 1000).unwrap_or(i32::MAX).max(1);
        self.target = target;
        self.step = target.saturating_sub(self.level).saturating_abs().checked_div(samples).unwrap_or(0).max(1);
    }

    pub(crate) fn advance(&mut self) -> i32 {
        if self.level < self.target {
            self.level = self.level.saturating_add(self.step).min(self.target);
        } else if self.level > self.target {
            self.level = self.level.saturating_sub(self.step).max(self.target);
        }
        self.level
    }

    /// The current level.
    pub(crate) fn level(&self) -> i32 {
        self.level
    }

    /// The level it ramps to.
    pub(crate) fn target(&self) -> i32 {
        self.target
    }

    pub(crate) fn settled_at_zero(&self) -> bool {
        self.level == 0 && self.target == 0
    }
}

/// A whole percentage as a level.
pub(crate) fn level(percent: u8) -> i32 {
    i32::from(percent.min(100)).saturating_mul(UNITY) / 100
}

/// `value` at `level` (1.0 = [`UNITY`]).
pub(crate) fn scale(value: i32, level: i32) -> i32 {
    i32::try_from(i64::from(value).saturating_mul(i64::from(level)) >> 16).unwrap_or(0)
}
