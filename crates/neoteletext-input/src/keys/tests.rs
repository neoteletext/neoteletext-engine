#![allow(clippy::unwrap_used, reason = "a test")]

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
