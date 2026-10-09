//! Broadcast service data, packet 8/30 (area SVC, layer L2).

use crate::consts::{
    DEFAULT_INITIAL_PAGE, MJD_REFERENCE, NIBBLE_MASK, SERVICE_DIGIT_OFFSET, SERVICE_FORMAT_2_BIT,
    STATUS_MESSAGE_BYTES,
};
use crate::packet::ServicePacket;
use crate::types::{PageAddress, SubCode};

/// A calendar date and time of day.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DateTime {
    /// Year.
    pub year: i32,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day of the month, 1 to 31.
    pub day: u8,
    /// Hour, 0 to 23.
    pub hour: u8,
    /// Minute, 0 to 59.
    pub minute: u8,
    /// Second, 0 to 59.
    pub second: u8,
}

/// Service data interpreted from packet 8/30 (REQ-SVC-001 to 005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServiceInfo {
    /// Format 2 (designation codes 2 and 3) carries Programme Delivery
    /// Control data instead of date and time.
    pub format2: bool,
    /// The initial page and sub-code, if decodable.
    pub initial_page: Option<(PageAddress, SubCode)>,
    /// Format 1 network identification.
    pub network_id: Option<u16>,
    /// Format 1 Coordinated Universal Time. The time refers to the next
    /// second boundary after the packet (clause 9.8.1).
    pub utc: Option<DateTime>,
    /// Local time offset in half-hours, from the time offset byte.
    pub offset_half_hours: Option<i8>,
    /// The 20-character status message, 7-bit, parity removed.
    pub status: [u8; STATUS_MESSAGE_BYTES],
}

/// Interprets an 8/30 packet.
#[must_use]
pub fn interpret(packet: &ServicePacket) -> ServiceInfo {
    let format2 = packet.designation & SERVICE_FORMAT_2_BIT != 0;
    let initial_page = packet.initial_page.and_then(|(magazine, page, subcode)| {
        PageAddress::from_mpp((u16::from(magazine.number()) << 8) | u16::from(page.0))
            .map(|address| (address, subcode))
    });
    let mut status = [0; STATUS_MESSAGE_BYTES];
    for (out, &byte) in status.iter_mut().zip(packet.status.iter()) {
        *out = byte & crate::consts::CODE_MASK_7BIT;
    }
    if format2 {
        return ServiceInfo { format2, initial_page, network_id: None, utc: None, offset_half_hours: None, status };
    }
    ServiceInfo {
        format2,
        initial_page,
        network_id: Some(packet.network_id),
        utc: date_time(packet.date_time),
        offset_half_hours: Some(offset(packet.time_offset)),
        status,
    }
}

/// The initial page (REQ-SVC-004): from 8/30, or page 100 (Annex D.3).
#[must_use]
pub fn initial_page(info: Option<&ServiceInfo>) -> PageAddress {
    info.and_then(|info| info.initial_page.map(|(address, _)| address))
        .or_else(|| PageAddress::from_mpp(DEFAULT_INITIAL_PAGE))
        .unwrap_or(PageAddress {
            magazine: crate::types::Magazine::from_bits(1),
            page: crate::types::PageNumber(0),
        })
}

/// Bit 7 of the time offset byte marks a negative offset; bits 2 to 6 hold
/// half-hours (Table 18, numbering bits from 1).
const OFFSET_NEGATIVE: u8 = 0x40;
const OFFSET_HALF_HOURS_MASK: u8 = 0x1F;

fn offset(byte: u8) -> i8 {
    let half_hours = ((byte >> 1) & OFFSET_HALF_HOURS_MASK).cast_signed();
    if byte & OFFSET_NEGATIVE == 0 { half_hours } else { half_hours.wrapping_neg() }
}

/// The digits of bytes 16 to 21, high nibble first. Digit order verified
/// on 10 October 2026 against live 8/30 packets whose clock matched UTC
/// (research/answers-teletext-sources.md section 2.1).
fn nibbles(bytes: [u8; 6]) -> [u8; 12] {
    let mut out = [0; 12];
    for ([high, low], byte) in out.as_chunks_mut::<2>().0.iter_mut().zip(bytes) {
        *high = byte >> 4;
        *low = byte & NIBBLE_MASK;
    }
    out
}

/// A transmitted digit, which is the value plus one; `None` if not 0 to 9.
fn digit(transmitted: u8) -> Option<u32> {
    let value = transmitted.checked_sub(SERVICE_DIGIT_OFFSET)?;
    (value <= 9).then_some(u32::from(value))
}

fn number(digits: &[u8]) -> Option<u32> {
    digits.iter().try_fold(0u32, |acc, &d| acc.checked_mul(10)?.checked_add(digit(d)?))
}

/// Decodes the Modified Julian Date and UTC fields (REQ-SVC-002).
fn date_time(bytes: [u8; 6]) -> Option<DateTime> {
    let digits = nibbles(bytes);
    // The first nibble of byte 16 is not part of the five MJD digits.
    let mjd = number(digits.get(1..6)?)?;
    let hour = number(digits.get(6..8)?)?;
    let minute = number(digits.get(8..10)?)?;
    let second = number(digits.get(10..12)?)?;
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let (year, month, day) = civil_from_mjd(mjd)?;
    Some(DateTime { year, month, day, hour: hour as u8, minute: minute as u8, second: second as u8 })
}

/// Days from MJD 0 (17 November 1858) to 1 March of year 0 in the proleptic
/// Gregorian calendar used by the civil-date algorithm below.
const MJD_TO_EPOCH_MARCH_0000: i64 = 678_881;

/// Converts a Modified Julian Date to a calendar date, using the integer
/// "days from civil" inverse (era of 400 years, years starting in March).
fn civil_from_mjd(mjd: u32) -> Option<(i32, u8, u8)> {
    let days = i64::from(mjd).checked_add(MJD_TO_EPOCH_MARCH_0000)?;
    let era = days.checked_div(146_097)?;
    let day_of_era = days.checked_sub(era.checked_mul(146_097)?)?;
    let year_of_era = day_of_era
        .checked_sub(day_of_era.checked_div(1460)?)?
        .checked_add(day_of_era.checked_div(36_524)?)?
        .checked_sub(day_of_era.checked_div(146_096)?)?
        .checked_div(365)?;
    let day_of_year = day_of_era.checked_sub(
        year_of_era
            .checked_mul(365)?
            .checked_add(year_of_era.checked_div(4)?)?
            .checked_sub(year_of_era.checked_div(100)?)?,
    )?;
    let month_index = day_of_year.checked_mul(5)?.checked_add(2)?.checked_div(153)?;
    let day = day_of_year
        .checked_sub(month_index.checked_mul(153)?.checked_add(2)?.checked_div(5)?)?
        .checked_add(1)?;
    let month = if month_index < 10 { month_index.checked_add(3)? } else { month_index.checked_sub(9)? };
    let mut year = year_of_era.checked_add(era.checked_mul(400)?)?;
    if month <= 2 {
        year = year.checked_add(1)?;
    }
    Some((i32::try_from(year).ok()?, u8::try_from(month).ok()?, u8::try_from(day).ok()?))
}

/// The reference point of REQ-SVC-002, exposed for its test.
#[must_use]
pub fn mjd_to_date(mjd: u32) -> Option<(i32, u8, u8)> {
    civil_from_mjd(mjd)
}

const _: () = assert!(MJD_REFERENCE == 45_000);

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
}
