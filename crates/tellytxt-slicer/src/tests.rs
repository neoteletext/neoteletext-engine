//! Slicer tests with synthetic signals (REQ-TST-060, REQ-FSM-009).
//!
//! Tests marked `#[ignore]` print measurements for the report; run them
//! with `cargo test -p tellytxt-slicer --release -- --ignored --nocapture`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::similar_names
)]

use std::vec;
use std::vec::Vec;

use tellytxt_core::consts::{BIT_RATE_HZ, CLOCK_RUN_IN, PACKET_BYTES_WITH_RUN_IN};
use tellytxt_core::ecc::{hamming84, with_odd_parity};
use tellytxt_core::packet::{Packet, PacketMeta, encode};
use tellytxt_core::trace::{Machine, TraceEntry};
use tellytxt_core::types::Magazine;

use crate::consts::{
    BITS_PER_BYTE, NOMINAL_AMPLITUDE_8BIT, SAMPLE_16_PER_8, TIMING_EARLY_NS, TIMING_LATE_NS, TIMING_REFERENCE_BIT,
    TIMING_REFERENCE_NS,
};
use crate::fsm::{SlicerAction, SlicerEvent, SlicerState};
use crate::generator::{Echo, Generator, LineBytes, Rng, SignalParams, line_bytes, quantise_u8, quantise_u16le};
use crate::raw::{GeometryError, Preset, RawGeometry, RawReader};
use crate::slicer::{Config, Counters, Line, Rejection, SampleFormat, Sliced, Slicer};

/// The four sample rates of REQ-SLC-002, hertz.
pub(crate) const RATES: [u32; 4] = [17_734_475, 27_000_000, 28_636_363, 35_468_950];
/// Nanoseconds in a full line.
const LINE_NS: f64 = 64_000.0;
/// Index of the framing code byte in the line bytes.
const FRAMING_INDEX: usize = 2;
/// Index of the first address byte in the line bytes.
const ADDRESS_INDEX: usize = 3;
/// The System A framing code, 11100111 in transmission order (ITU-R BT.653
/// Table 1a item 2.1), first bit as bit 0.
const SYSTEM_A_FRAMING: u8 = 0b1110_0111;

pub(crate) fn samples_per_line(rate: u32) -> usize {
    (f64::from(rate) * LINE_NS / 1e9).round() as usize
}

fn bit_ns() -> f32 {
    1e9 / BIT_RATE_HZ as f32
}

pub(crate) fn random_packet(rng: &mut Rng) -> Packet {
    let magazine = Magazine::from_bits(rng.byte());
    let row = rng.byte() % 32;
    let mut packet = encode::address(magazine, row);
    for byte in &mut packet.bytes[2..] {
        *byte = with_odd_parity(rng.byte() & 0x7F);
    }
    packet
}

pub(crate) fn render(generator: &mut Generator, params: &SignalParams, bytes: &LineBytes, len: usize) -> Vec<u8> {
    let mut values = vec![0.0_f32; len];
    generator.line(params, bytes, &mut values);
    let mut out = vec![0u8; len];
    quantise_u8(&values, &mut out);
    out
}

fn slice_bytes(slicer: &mut Slicer, samples: &[u8], rate: u32) -> Result<Sliced, Rejection> {
    slicer.slice(&Line::new(samples, SampleFormat::U8, rate))
}

/// Results of a batch of lines.
#[derive(Default, Debug, Clone, Copy)]
struct Tally {
    counters: Counters,
    lines: u32,
    accepted: u32,
    exact: u32,
    address_correct: u32,
}

impl Tally {
    fn percent(count: u32, lines: u32) -> f64 {
        100.0 * f64::from(count) / f64::from(lines)
    }

    fn address_percent(&self) -> f64 {
        Self::percent(self.address_correct, self.lines)
    }

    fn exact_percent(&self) -> f64 {
        Self::percent(self.exact, self.lines)
    }
}

fn run_batch(rate: u32, params: &SignalParams, lines: u32, seed: u64, config: Config) -> Tally {
    let mut generator = Generator::new(seed);
    let mut slicer = Slicer::new(config);
    let len = samples_per_line(rate);
    let mut tally = Tally::default();
    for _ in 0..lines {
        let packet = random_packet(generator.rng());
        let samples = render(&mut generator, params, &line_bytes(&packet), len);
        tally.lines += 1;
        if let Ok(sliced) = slice_bytes(&mut slicer, &samples, rate) {
            tally.accepted += 1;
            if sliced.packet == packet {
                tally.exact += 1;
            }
            let want = (hamming84(packet.bytes[0]), hamming84(packet.bytes[1]));
            let got = (hamming84(sliced.packet.bytes[0]), hamming84(sliced.packet.bytes[1]));
            if want == got {
                tally.address_correct += 1;
            }
        }
    }
    tally.counters = slicer.counters();
    tally
}

fn known_packet() -> Packet {
    let mut rng = Rng::new(42);
    random_packet(&mut rng)
}

/// Slices one line with tracing on and returns the outcome and the trace.
fn traced(samples: &[u8], rate: u32, config: Config) -> (Result<Sliced, Rejection>, Vec<TraceEntry>, Counters) {
    let mut slicer = Slicer::new(Config { trace: true, ..config });
    let mut trace = Vec::new();
    let line = Line { timestamp_us: Some(1234), ..Line::new(samples, SampleFormat::U8, rate) };
    let outcome = slicer.slice_traced(&line, |entry| trace.push(entry));
    (outcome, trace, slicer.counters())
}

fn line_with(bytes: &LineBytes, params: &SignalParams, rate: u32) -> Vec<u8> {
    let mut generator = Generator::new(9);
    render(&mut generator, params, bytes, samples_per_line(rate))
}

const RATE: u32 = RATES[3];

