//! The software data slicer (area SLC): one sampled line in, at most one
//! packet out.
//!
//! [`Slicer::slice`] runs the line slicer machine of REQ-FSM-002 once per
//! line. The machine is in [`crate::fsm`]; this module supplies the events
//! (what each state measured) and performs the actions.
//!
//! The work for one line is:
//!
//! 1. Convert the samples and estimate the black level (REQ-SLC-003), from
//!    the back porch when the synchronising edge is known, otherwise from the
//!    most common level between the synchronising pulse and the middle of
//!    the line's range. Samples are held relative to it.
//! 2. Search for the run-in (REQ-SLC-004) with a sliding single-frequency
//!    correlation at half the bit rate over 14 bit periods: the alternating
//!    run-in bits 3 to 16 are a tone at that frequency, so the first window
//!    whose tone is strong and dominant is the run-in. Bits 1 and 2 are not
//!    needed. The tone's phase gives the bit phase to a fraction of a
//!    sample.
//! 3. Put the threshold at the mean of the 14 run-in bit-centre values
//!    (midway between "0" and "1"), take the data amplitude from the tone's
//!    strength, and score how well the bit centres match the pattern.
//! 4. Gate on amplitude and match (REQ-SLC-005), then hold the threshold.
//! 5. Look for the framing code within ±2 bits (REQ-SLC-008, decision E.24).
//! 6. Read 336 bits at bit centres with a fixed bit period of
//!    sample rate / 6,937,500 (REQ-SLC-006 Phase 1), interpolating linearly
//!    between samples (REQ-SLC-007), first bit as bit 0 (REQ-SLC-009).
//! 7. Check both address bytes with Hamming 8/4 (REQ-SLC-010).
//!
//! Floating point is used for the signal measurements (REQ-GEN-016 allows
//! it in the slicer). The state machine, the counters and byte assembly are
//! integer; the fixed-point form of REQ-SLC-015 (Phase 2) would replace only
//! the measuring functions and [`crate::math`].

use neoteletext_core::consts::{ADDRESS_BYTE_0, ADDRESS_BYTE_1, BIT_RATE_HZ, FRAMING_CODE, PACKET_BITS, PACKET_BYTES};
use neoteletext_core::ecc::hamming84;
use neoteletext_core::packet::{Packet, PacketMeta, SliceQuality};
use neoteletext_core::trace::{Machine, TraceEntry};
use neoteletext_core::types::Instant;

use crate::consts::{
    BACK_PORCH_GUARD_NS, BITS_PER_BYTE, BLACK_HIGH_PERCENTILE, BLACK_HISTOGRAM_BINS, BLACK_HISTOGRAM_SMOOTH,
    BLACK_LOW_PERCENTILE, BLACK_REFINE_BINS, BURST_CYCLES_MAX, BURST_START_NS, DEFAULT_MIN_AMPLITUDE_PERCENT,
    DEFAULT_MIN_MATCH_PERCENT, DETECT_AMPLITUDE_FRACTION, DETECT_CONCENTRATION, FRAMING_BITS, FRAMING_SEARCH_BITS,
    MAX_LINE_SAMPLES, MIN_SAMPLE_RATE_HZ, NOMINAL_AMPLITUDE_8BIT, NOMINAL_AMPLITUDE_16BIT, PAL_SUBCARRIER_HZ,
    PEAK_FOLLOW_BITS, PERCENT, PHASOR_RENORMALISE_STEPS, RUN_IN_MATCH_BITS, TIMING_EARLY_NS, TIMING_REFERENCE_BIT,
    TIMING_REFERENCE_NS,
};
use crate::fsm::{SlicerAction, SlicerEvent, SlicerState, slicer_transition};
use crate::math::{
    atan2, ceil_index, f32_from_i32, f32_from_u32, f32_from_usize, floor_index, round_i32, sin_cos, sqrt,
    to_u8_saturating, to_u16_saturating,
};

/// Nanoseconds per second, for converting times to sample positions.
const NS_PER_SECOND: f32 = 1e9;
/// Bits read around the expected framing code: the code and two bits
/// either side (REQ-SLC-008).
const FRAMING_WINDOW_BITS: usize = FRAMING_BITS + 2 * FRAMING_SEARCH_BITS;
/// Bit periods from the start of run-in bit 3 to the end of the last packet
/// bit when the framing code is as late as the search allows.
const SPAN_FROM_BIT_3: usize = RUN_IN_MATCH_BITS + FRAMING_BITS + PACKET_BITS + FRAMING_SEARCH_BITS;
/// One more bit period than [`SPAN_FROM_BIT_3`]: the bit phase chosen from
/// the run-in tone may put bit 3 up to one period after the window start.
const SPAN_WITH_PHASE_BITS: usize = SPAN_FROM_BIT_3 + 1;
/// Samples needed after the last bit centre for linear interpolation, plus
/// one for rounding of the span.
const INTERPOLATION_GUARD_SAMPLES: usize = 2;
/// Transitions in one run of the machine are at most one per non-final
/// state, of which there are seven; the driver loop is bounded by this,
/// with one more pass to see the final state.
const MAX_TRANSITIONS: usize = 8;

