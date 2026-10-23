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

use neoteletext_core::consts::{ADDRESS_BYTE_0, ADDRESS_BYTE_1, BIT_RATE_HZ, PACKET_BITS, PACKET_BYTES};
use neoteletext_core::ecc::hamming84;
use neoteletext_core::packet::Packet;

use crate::consts::{
    BLACK_HISTOGRAM_BINS, FRAMING_BITS, FRAMING_SEARCH_BITS, MAX_LINE_SAMPLES, MIN_SAMPLE_RATE_HZ, RUN_IN_MATCH_BITS,
};
use crate::fsm::SlicerEvent;
use crate::math::{ceil_index, f32_from_u32, f32_from_usize, floor_index, sin_cos};

mod bits;
mod black;
mod config;
mod framing;
mod line;
mod machine;
mod outcome;
mod run_in;

pub use config::Config;
pub use line::{Line, SampleFormat};
pub use outcome::{Counters, Rejection, Sliced};

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
