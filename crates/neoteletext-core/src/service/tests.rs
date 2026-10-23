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
