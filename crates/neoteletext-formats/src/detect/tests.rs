//! Detection tests for every format, including corrupted starts
//! (REQ-INP-005), and the automatic reader. The tests of a format run when
//! its feature is compiled in; the tests of formats left out check that
//! they are reported, not read (decision E.61).
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation
)]

use super::*;
#[cfg(all(
    feature = "transport",
    feature = "sliced",
    feature = "tti",
    feature = "ep1",
    feature = "edittf",
    feature = "dump"
))]
use crate::consts::DUMP_BYTES_25;
#[cfg(feature = "sliced")]
use crate::consts::{SLICED_HEADER_BYTES, SLICED_RECORD_BYTES, SLICED_TELETEXT_B};
#[cfg(feature = "tti")]
use crate::encode::encode_page;
use crate::item::{Diagnostic, DiagnosticKind};
use crate::testing::{Owned, diagnostics, packets, pages, same_in_any_chunks};
use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::ecc::reverse_bits;
use neoteletext_core::packet::{Packet, encode};
use neoteletext_core::types::Magazine;
use std::vec::Vec;

/// Packets with valid addresses and pseudo-random bodies.
fn sample_packets(count: usize) -> Vec<Packet> {
    let mut seed = 0x1234_5678u32;
    (0..count)
        .map(|index| {
            let mut packet = encode::address(Magazine::new((index % 8 + 1) as u8).unwrap(), (index % 26) as u8);
            for byte in packet.bytes.iter_mut().skip(2) {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                *byte = (seed >> 16) as u8;
            }
            packet
        })
        .collect()
}

/// A byte no Hamming 8/4 decoder can correct, so that no packet layout fits
/// a stream of it.
fn uncorrectable() -> u8 {
    (0..=u8::MAX).find(|&byte| neoteletext_core::ecc::hamming84(byte).is_none()).unwrap()
}

fn serialise(variant: T42Variant, packets: &[Packet]) -> Vec<u8> {
    let mut out = Vec::new();
    for packet in packets {
        out.extend_from_slice(variant.prefix());
        out.extend(
            packet.bytes.iter().map(|&byte| if variant == T42Variant::Reversed { reverse_bits(byte) } else { byte }),
        );
    }
    out
}

#[test]
fn detects_each_t42_layout_even_with_a_corrupted_start() {
    let packets = sample_packets(DETECTION_BYTES / PACKET_BYTES + 20);
    for variant in T42Variant::ALL {
        let stream = serialise(variant, &packets);
        assert_eq!(detect(&stream, false), Detection::Found { format: Format::T42(variant), skip: 0 }, "{variant:?}");
        // Drop the first 17 bytes: the first packet is cut short.
        let cut = &stream[17..];
        let skip = variant.unit_bytes() - 17;
        assert_eq!(detect(cut, false), Detection::Found { format: Format::T42(variant), skip }, "{variant:?} cut");
    }
    let short = serialise(T42Variant::Plain, &packets[..10]);
    assert_eq!(detect(&short, false), Detection::NeedMore);
    assert_eq!(detect(&short, true), Detection::Found { format: Format::T42(T42Variant::Plain), skip: 0 });
}

/// Every format detection can choose.
fn every_format() -> Vec<Format> {
    let mut formats: Vec<Format> = T42Variant::ALL.into_iter().map(Format::T42).collect();
    formats.extend(TsPacketSize::ALL.map(Format::TransportStream));
    formats.extend([
        Format::Pes,
        Format::Sliced(ByteOrder::Little),
        Format::Sliced(ByteOrder::Big),
        Format::Tti,
        Format::Ep1,
        Format::EditTf,
        Format::ScreenDump,
    ]);
    formats
}

