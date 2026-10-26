//! The shape of a SILK frame: the internal and output rates, the frame duration, and the
//! lengths, orders and signal types that follow from them (RFC 6716 §4.2.2, §4.2.7.3).

/// The highest internal rate, in kHz.
pub const MAX_INTERNAL_KHZ: usize = 16;
/// `SUB_FRAME_LENGTH_MS`.
pub const SUBFRAME_MS: usize = 5;
/// `MAX_NB_SUBFR`: the subframes of a 20 ms frame.
pub const MAX_SUBFRAMES: usize = 4;
/// `MAX_SUB_FRAME_LENGTH`.
pub const MAX_SUBFRAME_LENGTH: usize = SUBFRAME_MS * MAX_INTERNAL_KHZ;
/// `MAX_FRAME_LENGTH`: 20 ms at 16 kHz.
pub const MAX_FRAME_LENGTH: usize = MAX_SUBFRAMES * MAX_SUBFRAME_LENGTH;
/// `LTP_MEM_LENGTH_MS`: the history the long-term predictor reaches into.
pub const LTP_MEMORY_MS: usize = 20;
/// The longest LTP memory, 20 ms at 16 kHz.
pub const MAX_LTP_MEMORY: usize = LTP_MEMORY_MS * MAX_INTERNAL_KHZ;
/// `MAX_LPC_ORDER`.
pub const MAX_LPC_ORDER: usize = 16;
/// `LTP_ORDER`: the taps of the long-term predictor.
pub const LTP_ORDER: usize = 5;
/// `PE_MIN_LAG_MS`, `PE_MAX_LAG_MS`: the range of the pitch lag (RFC 6716 §4.2.7.6.1).
pub const MIN_PITCH_LAG_MS: i32 = 2;
pub const MAX_PITCH_LAG_MS: i32 = 18;

/// The internal sampling rate of a SILK frame, set by the Opus bandwidth (RFC 6716 §2.1.3):
/// narrowband 8 kHz, mediumband 12 kHz, wideband (and every Hybrid frame) 16 kHz.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InternalRate {
    /// 8 kHz.
    Narrowband,
    /// 12 kHz.
    Mediumband,
    /// 16 kHz.
    Wideband,
}

impl InternalRate {
    /// The rate in kHz.
    #[inline]
    #[must_use]
    pub const fn khz(self) -> i32 {
        match self {
            Self::Narrowband => 8,
            Self::Mediumband => 12,
            Self::Wideband => 16,
        }
    }

    /// The samples in 1 ms.
    #[inline]
    #[must_use]
    pub const fn samples_per_ms(self) -> usize {
        match self {
            Self::Narrowband => 8,
            Self::Mediumband => 12,
            Self::Wideband => 16,
        }
    }

    /// The order of the LPC filter (RFC 6716 §4.2.7.5): 10 below wideband, 16 at it.
    #[inline]
    #[must_use]
    pub const fn lpc_order(self) -> LpcOrder {
        match self {
            Self::Narrowband | Self::Mediumband => LpcOrder::Ten,
            Self::Wideband => LpcOrder::Sixteen,
        }
    }
}

/// The output rate, in kHz: the profile's only one, 48 kHz.
pub const OUTPUT_KHZ: usize = 48;

/// The duration of a frame: every received frame is 20 ms; concealment also asks for 10 ms
/// (at a SILK-to-CELT transition).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FrameDuration {
    /// 10 ms: two subframes.
    Ms10,
    /// 20 ms: four subframes.
    Ms20,
}

impl FrameDuration {
    /// The samples of the duration at the output rate.
    #[inline]
    #[must_use]
    pub const fn output_samples(self) -> usize {
        match self {
            Self::Ms10 => 10 * OUTPUT_KHZ,
            Self::Ms20 => 20 * OUTPUT_KHZ,
        }
    }
}

/// The order of the LPC filter.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LpcOrder {
    /// Narrowband and mediumband.
    Ten,
    /// Wideband.
    Sixteen,
}

