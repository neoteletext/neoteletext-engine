//! The transport stream reader fed packets whose fields the input sets:
//! the PID, the payload unit start, the adaptation field and its
//! discontinuity indicator, and the continuity counter (REQ-TST-040,
//! REQ-INP-006, REQ-INP-007), so continuity and section reassembly are
//! explored directly rather than through format detection.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_formats::Item;
use neoteletext_formats::consts::{
    PID_NULL, PID_PAT, TS_ADAPTATION_PRESENT, TS_ADAPTATION_SHIFT, TS_CONTINUITY_MASK, TS_DISCONTINUITY,
    TS_FEC_PACKET_BYTES, TS_HEADER_BYTES, TS_M2TS_PACKET_BYTES, TS_PACKET_BYTES, TS_PAYLOAD_PRESENT,
    TS_PAYLOAD_UNIT_START, TS_PID_HIGH_MASK, TS_SYNC_BYTE, TS_TRANSPORT_ERROR,
};
use neoteletext_formats::item::Reader;
use neoteletext_formats::transport::{TsConfig, TsPacketSize, TsReader};

/// The PIDs a packet may carry: the PAT, the null packet, and three that a
/// PAT in the input can name as a PMT or that the configuration names as
/// teletext, so tables and the stream they announce meet.
const PIDS: [u16; 5] = [PID_PAT, PID_NULL, 0x0100, 0x0101, 0x1000];
/// The PID the configuration may name as teletext: one of `PIDS`.
const TELETEXT_PID: u16 = 0x0101;
/// A record: PID choice, flags, adaptation length, payload length, then
/// the payload.
const RECORD_HEADER: usize = 4;
/// The bytes after the 4-byte header that adaptation field and payload share.
const BODY_BYTES: usize = TS_PACKET_BYTES - TS_HEADER_BYTES;
/// Stuffing (ISO/IEC 13818-1 clause 2.4.3.5).
const STUFFING: u8 = 0xFF;
/// Bits of a record's flag byte.
const FLAG_UNIT_START: u8 = 0x01;
const FLAG_ERROR: u8 = 0x02;
const FLAG_DISCONTINUITY: u8 = 0x04;
/// Bits 3 and 4 of the flag byte are the adaptation field control; bits 4
/// to 7 the continuity counter.
const FLAG_ADAPTATION_SHIFT: u32 = 3;
const FLAG_COUNTER_SHIFT: u32 = 4;
/// Bytes of an M2TS packet before its sync byte.
const M2TS_PREFIX: usize = TS_M2TS_PACKET_BYTES - TS_PACKET_BYTES;
/// Bytes of a Reed-Solomon packet after its 188 bytes.
const FEC_SUFFIX: usize = TS_FEC_PACKET_BYTES - TS_PACKET_BYTES;

/// Appends one 188-byte packet made from the record at the start of
/// `records`; the records after it.
fn packet<'a>(records: &'a [u8], out: &mut Vec<u8>) -> &'a [u8] {
    let [pid_choice, flags, adaptation_length, payload_length, rest @ ..] = records else { return &[] };
    let pid = PIDS[usize::from(*pid_choice) % PIDS.len()];
    let control = (flags >> FLAG_ADAPTATION_SHIFT) & 0b11;
    let counter = (flags >> FLAG_COUNTER_SHIFT) & TS_CONTINUITY_MASK;
    let [pid_high, pid_low] = pid.to_be_bytes();
    let mut first = pid_high & TS_PID_HIGH_MASK;
    if flags & FLAG_UNIT_START != 0 {
        first |= TS_PAYLOAD_UNIT_START;
    }
    if flags & FLAG_ERROR != 0 {
        first |= TS_TRANSPORT_ERROR;
    }
    let start = out.len();
    out.extend_from_slice(&[TS_SYNC_BYTE, first, pid_low, (control << TS_ADAPTATION_SHIFT) | counter]);
    let mut body = Vec::with_capacity(BODY_BYTES);
    if control & TS_ADAPTATION_PRESENT != 0 {
        // The length byte is not checked here: a length past the packet is
        // an input the reader must survive.
        body.push(*adaptation_length);
        let indicator = if flags & FLAG_DISCONTINUITY != 0 { TS_DISCONTINUITY } else { 0 };
        body.push(indicator);
        let fill = usize::from(*adaptation_length).saturating_sub(1).min(BODY_BYTES - body.len());
        body.resize(body.len() + fill, STUFFING);
    }
    let take = usize::from(*payload_length).min(rest.len());
    let (payload, rest) = rest.split_at(take);
    if control & TS_PAYLOAD_PRESENT != 0 {
        body.extend_from_slice(payload);
    }
    body.resize(BODY_BYTES, STUFFING);
    out.extend_from_slice(&body);
    assert_eq!(out.len() - start, TS_PACKET_BYTES, "the packet's fields fill it exactly");
    rest
}

fuzz_target!(|data: &[u8]| {
    let [settings, piece, body @ ..] = data else { return };
    let mut records = body;
    let size = match settings & 0b11 {
        0 => None,
        1 => Some(TsPacketSize::Standard),
        2 => Some(TsPacketSize::M2ts),
        _ => Some(TsPacketSize::Fec),
    };
    let config = TsConfig {
        packet_size: size,
        pid: (settings & 0b100 != 0).then_some(TELETEXT_PID),
        program: (settings & 0b1000 != 0).then_some(u16::from(settings >> 4)),
        source: None,
    };
    let mut stream = Vec::new();
    while records.len() >= RECORD_HEADER {
        if size == Some(TsPacketSize::M2ts) {
            stream.extend_from_slice(&[0; M2TS_PREFIX]);
        }
        records = packet(records, &mut stream);
        if size == Some(TsPacketSize::Fec) {
            stream.extend_from_slice(&[0; FEC_SUFFIX]);
        }
    }
    let mut reader = Box::new(TsReader::new(config));
    let mut sink = |item: Item<'_>| {
        core::hint::black_box(&item);
    };
    for chunk in stream.chunks(usize::from(*piece).max(1)) {
        reader.feed(chunk, &mut sink);
    }
    reader.finish(&mut sink);
});
