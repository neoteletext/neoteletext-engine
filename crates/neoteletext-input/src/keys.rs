//! Functional key codes (`neoteletext-common/INPUT.md` section 5.6,
//! REQ-INT-105): one code for every key that is not a character, which
//! never changes. Codes 1 to 127 take one byte and hold the commonest keys,
//! teletext's colour, Teletext and channel keys among them; codes from
//! [`FIRST_NAMED`] are the W3C `KeyboardEvent.key` names of
//! [`KEY_NAMES`](crate::names::KEY_NAMES) not given a code below, in that
//! table's fixed order. Both only ever grow.
//!
//! The `base` field (section 5.7) gives the character a physical key has on
//! a United States layout, from the W3C `KeyboardEvent.code` name of the
//! key: [`us_base`].

use crate::names::KEY_NAMES;

/// Where the codes of the remaining W3C names begin.
pub const FIRST_NAMED: u32 = 128;

/// Codes 0 to 127: the key each names, or "" for a code not in use.
/// INPUT.md section 5.6's table, written out.
const ONE_BYTE: [&str; 128] = [
    "", // 0 is not a key
    "Escape",
    "Enter",
    "Tab",
    "Backspace",
    "Insert",
    "Delete",
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "ArrowDown", // 10
    "PageUp",
    "PageDown",
    "Home",
    "End",
    "CapsLock",
    "ScrollLock",
    "NumLock",
    "PrintScreen",
    "Pause",
    "ContextMenu", // 20, the Menu key
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10", // 30
    "F11",
    "F12",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20", // 40
    "F21",
    "F22",
    "F23",
    "F24",
    "F25",
    "F26",
    "F27",
    "F28",
    "F29",
    "F30", // 50
    "F31",
    "F32",
    "F33",
    "F34",
    "F35",
    "",
    "",
    "",
    "",        // 56 to 59 reserved
    "Numpad0", // 60
    "Numpad1",
    "Numpad2",
    "Numpad3",
    "Numpad4",
    "Numpad5",
    "Numpad6",
    "Numpad7",
    "Numpad8",
    "Numpad9",
    "NumpadDecimal", // 70
    "NumpadDivide",
    "NumpadMultiply",
    "NumpadSubtract",
    "NumpadAdd",
    "NumpadEnter",
    "NumpadEqual",
    "",
    "",
    "",          // 77 to 79 reserved
    "ShiftLeft", // 80
    "ControlLeft",
    "AltLeft",
    "SuperLeft",
    "HyperLeft",
    "MetaLeft",
    "ShiftRight",
    "ControlRight",
    "AltRight",
    "SuperRight",
    "HyperRight", // 90
    "MetaRight",
    "ColorF0Red",
    "ColorF1Green",
    "ColorF2Yellow",
    "ColorF3Blue",
    "ColorF4Grey",
    "ColorF5Brown",
    "Teletext",
    "ChannelUp",
    "ChannelDown", // 100
    "Exit",
    "Info",
    "Guide",
    "BrowserBack",
    "BrowserHome",
    "AudioVolumeUp",
    "AudioVolumeDown",
    "",
    "",
    "", // 108 to 110 reserved
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "", // to 120
    "",
    "",
    "",
    "",
    "",
    "",
    "", // to 127
];

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

/// The keys that only modify others or lock a state (INPUT.md section 6.3):
/// they go only in `ALL_KEYS` or `KEY_RELEASE`, as the Kitty keyboard
/// protocol reports them only when asked. The located modifiers are codes
/// 80 to 91 and the lock keys 15 to 17; the rest are W3C names.
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

/// Whether `code` is the dead key: an accent pressed before its letter
/// (section 5.7 rule 4), which goes only in `ALL_KEYS`.
#[must_use]
pub fn is_dead(code: u32) -> bool {
    name(code) == Some("Dead")
}

/// The W3C names given no code below 128, in their table's order.
fn remaining() -> impl Iterator<Item = &'static str> {
    KEY_NAMES.iter().copied().filter(|name| !ONE_BYTE.contains(name))
}