/// The order in which framing code offsets are tried: nearest first.
const FRAMING_OFFSETS: [i32; 2 * FRAMING_SEARCH_BITS + 1] = framing_offsets();

// Evaluated only in the constant above, where overflow or an index out of
// range is a compile error, not a run-time panic.
#[allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
const fn framing_offsets() -> [i32; 2 * FRAMING_SEARCH_BITS + 1] {
    let mut offsets = [0; 2 * FRAMING_SEARCH_BITS + 1];
    let mut distance = 1;
    let mut index = 1;
    while distance <= FRAMING_SEARCH_BITS {
        // The search reach is a small constant, so the conversion is exact.
        #[allow(clippy::cast_possible_wrap)]
        let signed = distance as i32;
        offsets[index] = -signed;
        offsets[index + 1] = signed;
        index += 2;
        distance += 1;
    }
    offsets
}

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

    const fn nominal_amplitude(&self, format: SampleFormat) -> f32 {
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

/// Counts of lines by outcome (REQ-FSM-002 actions).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Counters {
    /// Lines received.
    pub lines: u32,
    /// Packets output.
    pub accepted: u32,
    /// Lines where no run-in was found.
    pub no_signal: u32,
    /// Lines below the amplitude or match minimum.
    pub below_gate: u32,
    /// Lines without an acceptable framing code.
    pub no_framing_code: u32,
    /// Lines with an uncorrectable address byte.
    pub bad_address: u32,
    /// Packets output whose framing code needed a correction.
    pub framing_corrected: u32,
}

/// Why a line gave no packet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rejection {
    /// No run-in in the search window.
    NoSignal,
    /// Amplitude or match below the minimum.
    BelowGate,
    /// No acceptable framing code.
    NoFramingCode,
    /// An address byte was uncorrectable.
    BadAddress,
}

/// A packet recovered from a line, with its metadata and quality.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sliced {
    /// The packet.
    pub packet: Packet,
    /// Line, field, time and slicer quality (REQ-SLC-012).
    pub meta: PacketMeta,
}

/// Values that depend only on the sample rate, kept between lines.
#[derive(Clone, Copy, PartialEq, Debug)]
struct RateParameters {
    sample_rate_hz: u32,
    /// Samples per bit.
    samples_per_bit: f32,
    /// Samples in the run-in match window: 14 bit periods.
    window: usize,
    /// Angular frequency of half the bit rate, radians per sample.
    omega: f32,
    /// The phasor step e^(−jω).
    step: (f32, f32),
    /// e^(−jωL), from a window's first sample to the one after its last.
    window_step: (f32, f32),
    /// Σ e^(−jωk) for k in 0 to L−1, for removing the window's mean.
    window_sum: (f32, f32),
    /// Samples from the window start that must exist for a whole packet.
    needed: usize,
}

impl RateParameters {
    fn new(sample_rate_hz: u32) -> Option<Self> {
        if sample_rate_hz < MIN_SAMPLE_RATE_HZ {
            return None;
        }
        let samples_per_bit = f32_from_u32(sample_rate_hz) / f32_from_u32(BIT_RATE_HZ);
        let window = floor_index(f32_from_usize(RUN_IN_MATCH_BITS) * samples_per_bit + 0.5);
        let omega = core::f32::consts::PI / samples_per_bit;
        let (sine, cosine) = sin_cos(omega);
        let step = (cosine, -sine);
        let (sine_l, cosine_l) = sin_cos(omega * f32_from_usize(window));
        let window_step = (cosine_l, -sine_l);
        let mut window_sum = (0.0, 0.0);
        let mut phasor = (1.0, 0.0);
        for _ in 0..window {
            window_sum = (window_sum.0 + phasor.0, window_sum.1 + phasor.1);
            phasor = complex_multiply(phasor, step);
        }
        let needed = ceil_index(f32_from_usize(SPAN_WITH_PHASE_BITS) * samples_per_bit)
            .saturating_add(INTERPOLATION_GUARD_SAMPLES);
        Some(Self { sample_rate_hz, samples_per_bit, window, omega, step, window_step, window_sum, needed })
    }
}

