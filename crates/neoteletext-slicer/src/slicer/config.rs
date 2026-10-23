//! The slicer's settings.

use crate::consts::{
    DEFAULT_MIN_AMPLITUDE_PERCENT, DEFAULT_MIN_MATCH_PERCENT, NOMINAL_AMPLITUDE_8BIT, NOMINAL_AMPLITUDE_16BIT,
};

use super::line::SampleFormat;

/// Slicer settings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Config {
    /// First sample at which the run-in may start (REQ-SLC-004).
    pub search_start: usize,
    /// Last sample at which the run-in may start; `None` for the whole line.
    pub search_end: Option<usize>,
    /// Minimum data amplitude, percent of nominal (REQ-SLC-005).
    pub min_amplitude_percent: u16,
    /// Minimum run-in match quality, percent (REQ-SLC-005).
    pub min_match_percent: u8,
    /// Accept a framing code with one wrong bit when the address bytes
    /// decode (decision E.24). `false` is the "strict" option.
    pub framing_tolerance: bool,
    /// The nominal data amplitude of 8-bit samples: the "1" level above
    /// black, in sample codes.
    pub nominal_amplitude_8bit: f32,
    /// The nominal data amplitude of 16-bit samples, in sample codes.
    pub nominal_amplitude_16bit: f32,
    /// Report every transition (REQ-FSM-010).
    pub trace: bool,
}

impl Config {
    /// The defaults: whole-line search, the REQ-SLC-005 minimums, framing
    /// tolerance on (decision E.24), BT.601 levels, no trace.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            search_start: 0,
            search_end: None,
            min_amplitude_percent: DEFAULT_MIN_AMPLITUDE_PERCENT,
            min_match_percent: DEFAULT_MIN_MATCH_PERCENT,
            framing_tolerance: true,
            nominal_amplitude_8bit: NOMINAL_AMPLITUDE_8BIT,
            nominal_amplitude_16bit: NOMINAL_AMPLITUDE_16BIT,
            trace: false,
        }
    }

    pub(super) const fn nominal_amplitude(&self, format: SampleFormat) -> f32 {
        match format {
            SampleFormat::U8 => self.nominal_amplitude_8bit,
            SampleFormat::U16Le => self.nominal_amplitude_16bit,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}