fn nominal() -> SignalParams {
    SignalParams::nominal(RATE as f32)
}

/// Asserts that the trace holds the transition `from --event/action--> to`.
fn assert_row(trace: &[TraceEntry], from: SlicerState, event: SlicerEvent, action: SlicerAction, to: SlicerState) {
    let found = trace.iter().any(|entry| {
        entry.from == from.name()
            && entry.event == event.name()
            && entry.action == action.name()
            && entry.to == to.name()
    });
    assert!(found, "missing {from:?} --{event:?}/{action:?}--> {to:?} in {trace:#?}");
}

// ---------------------------------------------------------------------------
// REQ-FSM-009: one test per row of the REQ-FSM-002 table
// ---------------------------------------------------------------------------

#[test]
fn row_01_line_received_estimates_black_level() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (_, trace, counters) = traced(&samples, RATE, Config::new());
    assert_row(
        &trace,
        SlicerState::Start,
        SlicerEvent::LineReceived,
        SlicerAction::EstimateBlackLevel,
        SlicerState::SearchingRunIn,
    );
    assert_eq!(trace[0].from, SlicerState::Start.name());
    assert_eq!(counters.lines, 1);
}

#[test]
fn row_02_run_in_found_records_position() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (outcome, trace, _) = traced(&samples, RATE, Config::new());
    assert_row(
        &trace,
        SlicerState::SearchingRunIn,
        SlicerEvent::RunInFound,
        SlicerAction::RecordPosition,
        SlicerState::Measuring,
    );
    // The recorded position is the start of run-in bit 1 (REQ-SLC-012).
    let expected_ns = TIMING_REFERENCE_NS as f32 - (TIMING_REFERENCE_BIT as f32 - 0.5) * bit_ns();
    let expected = expected_ns * RATE as f32 / 1e9;
    let position = outcome.unwrap().meta.quality.unwrap().position as f32;
    assert!((position - expected).abs() <= 1.0, "position {position}, expected {expected}");
}

#[test]
fn row_03_window_exhausted_counts_no_signal() {
    let samples = vec![16u8; samples_per_line(RATE)];
    let (outcome, trace, counters) = traced(&samples, RATE, Config::new());
    assert_eq!(outcome, Err(Rejection::NoSignal));
    assert_row(
        &trace,
        SlicerState::SearchingRunIn,
        SlicerEvent::WindowExhausted,
        SlicerAction::CountNoSignal,
        SlicerState::Rejected,
    );
    assert_eq!(counters, Counters { lines: 1, no_signal: 1, ..Counters::default() });
}

#[test]
fn row_04_measuring_estimates_levels() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (outcome, trace, _) = traced(&samples, RATE, Config::new());
    assert_row(&trace, SlicerState::Measuring, SlicerEvent::Estimated, SlicerAction::None, SlicerState::GateCheck);
    let quality = outcome.unwrap().meta.quality.unwrap();
    assert!((97..=103).contains(&quality.amplitude_percent), "amplitude {}", quality.amplitude_percent);
    assert!(quality.match_percent >= 98, "match {}", quality.match_percent);
}

#[test]
fn row_05_gate_passed_holds_threshold() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (_, trace, _) = traced(&samples, RATE, Config::new());
    assert_row(
        &trace,
        SlicerState::GateCheck,
        SlicerEvent::GatePassed,
        SlicerAction::HoldThreshold,
        SlicerState::LookingForFramingCode,
    );
}

#[test]
fn row_06_below_gate_counts() {
    // 40 percent of nominal amplitude, below the default 50 percent.
    let params = SignalParams { data_amplitude: NOMINAL_AMPLITUDE_8BIT * 0.4, ..nominal() };
    let samples = line_with(&line_bytes(&known_packet()), &params, RATE);
    let (outcome, trace, counters) = traced(&samples, RATE, Config::new());
    assert_eq!(outcome, Err(Rejection::BelowGate));
    assert_row(
        &trace,
        SlicerState::GateCheck,
        SlicerEvent::BelowGate,
        SlicerAction::CountBelowGate,
        SlicerState::Rejected,
    );
    assert_eq!(counters.below_gate, 1);
    // A match quality minimum above 100 rejects even a clean line.
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (outcome, _, _) = traced(&samples, RATE, Config { min_match_percent: 101, ..Config::new() });
    assert_eq!(outcome, Err(Rejection::BelowGate));
}

#[test]
fn row_07_exact_framing_code() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (outcome, trace, _) = traced(&samples, RATE, Config::new());
    assert_row(
        &trace,
        SlicerState::LookingForFramingCode,
        SlicerEvent::FramingExact,
        SlicerAction::None,
        SlicerState::CollectingBytes,
    );
    assert!(!outcome.unwrap().meta.quality.unwrap().framing_corrected);
}

#[test]
fn row_08_one_bit_framing_error_is_corrected() {
    let mut bytes = line_bytes(&known_packet());
    bytes[FRAMING_INDEX] ^= 0x08;
    let samples = line_with(&bytes, &nominal(), RATE);
    let (outcome, trace, counters) = traced(&samples, RATE, Config::new());
    assert_row(
        &trace,
        SlicerState::LookingForFramingCode,
        SlicerEvent::FramingOneBitError,
        SlicerAction::MarkFramingCorrected,
        SlicerState::CollectingBytes,
    );
    let sliced = outcome.unwrap();
    assert_eq!(sliced.packet, known_packet());
    assert!(sliced.meta.quality.unwrap().framing_corrected);
    assert_eq!(counters.framing_corrected, 1);
}

