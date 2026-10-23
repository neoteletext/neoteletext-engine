//! REQ-SLC-003 and REQ-SLC-005: black level and amplitude.
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
