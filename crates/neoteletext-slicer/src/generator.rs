//! Synthetic teletext line generator (REQ-TST-060).
//!
//! Produces sampled lines to EN 300 706 at any sample rate: NRZ data at
//! 6.9375 Mbit/s (clause 5.3), "0" at black and "1" at the given amplitude
//! above it (clause 5.2), the timing reference bit 12.0 µs after the
//! synchronising edge (clause 6.3), and pulses shaped by a raised-cosine
//! (Nyquist) filter, whose spectrum is skew-symmetric about half the bit
//! rate and zero by 5 MHz (clause 5.4). On top of that it can add:
//!
//! - a black-level offset and a linear drift across the line;
//! - a timing offset and a bit-rate error;
//! - reduced or absent run-in bits 1 and 2 (clause 6.1, note);
//! - high-frequency loss, as a first-order low-pass filter;
//! - an echo of given delay and gain;
//! - additive white Gaussian noise from a seeded, deterministic generator;
//! - a synchronising pulse and PAL colour burst (ITU-R BT.470).
//!
//! It uses floating point, which REQ-GEN-016 permits in the slicer crate.

use core::f32::consts::{PI, TAU};

use neoteletext_core::consts::{BIT_RATE_HZ, CLOCK_RUN_IN, FRAMING_CODE, PACKET_BYTES, PACKET_BYTES_WITH_RUN_IN};
use neoteletext_core::packet::Packet;

use crate::consts::{
    BITS_PER_BYTE, BITS_PER_LINE, BLACK_TO_WHITE_MV, BT601_BLACK_8BIT, BURST_CYCLES, BURST_PEAK_TO_PEAK_MV,
    BURST_START_NS, LINE_DATA_BITS, NOMINAL_AMPLITUDE_8BIT, ONE_LEVEL_PERCENT_OF_WHITE, PAL_SUBCARRIER_HZ, PERCENT,
    RUN_IN_UNRELIABLE_BITS, SYNC_DEPTH_MV, SYNC_WIDTH_NS, TIMING_REFERENCE_BIT, TIMING_REFERENCE_NS,
};
use crate::math::{exp, f32_from_i32, f32_from_u32, f32_from_usize, floor_i32, ln, sin_cos, to_u8_saturating};

mod rng;

pub use rng::Rng;

/// Nanoseconds per second.
const NS_PER_SECOND: f32 = 1e9;
/// Roll-off of the raised-cosine pulse. The spectrum then ends at
/// (1 + 0.4) × 3.47 MHz = 4.86 MHz, "substantially zero by 5 MHz"
/// (EN 300 706 clause 5.4).
pub const ROLL_OFF: f32 = 0.4;
/// Bit periods either side of a pulse's centre that are generated; the
/// raised-cosine tail beyond eight bits is below 0.1 percent.
const PULSE_SPAN_BITS: i32 = 8;
/// Table points per bit period of the pulse shape.
const PULSE_TABLE_PER_BIT: i32 = 64;
/// Entries in the pulse table: the span both sides plus the centre.
const PULSE_TABLE_LEN: usize = (2 * PULSE_SPAN_BITS * PULSE_TABLE_PER_BIT + 1) as usize;
/// Below this distance from the raised cosine's removable singularity the
/// limit value is used.
const SINGULARITY_GUARD: f32 = 1e-4;
/// The nominal line period, nanoseconds: 444 bit periods (clause 5.3).
// Both values are below 2^24, so f32 holds them exactly.
#[allow(clippy::cast_precision_loss)]
const LINE_PERIOD_NS: f32 = BITS_PER_LINE as f32 * NS_PER_SECOND / BIT_RATE_HZ as f32;

/// The bytes of one teletext line in transmission order: two run-in bytes,
/// the framing code and the 42 packet bytes (clause 7.1).
pub type LineBytes = [u8; PACKET_BYTES_WITH_RUN_IN];

/// The line bytes that carry `packet`.
#[must_use]
pub fn line_bytes(packet: &Packet) -> LineBytes {
    let mut bytes = [0; PACKET_BYTES_WITH_RUN_IN];
    let header = [CLOCK_RUN_IN, CLOCK_RUN_IN, FRAMING_CODE];
    for (slot, value) in bytes.iter_mut().zip(header.iter().chain(packet.bytes.iter())) {
        *slot = *value;
    }
    bytes
}

