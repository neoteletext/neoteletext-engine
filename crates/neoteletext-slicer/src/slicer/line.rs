//! One sampled line as the slicer receives it (REQ-SLC-001).

/// How the samples of a line are stored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SampleFormat {
    /// One unsigned byte per sample.
    U8,
    /// Two bytes per sample, unsigned, least significant byte first.
    U16Le,
}

impl SampleFormat {
    /// Bytes per sample.
    #[must_use]
    pub const fn bytes_per_sample(self) -> usize {
        match self {
            Self::U8 => 1,
            Self::U16Le => 2,
        }
    }
}

/// One sampled television line (REQ-SLC-001).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Line<'a> {
    /// The samples, in [`Line::format`].
    pub samples: &'a [u8],
    /// The sample format.
    pub format: SampleFormat,
    /// The sample rate in hertz.
    pub sample_rate_hz: u32,
    /// The television line number, where known.
    pub line: Option<u16>,
    /// The field, where known: `Some(false)` first, `Some(true)` second.
    pub second_field: Option<bool>,
    /// The time of the first sample after the half-amplitude point of the
    /// line synchronising pulse's leading edge, in nanoseconds, where known.
    /// Negative when the first sample comes before the edge.
    pub sync_offset_ns: Option<i32>,
    /// A time stamp in microseconds, copied to the packet and the trace.
    pub timestamp_us: Option<u64>,
}

impl<'a> Line<'a> {
    /// A line with only its samples, format and rate known.
    #[must_use]
    pub const fn new(samples: &'a [u8], format: SampleFormat, sample_rate_hz: u32) -> Self {
        Self {
            samples,
            format,
            sample_rate_hz,
            line: None,
            second_field: None,
            sync_offset_ns: None,
            timestamp_us: None,
        }
    }
}
