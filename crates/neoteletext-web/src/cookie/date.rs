//! The cookie date algorithm (RFC 6265bis section 5.1.1) and the civil
//! calendar it needs.

use crate::bytes::parse_decimal;
use crate::consts::SECONDS_PER_DAY;

/// True for a delimiter of the cookie date grammar (RFC 6265bis section
/// 5.1.1).
const fn is_date_delimiter(byte: u8) -> bool {
    matches!(byte, 0x09 | 0x20..=0x2F | 0x3B..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E)
}

/// Splits leading digits (at most `max`, at least `min`) from a token
/// whose remainder must not start with a digit.
fn leading_digits(token: &[u8], min: usize, max: usize) -> Option<(u64, &[u8])> {
    let count = token.iter().take_while(|b| b.is_ascii_digit()).count();
    if count < min || count > max {
        return None;
    }
    Some((parse_decimal(token.get(..count)?, max)?, token.get(count..)?))
}

const MONTHS: [&[u8; 3]; 12] =
    [b"jan", b"feb", b"mar", b"apr", b"may", b"jun", b"jul", b"aug", b"sep", b"oct", b"nov", b"dec"];

/// The cookie date algorithm (RFC 6265bis section 5.1.1), giving seconds
/// since 1970. Dates before 1970 give 0, which is always in the past.
#[must_use]
pub fn parse_cookie_date(text: &[u8]) -> Option<u64> {
    let mut time: Option<(u64, u64, u64)> = None;
    let mut day: Option<u64> = None;
    let mut month: Option<u64> = None;
    let mut year: Option<u64> = None;
    for token in text.split(|&b| is_date_delimiter(b)).filter(|token| !token.is_empty()) {
        if time.is_none()
            && let Some(parsed) = parse_time_token(token)
        {
            time = Some(parsed);
            continue;
        }
        if day.is_none()
            && let Some((value, _)) = leading_digits(token, 1, 2)
        {
            day = Some(value);
            continue;
        }
        if month.is_none()
            && let Some(index) =
                MONTHS.iter().position(|name| token.get(..3).is_some_and(|head| head.eq_ignore_ascii_case(*name)))
        {
            month = Some((index as u64).saturating_add(1));
            continue;
        }
        if year.is_none()
            && let Some((value, _)) = leading_digits(token, 2, 4)
        {
            year = Some(value);
        }
    }
    let (hour, minute, second) = time?;
    let (day, month, mut year) = (day?, month?, year?);
    // Section 5.1.1 steps 3 and 4: two-digit years.
    if (70..=99).contains(&year) {
        year = year.saturating_add(1900);
    } else if year <= 69 {
        year = year.saturating_add(2000);
    }
    if !(1..=31).contains(&day) || year < 1601 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    if day > days_in_month(year, month) {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    let time_of_day = hour
        .saturating_mul(SECONDS_PER_HOUR)
        .saturating_add(minute.saturating_mul(SECONDS_PER_MINUTE))
        .saturating_add(second);
    Some(days.saturating_mul(SECONDS_PER_DAY).saturating_add(time_of_day))
}

fn parse_time_token(token: &[u8]) -> Option<(u64, u64, u64)> {
    let (hour, rest) = leading_digits(token, 1, 2)?;
    let rest = rest.strip_prefix(b":")?;
    let (minute, rest) = leading_digits(rest, 1, 2)?;
    let rest = rest.strip_prefix(b":")?;
    let (second, _) = leading_digits(rest, 1, 2)?;
    Some((hour, minute, second))
}

const SECONDS_PER_HOUR: u64 = 3600;
const SECONDS_PER_MINUTE: u64 = 60;
/// Gregorian leap-year cycle lengths.
const LEAP_EVERY: u64 = 4;
const LEAP_SKIPPED_EVERY: u64 = 100;
const LEAP_RESTORED_EVERY: u64 = 400;

const fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(LEAP_EVERY) && !year.is_multiple_of(LEAP_SKIPPED_EVERY))
        || year.is_multiple_of(LEAP_RESTORED_EVERY)
}

const fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to a date, or 0 for earlier dates: the
/// days-from-civil algorithm on the proleptic Gregorian calendar.
fn days_from_civil(year: u64, month: u64, day: u64) -> Option<u64> {
    const DAYS_PER_ERA: u64 = 146_097;
    const UNIX_EPOCH_FROM_0000_03_01: u64 = 719_468;
    let y = if month <= 2 { year.checked_sub(1)? } else { year };
    let era = y / LEAP_RESTORED_EVERY;
    let year_of_era = y.checked_sub(era.checked_mul(LEAP_RESTORED_EVERY)?)?;
    let shifted_month = if month > 2 { month.checked_sub(3)? } else { month.checked_add(9)? };
    // Days before the month in a year starting in March: 153 days per five
    // months, rounded.
    let day_of_year = (153u64.checked_mul(shifted_month)?.checked_add(2)? / 5).checked_add(day.checked_sub(1)?)?;
    let day_of_era = year_of_era
        .checked_mul(365)?
        .checked_add(year_of_era / LEAP_EVERY)?
        .checked_sub(year_of_era / LEAP_SKIPPED_EVERY)?
        .checked_add(day_of_year)?;
    let days = era.checked_mul(DAYS_PER_ERA)?.checked_add(day_of_era)?;
    Some(days.saturating_sub(UNIX_EPOCH_FROM_0000_03_01))
}