/// The packet bytes of a line, the inverse of [`line_bytes`].
#[must_use]
pub fn packet_of(bytes: &LineBytes) -> Packet {
    let mut packet = Packet { bytes: [0; PACKET_BYTES] };
    for (slot, value) in packet.bytes.iter_mut().zip(bytes.iter().skip(PACKET_BYTES_WITH_RUN_IN - PACKET_BYTES)) {
        *slot = *value;
    }
    packet
}

/// An echo: a delayed, scaled copy of the video added to it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Echo {
    /// Delay in nanoseconds.
    pub delay_ns: f32,
    /// Gain of the copy; negative inverts it.
    pub gain: f32,
}

/// What the generated line looks like.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SignalParams {
    /// Sample rate in hertz.
    pub sample_rate_hz: f32,
    /// Time of the first sample after the synchronising edge, nanoseconds.
    pub start_ns: f32,
    /// Black level at the first sample, in sample codes.
    pub black_level: f32,
    /// Change of the black level from the first to the last sample.
    pub black_drift: f32,
    /// The "1" level above black, in sample codes.
    pub data_amplitude: f32,
    /// Timing offset of the data from nominal, nanoseconds (EN 300 706
    /// allows −1000 to +400).
    pub timing_offset_ns: f32,
    /// Bit rate error as a fraction: 0.005 is 0.5 percent fast.
    pub bit_rate_error: f32,
    /// Amplitude of run-in bits 1 and 2 relative to the rest: 1 is full, 0
    /// is absent (clause 6.1, note).
    pub first_bits_scale: f32,
    /// Corner frequency of a first-order low-pass filter for
    /// high-frequency loss, hertz; `None` for none.
    pub low_pass_hz: Option<f32>,
    /// An echo; `None` for none.
    pub echo: Option<Echo>,
    /// Root-mean-square of the added white Gaussian noise, sample codes.
    pub noise_rms: f32,
    /// Draw the synchronising pulse and colour burst.
    pub sync_and_burst: bool,
}

impl SignalParams {
    /// A clean, nominal line in 8-bit BT.601 coding, sampled from the
    /// synchronising edge, at `sample_rate_hz`.
    #[must_use]
    pub const fn nominal(sample_rate_hz: f32) -> Self {
        Self {
            sample_rate_hz,
            start_ns: 0.0,
            #[allow(clippy::cast_precision_loss)]
            black_level: BT601_BLACK_8BIT as f32,
            black_drift: 0.0,
            data_amplitude: NOMINAL_AMPLITUDE_8BIT,
            timing_offset_ns: 0.0,
            bit_rate_error: 0.0,
            first_bits_scale: 1.0,
            low_pass_hz: None,
            echo: None,
            noise_rms: 0.0,
            sync_and_burst: true,
        }
    }

    /// The noise level that gives `decibels` between the data amplitude and
    /// the RMS noise (REQ-SLC-013): amplitude / 10^(dB/20).
    #[must_use]
    pub fn noise_for_snr_db(&self, decibels: f32) -> f32 {
        self.data_amplitude / exp(decibels / 20.0 * ln(10.0))
    }
}

/// The signal generator.
#[derive(Clone, PartialEq, Debug)]
pub struct Generator {
    pulse: [f32; PULSE_TABLE_LEN],
    rng: Rng,
}

