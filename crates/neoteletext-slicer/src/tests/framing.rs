//! REQ-SLC-008 and decision E.24: framing code.
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