fn complex_multiply(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

/// The best run-in candidate of the search.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Candidate {
    /// First sample of the match window.
    start: usize,
    /// The window's tone at half the bit rate, phase relative to `start`.
    tone: (f32, f32),
}

/// Measurements of one line, filled in as the machine runs.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Work {
    candidate: Candidate,
    /// Position of the centre of run-in bit 3, in samples.
    bit3_centre: f32,
    one_level: f32,
    zero_level: f32,
    threshold: f32,
    held_threshold: f32,
    amplitude_percent: u16,
    match_percent: u8,
    /// The framing code offset from its expected position, in bits.
    framing_offset: i32,
    framing_corrected: bool,
    bytes: [u8; PACKET_BYTES],
    packet: Option<Packet>,
}

impl Work {
    const fn new() -> Self {
        Self {
            candidate: Candidate { start: 0, tone: (0.0, 0.0) },
            bit3_centre: 0.0,
            one_level: 0.0,
            zero_level: 0.0,
            threshold: 0.0,
            held_threshold: 0.0,
            amplitude_percent: 0,
            match_percent: 0,
            framing_offset: 0,
            framing_corrected: false,
            bytes: [0; PACKET_BYTES],
            packet: None,
        }
    }
}

/// The software data slicer.
#[derive(Clone, PartialEq, Debug)]
pub struct Slicer {
    config: Config,
    counters: Counters,
    rate: Option<RateParameters>,
    /// The current line's samples, relative to its black level.
    samples: [f32; MAX_LINE_SAMPLES],
    len: usize,
    /// The current line's sample format.
    format: SampleFormat,
    /// The current line's black level, in sample codes.
    black: f32,
}

impl Slicer {
    /// A slicer with the given settings.
    #[must_use]
    pub const fn new(config: Config) -> Self {
        Self {
            config,
            counters: Counters {
                lines: 0,
                accepted: 0,
                no_signal: 0,
                below_gate: 0,
                no_framing_code: 0,
                bad_address: 0,
                framing_corrected: 0,
            },
            rate: None,
            samples: [0.0; MAX_LINE_SAMPLES],
            len: 0,
            format: SampleFormat::U8,
            black: 0.0,
        }
    }

    /// The settings.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Changes the settings.
    pub const fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    /// The counters since creation or the last [`Slicer::reset_counters`].
    #[must_use]
    pub const fn counters(&self) -> Counters {
        self.counters
    }

    /// The black level estimated for the last line (REQ-SLC-003), in sample
    /// codes of its format.
    #[must_use]
    pub const fn black_level(&self) -> f32 {
        self.black
    }

    /// Sets every counter to zero.
    pub fn reset_counters(&mut self) {
        self.counters = Counters::default();
    }

    /// Slices one line.
    pub fn slice(&mut self, line: &Line<'_>) -> Result<Sliced, Rejection> {
        self.slice_traced(line, |_| {})
    }

    /// Slices one line, passing each transition to `trace` when tracing is
    /// on in the settings (REQ-FSM-010).
    pub fn slice_traced<F: FnMut(TraceEntry)>(&mut self, line: &Line<'_>, mut trace: F) -> Result<Sliced, Rejection> {
        let mut state = SlicerState::Start;
        let mut work = Work::new();
        let mut outcome = Err(Rejection::NoSignal);
        for _ in 0..MAX_TRANSITIONS {
            let event = match state {
                SlicerState::Start => SlicerEvent::LineReceived,
                SlicerState::SearchingRunIn => self.search(&mut work),
                SlicerState::Measuring => self.measure(&mut work),
                SlicerState::GateCheck => self.gate(&work),
                SlicerState::LookingForFramingCode => self.find_framing_code(&mut work),
                SlicerState::CollectingBytes => self.collect(&mut work),
                SlicerState::CheckingAddress => check_address(&work),
                SlicerState::Accepted | SlicerState::Rejected => break,
            };
            let (next, action) = slicer_transition(state, event);
            self.perform(action, line, &mut work, &mut outcome);
            if self.config.trace {
                trace(TraceEntry {
                    machine: Machine::Slicer,
                    time: Instant(line.timestamp_us.unwrap_or(0)),
                    from: state.name(),
                    event: event.name(),
                    action: action.name(),
                    to: next.name(),
                });
            }
            state = next;
        }
        outcome
    }