#[test]
fn row_09_framing_code_not_found() {
    // Two wrong bits.
    let mut bytes = line_bytes(&known_packet());
    bytes[FRAMING_INDEX] ^= 0x81;
    let samples = line_with(&bytes, &nominal(), RATE);
    let (outcome, trace, counters) = traced(&samples, RATE, Config::new());
    assert_eq!(outcome, Err(Rejection::NoFramingCode));
    assert_row(
        &trace,
        SlicerState::LookingForFramingCode,
        SlicerEvent::FramingNotFound,
        SlicerAction::CountNoFramingCode,
        SlicerState::Rejected,
    );
    assert_eq!(counters.no_framing_code, 1);
}

#[test]
fn row_10_bits_read_assembles_bytes() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (outcome, trace, _) = traced(&samples, RATE, Config::new());
    assert_row(
        &trace,
        SlicerState::CollectingBytes,
        SlicerEvent::BitsRead,
        SlicerAction::AssembleBytes,
        SlicerState::CheckingAddress,
    );
    assert_eq!(outcome.unwrap().packet, known_packet());
}

#[test]
fn row_11_valid_address_outputs_packet() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (outcome, trace, counters) = traced(&samples, RATE, Config::new());
    assert_row(
        &trace,
        SlicerState::CheckingAddress,
        SlicerEvent::AddressValid,
        SlicerAction::OutputPacket,
        SlicerState::Accepted,
    );
    let sliced = outcome.unwrap();
    assert_eq!(sliced.packet, known_packet());
    assert_eq!(sliced.meta.timestamp_us, Some(1234));
    assert_eq!(counters, Counters { lines: 1, accepted: 1, ..Counters::default() });
}

#[test]
fn row_12_uncorrectable_address_counts_bad_address() {
    // Two bit errors in a Hamming 8/4 byte are detected, not corrected.
    let mut bytes = line_bytes(&known_packet());
    bytes[ADDRESS_INDEX + 1] ^= 0x03;
    let samples = line_with(&bytes, &nominal(), RATE);
    let (outcome, trace, counters) = traced(&samples, RATE, Config::new());
    assert_eq!(outcome, Err(Rejection::BadAddress));
    assert_row(
        &trace,
        SlicerState::CheckingAddress,
        SlicerEvent::AddressUncorrectable,
        SlicerAction::CountBadAddress,
        SlicerState::Rejected,
    );
    assert_eq!(counters.bad_address, 1);
}

/// REQ-FSM-010: a known input gives the expected trace; tracing off gives
/// none.
#[test]
fn trace_of_an_accepted_line() {
    use SlicerAction as A;
    use SlicerEvent as E;
    use SlicerState as S;
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let (_, trace, _) = traced(&samples, RATE, Config::new());
    let steps: Vec<(&str, &str, &str, &str)> = trace.iter().map(|e| (e.from, e.event, e.action, e.to)).collect();
    let expected = [
        (S::Start, E::LineReceived, A::EstimateBlackLevel, S::SearchingRunIn),
        (S::SearchingRunIn, E::RunInFound, A::RecordPosition, S::Measuring),
        (S::Measuring, E::Estimated, A::None, S::GateCheck),
        (S::GateCheck, E::GatePassed, A::HoldThreshold, S::LookingForFramingCode),
        (S::LookingForFramingCode, E::FramingExact, A::None, S::CollectingBytes),
        (S::CollectingBytes, E::BitsRead, A::AssembleBytes, S::CheckingAddress),
        (S::CheckingAddress, E::AddressValid, A::OutputPacket, S::Accepted),
    ];
    let expected: Vec<(&str, &str, &str, &str)> =
        expected.iter().map(|(s, e, a, t)| (s.name(), e.name(), a.name(), t.name())).collect();
    assert_eq!(steps, expected);
    assert!(trace.iter().all(|e| e.machine == Machine::Slicer && e.time.0 == 1234));

    let mut slicer = Slicer::new(Config::new());
    let mut entries = 0;
    let outcome = slicer.slice_traced(&Line::new(&samples, SampleFormat::U8, RATE), |_| entries += 1);
    assert!(outcome.is_ok());
    assert_eq!(entries, 0);
}

// ---------------------------------------------------------------------------
// REQ-SLC-002, REQ-SLC-007, REQ-SLC-009, REQ-SLC-013: rates and noise
// ---------------------------------------------------------------------------

#[test]
fn clean_lines_decode_at_every_rate() {
    for rate in RATES {
        let tally = run_batch(rate, &SignalParams::nominal(rate as f32), 200, 1, Config::new());
        assert_eq!(tally.exact, tally.lines, "rate {rate}: {tally:?}");
    }
}

#[test]
fn twenty_db_noise_keeps_addresses() {
    for rate in RATES {
        let params = SignalParams::nominal(rate as f32);
        let params = SignalParams { noise_rms: params.noise_for_snr_db(20.0), ..params };
        let tally = run_batch(rate, &params, 500, 3, Config::new());
        assert!(tally.address_percent() >= 99.0, "rate {rate}: {tally:?}");
    }
}

#[test]
fn sixteen_bit_samples() {
    let rate = RATES[0];
    let params = SignalParams::nominal(rate as f32);
    let mut generator = Generator::new(5);
    let mut slicer = Slicer::new(Config::new());
    let len = samples_per_line(rate);
    for _ in 0..50 {
        let packet = random_packet(generator.rng());
        let mut values = vec![0.0_f32; len];
        generator.line(&params, &line_bytes(&packet), &mut values);
        let mut bytes = vec![0u8; len * 2];
        quantise_u16le(&values, SAMPLE_16_PER_8 as f32, &mut bytes);
        let sliced = slicer.slice(&Line::new(&bytes, SampleFormat::U16Le, rate)).unwrap();
        assert_eq!(sliced.packet, packet);
        let amplitude = sliced.meta.quality.unwrap().amplitude_percent;
        assert!((97..=103).contains(&amplitude), "amplitude {amplitude}");
    }
}

// ---------------------------------------------------------------------------
// REQ-SLC-004 and REQ-SLC-006: timing, run-in, bit rate
// ---------------------------------------------------------------------------

