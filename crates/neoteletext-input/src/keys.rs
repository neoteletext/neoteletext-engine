//! Functional key codes (`neoteletext-common/INPUT.md` section 5.6,
//! REQ-INT-105): one code for every key that is not a character, which
//! never changes. Codes 1 to 127 take one byte and hold the commonest keys,
//! teletext's colour, Teletext and channel keys among them; codes from
//! [`FIRST_NAMED`] are the W3C `KeyboardEvent.key` names of
//! [`KEY_NAMES`] not given a code below, in that
//! table's fixed order. Both only ever grow.
//!
//! The `base` field (section 5.7) gives the character a physical key has on
//! a United States layout, from the W3C `KeyboardEvent.code` name of the
//! key: [`us_base`].

use crate::names::KEY_NAMES;
use one_byte::ONE_BYTE;

mod one_byte;
mod us_layout;

pub use us_layout::us_base;

/// Where the codes of the remaining W3C names begin.
pub const FIRST_NAMED: u32 = 128;

/// The code of a key named as W3C names it, or as section 5.6 does for the
/// keypad and the located modifiers.
#[must_use]
pub fn code(name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    if let Some(place) = ONE_BYTE.iter().position(|each| *each == name) {
        return u32::try_from(place).ok();
    }
    let place = remaining().position(|each| each == name)?;
    FIRST_NAMED.checked_add(u32::try_from(place).ok()?)
}

/// The name of a code, or `None` for a code not in use.
#[must_use]
pub fn name(code: u32) -> Option<&'static str> {
    if code < FIRST_NAMED {
        return ONE_BYTE.get(usize::try_from(code).ok()?).copied().filter(|name| !name.is_empty());
    }
    remaining().nth(usize::try_from(code.checked_sub(FIRST_NAMED)?).ok()?)
}

/// Whether `code` names a key: a receiver of the stream refuses a KEY with
/// a functional code that names none.
#[must_use]
pub fn is_code(code: u32) -> bool {
    name(code).is_some()
}

/// The code of a key the sender cannot name.
#[must_use]
pub fn unidentified() -> u32 {
    code("Unidentified").unwrap_or(FIRST_NAMED)
}

/// The keys that only modify others or lock a state, for a receiver that
/// tells them by where they are on the keyboard. The located modifiers are
/// codes 80 to 91 and the lock keys 15 to 17; the rest are W3C names.
const MODIFIER_NAMES: [&str; 11] =
    ["Alt", "AltGraph", "Control", "Fn", "FnLock", "Hyper", "Meta", "Shift", "Super", "Symbol", "SymbolLock"];
const LOCKS: core::ops::RangeInclusive<u32> = 15..=17;
const LOCATED_MODIFIERS: core::ops::RangeInclusive<u32> = 80..=91;

/// Whether `code` is a key that only modifies others or locks a state.
#[must_use]
pub fn is_modifier(code: u32) -> bool {
    LOCKS.contains(&code)
        || LOCATED_MODIFIERS.contains(&code)
        || name(code).is_some_and(|name| MODIFIER_NAMES.contains(&name))
}

/// The W3C names given no code below 128, in their table's order.
fn remaining() -> impl Iterator<Item = &'static str> {
    KEY_NAMES.iter().copied().filter(|name| !ONE_BYTE.contains(name))
}

#[cfg(test)]
mod tests;