/// Sliced records of teletext packets.
#[cfg(feature = "sliced")]
fn sliced_stream(order: ByteOrder) -> Vec<u8> {
    let mut stream = Vec::new();
    for (index, packet) in sample_packets(40).iter().enumerate() {
        let words = [SLICED_TELETEXT_B, (index % 2) as u32, 7, 0];
        for word in words {
            stream.extend_from_slice(&match order {
                ByteOrder::Little => word.to_le_bytes(),
                ByteOrder::Big => word.to_be_bytes(),
            });
        }
        stream.extend_from_slice(&packet.bytes);
        stream.extend_from_slice(&[0; SLICED_RECORD_BYTES - SLICED_HEADER_BYTES - PACKET_BYTES]);
    }
    stream
}

/// With every format compiled in, each format is compiled, read by its own
/// reader and found by detection in a sample of it. The match is
/// exhaustive, so a new format needs a sample here.
#[cfg(all(
    feature = "transport",
    feature = "sliced",
    feature = "tti",
    feature = "ep1",
    feature = "edittf",
    feature = "dump"
))]
#[test]
fn detection_finds_every_format() {
    let transport =
        |prefix, suffix| crate::transport::tests::flatten(&crate::transport::tests::stream(), prefix, suffix);
    for format in every_format() {
        let sample = match format {
            Format::T42(variant) => serialise(variant, &sample_packets(DETECTION_PACKETS)),
            Format::TransportStream(TsPacketSize::Standard) => transport(0, 0),
            Format::TransportStream(TsPacketSize::M2ts) => transport(4, 0),
            Format::TransportStream(TsPacketSize::Fec) => transport(0, 16),
            Format::Pes => crate::pes::tests::pes_packet(0, &[]),
            Format::Sliced(order) => sliced_stream(order),
            Format::Tti => b"DE,page\nPN,10000\n".to_vec(),
            Format::Ep1 => crate::ep1::tests::ep1_file(0x09, &[]),
            Format::EditTf => b"https://edit.tf/#0:QIECBAg".to_vec(),
            Format::ScreenDump => std::vec![neoteletext_core::consts::SPACE; DUMP_BYTES_25],
        };
        assert!(format.is_compiled(), "{format:?}");
        assert!(!matches!(AnyReader::new(format, 0, &AutoConfig::default()), AnyReader::NotCompiled(_)), "{format:?}");
        assert_eq!(detect(&sample, true), Detection::Found { format, skip: 0 }, "{format:?}");
    }
}

/// A format asked for that is not compiled in is reported once, and its
/// input ignored; a compiled format is never reported missing.
#[test]
fn formats_not_compiled_are_reported() {
    for format in every_format() {
        let config = AutoConfig { format: Some(format), encode_pages: true, ..AutoConfig::default() };
        let items = same_in_any_chunks(|| AutoReader::new(config), b"PN,10000\nOL,1,text\n");
        let missing = Diagnostic::at(0, DiagnosticKind::FormatNotCompiled { format });
        if format.is_compiled() {
            assert!(!diagnostics(&items).contains(&missing), "{format:?}");
        } else {
            assert_eq!(items, [Owned::Diagnostic(missing)], "{format:?}");
        }
    }
}

/// Without the TTI reader, TTI text is not taken for TTI.
#[cfg(not(feature = "tti"))]
#[test]
fn tti_is_not_detected_without_its_reader() {
    let text = b"DE,page\nPN,10000\nOL,1,text\n";
    assert!(!Format::Tti.is_compiled());
    assert!(!matches!(detect(text, true), Detection::Found { format: Format::Tti, .. }));
    let items = same_in_any_chunks(|| AutoReader::new(AutoConfig::default()), text);
    assert!(!items.iter().any(|item| matches!(item, Owned::Other(text) if text.contains("Tti"))));
    assert_eq!(pages(&items), []);
}

