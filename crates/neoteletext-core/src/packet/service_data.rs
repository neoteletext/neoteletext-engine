//! Broadcast service data, packet 8/30 (clause 9.8.1, Table 18; REQ-SVC-001
//! and REQ-SVC-003).

use super::Packet;
use super::editorial_links::page_reference;
use crate::consts::STATUS_MESSAGE_BYTES;
use crate::ecc::reverse_bits;
use crate::types::{Magazine, PageNumber, SubCode};

/// Broadcast service data, packet 8/30 (REQ-PKT/SVC-001).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServicePacket {
    /// The designation code, 0 to 3.
    pub designation: u8,
    /// The initial page magazine, page and sub-code, `None` if uncorrectable.
    pub initial_page: Option<(Magazine, PageNumber, SubCode)>,
    /// Format 1 network identification, most significant bit first.
    pub network_id: u16,
    /// Format 1 time offset byte, raw.
    pub time_offset: u8,
    /// Format 1 Modified Julian Date and UTC bytes 16 to 21, raw.
    pub date_time: [u8; 6],
    /// Format 2 Programme Delivery Control bytes 13 to 25, raw (Part G).
    pub pdc: [u8; 13],
    /// Status message bytes 26 to 45, raw with parity.
    pub status: [u8; STATUS_MESSAGE_BYTES],
}

/// Indices within 8/30 (clause 9.8.1, Table 18): initial page bytes 7 to
/// 12, network identification 13 and 14, time offset 15, date and time 16 to
/// 21, Programme Delivery Control 13 to 25, status 26 to 45.
const SERVICE_INITIAL_PAGE: usize = 3;
const SERVICE_NETWORK_ID: usize = 9;
const SERVICE_TIME_OFFSET: usize = 11;
const SERVICE_DATE_TIME: usize = 12;
const SERVICE_PDC: usize = 9;
const SERVICE_STATUS: usize = 22;

fn copy_bytes<const N: usize>(packet: &Packet, start: usize) -> [u8; N] {
    let mut out = [0; N];
    if let Some(source) = packet.bytes.get(start..start.saturating_add(N)) {
        out.copy_from_slice(source);
    }
    out
}

/// Decodes packet 8/30 (REQ-SVC-001 and REQ-SVC-003).
pub(super) fn decode_service(packet: &Packet, designation: u8) -> ServicePacket {
    // The initial page magazine is absolute here (Table 18), not relative.
    let initial_page = page_reference(packet, SERVICE_INITIAL_PAGE)
        .map(|(page, subcode, bits)| (Magazine::from_bits(bits), page, subcode));
    // The network identification is sent most significant bit first, so each
    // byte is reversed to read it (Table 18).
    let network_id = (u16::from(reverse_bits(packet.byte(SERVICE_NETWORK_ID))) << 8)
        | u16::from(reverse_bits(packet.byte(SERVICE_NETWORK_ID.saturating_add(1))));
    ServicePacket {
        designation,
        initial_page,
        network_id,
        time_offset: packet.byte(SERVICE_TIME_OFFSET),
        date_time: copy_bytes(packet, SERVICE_DATE_TIME),
        pdc: copy_bytes(packet, SERVICE_PDC),
        status: copy_bytes(packet, SERVICE_STATUS),
    }
}
