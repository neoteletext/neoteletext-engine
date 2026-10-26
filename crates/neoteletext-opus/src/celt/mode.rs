//! The layout of CELT's one mode (RFC 6716 §4.3): 48 kHz, 21 bands, 2.5 ms blocks of 120
//! samples overlapping by 120, and the frame durations the profile meets.

/// The mode's bands (`nbEBands`).
pub const NB_BANDS: usize = 21;
/// The overlap of consecutive MDCTs, and the length of `WINDOW`.
pub const OVERLAP: usize = 120;
/// The samples of a 2.5 ms block at 48 kHz (`shortMdctSize`).
pub const SHORT_BLOCK: usize = 120;
/// The samples of the longest frame, 20 ms at 48 kHz.
pub const MAX_FRAME: usize = SHORT_BLOCK << 3;
/// The largest band a frame holds: eight blocks of the widest band.
pub const MAX_BAND: usize = 8 * 22;
/// `BITRES`: allocations count eighths of a bit.
pub const BITRES: u32 = 3;

/// A frame's duration, as LM, the log2 of its 2.5 ms blocks (RFC 6716 §4.3). The profile meets
/// only these three; a 10 ms frame (LM 2) is refused.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names, reason = "each name carries its unit")]
pub enum FrameDuration {
    /// 2.5 ms, LM 0.
    TwoAndAHalfMs,
    /// 5 ms, LM 1.
    FiveMs,
    /// 20 ms, LM 3.
    TwentyMs,
}

impl FrameDuration {
    /// LM.
    #[inline]
    #[must_use]
    pub const fn lm(self) -> i32 {
        match self {
            Self::TwoAndAHalfMs => 0,
            Self::FiveMs => 1,
            Self::TwentyMs => 3,
        }
    }

    /// This duration's row of a table that keeps a row for each LM the profile meets.
    #[inline]
    #[must_use]
    pub const fn row<T>(self, rows: &[T; 3]) -> &T {
        let [two_and_a_half_ms, five_ms, twenty_ms] = rows;
        match self {
            Self::TwoAndAHalfMs => two_and_a_half_ms,
            Self::FiveMs => five_ms,
            Self::TwentyMs => twenty_ms,
        }
    }

    /// The 2.5 ms blocks of the frame, `1 << lm`.
    #[inline]
    #[must_use]
    pub const fn blocks(self) -> usize {
        match self {
            Self::TwoAndAHalfMs => 1,
            Self::FiveMs => 2,
            Self::TwentyMs => 8,
        }
    }

    /// The samples of the frame at 48 kHz.
    #[inline]
    #[must_use]
    pub const fn samples(self) -> usize {
        match self {
            Self::TwoAndAHalfMs => SHORT_BLOCK,
            Self::FiveMs => 2 * SHORT_BLOCK,
            Self::TwentyMs => 8 * SHORT_BLOCK,
        }
    }
}

// Each duration's blocks are `1 << lm`, and its samples a block's for each.
const _: () = {
    let mut durations: &[FrameDuration] =
        &[FrameDuration::TwoAndAHalfMs, FrameDuration::FiveMs, FrameDuration::TwentyMs];
    while let [duration, rest @ ..] = durations {
        assert!(duration.blocks() == 1 << duration.lm());
        assert!(duration.samples() == SHORT_BLOCK * duration.blocks());
        durations = rest;
    }
};
