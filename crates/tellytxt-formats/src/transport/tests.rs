//! Transport stream tests: a stream built here with a PAT, a PMT whose
//! teletext descriptor lists two languages, and PES packets of data units.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

use super::*;
use crate::consts::DATA_UNIT_TELETEXT;
use crate::pes::tests::{data_unit, pes_packet};
use crate::testing::{Owned, diagnostics, packets, run, same_in_any_chunks};
use std::vec::Vec;
use tellytxt_core::consts::PACKET_BYTES;
use tellytxt_core::packet::Packet;

pub(crate) const PMT_PID: u16 = 0x100;
pub(crate) const TELETEXT_PID: u16 = 0x201;
const PAYLOAD_BYTES: usize = TS_PACKET_BYTES - TS_HEADER_BYTES;

/// Splits a payload into transport packets, padding the last with an
/// adaptation field.
pub(crate) fn packetise(pid: u16, payload: &[u8], counter: &mut u8, out: &mut Vec<[u8; TS_PACKET_BYTES]>) {
    let mut first = true;
    for chunk in payload.chunks(PAYLOAD_BYTES) {
        let mut packet = [0xFFu8; TS_PACKET_BYTES];
        packet[0] = TS_SYNC_BYTE;
        packet[1] = (u8::from(first) << 6) | (pid >> 8) as u8;
        packet[2] = pid as u8;
        if chunk.len() == PAYLOAD_BYTES {
            packet[3] = 0x10 | *counter;
            packet[4..].copy_from_slice(chunk);
        } else {
            packet[3] = 0x30 | *counter;
            let stuffing = PAYLOAD_BYTES - chunk.len() - 1;
            packet[4] = stuffing as u8;
            if stuffing > 0 {
                packet[5] = 0;
            }
            packet[5 + stuffing..].copy_from_slice(chunk);
        }
        *counter = (*counter + 1) & 0xF;
        first = false;
        out.push(packet);
    }
}

fn section(mut body: Vec<u8>) -> Vec<u8> {
    let length = body.len() + 4 - 3;
    body[1] = 0xB0 | (length >> 8) as u8;
    body[2] = length as u8;
    let crc = crc32(&body);
    body.extend_from_slice(&crc.to_be_bytes());
    let mut payload = std::vec![0];
    payload.extend(body);
    payload
}

pub(crate) fn pat() -> Vec<u8> {
    section(std::vec![
        TABLE_ID_PAT,
        0,
        0,
        0,
        1,
        0xC1,
        0,
        0,
        0,
        0,
        0xE0,
        0x10,
        0,
        1,
        0xE0 | (PMT_PID >> 8) as u8,
        PMT_PID as u8
    ])
}

pub(crate) fn pmt() -> Vec<u8> {
    // English initial page 100, and German subtitles on page 888 (magazine
    // transmitted as 0).
    let descriptor = [DESCRIPTOR_TELETEXT, 10, b'e', b'n', b'g', 0x08 | 1, 0x00, b'd', b'e', b'u', 0x10, 0x88];
    let mut body = std::vec![TABLE_ID_PMT, 0, 0, 0, 1, 0xC1, 0, 0, 0xE2, 0x00, 0xF0, 0x00];
    body.extend_from_slice(&[0x02, 0xE2, 0x00, 0xF0, 0x00]);
    body.extend_from_slice(&[STREAM_TYPE_PRIVATE_PES, 0xE0 | (TELETEXT_PID >> 8) as u8, TELETEXT_PID as u8, 0xF0]);
    body.push(descriptor.len() as u8);
    body.extend_from_slice(&descriptor);
    section(body)
}

pub(crate) fn sample(seed: u8) -> Packet {
    let mut bytes = [0; PACKET_BYTES];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = seed.wrapping_mul(3).wrapping_add(index as u8);
    }
    Packet { bytes }
}

/// A stream: PAT, PMT, then two PES packets of four data units each, each
/// PES filling two transport packets.
pub(crate) fn stream() -> Vec<[u8; TS_PACKET_BYTES]> {
    let mut packets = Vec::new();
    packetise(PID_PAT, &pat(), &mut 0, &mut packets);
    packetise(PMT_PID, &pmt(), &mut 0, &mut packets);
    let mut counter = 0;
    for pes in 0..2u8 {
        let units: Vec<_> =
            (0..4).map(|unit| data_unit(DATA_UNIT_TELETEXT, &sample(pes * 4 + unit), 7 + unit)).collect();
        packetise(TELETEXT_PID, &pes_packet(u64::from(pes) * 3600, &units), &mut counter, &mut packets);
    }
    packets
}

pub(crate) fn flatten(packets: &[[u8; TS_PACKET_BYTES]], prefix: usize, suffix: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for packet in packets {
        out.extend(std::iter::repeat_n(0x11, prefix));
        out.extend_from_slice(packet);
        out.extend(std::iter::repeat_n(0x22, suffix));
    }
    out
}

pub(crate) fn expected_packets() -> Vec<Packet> {
    (0..8).map(sample).collect()
}

