//! REQ-SLC-010: other signals are rejected, and untrusted input never panics.
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
pub(super) fn biphase_false_packets(element_rate: f32, lines: u32, config: Config) -> (u32, u32) {
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