#[test]
fn timing_across_the_full_tolerance() {
    let early = -(TIMING_EARLY_NS as i32);
    let late = TIMING_LATE_NS as i32;
    for rate in RATES {
        let mut generator = Generator::new(11);
        let mut slicer = Slicer::new(Config::new());
        for offset in (early..=late).step_by(25) {
            let params = SignalParams { timing_offset_ns: offset as f32, ..SignalParams::nominal(rate as f32) };
            let packet = random_packet(generator.rng());
            let samples = render(&mut generator, &params, &line_bytes(&packet), samples_per_line(rate));
            let sliced = slice_bytes(&mut slicer, &samples, rate).unwrap_or_else(|e| panic!("{rate} {offset}: {e:?}"));
            assert_eq!(sliced.packet, packet, "rate {rate} offset {offset}");
        }
    }
}

#[test]
fn capture_windows_starting_anywhere() {
    // Lines whose first sample is at various times after the
    // synchronising edge, as different capture devices produce, cut to
    // 57.7 µs as in the bt8x8 capture. The window must still hold the whole
    // packet, from 9.2 µs at the earliest to 62.5 µs at the latest.
    for rate in RATES {
        let mut generator = Generator::new(12);
        let mut slicer = Slicer::new(Config::new());
        let len = (f64::from(rate) * 57_700.0 / 1e9) as usize;
        for start in [5_000.0, 6_000.0, 7_000.0, 8_000.0, 9_000.0] {
            let params = SignalParams { start_ns: start, ..SignalParams::nominal(rate as f32) };
            let packet = random_packet(generator.rng());
            let samples = render(&mut generator, &params, &line_bytes(&packet), len);
            let sliced = slice_bytes(&mut slicer, &samples, rate).unwrap_or_else(|e| panic!("{rate} {start}: {e:?}"));
            assert_eq!(sliced.packet, packet, "rate {rate} start {start}");
        }
    }
}

#[test]
fn first_two_run_in_bits_reduced_or_absent() {
    for rate in RATES {
        for scale in [0.0, 0.3, 0.6] {
            let params = SignalParams { first_bits_scale: scale, ..SignalParams::nominal(rate as f32) };
            let tally = run_batch(rate, &params, 20, 13, Config::new());
            assert_eq!(tally.exact, tally.lines, "rate {rate} scale {scale}");
        }
    }
}

#[test]
fn bit_rate_error_with_fixed_period() {
    // The Phase 1 clock has a fixed period, so a bit-rate error moves the
    // late bits off their centres: whole packets survive up to 0.05
    // percent, and addresses (just after the run-in) up to 0.5 percent.
    // Following larger errors needs the Phase 2 tracking loop.
    for rate in RATES {
        for error in [-0.0005_f32, 0.0005] {
            let params = SignalParams { bit_rate_error: error, ..SignalParams::nominal(rate as f32) };
            let tally = run_batch(rate, &params, 20, 14, Config::new());
            assert_eq!(tally.exact, tally.lines, "rate {rate} error {error}");
        }
        for error in [-0.005_f32, 0.005] {
            let params = SignalParams { bit_rate_error: error, ..SignalParams::nominal(rate as f32) };
            let tally = run_batch(rate, &params, 20, 14, Config::new());
            assert_eq!(tally.address_correct, tally.lines, "rate {rate} error {error}");
        }
    }
}

#[test]
fn search_window_limits_the_search() {
    let samples = line_with(&line_bytes(&known_packet()), &nominal(), RATE);
    let mut slicer = Slicer::new(Config { search_end: Some(100), ..Config::new() });
    assert_eq!(slice_bytes(&mut slicer, &samples, RATE), Err(Rejection::NoSignal));
    let mut slicer = Slicer::new(Config { search_start: 300, search_end: Some(400), ..Config::new() });
    assert_eq!(slice_bytes(&mut slicer, &samples, RATE).unwrap().packet, known_packet());
}

// ---------------------------------------------------------------------------
// REQ-SLC-003 and REQ-SLC-005: black level and amplitude
// ---------------------------------------------------------------------------

/// Slices `count` random packets with `params` and checks each packet and
/// the black level estimate, which should be within `tolerance` codes of
/// `black(params)`.
fn check_black(params: &SignalParams, count: usize, tolerance: f32, sync_offset_ns: Option<i32>) {
    let mut generator = Generator::new(15);
    let mut slicer = Slicer::new(Config::new());
    let expected = params.black_level + params.black_drift / 2.0;
    for _ in 0..count {
        let packet = random_packet(generator.rng());
        let samples = render(&mut generator, params, &line_bytes(&packet), samples_per_line(RATE));
        let line = Line { sync_offset_ns, ..Line::new(&samples, SampleFormat::U8, RATE) };
        let sliced = slicer.slice(&line).unwrap_or_else(|e| panic!("{params:?}: {e:?}"));
        assert_eq!(sliced.packet, packet);
        let black = slicer.black_level();
        assert!((black - expected).abs() <= tolerance, "black {black}, expected {expected}: {params:?}");
    }
}

#[test]
fn offset_black_levels() {
    // Black from code 20 to 100, the synchronising pulse clipped at code 0
    // for the lower ones, and the peaks clipped at 255 for the highest.
    for black in [20.0_f32, 36.0, 60.0, 100.0] {
        let params = SignalParams { black_level: black, ..nominal() };
        check_black(&params, 20, 1.0, None);
        check_black(&params, 5, 1.0, Some(0));
    }
}