impl LpcOrder {
    /// The number of coefficients.
    #[inline]
    #[must_use]
    pub const fn coefficients(self) -> usize {
        match self {
            Self::Ten => 10,
            Self::Sixteen => 16,
        }
    }
}

/// The signal type of a frame (RFC 6716 §4.2.7.3), `TYPE_NO_VOICE_ACTIVITY`, `TYPE_UNVOICED`,
/// `TYPE_VOICED`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SignalType {
    /// No voice activity.
    Inactive,
    /// Voice activity without a pitch.
    Unvoiced,
    /// Voice activity with a pitch: the long-term predictor runs.
    Voiced,
}

impl SignalType {
    /// The row of a table with one for each signal type, in the C's order.
    #[inline]
    #[must_use]
    pub const fn select<T>(self, rows: &[T; 3]) -> &T {
        let [inactive, unvoiced, voiced] = rows;
        match self {
            Self::Inactive => inactive,
            Self::Unvoiced => unvoiced,
            Self::Voiced => voiced,
        }
    }

    /// The row of a table with one for unvoiced frames (with or without voice activity), then
    /// one for voiced: the C's `signalType >> 1`.
    #[inline]
    #[must_use]
    pub const fn select_by_voicing<T>(self, rows: &[T; 2]) -> &T {
        let [unvoiced, voiced] = rows;
        match self {
            Self::Inactive | Self::Unvoiced => unvoiced,
            Self::Voiced => voiced,
        }
    }
}

/// The quantisation offset type of a frame (RFC 6716 §4.2.7.3), which with the signal type
/// selects the excitation's offset and its signs' probabilities.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QuantOffsetType {
    /// Type 0.
    Low,
    /// Type 1.
    High,
}

impl QuantOffsetType {
    /// The row of a table with one for each type, in the C's order.
    #[inline]
    #[must_use]
    pub const fn select<T>(self, rows: &[T; 2]) -> &T {
        let [low, high] = rows;
        match self {
            Self::Low => low,
            Self::High => high,
        }
    }
}

/// The lengths of a frame at one rate and duration.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub rate: InternalRate,
    pub duration: FrameDuration,
}

impl Layout {
    /// The 5 ms subframes: 2 or 4.
    #[inline]
    #[must_use]
    pub const fn subframes(self) -> usize {
        match self.duration {
            FrameDuration::Ms10 => 10 / SUBFRAME_MS,
            FrameDuration::Ms20 => 20 / SUBFRAME_MS,
        }
    }

    /// The samples of a subframe.
    #[inline]
    #[must_use]
    pub const fn subframe_length(self) -> usize {
        match self.rate {
            InternalRate::Narrowband => SUBFRAME_MS * 8,
            InternalRate::Mediumband => SUBFRAME_MS * 12,
            InternalRate::Wideband => SUBFRAME_MS * 16,
        }
    }

    /// The samples of the frame.
    #[inline]
    #[must_use]
    pub const fn frame_length(self) -> usize {
        match (self.duration, self.rate) {
            (FrameDuration::Ms10, InternalRate::Narrowband) => 10 * 8,
            (FrameDuration::Ms10, InternalRate::Mediumband) => 10 * 12,
            // 160 samples both
            (FrameDuration::Ms10, InternalRate::Wideband) | (FrameDuration::Ms20, InternalRate::Narrowband) => 10 * 16,
            (FrameDuration::Ms20, InternalRate::Mediumband) => 20 * 12,
            (FrameDuration::Ms20, InternalRate::Wideband) => 20 * 16,
        }
    }

    /// The samples of the LTP memory: 20 ms.
    #[inline]
    #[must_use]
    pub const fn ltp_memory_length(self) -> usize {
        match self.rate {
            InternalRate::Narrowband => LTP_MEMORY_MS * 8,
            InternalRate::Mediumband => LTP_MEMORY_MS * 12,
            InternalRate::Wideband => LTP_MEMORY_MS * 16,
        }
    }
}