    fn perform(
        &mut self,
        action: SlicerAction,
        line: &Line<'_>,
        work: &mut Work,
        outcome: &mut Result<Sliced, Rejection>,
    ) {
        let counters = &mut self.counters;
        match action {
            SlicerAction::None => {}
            SlicerAction::EstimateBlackLevel => {
                counters.lines = counters.lines.saturating_add(1);
                self.load(line);
            }
            SlicerAction::RecordPosition => {
                work.bit3_centre = self.bit3_centre(work.candidate);
            }
            SlicerAction::CountNoSignal => {
                counters.no_signal = counters.no_signal.saturating_add(1);
                *outcome = Err(Rejection::NoSignal);
            }
            SlicerAction::HoldThreshold => work.held_threshold = work.threshold,
            SlicerAction::CountBelowGate => {
                counters.below_gate = counters.below_gate.saturating_add(1);
                *outcome = Err(Rejection::BelowGate);
            }
            SlicerAction::MarkFramingCorrected => work.framing_corrected = true,
            SlicerAction::CountNoFramingCode => {
                counters.no_framing_code = counters.no_framing_code.saturating_add(1);
                *outcome = Err(Rejection::NoFramingCode);
            }
            SlicerAction::AssembleBytes => work.packet = Some(Packet { bytes: work.bytes }),
            SlicerAction::OutputPacket => {
                counters.accepted = counters.accepted.saturating_add(1);
                if work.framing_corrected {
                    counters.framing_corrected = counters.framing_corrected.saturating_add(1);
                }
                let packet = work.packet.unwrap_or(Packet { bytes: work.bytes });
                let quality = SliceQuality {
                    amplitude_percent: work.amplitude_percent,
                    match_percent: work.match_percent,
                    position: self.run_in_position(work),
                    framing_corrected: work.framing_corrected,
                };
                let meta = PacketMeta {
                    line: line.line,
                    second_field: line.second_field,
                    timestamp_us: line.timestamp_us,
                    source: None,
                    quality: Some(quality),
                };
                *outcome = Ok(Sliced { packet, meta });
            }
            SlicerAction::CountBadAddress => {
                counters.bad_address = counters.bad_address.saturating_add(1);
                *outcome = Err(Rejection::BadAddress);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Black level (REQ-SLC-003)
    // -----------------------------------------------------------------------

    /// Converts the line's samples, estimates its black level and stores the
    /// samples relative to it.
    fn load(&mut self, line: &Line<'_>) {
        if self.rate.is_none_or(|rate| rate.sample_rate_hz != line.sample_rate_hz) {
            self.rate = RateParameters::new(line.sample_rate_hz);
        }
        let mut histogram = Histogram::new();
        let mut len = 0_usize;
        match line.format {
            SampleFormat::U8 => {
                for (slot, &sample) in self.samples.iter_mut().zip(line.samples) {
                    histogram.add(usize::from(sample), f32::from(sample));
                    *slot = f32::from(sample);
                    len = len.saturating_add(1);
                }
            }
            SampleFormat::U16Le => {
                for (slot, pair) in self.samples.iter_mut().zip(line.samples.as_chunks::<2>().0) {
                    // REQ-GEN-007: assembled as the format names it,
                    // whatever the host's byte order.
                    let sample = u16::from_le_bytes(*pair);
                    histogram.add(usize::from(sample >> u8::BITS), f32::from(sample));
                    *slot = f32::from(sample);
                    len = len.saturating_add(1);
                }
            }
        }
        self.len = len;
        self.format = line.format;
        let back_porch = self.back_porch_black(line);
        let black = back_porch.unwrap_or_else(|| histogram.black());
        self.black = black;
        for sample in self.samples.iter_mut().take(len) {
            *sample -= black;
        }
    }

    /// The mean of the back porch, between the colour burst and the earliest
    /// data, when the synchronising edge is known and the window is inside
    /// the line.
    fn back_porch_black(&self, line: &Line<'_>) -> Option<f32> {
        let offset = f32_from_i32(line.sync_offset_ns?);
        let rate = self.rate?;
        let bit_ns = NS_PER_SECOND / f32_from_u32(BIT_RATE_HZ);
        let burst_end = f32_from_u32(BURST_START_NS)
            + f32_from_u32(BURST_CYCLES_MAX) * NS_PER_SECOND / PAL_SUBCARRIER_HZ
            + f32_from_u32(BACK_PORCH_GUARD_NS);
        // The earliest reference bit centre, back to the start of bit 1 and
        // one more bit for the pulse's precursor.
        let earliest_data =
            f32_from_u32(TIMING_REFERENCE_NS - TIMING_EARLY_NS) - (f32_from_usize(TIMING_REFERENCE_BIT) + 0.5) * bit_ns;
        let to_sample = |ns: f32| (ns - offset) * f32_from_u32(rate.sample_rate_hz) / NS_PER_SECOND;
        let first = to_sample(burst_end);
        let last = to_sample(earliest_data);
        if first < 0.0 || last <= first {
            return None;
        }
        let (first, last) = (ceil_index(first), floor_index(last));
        let window = self.samples.get(first..=last.min(self.len.checked_sub(1)?))?;
        if window.is_empty() {
            return None;
        }
        Some(window.iter().sum::<f32>() / f32_from_usize(window.len()))
    }

    // -----------------------------------------------------------------------
    // Run-in search (REQ-SLC-004)
    // -----------------------------------------------------------------------

    fn search(&self, work: &mut Work) -> SlicerEvent {
        let Some(rate) = self.rate else { return SlicerEvent::WindowExhausted };
        let first = self.config.search_start;
        let mut last = self.len.saturating_sub(rate.needed);
        if let Some(end) = self.config.search_end {
            last = last.min(end);
        }
        if first > last || self.len < rate.needed {
            return SlicerEvent::WindowExhausted;
        }
        match self.find_run_in(&rate, first, last, self.config.nominal_amplitude(self.format)) {
            Some(candidate) => {
                work.candidate = candidate;
                SlicerEvent::RunInFound
            }
            None => SlicerEvent::WindowExhausted,
        }
    }

    /// Slides the 14-bit match window from `first` to `last` and returns the
    /// strongest window near the first one that passes the detector.
    fn find_run_in(&self, rate: &RateParameters, first: usize, last: usize, nominal: f32) -> Option<Candidate> {
        let samples = self.samples.get(..self.len)?;
        let window = rate.window;
        let window_f = f32_from_usize(window);
        // A run-in of "1" level `a` above "0" is a tone of amplitude a/2,
        // whose correlation magnitude over the window is (a/2)(L/2).
        let floor_amplitude =
            nominal * f32::from(self.config.min_amplitude_percent) / PERCENT * DETECT_AMPLITUDE_FRACTION;
        let floor_magnitude = floor_amplitude / 2.0 * window_f / 2.0;
        let energy_floor = floor_magnitude * floor_magnitude;
        let follow = ceil_index(f32_from_usize(PEAK_FOLLOW_BITS) * rate.samples_per_bit);

        // The window's sums at `first`.
        let (sine, cosine) = sin_cos(rate.omega * f32_from_usize(first));
        let mut phasor = (cosine, -sine);
        let mut tone = (0.0_f32, 0.0_f32);
        let mut sum = 0.0_f32;
        let mut sum_squares = 0.0_f32;
        let mut running = phasor;
        for &value in samples.get(first..first.checked_add(window)?)? {
            tone = (tone.0 + value * running.0, tone.1 + value * running.1);
            sum += value;
            sum_squares += value * value;
            running = complex_multiply(running, rate.step);
        }
        let mut ahead = complex_multiply(phasor, rate.window_step);

        let mut best: Option<(f32, Candidate)> = None;
        let mut stop_after = last;
        let outgoing = samples.get(first..)?.iter();
        let incoming = samples.get(first.checked_add(window)?..)?.iter();
        for (step, (&old, &new)) in
            outgoing.zip(incoming).enumerate().take(last.saturating_sub(first).saturating_add(1))
        {
            let position = first.saturating_add(step);
            // The tone relative to the window start, with the window's mean
            // removed so the black level and the data's average cannot leak
            // into it.
            let relative = complex_multiply(tone, (phasor.0, -phasor.1));
            let mean = sum / window_f;
            let centred = (relative.0 - mean * rate.window_sum.0, relative.1 - mean * rate.window_sum.1);
            let energy = centred.0 * centred.0 + centred.1 * centred.1;
            let varying = (sum_squares - sum * mean).max(0.0);
            match best {
                None => {
                    if energy >= energy_floor && energy >= DETECT_CONCENTRATION * varying * window_f / 2.0 {
                        best = Some((energy, Candidate { start: position, tone: centred }));
                        stop_after = position.saturating_add(follow).min(last);
                    }
                }
                Some((best_energy, _)) => {
                    if energy > best_energy {
                        best = Some((energy, Candidate { start: position, tone: centred }));
                    }
                }
            }
            if position >= stop_after {
                break;
            }
            // Slide by one sample.
            tone = (tone.0 - old * phasor.0 + new * ahead.0, tone.1 - old * phasor.1 + new * ahead.1);
            sum += new - old;
            sum_squares += new * new - old * old;
            phasor = complex_multiply(phasor, rate.step);
            ahead = complex_multiply(ahead, rate.step);
            if step % PHASOR_RENORMALISE_STEPS == 0 {
                phasor = renormalise(phasor);
                ahead = renormalise(ahead);
            }
        }
        best.map(|(_, candidate)| candidate)
    }

    /// The centre of run-in bit 3 from the candidate's tone phase.
    fn bit3_centre(&self, candidate: Candidate) -> f32 {
        let Some(rate) = self.rate else { return 0.0 };
        let start = f32_from_usize(candidate.start);
        // The run-in is a cosine peaking at the centres of its "1" bits:
        // its tone relative to the window start has phase −ω(c − start).
        let phase = atan2(candidate.tone.1, candidate.tone.0);
        let one_centre = start - phase / rate.omega;
        // Bit 3 is a "1"; the window starts near bit 3, so its centre is
        // the "1" centre (period two bits) nearest start + half a bit.
        let period = 2.0 * rate.samples_per_bit;
        let expected = start + rate.samples_per_bit / 2.0;
        one_centre + period * f32_from_i32(round_i32((expected - one_centre) / period))
    }

    // -----------------------------------------------------------------------
    // Levels, threshold and match (REQ-SLC-004, REQ-SLC-005)
    // -----------------------------------------------------------------------

    /// Estimates the levels, threshold and match from the run-in.
    ///
    /// The threshold is the mean of the 14 bit-centre values: half are "1"
    /// and half "0", so it lies midway, and interpolation errors at peaks
    /// and troughs cancel in it. The data amplitude ("1" above "0") is
    /// taken from the run-in tone instead of the bit centres, because
    /// linear interpolation of a tone at a few samples per bit cuts its
    /// peaks: a tone of peak-to-peak `a` has correlation magnitude a·L/4.
    fn measure(&self, work: &mut Work) -> SlicerEvent {
        let Some(rate) = self.rate else { return SlicerEvent::Estimated };
        let mut values = [0.0_f32; RUN_IN_MATCH_BITS];
        let mut position = work.bit3_centre;
        for value in &mut values {
            *value = self.at(position);
            position += rate.samples_per_bit;
        }
        work.threshold = values.iter().sum::<f32>() / f32_from_usize(RUN_IN_MATCH_BITS);
        let tone = work.candidate.tone;
        let amplitude = 4.0 * sqrt(tone.0 * tone.0 + tone.1 * tone.1) / f32_from_usize(rate.window);
        work.one_level = work.threshold + amplitude / 2.0;
        work.zero_level = work.threshold - amplitude / 2.0;
        // The match: how far the bit-centre values agree in sign with the
        // alternating pattern, weighted by their distance from the
        // threshold; 100 for a clean run-in, about 0 for a random signal.
        let mut agreement = 0.0;
        let mut total = 0.0;
        for (index, value) in values.iter().enumerate() {
            let deviation = *value - work.threshold;
            agreement += if index % 2 == 0 { deviation } else { -deviation };
            total += deviation.abs();
        }
        let matched = if total > 0.0 { (agreement / total).max(0.0) * PERCENT } else { 0.0 };
        work.match_percent = to_u8_saturating(matched);
        let nominal = self.config.nominal_amplitude(self.format);
        work.amplitude_percent = to_u16_saturating(amplitude / nominal * PERCENT);
        SlicerEvent::Estimated
    }

    fn gate(&self, work: &Work) -> SlicerEvent {
        if work.amplitude_percent >= self.config.min_amplitude_percent
            && work.match_percent >= self.config.min_match_percent
        {
            SlicerEvent::GatePassed
        } else {
            SlicerEvent::BelowGate
        }
    }

    // -----------------------------------------------------------------------
    // Framing code (REQ-SLC-008, decision E.24)
    // -----------------------------------------------------------------------

    fn find_framing_code(&self, work: &mut Work) -> SlicerEvent {
        // Bits from FRAMING_SEARCH_BITS before the expected framing code to
        // as many after it; bit 0 of `window` is the earliest.
        let first_index = RUN_IN_MATCH_BITS - FRAMING_SEARCH_BITS;
        let mut window: u32 = 0;
        for bit in 0..FRAMING_WINDOW_BITS {
            if self.bit(work, first_index.saturating_add(bit)) {
                window |= 1 << bit;
            }
        }
        let candidate = |offset: i32| {
            let reach = i32::try_from(FRAMING_SEARCH_BITS).unwrap_or(0);
            let shift = u32::try_from(offset.saturating_add(reach)).unwrap_or(0);
            ((window >> shift) & u32::from(u8::MAX)) as u8
        };
        for offset in FRAMING_OFFSETS {
            if candidate(offset) == FRAMING_CODE {
                work.framing_offset = offset;
                return SlicerEvent::FramingExact;
            }
        }
        if self.config.framing_tolerance {
            for offset in FRAMING_OFFSETS {
                // Exactly one bit differs: the difference is a power of two.
                if (candidate(offset) ^ FRAMING_CODE).is_power_of_two() {
                    work.framing_offset = offset;
                    return SlicerEvent::FramingOneBitError;
                }
            }
        }
        SlicerEvent::FramingNotFound
    }

    // -----------------------------------------------------------------------
    // Bits and bytes (REQ-SLC-006, REQ-SLC-007, REQ-SLC-009)
    // -----------------------------------------------------------------------

    fn collect(&self, work: &mut Work) -> SlicerEvent {
        let offset = isize::try_from(work.framing_offset).unwrap_or(0);
        let mut index = (RUN_IN_MATCH_BITS + FRAMING_BITS).saturating_add_signed(offset);
        let mut bytes = [0; PACKET_BYTES];
        for byte in &mut bytes {
            let mut value = 0u8;
            for bit in 0..BITS_PER_BYTE {
                if self.bit(work, index) {
                    value |= 1 << bit;
                }
                index = index.saturating_add(1);
            }
            *byte = value;
        }
        work.bytes = bytes;
        SlicerEvent::BitsRead
    }

    /// The bit `index` bit periods after run-in bit 3 (index 0), decided at
    /// its centre against the held threshold.
    fn bit(&self, work: &Work, index: usize) -> bool {
        let Some(rate) = self.rate else { return false };
        let position = work.bit3_centre + f32_from_usize(index) * rate.samples_per_bit;
        self.at(position) > work.held_threshold
    }

    /// The signal at a fractional sample position, linearly interpolated
    /// (REQ-SLC-007); 0 (black) outside the line.
    fn at(&self, position: f32) -> f32 {
        if position < 0.0 {
            return 0.0;
        }
        let index = floor_index(position);
        let fraction = position - f32_from_usize(index);
        let here = self.samples.get(index).copied().unwrap_or(0.0);
        let next = self.samples.get(index.saturating_add(1)).copied().unwrap_or(here);
        here + fraction * (next - here)
    }

    /// The start of run-in bit 1 in samples, corrected by the framing code
    /// offset; 0 if it falls before the line.
    fn run_in_position(&self, work: &Work) -> u32 {
        let Some(rate) = self.rate else { return 0 };
        // Bit 3's centre less two and a half bits is the start of bit 1.
        let lead = 2.5 - f32_from_i32(work.framing_offset);
        let start = work.bit3_centre - lead * rate.samples_per_bit;
        u32::try_from(round_i32(start).max(0)).unwrap_or(0)
    }
}

/// Address check (REQ-SLC-010).
fn check_address(work: &Work) -> SlicerEvent {
    let first = work.bytes.get(ADDRESS_BYTE_0).copied().and_then(hamming84);
    let second = work.bytes.get(ADDRESS_BYTE_1).copied().and_then(hamming84);
    if first.is_some() && second.is_some() { SlicerEvent::AddressValid } else { SlicerEvent::AddressUncorrectable }
}

/// Scales a phasor back to unit magnitude with one Newton step for
/// 1/√(|p|²), which is enough because rounding keeps |p| within 1e-5 of 1.
fn renormalise(phasor: (f32, f32)) -> (f32, f32) {
    let magnitude_squared = phasor.0 * phasor.0 + phasor.1 * phasor.1;
    let scale = (3.0 - magnitude_squared) / 2.0;
    (phasor.0 * scale, phasor.1 * scale)
}

/// A histogram of the line's levels, for the black level when the
/// synchronising edge is unknown.
struct Histogram {
    counts: [u16; BLACK_HISTOGRAM_BINS],
    sums: [f32; BLACK_HISTOGRAM_BINS],
}

impl Histogram {
    const fn new() -> Self {
        Self { counts: [0; BLACK_HISTOGRAM_BINS], sums: [0.0; BLACK_HISTOGRAM_BINS] }
    }

    fn add(&mut self, bin: usize, value: f32) {
        if let Some(count) = self.counts.get_mut(bin) {
            *count = count.saturating_add(1);
        }
        if let Some(sum) = self.sums.get_mut(bin) {
            *sum += value;
        }
    }

    /// The black level when the synchronising edge is unknown.
    ///
    /// Blanking and the "0" bits are at black (EN 300 706 clause 5.2), so
    /// black is the most common level once two things are set aside: the
    /// synchronising pulse, the lowest [`BLACK_LOW_PERCENTILE`] percent of
    /// the samples, and the "1" bits, which can outnumber the "0" bits on a
    /// row of solid mosaics. Black is therefore looked for between that
    /// percentile and the midpoint of the line's range, on a smoothed
    /// histogram, and refined to the mean of the samples around the peak.
    fn black(&self) -> f32 {
        let total: u32 = self.counts.iter().map(|&count| u32::from(count)).sum();
        let low_bin = self.percentile_bin(total, BLACK_LOW_PERCENTILE);
        let high_bin = self.percentile_bin(total, BLACK_HIGH_PERCENTILE);
        let middle = low_bin.midpoint(high_bin);
        let mut best_bin = low_bin;
        let mut best_count = 0u32;
        for bin in low_bin..=middle {
            let (count, _) = self.around(bin, BLACK_HISTOGRAM_SMOOTH, low_bin);
            if count > best_count {
                best_count = count;
                best_bin = bin;
            }
        }
        let (count, sum) = self.around(best_bin, BLACK_REFINE_BINS, low_bin);
        if count == 0 { 0.0 } else { sum / f32_from_u32(count) }
    }

    /// The count and sum of the samples in the bins within `reach` of
    /// `bin`, not below `floor`, so the synchronising pulse never counts.
    fn around(&self, bin: usize, reach: usize, floor: usize) -> (u32, f32) {
        let low = bin.saturating_sub(reach).max(floor);
        let high = bin.saturating_add(reach).min(BLACK_HISTOGRAM_BINS - 1);
        let count = self.counts.get(low..=high).unwrap_or(&[]).iter().map(|&count| u32::from(count)).sum();
        let sum = self.sums.get(low..=high).unwrap_or(&[]).iter().sum();
        (count, sum)
    }

    /// The first bin at which the cumulative count reaches `percent` of
    /// `total`.
    fn percentile_bin(&self, total: u32, percent: u32) -> usize {
        let target = total.saturating_mul(percent) / 100;
        let mut cumulative = 0u32;
        for (bin, &count) in self.counts.iter().enumerate() {
            cumulative = cumulative.saturating_add(u32::from(count));
            if cumulative > target {
                return bin;
            }
        }
        BLACK_HISTOGRAM_BINS - 1
    }
}

/// Assertions that the constants agree where they must.
const _: () = {
    assert!(FRAMING_WINDOW_BITS <= u32::BITS as usize);
    assert!(BLACK_HISTOGRAM_BINS == u8::MAX as usize + 1);
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::cast_precision_loss)]
mod tests {
    use std::vec::Vec;

    use super::{Config, Line, SampleFormat, Slicer};
    use crate::generator::{Generator, SignalParams, line_bytes};
    use crate::raw::{Preset, RawGeometry};
    use crate::tests::{random_packet, render};

    /// Lines per second the slicer must reach: ten times real time for 35
    /// lines a frame at 25 frames a second (REQ-GEN-013).
    const REQUIRED_LINES_PER_SECOND: f64 = 10.0 * 35.0 * 25.0;

    /// Slices `lines` repeatedly for about a second and returns lines per
    /// second.
    fn lines_per_second(lines: &[Vec<u8>], rate: u32) -> f64 {
        let mut slicer = Slicer::new(Config::new());
        let start = std::time::Instant::now();
        let mut count = 0u64;
        while start.elapsed().as_secs_f64() < 1.0 {
            for samples in lines {
                let outcome = slicer.slice(&Line::new(samples, SampleFormat::U8, rate));
                core::hint::black_box(&outcome);
                count += 1;
            }
        }
        count as f64 / start.elapsed().as_secs_f64()
    }

    /// REQ-GEN-013 throughput, on bt8x8 lines (35.47 MHz, 2048 samples)
    /// with noise at 20 dB: lines carrying packets, and empty lines, which
    /// are the slowest because the run-in search covers the whole line.
    #[test]
    #[ignore = "performance measurement; run in release mode"]
    fn throughput() {
        let geometry = RawGeometry::preset(Preset::Bt8x8);
        let rate = geometry.sample_rate_hz;
        let nominal = SignalParams { start_ns: 7_000.0, ..SignalParams::nominal(rate as f32) };
        let params = SignalParams { noise_rms: nominal.noise_for_snr_db(20.0), ..nominal };
        let mut generator = Generator::new(30);
        let mut teletext = Vec::new();
        let mut empty = Vec::new();
        for _ in 0..500 {
            let packet = random_packet(generator.rng());
            teletext.push(render(&mut generator, &params, &line_bytes(&packet), geometry.samples_per_line));
            empty.push(render(&mut generator, &params, &[0; 45], geometry.samples_per_line));
        }
        let with_packets = lines_per_second(&teletext, rate);
        let without = lines_per_second(&empty, rate);
        std::println!("throughput: {with_packets:.0} lines/s with packets, {without:.0} lines/s empty");
        std::println!("required: {REQUIRED_LINES_PER_SECOND:.0} lines/s");
        assert!(with_packets >= REQUIRED_LINES_PER_SECOND && without >= REQUIRED_LINES_PER_SECOND);
    }
}