#[cfg(all(feature = "transport", feature = "tti", feature = "ep1", feature = "edittf", feature = "dump"))]
#[test]
fn detects_signatures() {
    let transport = crate::transport::tests::flatten(&crate::transport::tests::stream(), 0, 0);
    let mut padded = transport.clone();
    while padded.len() < DETECTION_BYTES {
        padded.extend_from_slice(&transport);
    }
    assert_eq!(
        detect(&padded, false),
        Detection::Found { format: Format::TransportStream(TsPacketSize::Standard), skip: 0 }
    );
    assert_eq!(
        detect(&transport, true),
        Detection::Found { format: Format::TransportStream(TsPacketSize::Standard), skip: 0 }
    );
    let m2ts = crate::transport::tests::flatten(&crate::transport::tests::stream(), 4, 0);
    assert_eq!(detect(&m2ts, true), Detection::Found { format: Format::TransportStream(TsPacketSize::M2ts), skip: 0 });
    let fec = crate::transport::tests::flatten(&crate::transport::tests::stream(), 0, 16);
    assert_eq!(detect(&fec, true), Detection::Found { format: Format::TransportStream(TsPacketSize::Fec), skip: 0 });
    let pes = crate::pes::tests::pes_packet(0, &[]);
    assert_eq!(detect(&pes, true), Detection::Found { format: Format::Pes, skip: 0 });
    let ep1 = crate::ep1::tests::ep1_file(0x09, &[]);
    assert_eq!(detect(&ep1, true), Detection::Found { format: Format::Ep1, skip: 0 });
    assert_eq!(detect(b"\xEF\xBB\xBFDE,page\nPN,10000\n", true), Detection::Found { format: Format::Tti, skip: 3 });
    assert_eq!(detect(b"https://edit.tf/#0:QIECBAg", true), Detection::Found { format: Format::EditTf, skip: 0 });
    assert_eq!(detect(&[0x20; 1000], true), Detection::Found { format: Format::ScreenDump, skip: 0 });
    assert_eq!(detect(&[0xA0; 960], true), Detection::Found { format: Format::ScreenDump, skip: 0 });
    assert_eq!(detect(&std::vec![uncorrectable(); 20_000], true), Detection::Unknown);
}

#[cfg(feature = "sliced")]
#[test]
fn detects_sliced_records_in_both_byte_orders() {
    for order in [ByteOrder::Little, ByteOrder::Big] {
        let stream = sliced_stream(order);
        assert_eq!(detect(&stream, true), Detection::Found { format: Format::Sliced(order), skip: 0 }, "{order:?}");
    }
}

#[test]
fn auto_reader_detects_then_reads_in_any_chunks() {
    let packets_in = sample_packets(DETECTION_PACKETS * 2);
    let mut stream = std::vec![0xAA; 5];
    stream.extend(serialise(T42Variant::Reversed, &packets_in));
    let items = same_in_any_chunks(|| AutoReader::new(AutoConfig::default()), &stream);
    assert!(matches!(&items[0], Owned::Other(text) if text.contains("Reversed")));
    assert_eq!(packets(&items), packets_in);
    assert_eq!(diagnostics(&items), []);
}

#[cfg(feature = "tti")]
#[test]
fn auto_reader_encodes_pages_when_asked() {
    let text = crate::tti::tests::FORM_EXAMPLE;
    let config = AutoConfig { encode_pages: true, source: Some(3), ..AutoConfig::default() };
    let items = same_in_any_chunks(|| AutoReader::new(config), text);
    let page = pages(&items)[0];
    let mut expected = Vec::new();
    encode_page(&page, &mut |packet| expected.push(packet));
    assert_eq!(packets(&items), expected);
    assert!(items.iter().any(|item| matches!(item, Owned::Packet(_, meta) if meta.source == Some(3))));
    // An explicit format skips detection.
    let config = AutoConfig { format: Some(Format::Tti), ..AutoConfig::default() };
    let items = same_in_any_chunks(|| AutoReader::new(config), text);
    assert!(!items.iter().any(|item| matches!(item, Owned::Other(_))));
    assert_eq!(pages(&items).len(), 1);
}

#[test]
fn auto_reader_reports_unknown_input() {
    let items = same_in_any_chunks(|| AutoReader::new(AutoConfig::default()), &std::vec![uncorrectable(); 20_000]);
    assert_eq!(diagnostics(&items), [Diagnostic::at(0, DiagnosticKind::UnknownFormat)]);
}
