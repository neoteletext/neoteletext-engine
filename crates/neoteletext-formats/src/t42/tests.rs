//! T42 reader tests: the three unit sizes, bit reversal, resynchronisation and skipped bytes.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use super::*;
use neoteletext_core::packet::encode;
use neoteletext_core::types::Magazine;
use std::vec::Vec;

pub(crate) fn sample_packets(count: usize) -> Vec<Packet> {
    (0..count)
        .map(|index| {
            let mut packet = encode::address(Magazine::new((index % 8 + 1) as u8).unwrap(), (index % 26) as u8);
            for (byte_index, byte) in packet.bytes.iter_mut().enumerate().skip(2) {
                *byte = (index * 7 + byte_index) as u8;
            }
            packet
        })
        .collect()
}

pub(crate) fn serialise(variant: T42Variant, packets: &[Packet]) -> Vec<u8> {
    let mut out = Vec::new();
    for packet in packets {
        out.extend_from_slice(variant.prefix());
        for &byte in &packet.bytes {
            out.push(if variant == T42Variant::Reversed { reverse_bits(byte) } else { byte });
        }
    }
    out
}

fn read(variant: T42Variant, input: &[u8], chunk: usize) -> (Vec<Packet>, Vec<Diagnostic>) {
    let mut reader = T42Reader::new(variant, None);
    let mut packets = Vec::new();
    let mut diagnostics = Vec::new();
    let mut out = |item: Item<'_>| match item {
        Item::Packet { packet, .. } => packets.push(packet),
        Item::Diagnostic(diagnostic) => diagnostics.push(diagnostic),
        _ => {}
    };
    for piece in input.chunks(chunk.max(1)) {
        reader.feed(piece, &mut out);
    }
    reader.finish(&mut out);
    (packets, diagnostics)
}

#[test]
fn every_variant_round_trips_in_any_chunk_size() {
    let packets = sample_packets(20);
    for variant in T42Variant::ALL {
        let stream = serialise(variant, &packets);
        let whole = read(variant, &stream, stream.len());
        assert_eq!(whole.0, packets, "{variant:?}");
        assert_eq!(whole.1, []);
        for chunk in [1, 5, 41, 42, 43, 100] {
            assert_eq!(read(variant, &stream, chunk), whole, "{variant:?} chunk {chunk}");
        }
    }
}

#[test]
fn framed_streams_resynchronise_after_garbage() {
    let packets = sample_packets(5);
    for variant in [T42Variant::Framed, T42Variant::RunIn] {
        let mut stream = serialise(variant, &packets[..2]);
        let garbage_at = stream.len();
        stream.extend_from_slice(&[1, 2, 3]);
        stream.extend_from_slice(&serialise(variant, &packets[2..]));
        for chunk in [1, 7, stream.len()] {
            let (got, diagnostics) = read(variant, &stream, chunk);
            assert_eq!(got, packets, "{variant:?}");
            assert_eq!(diagnostics, [Diagnostic::at(garbage_at as u64, DiagnosticKind::SkippedBytes { count: 3 })]);
        }
    }
}

#[test]
fn run_in_with_an_extra_run_in_byte_resynchronises() {
    let packets = sample_packets(2);
    let mut stream = std::vec![CLOCK_RUN_IN];
    stream.extend_from_slice(&serialise(T42Variant::RunIn, &packets));
    let (got, diagnostics) = read(T42Variant::RunIn, &stream, 1);
    assert_eq!(got, packets);
    assert_eq!(diagnostics, [Diagnostic::at(0, DiagnosticKind::SkippedBytes { count: 1 })]);
}

#[test]
fn truncated_tail_is_reported_with_its_offset() {
    let packets = sample_packets(3);
    let mut stream = serialise(T42Variant::Plain, &packets);
    stream.truncate(100);
    let (got, diagnostics) = read(T42Variant::Plain, &stream, 9);
    assert_eq!(got.len(), 2);
    assert_eq!(diagnostics, [Diagnostic::at(84, DiagnosticKind::Truncated { bytes: 16 })]);
}

#[test]
fn skip_discards_a_partial_first_packet() {
    let packets = sample_packets(3);
    let mut stream = std::vec![9, 9, 9];
    stream.extend_from_slice(&serialise(T42Variant::Plain, &packets));
    let mut reader = T42Reader::with_skip(T42Variant::Plain, Some(4), 3);
    let mut got = Vec::new();
    for byte in &stream {
        reader.feed(core::slice::from_ref(byte), &mut |item| {
            if let Item::Packet { packet, meta } = item {
                assert_eq!(meta.source, Some(4));
                got.push(packet);
            }
        });
    }
    assert_eq!(got, packets);
}