#[test]
fn finds_teletext_through_pat_and_pmt() {
    let input = flatten(&stream(), 0, 0);
    let items = same_in_any_chunks(|| TsReader::new(TsConfig::default()), &input);
    assert_eq!(packets(&items), expected_packets());
    assert_eq!(diagnostics(&items), []);
    let others: Vec<_> =
        items.iter().filter_map(|item| if let Owned::Other(text) = item { Some(text.clone()) } else { None }).collect();
    assert_eq!(others.len(), 4, "{others:?}");
    assert!(others[0].contains("language: [101, 110, 103]") && others[0].contains("Initial"), "{}", others[0]);
    assert!(others[1].contains("language: [100, 101, 117]") && others[1].contains("Subtitle"), "{}", others[1]);
    assert!(others[2].contains("StreamSelected { program: Some(1), pid: 513 }"), "{}", others[2]);
    assert!(others[3].contains("SubtitlePage") && others[3].contains("Magazine(8)"), "{}", others[3]);
    assert!(others[3].contains("PageNumber(136)"), "{}", others[3]);
    let Some(Owned::Packet(_, meta)) = items.iter().find(|item| matches!(item, Owned::Packet(..))) else { panic!() };
    assert_eq!(meta.line, Some(7));
    assert_eq!(meta.timestamp_us, Some(0));
}

#[test]
fn reads_192_and_204_byte_packets() {
    for (size, prefix, suffix) in [(TsPacketSize::M2ts, 4, 0), (TsPacketSize::Fec, 0, 16)] {
        let input = flatten(&stream(), prefix, suffix);
        let detected = run(&mut TsReader::new(TsConfig::default()), &input, 500);
        assert_eq!(packets(&detected), expected_packets(), "{size:?}");
        assert_eq!(diagnostics(&detected), [], "{size:?}");
        let config = TsConfig { packet_size: Some(size), ..TsConfig::default() };
        let items = same_in_any_chunks(|| TsReader::new(config), &input);
        assert_eq!(packets(&items), expected_packets());
    }
}

#[test]
fn resynchronises_and_reports_offsets() {
    let packets_in = stream();
    let mut input = std::vec![0x00; 50];
    input.extend(flatten(&packets_in[..3], 0, 0));
    let garbage_at = input.len();
    input.extend_from_slice(&[0x01; 30]);
    input.extend(flatten(&packets_in[3..], 0, 0));
    let items = same_in_any_chunks(|| TsReader::new(TsConfig::default()), &input);
    let found = diagnostics(&items);
    assert_eq!(found[0], Diagnostic::at(0, DiagnosticKind::SkippedBytes { count: 50 }));
    assert_eq!(found[1], Diagnostic::at(garbage_at as u64, DiagnosticKind::SkippedBytes { count: 30 }));
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(packets(&items), expected_packets());
}

#[test]
fn discards_errors_and_reports_continuity_breaks() {
    let mut packets_in = stream();
    // The first PES fills transport packets 2 and 3, the second 4 and 5.
    let pes_start = 2;
    packets_in[pes_start + 1][1] |= TS_TRANSPORT_ERROR;
    let input = flatten(&packets_in, 0, 0);
    let items = same_in_any_chunks(|| TsReader::new(TsConfig::default()), &input);
    let found = diagnostics(&items);
    let offset = |index: usize| (index * TS_PACKET_BYTES) as u64;
    assert_eq!(found[0], Diagnostic::at(offset(pes_start + 1), DiagnosticKind::TransportError { pid: TELETEXT_PID }));
    let kind = DiagnosticKind::ContinuityBreak { pid: TELETEXT_PID, expected: 1, found: 2 };
    assert_eq!(found[1], Diagnostic::at(offset(pes_start + 2), kind));
    // The first PES loses its last unit, which straddled the lost packet;
    // the second is whole.
    let got = packets(&items);
    let expected = expected_packets();
    assert_eq!(got[..3], expected[..3]);
    assert_eq!(got[3..], expected[4..]);
}

#[test]
fn selects_by_pid_without_tables() {
    let packets_in = stream();
    let input = flatten(&packets_in[2..], 0, 0);
    let config = TsConfig { pid: Some(TELETEXT_PID), ..TsConfig::default() };
    let items = same_in_any_chunks(|| TsReader::new(config), &input);
    assert_eq!(packets(&items), expected_packets());
    let unselected = run(&mut TsReader::new(TsConfig::default()), &input, TS_PACKET_BYTES);
    assert_eq!(packets(&unselected), []);
    assert_eq!(diagnostics(&unselected).last().unwrap().kind, DiagnosticKind::NoTeletextStream);
}

#[test]
fn selects_by_program() {
    let input = flatten(&stream(), 0, 0);
    let config = TsConfig { program: Some(2), ..TsConfig::default() };
    let items = run(&mut TsReader::new(config), &input, 1000);
    assert_eq!(packets(&items), []);
    let config = TsConfig { program: Some(1), ..TsConfig::default() };
    assert_eq!(packets(&run(&mut TsReader::new(config), &input, 1000)), expected_packets());
}

#[test]
fn rejects_sections_with_a_bad_crc() {
    let mut packets_in = stream();
    // Corrupt one PMT byte: the CRC no longer holds, so no stream is found.
    packets_in[1][TS_PACKET_BYTES - 1] ^= 0x01;
    let items = run(&mut TsReader::new(TsConfig::default()), &flatten(&packets_in, 0, 0), 188);
    assert_eq!(packets(&items), []);
    assert_eq!(diagnostics(&items)[0].kind, DiagnosticKind::SectionCrc { pid: PMT_PID });
}

#[test]
fn crc_of_a_section_with_its_crc_is_zero() {
    assert_eq!(crc32(&[]), CRC32_INITIAL);
    let payload = pat();
    assert_eq!(crc32(&payload[1..]), 0);
    // The CRC_32 of "123456789" under the MPEG-2 parameters is 0x0376E6E7.
    assert_eq!(crc32(b"123456789"), 0x0376_E6E7);
}
