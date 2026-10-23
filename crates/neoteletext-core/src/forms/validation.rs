//! Validation of a form's values (Appendix C.5): the rules of each field
//! kind.

use super::spec::{FieldKind, FieldSpec};
use super::{FormModel, VALUE_CHARS};
use crate::charset::{CharRef, CharSet, G0, to_unicode};

/// Why validation failed (Appendix C.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Invalid {
    /// A required field is empty.
    Required,
    /// Longer than `maxlen`.
    TooLong,
    /// Does not match the pattern.
    Pattern,
    /// Not a whole number, out of range or off-step.
    Number,
    /// Not a valid date or time in range.
    DateTime,
    /// Not an email address.
    Email,
    /// Not a telephone number.
    Tel,
}

impl FormModel {
    /// Checks every field (Appendix C.5). The first failure is returned.
    pub fn validate(&self) -> Result<(), (usize, Invalid)> {
        for (index, field) in self.spec.fields.as_slice().iter().enumerate() {
            let value = self.value(index);
            let text = |out: &mut [u8; VALUE_CHARS]| -> usize {
                let mut len = 0usize;
                for (slot, &code) in out.iter_mut().zip(value) {
                    *slot = ascii_of(code, self.g0);
                    len = len.saturating_add(1);
                }
                len
            };
            let mut buffer = [0u8; VALUE_CHARS];
            let len = text(&mut buffer);
            let ascii = buffer.get(..len).unwrap_or(&[]);
            if !field.is_textual() {
                continue;
            }
            if ascii.is_empty() {
                if field.required {
                    return Err((index, Invalid::Required));
                }
                continue;
            }
            if field.maxlen != 0 && ascii.len() > usize::from(field.maxlen) {
                return Err((index, Invalid::TooLong));
            }
            let pattern = field.pattern.as_slice();
            if !pattern.is_empty() && !matches_pattern(ascii, pattern) {
                return Err((index, Invalid::Pattern));
            }
            let check = match field.kind {
                FieldKind::Email => valid_email(ascii).then_some(()).ok_or(Invalid::Email),
                FieldKind::Tel => (pattern.is_empty() && !valid_tel(ascii)).then_some(Invalid::Tel).map_or(Ok(()), Err),
                FieldKind::Number => valid_number(ascii, field).then_some(()).ok_or(Invalid::Number),
                FieldKind::Date => valid_date(ascii, field).then_some(()).ok_or(Invalid::DateTime),
                FieldKind::Time => valid_time(ascii, field).then_some(()).ok_or(Invalid::DateTime),
                _ => Ok(()),
            };
            check.map_err(|reason| (index, reason))?;
        }
        Ok(())
    }
}

/// The ASCII form of a G0 code for validation: digits, letters, `@`, `.`
/// and so on map to themselves; anything else becomes `?`.
fn ascii_of(code: u8, g0: G0) -> u8 {
    let c = to_unicode(CharRef::new(CharSet::G0(g0), code)).base;
    if c.is_ascii() { c as u8 } else { b'?' }
}

fn matches_pattern(value: &[u8], pattern: &[u8]) -> bool {
    value.len() <= pattern.len()
        && value.iter().zip(pattern).all(|(&v, &p)| match p {
            b'9' => v.is_ascii_digit(),
            b'A' => v.is_ascii_alphabetic(),
            _ => true,
        })
}

pub(super) fn valid_email(value: &[u8]) -> bool {
    // Exactly one "@": its first and last positions coincide.
    let first = value.iter().position(|&b| b == b'@');
    let last = value.iter().rposition(|&b| b == b'@');
    first == last && first.is_some_and(|p| p > 0 && p.saturating_add(1) < value.len())
}

fn valid_tel(value: &[u8]) -> bool {
    value.iter().all(|&b| b.is_ascii_digit() || b == b' ' || b == b'+')
}

fn parse_int(value: &[u8]) -> Option<i32> {
    let (negative, digits) = match value.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, value),
    };
    if digits.is_empty() {
        return None;
    }
    let magnitude = digits.iter().try_fold(0i32, |acc, &b| {
        if b.is_ascii_digit() { acc.checked_mul(10)?.checked_add(i32::from(b.wrapping_sub(b'0'))) } else { None }
    })?;
    Some(if negative { magnitude.checked_neg()? } else { magnitude })
}

pub(super) fn valid_number(value: &[u8], field: &FieldSpec) -> bool {
    let Some(number) = parse_int(value) else { return false };
    let in_range = number >= field.min && number <= field.max;
    let on_step =
        field.step == 0 || number.checked_sub(field.min.max(0)).and_then(|d| d.checked_rem(field.step)) == Some(0);
    in_range && on_step
}

/// `YYYY-MM-DD` with a real month and day; compared as text against limits.
pub(super) fn valid_date(value: &[u8], field: &FieldSpec) -> bool {
    let well_formed = value.len() == 10
        && value.get(4) == Some(&b'-')
        && value.get(7) == Some(&b'-')
        && value.iter().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !well_formed {
        return false;
    }
    let year = value.get(..4).and_then(parse_int).unwrap_or(0);
    let month = value.get(5..7).and_then(parse_int).unwrap_or(0);
    let day = value.get(8..10).and_then(parse_int).unwrap_or(0);
    (1..=days_in_month(year, month)).contains(&day) && within(value, field)
}

/// Days in a month of the Gregorian calendar (ISO 8601), zero for a month
/// outside 1 to 12.
fn days_in_month(year: i32, month: i32) -> i32 {
    let divides = |divisor: i32| year.checked_rem(divisor) == Some(0);
    let leap = divides(4) && (!divides(100) || divides(400));
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    }
}

/// `HH:MM`.
pub(super) fn valid_time(value: &[u8], field: &FieldSpec) -> bool {
    let well_formed = value.len() == 5
        && value.get(2) == Some(&b':')
        && value.iter().enumerate().all(|(i, b)| i == 2 || b.is_ascii_digit());
    let hour = value.get(..2).and_then(parse_int).unwrap_or(99);
    let minute = value.get(3..).and_then(parse_int).unwrap_or(99);
    well_formed && hour <= 23 && minute <= 59 && within(value, field)
}

/// Fixed-width dates and times compare correctly as text.
fn within(value: &[u8], field: &FieldSpec) -> bool {
    let min = field.min_text.as_slice();
    let max = field.max_text.as_slice();
    (min.is_empty() || value >= min) && (max.is_empty() || value <= max)
}