impl Generator {
    /// A generator whose noise is seeded with `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut pulse = [0.0; PULSE_TABLE_LEN];
        for (index, slot) in pulse.iter_mut().enumerate() {
            let offset = f32_from_usize(index) / f32_from_i32(PULSE_TABLE_PER_BIT) - f32_from_i32(PULSE_SPAN_BITS);
            *slot = raised_cosine(offset);
        }
        Self { pulse, rng: Rng::new(seed) }
    }

    /// The noise generator, for drawing random packet contents from the
    /// same seed.
    pub const fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    /// Generates one line carrying `bytes` into `out`, one value per
    /// sample, in sample codes (not yet rounded or clipped).
    pub fn line(&mut self, params: &SignalParams, bytes: &LineBytes, out: &mut [f32]) {
        let sample_ns = NS_PER_SECOND / params.sample_rate_hz;
        // A first-order low-pass: y += α(x − y) with α = 1 − e^(−2π·fc/fs).
        let alpha = params.low_pass_hz.map(|corner| 1.0 - exp(-TAU * corner / params.sample_rate_hz));
        let last = f32_from_usize(out.len().saturating_sub(1)).max(1.0);
        let mut filtered: Option<f32> = None;
        for (index, slot) in out.iter_mut().enumerate() {
            let n = f32_from_usize(index);
            let time = params.start_ns + n * sample_ns;
            let mut video = self.video(params, bytes, time);
            if let Some(echo) = params.echo {
                video += echo.gain * self.video(params, bytes, time - echo.delay_ns);
            }
            let shaped = match (alpha, filtered) {
                (Some(alpha), Some(previous)) => previous + alpha * (video - previous),
                (Some(_) | None, _) => video,
            };
            if alpha.is_some() {
                filtered = Some(shaped);
            }
            let black = params.black_level + params.black_drift * n / last;
            let noise = if params.noise_rms > 0.0 { params.noise_rms * self.rng.gaussian() } else { 0.0 };
            *slot = black + shaped + noise;
        }
    }

    /// The video at `time` nanoseconds after the synchronising edge,
    /// relative to black, without noise or filtering.
    fn video(&self, params: &SignalParams, bytes: &LineBytes, time: f32) -> f32 {
        let mut value = self.data(params, bytes, time);
        if params.sync_and_burst {
            value += sync_and_burst(params.data_amplitude, time);
        }
        value
    }

    fn data(&self, params: &SignalParams, bytes: &LineBytes, time: f32) -> f32 {
        let bit_ns = NS_PER_SECOND / (f32_from_u32(BIT_RATE_HZ) * (1.0 + params.bit_rate_error));
        // Bit indices from 0; the reference bit 13 is index 12.
        let reference_index = f32_from_usize(TIMING_REFERENCE_BIT - 1);
        let reference_ns = f32_from_u32(TIMING_REFERENCE_NS) + params.timing_offset_ns;
        let position = (time - reference_ns) / bit_ns + reference_index;
        let nearest = floor_i32(position + 0.5);
        let mut value = 0.0;
        for bit in nearest.saturating_sub(PULSE_SPAN_BITS)..=nearest.saturating_add(PULSE_SPAN_BITS) {
            let Ok(index) = usize::try_from(bit) else { continue };
            if index >= LINE_DATA_BITS || !bit_of(bytes, index) {
                continue;
            }
            let scale = if index < RUN_IN_UNRELIABLE_BITS { params.first_bits_scale } else { 1.0 };
            value += scale * self.pulse_at(position - f32_from_i32(bit));
        }
        value * params.data_amplitude
    }

    /// The pulse shape at `offset` bit periods from its centre.
    fn pulse_at(&self, offset: f32) -> f32 {
        let table_position = (offset + f32_from_i32(PULSE_SPAN_BITS)) * f32_from_i32(PULSE_TABLE_PER_BIT);
        let index = floor_i32(table_position);
        let Ok(base) = usize::try_from(index) else { return 0.0 };
        let fraction = table_position - f32_from_i32(index);
        let here = self.pulse.get(base).copied().unwrap_or(0.0);
        let next = self.pulse.get(base.saturating_add(1)).copied().unwrap_or(0.0);
        here + fraction * (next - here)
    }
}

/// Bit `index` of the line in transmission order, first bit of each byte
/// first (clause 7.1).
fn bit_of(bytes: &LineBytes, index: usize) -> bool {
    let byte = bytes.get(index / BITS_PER_BYTE).copied().unwrap_or(0);
    (byte >> (index % BITS_PER_BYTE)) & 1 == 1
}

/// The raised-cosine (Nyquist) pulse at `x` bit periods: 1 at 0 and 0 at
/// every other bit centre, so bit centres carry no intersymbol
/// interference.
fn raised_cosine(x: f32) -> f32 {
    let sinc = if x.abs() < SINGULARITY_GUARD { 1.0 } else { sin_cos(PI * x).0 / (PI * x) };
    let denominator = 1.0 - (2.0 * ROLL_OFF * x) * (2.0 * ROLL_OFF * x);
    if denominator.abs() < SINGULARITY_GUARD {
        // The limit at x = ±1/(2β) is (π/4)·sinc(1/(2β)).
        let edge = 1.0 / (2.0 * ROLL_OFF);
        return PI / 4.0 * sin_cos(PI * edge).0 / (PI * edge);
    }
    sinc * sin_cos(PI * ROLL_OFF * x).1 / denominator
}

