//! Measurements for the report, run with `--ignored` in release mode.
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

use super::rejection::biphase_false_packets;
use super::*;

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
