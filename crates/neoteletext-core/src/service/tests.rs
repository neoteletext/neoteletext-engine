//! Tests of the broadcast service data of packet 8/30 (area SVC).
#![allow(clippy::unwrap_used)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

#[test]
fn mjd_reference_point() {
    // REQ-SVC-002: day 45000 is 31 January 1982.
    assert_eq!(mjd_to_date(MJD_REFERENCE), Some((1982, 1, 31)));
    assert_eq!(mjd_to_date(61_322), Some((2026, 10, 9)));
    assert_eq!(mjd_to_date(54_296), Some((2007, 7, 15)));
}

#[test]
fn live_teefax_packet_decodes() {
    // Bytes 16 to 21 of a Teefax 8/30 captured on 9 October 2026 at
    // 22:47:44 UTC (research/answers-teletext-sources.md).
    let bytes = [0x07, 0x24, 0x33, 0x33, 0x58, 0x55];
    let time = date_time(bytes).unwrap();
    assert_eq!((time.year, time.month, time.day), (2026, 10, 9));
    assert_eq!((time.hour, time.minute, time.second), (22, 47, 44));
    assert_eq!(offset(0x85), 2);
    assert_eq!(offset(0x85 | OFFSET_NEGATIVE), -2);
}

#[test]
fn out_of_range_digits_are_invalid() {
    assert_eq!(date_time([0x07, 0x24, 0x33, 0x35, 0x58, 0x55]), None);
    assert_eq!(date_time([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), None);
}

use crate::ecc::with_odd_parity;
use crate::types::{Magazine, PageNumber};

/// A format 1 packet 8/30 as the packet layer decodes it: initial page 8A5
/// (magazine bits 000, absolute, meaning magazine 8), the Teefax clock of
/// `live_teefax_packet_decodes`, and a status message with its parity bits.
fn service_packet(designation: u8) -> ServicePacket {
    let mut status = [with_odd_parity(b' '); STATUS_MESSAGE_BYTES];
    for (slot, &byte) in status.iter_mut().zip(b"TEEFAX") {
        *slot = with_odd_parity(byte);
    }
    ServicePacket {
        designation,
        initial_page: Some((Magazine::from_bits(0), PageNumber(0xA5), SubCode::new(0x3F7F))),
        network_id: 0x2C2F,
        time_offset: 0x85 | OFFSET_NEGATIVE,
        date_time: [0x07, 0x24, 0x33, 0x33, 0x58, 0x55],
        pdc: [0x15; 13],
        status,
    }
}

/// REQ-SVC-001, REQ-SVC-002, REQ-SVC-005: format 1 gives the initial page
/// with its absolute magazine, the network identification, UTC from the
/// Modified Julian Date, the local offset in half-hours, and the status
/// message without its parity bits. Invalid date digits invalidate only
/// the time.
#[test]
fn format_1_service_data() {
    let info = interpret(&service_packet(0));
    assert!(!info.format2);
    assert_eq!(info.initial_page, Some((PageAddress::from_mpp(0x8A5).unwrap(), SubCode::new(0x3F7F))));
    assert_eq!(info.network_id, Some(0x2C2F));
    assert_eq!(info.utc, Some(DateTime { year: 2026, month: 10, day: 9, hour: 22, minute: 47, second: 44 }));
    assert_eq!(info.offset_half_hours, Some(-2));
    assert_eq!(&info.status[..8], b"TEEFAX  ");
    assert!(info.status.iter().all(|&byte| byte & !crate::consts::CODE_MASK_7BIT == 0));
    let bad_hour = ServicePacket { date_time: [0x07, 0x24, 0x33, 0x35, 0x58, 0x55], ..service_packet(1) };
    let info = interpret(&bad_hour);
    assert_eq!((info.utc, info.network_id, info.offset_half_hours), (None, Some(0x2C2F), Some(-2)));
}

/// REQ-SVC-003: designation codes 2 and 3 carry Programme Delivery Control
/// data, kept raw by the packet layer and not read as a network, a date or
/// an offset; the initial page and the status message are as in format 1.
#[test]
fn format_2_service_data() {
    for designation in [2, 3] {
        let info = interpret(&service_packet(designation));
        assert!(info.format2);
        assert_eq!((info.network_id, info.utc, info.offset_half_hours), (None, None, None));
        assert_eq!(info.initial_page.map(|(address, _)| address), PageAddress::from_mpp(0x8A5));
        assert_eq!(&info.status[..6], b"TEEFAX");
    }
}

/// REQ-SVC-004, Annex D.3: the initial page is the one 8/30 gives, and page
/// 100 when no 8/30 was received or its initial page was uncorrectable.
#[test]
fn initial_page_from_8_30_or_100() {
    let page_100 = PageAddress::from_mpp(0x100).unwrap();
    assert_eq!(initial_page(None), page_100);
    let info = interpret(&service_packet(0));
    assert_eq!(initial_page(Some(&info)), PageAddress::from_mpp(0x8A5).unwrap());
    let uncorrectable = interpret(&ServicePacket { initial_page: None, ..service_packet(0) });
    assert_eq!(initial_page(Some(&uncorrectable)), page_100);
}