#[test]
fn drifting_black_levels() {
    // A ramp across the line, in either direction, of up to 30 percent of
    // the data amplitude, with and without noise. The black level lies
    // somewhere on the ramp.
    for drift in [-0.3_f32, -0.15, 0.15, 0.3] {
        let params = SignalParams { black_level: 60.0, black_drift: drift * NOMINAL_AMPLITUDE_8BIT, ..nominal() };
        check_black(&params, 20, params.black_drift.abs() / 2.0, None);
        let noisy = SignalParams { noise_rms: params.noise_for_snr_db(20.0), ..params };
        let tally = run_batch(RATE, &noisy, 200, 16, Config::new());
        assert!(tally.address_percent() >= 99.0, "drift {drift}: {tally:?}");
    }
    // The black level changing from line to line.
    let mut generator = Generator::new(17);
    let mut slicer = Slicer::new(Config::new());
    for line in 0..40 {
        let black = 30.0 + 2.0 * line as f32;
        let params = SignalParams { black_level: black, ..nominal() };
        let packet = random_packet(generator.rng());
        let samples = render(&mut generator, &params, &line_bytes(&packet), samples_per_line(RATE));
        assert_eq!(slice_bytes(&mut slicer, &samples, RATE).unwrap().packet, packet);
        assert!((slicer.black_level() - black).abs() <= 1.0);
    }
}

#[test]
fn black_level_of_a_row_of_solid_mosaics() {
    // A row of solid mosaics is mostly "1": black must still be found,
    // from the back porch when the synchronising edge is known and from the
    // histogram when it is not.
    let mut bytes = line_bytes(&known_packet());
    for byte in &mut bytes[ADDRESS_INDEX + 2..] {
        *byte = with_odd_parity(0x7F);
    }
    let samples = line_with(&bytes, &SignalParams { black_level: 50.0, ..nominal() }, RATE);
    for sync_offset_ns in [Some(0), None, Some(20_000)] {
        let mut slicer = Slicer::new(Config::new());
        let line = Line { sync_offset_ns, ..Line::new(&samples, SampleFormat::U8, RATE) };
        assert!(slicer.slice(&line).is_ok());
        let black = slicer.black_level();
        assert!((black - 50.0).abs() <= 1.0, "{sync_offset_ns:?}: black {black}");
    }
}

#[test]
fn amplitude_working_range() {
    // The SAA5231 working range, 65 to 150 percent (REQ-SLC-005).
    for percent in [65.0_f32, 100.0, 150.0] {
        let params = SignalParams { data_amplitude: NOMINAL_AMPLITUDE_8BIT * percent / 100.0, ..nominal() };
        let samples = line_with(&line_bytes(&known_packet()), &params, RATE);
        let mut slicer = Slicer::new(Config::new());
        let sliced = slice_bytes(&mut slicer, &samples, RATE).unwrap();
        let measured = f32::from(sliced.meta.quality.unwrap().amplitude_percent);
        assert!((measured - percent).abs() <= 3.0, "{percent}: measured {measured}");
    }
    // The minimum is configurable.
    let params = SignalParams { data_amplitude: NOMINAL_AMPLITUDE_8BIT * 0.4, ..nominal() };
    let samples = line_with(&line_bytes(&known_packet()), &params, RATE);
    let mut slicer = Slicer::new(Config { min_amplitude_percent: 30, ..Config::new() });
    assert!(slice_bytes(&mut slicer, &samples, RATE).is_ok());
}

// ---------------------------------------------------------------------------
// REQ-SLC-008 and decision E.24: framing code
// ---------------------------------------------------------------------------

#[test]
fn every_single_bit_framing_error() {
    for bit in 0..BITS_PER_BYTE {
        let mut bytes = line_bytes(&known_packet());
        bytes[FRAMING_INDEX] ^= 1 << bit;
        let samples = line_with(&bytes, &nominal(), RATE);
        let mut slicer = Slicer::new(Config::new());
        let sliced = slice_bytes(&mut slicer, &samples, RATE).unwrap_or_else(|e| panic!("bit {bit}: {e:?}"));
        assert_eq!(sliced.packet, known_packet(), "bit {bit}");
        assert!(sliced.meta.quality.unwrap().framing_corrected, "bit {bit}");
        // The strict option refuses it.
        let mut strict = Slicer::new(Config { framing_tolerance: false, ..Config::new() });
        assert_eq!(slice_bytes(&mut strict, &samples, RATE), Err(Rejection::NoFramingCode), "bit {bit}");
    }
}

#[test]
fn corrected_framing_needs_a_valid_address() {
    let mut bytes = line_bytes(&known_packet());
    bytes[FRAMING_INDEX] ^= 0x10;
    bytes[ADDRESS_INDEX] ^= 0x0C;
    let samples = line_with(&bytes, &nominal(), RATE);
    let mut slicer = Slicer::new(Config::new());
    assert_eq!(slice_bytes(&mut slicer, &samples, RATE), Err(Rejection::BadAddress));
}

// ---------------------------------------------------------------------------
// REQ-SLC-010: other signals are rejected
// ---------------------------------------------------------------------------

/// Bits of a biphase-coded signal (each bit as two elements, `b` then
/// not `b`), packed into line bytes, first element first.
fn biphase(bits: &[bool]) -> LineBytes {
    let mut bytes = [0u8; PACKET_BYTES_WITH_RUN_IN];
    for (index, &bit) in bits.iter().enumerate().take(bytes.len() * BITS_PER_BYTE / 2) {
        for (half, value) in [(0, bit), (1, !bit)] {
            if value {
                let element = index * 2 + half;
                bytes[element / BITS_PER_BYTE] |= 1 << (element % BITS_PER_BYTE);
            }
        }
    }
    bytes
}