/// The character a physical key has on a United States layout, from its W3C
/// `KeyboardEvent.code` name, for a KEY's `base` (section 5.7); `None` for a
/// key that types no character there.
#[must_use]
pub fn us_base(code_name: &str) -> Option<char> {
    if let Some(letter) = code_name.strip_prefix("Key") {
        let mut characters = letter.chars();
        let (Some(first), None) = (characters.next(), characters.next()) else { return None };
        return first.is_ascii_uppercase().then(|| first.to_ascii_lowercase());
    }
    if let Some(digit) = code_name.strip_prefix("Digit") {
        let mut characters = digit.chars();
        let (Some(first), None) = (characters.next(), characters.next()) else { return None };
        return first.is_ascii_digit().then_some(first);
    }
    Some(match code_name {
        "Backquote" => '`',
        "Minus" => '-',
        "Equal" => '=',
        "BracketLeft" => '[',
        "BracketRight" => ']',
        "Backslash" => '\\',
        "Semicolon" => ';',
        "Quote" => '\'',
        "Comma" => ',',
        "Period" => '.',
        "Slash" => '/',
        "Space" => ' ',
        _ => return None,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use super::*;

    // REQ-INT-105.
    #[test]
    fn the_codes_are_fixed() {
        // A change would change what every sent key means (REQ-INT-105).
        assert_eq!(code("Escape"), Some(1));
        assert_eq!(code("Enter"), Some(2));
        assert_eq!(code("ArrowUp"), Some(9));
        assert_eq!(code("F1"), Some(21));
        assert_eq!(code("F35"), Some(55));
        assert_eq!(code("Numpad0"), Some(60));
        assert_eq!(code("NumpadEqual"), Some(76));
        assert_eq!(code("ShiftLeft"), Some(80));
        assert_eq!(code("MetaRight"), Some(91));
        assert_eq!(code("ColorF0Red"), Some(92));
        assert_eq!(code("Teletext"), Some(98));
        assert_eq!(code("AudioVolumeDown"), Some(107));
        assert_eq!(unidentified(), FIRST_NAMED, "the first remaining W3C name");
        assert_eq!(code("Alt"), Some(FIRST_NAMED + 1));
    }

    // REQ-INT-105.
    #[test]
    fn every_code_round_trips_and_unused_codes_name_nothing() {
        for code_value in 0..400 {
            if let Some(found) = name(code_value) {
                assert_eq!(code(found), Some(code_value), "{found}");
            }
        }
        for unused in [0, 56, 59, 77, 79, 108, 127] {
            assert_eq!(name(unused), None, "{unused}");
        }
        assert!(KEY_NAMES.iter().all(|each| code(each).is_some()), "every W3C name has a code");
        assert_eq!(code("NoSuchKey"), None);
        assert_eq!(code(""), None);
    }

    // REQ-INT-116.
    #[test]
    fn modifiers_locks_and_the_dead_key_are_known() {
        for modifier in ["ShiftLeft", "MetaRight", "CapsLock", "NumLock", "AltGraph", "Shift", "Fn"] {
            assert!(is_modifier(code(modifier).unwrap()), "{modifier}");
        }
        for other in ["Enter", "ArrowUp", "F1", "ColorF0Red", "Dead", "Unidentified"] {
            assert!(!is_modifier(code(other).unwrap()), "{other}");
        }
        assert!(MODIFIER_NAMES.iter().all(|name| code(name).is_some()), "every name is in the table");
        assert!(is_dead(code("Dead").unwrap()));
        assert!(!is_dead(code("Escape").unwrap()));
    }

    // REQ-INT-104.
    #[test]
    fn a_physical_key_has_its_united_states_character() {
        assert_eq!(us_base("KeyQ"), Some('q'));
        assert_eq!(us_base("Digit7"), Some('7'));
        assert_eq!(us_base("Semicolon"), Some(';'));
        assert_eq!(us_base("Space"), Some(' '));
        assert_eq!(us_base("ArrowUp"), None);
        assert_eq!(us_base("KeyQQ"), None);
    }
}
