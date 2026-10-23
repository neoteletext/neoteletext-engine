//! REQ-FSM-009: one test per row of the REQ-FSM-002 table.
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

use super::*;

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