#[test]
fn non_teletext_lines_are_rejected() {
    let len = samples_per_line(RATE);
    let mut generator = Generator::new(18);
    let mut cases: Vec<(&str, Vec<u8>, Rejection)> = Vec::new();

    cases.push(("black", vec![16u8; len], Rejection::NoSignal));
    cases.push(("grey", vec![120u8; len], Rejection::NoSignal));
    let mut ramp = vec![0u8; len];
    for (index, sample) in ramp.iter_mut().enumerate() {
        *sample = (16 + index * 200 / len) as u8;
    }
    cases.push(("ramp", ramp, Rejection::NoSignal));
    let empty = [0u8; PACKET_BYTES_WITH_RUN_IN];
    let noisy = SignalParams { noise_rms: 20.0, ..nominal() };
    cases.push(("sync, burst and noise", render(&mut generator, &noisy, &empty, len), Rejection::NoSignal));
    // A tone at half the bit rate all along the line passes the run-in
    // search and the gate but has no framing code.
    let tone = [CLOCK_RUN_IN; PACKET_BYTES_WITH_RUN_IN];
    cases.push(("tone", render(&mut generator, &nominal(), &tone, len), Rejection::NoFramingCode));
    // System A's framing code is two bits from System B's.
    let mut system_a = line_bytes(&known_packet());
    system_a[FRAMING_INDEX] = SYSTEM_A_FRAMING;
    cases.push(("System A framing", render(&mut generator, &nominal(), &system_a, len), Rejection::NoFramingCode));

    for (name, samples, expected) in &cases {
        let mut slicer = Slicer::new(Config::new());
        assert_eq!(slice_bytes(&mut slicer, samples, RATE).err(), Some(*expected), "{name}");
    }

    // Wide-screen signalling's shape (line 23: biphase, 5 M elements a
    // second) with random contents is never taken for teletext.
    let (accepted, _) = biphase_false_packets(5.0e6, 200, Config::new());
    assert_eq!(accepted, 0, "WSS");
    // VPS's shape (line 16: biphase, 10 M elements a second) with random
    // contents has stretches close to half the teletext bit rate. Decision
    // E.24's one-bit framing tolerance lets a few through when the address
    // bytes happen to decode; the strict option almost none.
    let (accepted, lines) = biphase_false_packets(10.0e6, 500, Config::new());
    assert!(accepted * 100 <= lines * 3, "VPS: {accepted} of {lines}");
    let strict = Config { framing_tolerance: false, ..Config::new() };
    let (accepted, lines) = biphase_false_packets(10.0e6, 500, strict);
    assert!(accepted * 200 <= lines, "VPS strict: {accepted} of {lines}");
}

/// Slices `lines` biphase lines of random contents at `element_rate`;
/// returns the packets accepted and the lines.
fn biphase_false_packets(element_rate: f32, lines: u32, config: Config) -> (u32, u32) {
    let len = samples_per_line(RATE);
    let mut generator = Generator::new(18);
    let mut slicer = Slicer::new(config);
    let params = SignalParams {
        bit_rate_error: element_rate / BIT_RATE_HZ as f32 - 1.0,
        timing_offset_ns: -1500.0,
        ..nominal()
    };
    let mut accepted = 0;
    for _ in 0..lines {
        let bits: Vec<bool> = (0..180).map(|_| generator.rng().byte() & 1 == 1).collect();
        let samples = render(&mut generator, &params, &biphase(&bits), len);
        if slice_bytes(&mut slicer, &samples, RATE).is_ok() {
            accepted += 1;
        }
    }
    (accepted, lines)
}

#[test]
fn noise_lines_give_no_packets() {
    let len = samples_per_line(RATE);
    let mut generator = Generator::new(19);
    let mut slicer = Slicer::new(Config::new());
    let empty = [0u8; PACKET_BYTES_WITH_RUN_IN];
    for line in 0..300 {
        let params = SignalParams { noise_rms: 10.0 + (line % 6) as f32 * 10.0, ..nominal() };
        let samples = render(&mut generator, &params, &empty, len);
        assert!(slice_bytes(&mut slicer, &samples, RATE).is_err());
    }
}

// ---------------------------------------------------------------------------
// Untrusted input
// ---------------------------------------------------------------------------

#[test]
fn hostile_lines_do_not_panic() {
    let mut slicer = Slicer::new(Config::new());
    let long = vec![0x80u8; 20_000];
    let mut rng = Rng::new(20);
    let random: Vec<u8> = (0..4096).map(|_| rng.byte()).collect();
    let cases: [(&[u8], SampleFormat, u32); 9] = [
        (&[], SampleFormat::U8, RATE),
        (&[1, 2, 3], SampleFormat::U16Le, RATE),
        (&long, SampleFormat::U8, RATE),
        (&long, SampleFormat::U16Le, RATE),
        (&random, SampleFormat::U8, RATE),
        (&random, SampleFormat::U8, 13_000_000),
        (&random, SampleFormat::U8, 0),
        (&random, SampleFormat::U8, u32::MAX),
        (&random, SampleFormat::U16Le, 13_500_000),
    ];
    for (samples, format, rate) in cases {
        let _ = slicer.slice(&Line::new(samples, format, rate));
        let config = Config { search_start: usize::MAX, search_end: Some(usize::MAX), ..Config::new() };
        let _ = Slicer::new(config).slice(&Line::new(samples, format, rate));
        let line = Line { sync_offset_ns: Some(i32::MIN), ..Line::new(samples, format, rate) };
        let _ = slicer.slice(&line);
    }
    assert_eq!(slicer.counters().lines, 18);
}

// ---------------------------------------------------------------------------
// REQ-INP-011: raw sampled video files
// ---------------------------------------------------------------------------

