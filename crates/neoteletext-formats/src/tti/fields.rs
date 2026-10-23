//! The TTI argument grammar: comma-separated arguments, numbers, flags,
//! page addresses, sub-codes and service names, used by every record
//! family.

use neoteletext_core::types::{PageAddress, SubCode};

use crate::page::SERVICE_NAME_BYTES;
use crate::text::Text;

/// Highest region and page function values.
pub(super) const REGION_LAST: u32 = 0xF;
/// Page coding is 3 bits (Table 3).
pub(super) const CODING_LAST: u32 = 0x7;

/// Sets `target` to as much of `value` as it holds, and returns whether
/// all of it was kept. A cut can land just after a carriage return in the
/// value; those the cut leaves at the end are dropped, since written back
/// before the line feed they would read as line ending (REQ-INP-013) and
/// the value would change on every read and write (F-103).
pub(super) fn set_value<const N: usize>(target: &mut Text<N>, value: &[u8]) -> bool {
    let mut kept = value.get(..N).unwrap_or(value);
    let whole = kept.len() == value.len();
    if !whole {
        while let Some(stripped) = kept.strip_suffix(b"\r") {
            kept = stripped;
        }
    }
    // `kept` fits, so `set` keeps all of it.
    target.set(kept) && whole
}

/// Comma-separated arguments.
#[derive(Clone, Copy, Debug)]
pub(super) struct Args<'a> {
    rest: Option<&'a [u8]>,
}

impl<'a> Args<'a> {
    pub(super) const fn new(text: &'a [u8]) -> Self {
        Self { rest: Some(text) }
    }

    /// The next argument; empty once the arguments run out.
    pub(super) fn next(&mut self) -> &'a [u8] {
        let Some(rest) = self.rest else { return &[] };
        if let Some(index) = rest.iter().position(|&byte| byte == b',') {
            self.rest = rest.get(index.saturating_add(1)..);
            rest.get(..index).unwrap_or(&[])
        } else {
            self.rest = None;
            rest
        }
    }

    /// Everything not yet taken, commas included.
    pub(super) fn rest(&mut self) -> &'a [u8] {
        self.rest.take().unwrap_or(&[])
    }
}

/// Parses an unsigned number in `radix` (10 or 16), ignoring surrounding
/// spaces. `None` when empty, malformed or above `u32::MAX`.
fn parse_number(text: &[u8], radix: u32) -> Option<u32> {
    let text = text.trim_ascii();
    if text.is_empty() || text.len() > 8 {
        return None;
    }
    let mut value = 0u32;
    for &byte in text {
        let digit = char::from(byte).to_digit(radix)?;
        value = value.checked_mul(radix)?.checked_add(digit)?;
    }
    Some(value)
}

pub(super) fn hex(text: &[u8]) -> Option<u32> {
    parse_number(text, 16)
}

pub(super) fn decimal(text: &[u8]) -> Option<u32> {
    parse_number(text, 10)
}

/// A `Y`/`N` flag; `Some(false)` for empty, `None` for anything else.
pub(super) fn flag(text: &[u8]) -> Option<bool> {
    match text.trim_ascii() {
        b"Y" | b"y" => Some(true),
        b"" | b"N" | b"n" => Some(false),
        other if other.eq_ignore_ascii_case(b"selected") => Some(true),
        _ => None,
    }
}

/// A three-digit `mpp` page address.
pub(super) fn page_address(text: &[u8]) -> Option<PageAddress> {
    let text = text.trim_ascii();
    if text.len() != 3 {
        return None;
    }
    PageAddress::from_mpp(u16::try_from(hex(text)?).ok()?)
}

/// An `FP` page: `mpp`, or `mppss` as in the Appendix C.7 example, whose
/// sub-page digits are not used.
pub(super) fn result_page(text: &[u8]) -> Option<PageAddress> {
    let text = text.trim_ascii();
    match text.len() {
        3 => page_address(text),
        5 => text.get(..3).and_then(page_address).filter(|_| decimal(text.get(3..).unwrap_or(&[])).is_some()),
        _ => None,
    }
}

/// A four-digit sub-code.
pub(super) fn subcode(text: &[u8]) -> Option<SubCode> {
    let text = text.trim_ascii();
    if text.len() != 4 {
        return None;
    }
    Some(SubCode::new(u16::try_from(hex(text)?).ok()?))
}

/// A service name: letters, digits and hyphens (Appendix E.1).
pub(super) fn service_name(text: &[u8]) -> Option<Text<SERVICE_NAME_BYTES>> {
    if text.is_empty() || !text.iter().all(|&byte| byte.is_ascii_alphanumeric() || byte == b'-') {
        return None;
    }
    let (name, truncated) = Text::from_bytes(text);
    if truncated { None } else { Some(name) }
}
