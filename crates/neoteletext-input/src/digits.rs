//! Decimal digits of any script (`neoteletext-common/INPUT.md` section 5.7
//! rule 8, REQ-INT-116): a layout that types native digits (`٣`, `३`)
//! sends those characters, and a channel that selects pages by number takes
//! every decimal digit, Unicode's general category `Nd`, as the digits 0 to
//! 9.

/// The digit zero of every run of `Nd` characters: Unicode 16.0.0's
/// `UnicodeData.txt`, as Python's `unicodedata` 16.0.0 lists it. Every
/// `Nd` character is one of 76 runs of ten, zero to nine in order (Unicode
/// section 4.6, "Numeric Value"), so a digit's value is its distance from
/// its run's zero. Sorted, for the search.
const ZEROS: [u32; 76] = [
    0x0030, 0x0660, 0x06F0, 0x07C0, 0x0966, 0x09E6, 0x0A66, 0x0AE6, 0x0B66, 0x0BE6, 0x0C66, 0x0CE6, 0x0D66, 0x0DE6,
    0x0E50, 0x0ED0, 0x0F20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40,
    0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10, 0x104A0, 0x10D30, 0x10D40, 0x11066,
    0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x116D0, 0x116DA, 0x11730, 0x118E0,
    0x11950, 0x11BF0, 0x11C50, 0x11D50, 0x11DA0, 0x11F50, 0x16130, 0x16A60, 0x16AC0, 0x16B50, 0x16D70, 0x1CCF0,
    0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E5F1, 0x1E950, 0x1FBF0,
];
/// Digits in a run.
const RADIX: u32 = 10;

/// The value of a decimal digit of any script, or `None` for a character
/// that is not one.
#[must_use]
pub fn decimal_digit(character: char) -> Option<u8> {
    let point = u32::from(character);
    let run = match ZEROS.binary_search(&point) {
        Ok(at) => at,
        Err(after) => after.checked_sub(1)?,
    };
    let value = point.checked_sub(*ZEROS.get(run)?)?;
    if value < RADIX { u8::try_from(value).ok() } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-INT-116.
    #[test]
    fn the_digits_of_every_script_are_0_to_9() {
        assert_eq!(decimal_digit('7'), Some(7));
        assert_eq!(decimal_digit('٣'), Some(3), "Arabic-Indic");
        assert_eq!(decimal_digit('۵'), Some(5), "extended Arabic-Indic, Persian");
        assert_eq!(decimal_digit('३'), Some(3), "Devanagari");
        assert_eq!(decimal_digit('๙'), Some(9), "Thai");
        assert_eq!(decimal_digit('９'), Some(9), "full width");
        assert_eq!(decimal_digit('\u{1FBF9}'), Some(9), "the last run");
        for not_a_digit in ['a', '/', ':', '½', 'Ⅳ', '²', '\u{065F}', '\u{0}', char::MAX] {
            assert_eq!(decimal_digit(not_a_digit), None, "{not_a_digit:?}");
        }
        assert!(ZEROS.is_sorted(), "the search needs them sorted");
    }
}