/// Builds `frames` bt8x8 frames; returns the file and, per line index in
/// the file, the packet it carries.
fn bt8x8_file(frames: usize) -> (Vec<u8>, Vec<Option<Packet>>) {
    let geometry = RawGeometry::preset(Preset::Bt8x8);
    let mut generator = Generator::new(21);
    // The capture starts about 7 µs after the synchronising edge, so the
    // run-in begins some 110 samples in and the next line's sync is at the
    // end of the 2048 samples.
    let params = SignalParams { start_ns: 7_000.0, ..SignalParams::nominal(geometry.sample_rate_hz as f32) };
    let mut file = Vec::new();
    let mut packets = Vec::new();
    for frame in 0..frames {
        for line in 0..geometry.lines_per_frame() {
            // Every third line is empty.
            let packet = (line % 3 != 2).then(|| random_packet(generator.rng()));
            let bytes = packet.as_ref().map_or([0u8; PACKET_BYTES_WITH_RUN_IN], line_bytes);
            file.extend(render(&mut generator, &params, &bytes, geometry.samples_per_line));
            packets.push(packet);
        }
        // The driver's frame sequence number in the last four bytes.
        let end = file.len();
        file[end - 4..].copy_from_slice(&(frame as u32 * 0x0101_0101 + 0x8000_00FF).to_le_bytes());
    }
    (file, packets)
}

#[test]
fn raw_bt8x8_file_in_odd_chunks() {
    let (file, packets) = bt8x8_file(3);
    let geometry = RawGeometry::preset(Preset::Bt8x8);
    let mut reader = RawReader::new(geometry, Config::new()).unwrap();
    let mut received: Vec<(Packet, PacketMeta)> = Vec::new();
    let sizes = [1usize, 7, 333, 4097, 2048, 65_539, 13];
    let mut offset = 0;
    let mut size_index = 0;
    while offset < file.len() {
        let size = sizes[size_index % sizes.len()].min(file.len() - offset);
        reader.feed(&file[offset..offset + size], |packet, meta| received.push((packet, meta)));
        offset += size;
        size_index += 1;
    }
    assert_eq!(reader.frames(), 3);
    let expected: Vec<(usize, Packet)> =
        packets.iter().enumerate().filter_map(|(index, packet)| packet.map(|p| (index, p))).collect();
    assert_eq!(received.len(), expected.len());
    for ((packet, meta), (index, want)) in received.iter().zip(&expected) {
        assert_eq!(packet, want, "line {index}");
        let in_frame = index % geometry.lines_per_frame();
        let field = in_frame / geometry.lines_per_field;
        let line = [7u16, 320][field] + (in_frame % geometry.lines_per_field) as u16;
        assert_eq!(meta.line, Some(line));
        assert_eq!(meta.second_field, Some(field == 1));
        let fields = (index / geometry.lines_per_frame() * 2 + field) as u64;
        assert_eq!(meta.timestamp_us, Some(fields * 20_000));
        assert!(meta.quality.is_some());
    }
}

#[test]
fn raw_reader_other_presets() {
    // One frame of each 16-line preset with a packet on every line.
    for preset in [Preset::Cx88, Preset::Saa7131, Preset::TbcVbi] {
        let geometry = RawGeometry::preset(preset);
        let mut generator = Generator::new(22);
        let rate = geometry.sample_rate_hz;
        // Start each window so that it holds the whole packet; the saa7131
        // window is only 53.3 µs, so it must start just before the data.
        let start = match preset {
            Preset::Saa7131 => 9_500.0,
            Preset::Cx88 => 6_000.0,
            Preset::Bt8x8 | Preset::Tbc | Preset::TbcVbi => 4_000.0,
        };
        let params = SignalParams { start_ns: start, ..SignalParams::nominal(rate as f32) };
        let mut file = Vec::new();
        let mut used = 0;
        for line in 0..geometry.lines_per_frame() {
            let packet = random_packet(generator.rng());
            let mut values = vec![0.0_f32; geometry.samples_per_line];
            generator.line(&params, &line_bytes(&packet), &mut values);
            let mut bytes = vec![0u8; geometry.line_bytes()];
            match geometry.format {
                SampleFormat::U8 => quantise_u8(&values, &mut bytes),
                SampleFormat::U16Le => quantise_u16le(&values, SAMPLE_16_PER_8 as f32, &mut bytes),
            }
            file.extend(bytes);
            let in_field = line % geometry.lines_per_field;
            if in_field >= geometry.first_used && in_field <= geometry.last_used {
                used += 1;
            }
        }
        let mut reader = RawReader::new(geometry, Config::new()).unwrap();
        let mut count = 0;
        for chunk in file.chunks(1001) {
            reader.feed(chunk, |_, meta| {
                assert_eq!(meta.line, None);
                count += 1;
            });
        }
        assert_eq!(count, used, "{preset:?}");
    }
}

#[test]
fn raw_geometry_presets_and_validation() {
    let bt = RawGeometry::preset(Preset::Bt8x8);
    assert_eq!(bt, RawGeometry::default());
    assert_eq!((bt.sample_rate_hz, bt.samples_per_line, bt.lines_per_field), (35_468_950, 2048, 16));
    let tbc = RawGeometry::preset(Preset::Tbc);
    assert_eq!((tbc.sample_rate_hz, tbc.samples_per_line, tbc.format), (17_730_000, 1135, SampleFormat::U16Le));
    assert_eq!((tbc.lines_per_field, tbc.first_used, tbc.last_used), (313, 6, 21));
    let cx = RawGeometry::preset(Preset::Cx88);
    assert_eq!((cx.lines_per_field, cx.first_used, cx.last_used), (18, 1, 16));
    for preset in [Preset::Bt8x8, Preset::Cx88, Preset::Saa7131, Preset::Tbc, Preset::TbcVbi] {
        assert_eq!(RawGeometry::preset(preset).validate(), Ok(()));
    }
    let bad = |change: fn(&mut RawGeometry)| {
        let mut geometry = bt;
        change(&mut geometry);
        RawReader::new(geometry, Config::new()).err()
    };
    assert_eq!(bad(|g| g.samples_per_line = 0), Some(GeometryError::LineLength));
    assert_eq!(bad(|g| g.samples_per_line = 1 << 20), Some(GeometryError::LineLength));
    assert_eq!(bad(|g| g.lines_per_field = 0), Some(GeometryError::NoLines));
    assert_eq!(bad(|g| g.last_used = 16), Some(GeometryError::UsedLines));
    assert_eq!(
        bad(|g| {
            g.first_used = 10;
            g.last_used = 5;
        }),
        Some(GeometryError::UsedLines)
    );
    assert_eq!(bad(|g| g.frame_trailer_bytes = 2048), Some(GeometryError::Trailer));
}

