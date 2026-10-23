//! PES parser tests, with the data-unit and PES-packet builders the transport and detection tests share.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

use super::*;
use crate::consts::{
    DATA_UNIT_FIELD_PARITY, DATA_UNIT_INVERTED, DATA_UNIT_SUBTITLE, DATA_UNIT_TELETEXT, DATA_UNIT_TELETEXT_LENGTH,
    PES_PTS_PRESENT, STREAM_ID_PRIVATE_1,
};
use neoteletext_core::consts::{FRAMING_CODE_DVB, FRAMING_CODE_DVB_INVERTED, PACKET_BYTES};
use neoteletext_core::ecc::reverse_bits;
use neoteletext_core::packet::{Packet, PacketMeta};
use std::vec::Vec;

/// A teletext data unit for `packet` on a given line of the first field.
pub(crate) fn data_unit(id: u8, packet: &Packet, line: u8) -> Vec<u8> {
    let framing = if id == DATA_UNIT_INVERTED { FRAMING_CODE_DVB_INVERTED } else { FRAMING_CODE_DVB };
    let mut unit = std::vec![id, DATA_UNIT_TELETEXT_LENGTH as u8, 0xC0 | DATA_UNIT_FIELD_PARITY | line, framing];
    unit.extend(packet.bytes.iter().map(|&byte| reverse_bits(byte)));
    unit
}

/// A PES packet with a PTS and the given data units, padded with
/// stuffing to the EN 300 472 header length.
pub(crate) fn pes_packet(pts: u64, units: &[Vec<u8>]) -> Vec<u8> {
    let mut body = std::vec![0x80, PES_PTS_PRESENT, 0x24];
    let pts_bytes = [
        0x21 | ((pts >> 29) & 0x0E) as u8,
        (pts >> 22) as u8,
        0x01 | ((pts >> 14) & 0xFE) as u8,
        (pts >> 7) as u8,
        0x01 | ((pts << 1) & 0xFE) as u8,
    ];
    body.extend_from_slice(&pts_bytes);
    body.resize(3 + 0x24, 0xFF);
    body.push(DATA_IDENTIFIER_EBU_FIRST);
    for unit in units {
        body.extend_from_slice(unit);
    }
    let mut packet = std::vec![0, 0, 1, STREAM_ID_PRIVATE_1];
    packet.extend_from_slice(&(body.len() as u16).to_be_bytes());
    packet.extend_from_slice(&body);
    packet
}

fn sample(seed: u8) -> Packet {
    let mut bytes = [0; PACKET_BYTES];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = seed.wrapping_add(index as u8);
    }
    Packet { bytes }
}

fn read(input: &[u8], chunk: usize) -> Vec<(Option<Packet>, Option<PacketMeta>, Option<Diagnostic>)> {
    let mut reader = PesReader::new(Some(1));
    let mut items = Vec::new();
    let mut out = |item: Item<'_>| match item {
        Item::Packet { packet, meta } => items.push((Some(packet), Some(meta), None)),
        Item::Diagnostic(diagnostic) => items.push((None, None, Some(diagnostic))),
        _ => {}
    };
    for piece in input.chunks(chunk) {
        reader.feed(piece, &mut out);
    }
    reader.finish(&mut out);
    items
}

#[test]
fn reads_packets_with_line_field_and_time() {
    let units = [data_unit(DATA_UNIT_TELETEXT, &sample(1), 7), data_unit(DATA_UNIT_SUBTITLE, &sample(2), 21)];
    let mut stuffing = std::vec![0xFF, 3, 0, 0, 0];
    stuffing.extend(data_unit(DATA_UNIT_INVERTED, &sample(3), 9));
    let pes = pes_packet(90_000, &[units[0].clone(), stuffing, units[1].clone()]);
    let whole = read(&pes, pes.len());
    assert_eq!(whole.len(), 3, "{whole:?}");
    let (packet, meta, _) = whole[0];
    assert_eq!(packet, Some(sample(1)));
    let meta = meta.unwrap();
    assert_eq!(meta.line, Some(7));
    assert_eq!(meta.second_field, Some(false));
    assert_eq!(meta.timestamp_us, Some(1_000_000));
    assert_eq!(meta.source, Some(1));
    assert_eq!(whole[1].0, Some(sample(3)));
    assert_eq!(whole[2].0, Some(sample(2)));
    assert_eq!(whole[2].1.unwrap().line, Some(21));
    for chunk in [1, 2, 13, 46] {
        assert_eq!(read(&pes, chunk), whole);
    }
}

#[test]
fn rejects_bad_identifier_and_framing_and_resynchronises() {
    let mut bad_framing = data_unit(DATA_UNIT_TELETEXT, &sample(1), 7);
    bad_framing[3] = 0x00;
    let mut stream = std::vec![0xAA, 0xBB];
    stream.extend(pes_packet(0, &[bad_framing, data_unit(DATA_UNIT_TELETEXT, &sample(4), 8)]));
    let mut bad_identifier = pes_packet(0, &[data_unit(DATA_UNIT_TELETEXT, &sample(5), 8)]);
    let identifier_at = stream.len() + 6 + 3 + 0x24;
    bad_identifier[6 + 3 + 0x24] = 0x20;
    stream.extend(bad_identifier);
    let got = read(&stream, 1);
    let diagnostics: Vec<_> = got.iter().filter_map(|item| item.2).collect();
    let packets: Vec<_> = got.iter().filter_map(|item| item.0).collect();
    assert_eq!(packets, [sample(4)]);
    assert_eq!(diagnostics[0], Diagnostic::at(0, DiagnosticKind::SkippedBytes { count: 2 }));
    assert_eq!(diagnostics[1].kind, DiagnosticKind::BadFramingCode { value: 0 });
    assert_eq!(diagnostics[2], Diagnostic::at(identifier_at as u64, DiagnosticKind::BadDataIdentifier { value: 0x20 }));
}

#[test]
fn skips_other_streams() {
    let mut stream = std::vec![0, 0, 1, 0xE0, 0, 3, 1, 2, 3];
    stream.extend(pes_packet(0, &[data_unit(DATA_UNIT_TELETEXT, &sample(9), 8)]));
    let got = read(&stream, 4);
    assert_eq!(got[0].2.unwrap().kind, DiagnosticKind::NotPrivateStream { stream_id: 0xE0 });
    assert_eq!(got[1].0, Some(sample(9)));
}

#[test]
fn pts_conversion() {
    let pts = 0x1_2345_6789_u64 & ((1 << 33) - 1);
    let pes = pes_packet(pts, &[data_unit(DATA_UNIT_TELETEXT, &sample(1), 7)]);
    let got = read(&pes, 7);
    assert_eq!(got[0].1.unwrap().timestamp_us, Some(pts * 100 / 9));
}
