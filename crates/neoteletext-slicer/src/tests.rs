//! Slicer tests with synthetic signals (REQ-TST-060, REQ-FSM-009).
//!
//! Tests marked `#[ignore]` print measurements for the report; run them
//! with `cargo test -p neoteletext-slicer --release -- --ignored --nocapture`.
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

use neoteletext_core::consts::{BIT_RATE_HZ, CLOCK_RUN_IN, PACKET_BYTES_WITH_RUN_IN};
use neoteletext_core::ecc::{hamming84, with_odd_parity};
use neoteletext_core::packet::{Packet, PacketMeta, encode};
use neoteletext_core::trace::{Machine, TraceEntry};
use neoteletext_core::types::Magazine;

use crate::consts::{
    BITS_PER_BYTE, NOMINAL_AMPLITUDE_8BIT, SAMPLE_16_PER_8, TIMING_EARLY_NS, TIMING_LATE_NS, TIMING_REFERENCE_BIT,
    TIMING_REFERENCE_NS,
};
use crate::fsm::{SlicerAction, SlicerEvent, SlicerState};
use crate::generator::{Echo, Generator, LineBytes, Rng, SignalParams, line_bytes, quantise_u8, quantise_u16le};
use crate::raw::{GeometryError, Preset, RawGeometry, RawReader};
use crate::slicer::{Config, Counters, Line, Rejection, SampleFormat, Sliced, Slicer};

mod framing;
mod fsm_rows;
mod levels;
mod raw;
mod rejection;
mod reports;
mod signal;

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