// ---------------------------------------------------------------------------
// Measurements for the report
// ---------------------------------------------------------------------------

#[test]
#[ignore = "measurement report; run in release mode"]
fn report_noise() {
    std::println!("REQ-SLC-013 noise (5000 lines each):");
    for rate in RATES {
        for snr in [20.0_f32, 15.0, 10.0] {
            let nominal = SignalParams::nominal(rate as f32);
            let params = SignalParams { noise_rms: nominal.noise_for_snr_db(snr), ..nominal };
            let t = run_batch(rate, &params, 5000, 2, Config::new());
            std::println!(
                "  {:>6.3} MHz {snr:>4} dB: addresses {:6.2}%  whole packets {:6.2}%  {:?}",
                f64::from(rate) / 1e6,
                t.address_percent(),
                t.exact_percent(),
                t.counters
            );
        }
    }
}

#[test]
#[ignore = "measurement report; run in release mode"]
fn report_impairments() {
    std::println!("Bit-rate error, fixed bit period (500 lines each):");
    for rate in RATES {
        for error in [0.0005_f32, 0.001, 0.0015, 0.002, 0.003, 0.005] {
            for sign in [-1.0_f32, 1.0] {
                let params = SignalParams { bit_rate_error: sign * error, ..SignalParams::nominal(rate as f32) };
                let t = run_batch(rate, &params, 500, 4, Config::new());
                std::println!(
                    "  {:>6.3} MHz {:+.2}%: addresses {:6.2}%  whole packets {:6.2}%",
                    f64::from(rate) / 1e6,
                    sign * error * 100.0,
                    t.address_percent(),
                    t.exact_percent()
                );
            }
        }
    }
    std::println!("High-frequency loss, first-order low-pass (500 lines each):");
    for rate in RATES {
        for corner in [5.0e6_f32, 4.0e6, 3.0e6, 2.5e6, 2.0e6] {
            let params = SignalParams { low_pass_hz: Some(corner), ..SignalParams::nominal(rate as f32) };
            let t = run_batch(rate, &params, 500, 5, Config::new());
            std::println!(
                "  {:>6.3} MHz corner {:.1} MHz: addresses {:6.2}%  whole packets {:6.2}%",
                f64::from(rate) / 1e6,
                corner / 1e6,
                t.address_percent(),
                t.exact_percent()
            );
        }
    }
    std::println!("Echo (500 lines each):");
    for rate in RATES {
        for (delay, gain) in
            [(200.0_f32, 0.2_f32), (500.0, 0.2), (800.0, 0.2), (200.0, 0.4), (500.0, 0.4), (800.0, -0.3)]
        {
            let params =
                SignalParams { echo: Some(Echo { delay_ns: delay, gain }), ..SignalParams::nominal(rate as f32) };
            let t = run_batch(rate, &params, 500, 6, Config::new());
            std::println!(
                "  {:>6.3} MHz {delay} ns x {gain}: addresses {:6.2}%  whole packets {:6.2}%",
                f64::from(rate) / 1e6,
                t.address_percent(),
                t.exact_percent()
            );
        }
    }
    std::println!("Black-level drift across the line, percent of data amplitude (500 lines each, 35.47 MHz):");
    for drift in [0.3_f32, 0.4, 0.45, 0.5, 0.6] {
        for sign in [-1.0_f32, 1.0] {
            let params = SignalParams {
                black_level: 80.0,
                black_drift: sign * drift * NOMINAL_AMPLITUDE_8BIT,
                ..SignalParams::nominal(RATE as f32)
            };
            let t = run_batch(RATE, &params, 500, 7, Config::new());
            std::println!(
                "  {:+.0}%: addresses {:6.2}%  whole packets {:6.2}%",
                sign * drift * 100.0,
                t.address_percent(),
                t.exact_percent()
            );
        }
    }
    std::println!("Random data after a valid run-in (false packets, 20000 lines, 35.47 MHz):");
    for tolerance in [true, false] {
        let mut generator = Generator::new(8);
        let mut slicer = Slicer::new(Config { framing_tolerance: tolerance, ..Config::new() });
        let mut accepted = 0;
        for _ in 0..20_000 {
            let mut bytes = [0u8; PACKET_BYTES_WITH_RUN_IN];
            for byte in &mut bytes {
                *byte = generator.rng().byte();
            }
            bytes[0] = CLOCK_RUN_IN;
            bytes[1] = CLOCK_RUN_IN;
            let samples = render(&mut generator, &SignalParams::nominal(RATE as f32), &bytes, samples_per_line(RATE));
            if slice_bytes(&mut slicer, &samples, RATE).is_ok() {
                accepted += 1;
            }
        }
        std::println!("  framing tolerance {tolerance}: {accepted} accepted, {:?}", slicer.counters());
    }
    std::println!("Biphase lines of random contents (false packets, 20000 lines, 35.47 MHz):");
    for (name, element_rate) in [("VPS shape", 10.0e6_f32), ("WSS shape", 5.0e6)] {
        for tolerance in [true, false] {
            let config = Config { framing_tolerance: tolerance, ..Config::new() };
            let (accepted, lines) = biphase_false_packets(element_rate, 20_000, config);
            std::println!("  {name}, framing tolerance {tolerance}: {accepted} of {lines} accepted");
        }
    }
}