/// The synchronising pulse and colour burst relative to black, scaled from
/// the BT.470 millivolt levels so that the data amplitude is 66 percent of
/// black-to-white.
fn sync_and_burst(data_amplitude: f32, time: f32) -> f32 {
    let per_mv =
        data_amplitude * PERCENT / (f32_from_u32(ONE_LEVEL_PERCENT_OF_WHITE) * f32_from_u32(BLACK_TO_WHITE_MV));
    let line_time = time - LINE_PERIOD_NS * f32_from_i32(floor_i32(time / LINE_PERIOD_NS));
    if line_time < f32_from_u32(SYNC_WIDTH_NS) {
        return -f32_from_u32(SYNC_DEPTH_MV) * per_mv;
    }
    let burst_start = f32_from_u32(BURST_START_NS);
    let burst_end = burst_start + f32_from_u32(BURST_CYCLES) * NS_PER_SECOND / PAL_SUBCARRIER_HZ;
    if line_time >= burst_start && line_time < burst_end {
        let phase = TAU * PAL_SUBCARRIER_HZ * (line_time - burst_start) / NS_PER_SECOND;
        return f32_from_u32(BURST_PEAK_TO_PEAK_MV) / 2.0 * per_mv * sin_cos(phase).0;
    }
    0.0
}

/// Rounds and clips generated values to 8-bit samples.
pub fn quantise_u8(values: &[f32], out: &mut [u8]) {
    for (slot, value) in out.iter_mut().zip(values) {
        *slot = to_u8_saturating(*value);
    }
}

/// Scales generated 8-bit values by `scale`, then rounds and clips them to
/// 16-bit little-endian samples.
pub fn quantise_u16le(values: &[f32], scale: f32, out: &mut [u8]) {
    for (pair, value) in out.as_chunks_mut::<2>().0.iter_mut().zip(values) {
        let sample = crate::math::to_u16_saturating(*value * scale);
        *pair = sample.to_le_bytes();
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_precision_loss
)]
mod tests {
    use super::*;

    #[test]
    fn pulse_is_nyquist() {
        assert!((raised_cosine(0.0) - 1.0).abs() < 1e-6);
        for k in 1..8 {
            assert!(raised_cosine(k as f32).abs() < 1e-5, "{k}");
            assert!(raised_cosine(-(k as f32)).abs() < 1e-5, "-{k}");
        }
        // Continuous through the removable singularity at 1/(2β) = 1.25.
        let edge = 1.0 / (2.0 * ROLL_OFF);
        assert!((raised_cosine(edge) - raised_cosine(edge + 0.001)).abs() < 1e-3);
    }

    #[test]
    fn line_bytes_round_trip() {
        let packet = Packet { bytes: core::array::from_fn(|i| i as u8) };
        let bytes = line_bytes(&packet);
        assert_eq!(&bytes[..3], &[0x55, 0x55, 0x27]);
        assert_eq!(packet_of(&bytes), packet);
    }

    #[test]
    fn bit_centres_carry_the_levels() {
        let params = SignalParams { sync_and_burst: false, ..SignalParams::nominal(6_937_500.0 * 4.0) };
        let mut generator = Generator::new(1);
        let packet = Packet { bytes: [0xFF; PACKET_BYTES] };
        let bytes = line_bytes(&packet);
        // Sample at four per bit, starting on bit 1's centre.
        let bit_ns = 1e9 / 6_937_500.0;
        let start = 12_000.0 - 12.0 * bit_ns;
        let mut out = [0.0_f32; 360 * 4];
        generator.line(&SignalParams { start_ns: start, ..params }, &bytes, &mut out);
        // Run-in: 1,0,1,0...
        for bit in 0..16 {
            let want = if bit % 2 == 0 { 16.0 + NOMINAL_AMPLITUDE_8BIT } else { 16.0 };
            assert!((out[bit * 4] - want).abs() < 0.5, "bit {bit}: {}", out[bit * 4]);
        }
    }

    #[test]
    fn noise_has_the_requested_spread() {
        let mut rng = Rng::new(7);
        let count = 20_000;
        let mut sum = 0.0_f64;
        let mut squares = 0.0_f64;
        for _ in 0..count {
            let value = f64::from(rng.gaussian());
            sum += value;
            squares += value * value;
        }
        let mean = sum / f64::from(count);
        let variance = squares / f64::from(count) - mean * mean;
        assert!(mean.abs() < 0.03, "mean {mean}");
        assert!((variance - 1.0).abs() < 0.05, "variance {variance}");
    }
}
