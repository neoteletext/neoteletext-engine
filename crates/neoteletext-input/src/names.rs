//! The numbered tables of key names (REQ-INT-003): what a key means, from
//! the W3C UI Events `KeyboardEvent.key` values (W3C, "UI Events
//! `KeyboardEvent` key Values", sections 3.2 to 3.18), and where it is, from
//! the `KeyboardEvent.code` values (W3C, "UI Events `KeyboardEvent` code
//! Values", sections 3.1 to 3.6).
//!
//! The order of each table is fixed: a name's number is its place, and a
//! name is only ever added at the end, so a record written today means the
//! same to every later receiver. The browser and a phone's layout map onto
//! these names one to one.

/// The first key code of a named key: a printable character is its own
/// Unicode scalar value in the Basic Multilingual Plane, and the named keys
/// take Private Use code points from here, which no keyboard types.
pub const NAMED_BASE: u16 = 0xF000;

/// The W3C `KeyboardEvent.key` names; [`NAMED_BASE`] plus a name's place is
/// its key code. Place 0, `Unidentified`, is a key the sender could not name.
pub const KEY_NAMES: [&str; 168] = [
    "Unidentified",
    "Alt",
    "AltGraph",
    "CapsLock",
    "Control",
    "Fn",
    "FnLock",
    "Meta",
    "NumLock",
    "ScrollLock",
    "Shift",
    "Symbol",
    "SymbolLock",
    "Hyper",
    "Super",
    "Enter",
    "Tab",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "End",
    "Home",
    "PageDown",
    "PageUp",
    "Backspace",
    "Clear",
    "Copy",
    "CrSel",
    "Cut",
    "Delete",
    "EraseEof",
    "ExSel",
    "Insert",
    "Paste",
    "Redo",
    "Undo",
    "Accept",
    "Again",
    "Attn",
    "Cancel",
    "ContextMenu",
    "Escape",
    "Execute",
    "Find",
    "Help",
    "Pause",
    "Play",
    "Props",
    "Select",
    "ZoomIn",
    "ZoomOut",
    "BrightnessDown",
    "BrightnessUp",
    "Eject",
    "LogOff",
    "Power",
    "PowerOff",
    "PrintScreen",
    "Hibernate",
    "Standby",
    "WakeUp",
    "AllCandidates",
    "Alphanumeric",
    "CodeInput",
    "Compose",
    "Convert",
    "Dead",
    "FinalMode",
    "GroupFirst",
    "GroupLast",
    "GroupNext",
    "GroupPrevious",
    "ModeChange",
    "NextCandidate",
    "NonConvert",
    "PreviousCandidate",
    "Process",
    "SingleCandidate",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
    "F21",
    "F22",
    "F23",
    "F24",
    "ChannelDown",
    "ChannelUp",
    "Close",
    "MailForward",
    "MailReply",
    "MailSend",
    "MediaClose",
    "MediaFastForward",
    "MediaPause",
    "MediaPlay",
    "MediaPlayPause",
    "MediaRecord",
    "MediaRewind",
    "MediaStop",
    "MediaTrackNext",
    "MediaTrackPrevious",
    "New",
    "Open",
    "Print",
    "Save",
    "SpellCheck",
    "AudioVolumeDown",
    "AudioVolumeUp",
    "AudioVolumeMute",
    "BrowserBack",
    "BrowserFavorites",
    "BrowserForward",
    "BrowserHome",
    "BrowserRefresh",
    "BrowserSearch",
    "BrowserStop",
    "ColorF0Red",
    "ColorF1Green",
    "ColorF2Yellow",
    "ColorF3Blue",
    "ColorF4Grey",
    "ColorF5Brown",
    "ClosedCaptionToggle",
    "Dimmer",
    "DVR",
    "Exit",
    "Guide",
    "Info",
    "InstantReplay",
    "MediaApps",
    "MediaAudioTrack",
    "MediaLast",
    "MediaSkipBackward",
    "MediaSkipForward",
    "MediaStepBackward",
    "MediaStepForward",
    "MediaTopMenu",
    "NavigateIn",
    "NavigateNext",
    "NavigateOut",
    "NavigatePrevious",
    "PinPToggle",
    "RandomToggle",
    "Settings",
    "Subtitle",
    "Teletext",
    "TV",
    "VideoModeNext",
    "Wink",
    "ZoomToggle",
];

/// The W3C `KeyboardEvent.code` names; a name's place is its physical key
/// code. Place 0 is a key whose position is unknown, as from a phone.
pub const PHYSICAL_NAMES: [&str; 167] = [
    "",
    "Backquote",
    "Backslash",
    "BracketLeft",
    "BracketRight",
    "Comma",
    "Digit0",
    "Digit1",
    "Digit2",
    "Digit3",
    "Digit4",
    "Digit5",
    "Digit6",
    "Digit7",
    "Digit8",
    "Digit9",
    "Equal",
    "IntlBackslash",
    "IntlRo",
    "IntlYen",
    "KeyA",
    "KeyB",
    "KeyC",
    "KeyD",
    "KeyE",
    "KeyF",
    "KeyG",
    "KeyH",
    "KeyI",
    "KeyJ",
    "KeyK",
    "KeyL",
    "KeyM",
    "KeyN",
    "KeyO",
    "KeyP",
    "KeyQ",
    "KeyR",
    "KeyS",
    "KeyT",
    "KeyU",
    "KeyV",
    "KeyW",
    "KeyX",
    "KeyY",
    "KeyZ",
    "Minus",
    "Period",
    "Quote",
    "Semicolon",
    "Slash",
    "AltLeft",
    "AltRight",
    "Backspace",
    "CapsLock",
    "ContextMenu",
    "ControlLeft",
    "ControlRight",
    "Enter",
    "MetaLeft",
    "MetaRight",
    "ShiftLeft",
    "ShiftRight",
    "Space",
    "Tab",
    "Convert",
    "KanaMode",
    "Lang1",
    "Lang2",
    "Lang3",
    "Lang4",
    "Lang5",
    "NonConvert",
    "Delete",
    "End",
    "Help",
    "Home",
    "Insert",
    "PageDown",
    "PageUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "NumLock",
    "Numpad0",
    "Numpad1",
    "Numpad2",
    "Numpad3",
    "Numpad4",
    "Numpad5",
    "Numpad6",
    "Numpad7",
    "Numpad8",
    "Numpad9",
    "NumpadAdd",
    "NumpadBackspace",
    "NumpadClear",
    "NumpadClearEntry",
    "NumpadComma",
    "NumpadDecimal",
    "NumpadDivide",
    "NumpadEnter",
    "NumpadEqual",
    "NumpadHash",
    "NumpadMemoryAdd",
    "NumpadMemoryClear",
    "NumpadMemoryRecall",
    "NumpadMemoryStore",
    "NumpadMemorySubtract",
    "NumpadMultiply",
    "NumpadParenLeft",
    "NumpadParenRight",
    "NumpadStar",
    "NumpadSubtract",
    "Escape",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
    "F21",
    "F22",
    "F23",
    "F24",
    "Fn",
    "FnLock",
    "PrintScreen",
    "ScrollLock",
    "Pause",
    "BrowserBack",
    "BrowserFavorites",
    "BrowserForward",
    "BrowserHome",
    "BrowserRefresh",
    "BrowserSearch",
    "BrowserStop",
    "Eject",
    "LaunchApp1",
    "LaunchApp2",
    "LaunchMail",
    "MediaPlayPause",
    "MediaSelect",
    "MediaStop",
    "MediaTrackNext",
    "MediaTrackPrevious",
    "Power",
    "Sleep",
    "AudioVolumeDown",
    "AudioVolumeMute",
    "AudioVolumeUp",
    "WakeUp",
];

/// The key code of a named key.
#[must_use]
pub fn named_key(name: &str) -> Option<u16> {
    let place = KEY_NAMES.iter().position(|each| *each == name)?;
    NAMED_BASE.checked_add(u16::try_from(place).ok()?)
}

/// The name of a named key code, or `None` for a character or a code
/// beyond the table.
#[must_use]
pub fn key_name(code: u16) -> Option<&'static str> {
    KEY_NAMES.get(usize::from(code.checked_sub(NAMED_BASE)?)).copied()
}

/// The physical key code of a `KeyboardEvent.code` name, or 0 when the
/// name is not in the table.
#[must_use]
pub fn physical_key(name: &str) -> u16 {
    PHYSICAL_NAMES.iter().position(|each| *each == name).and_then(|place| u16::try_from(place).ok()).unwrap_or(0)
}

/// The name of a physical key code; `None` for 0 and for a code beyond the
/// table.
#[must_use]
pub fn physical_name(code: u16) -> Option<&'static str> {
    if code == 0 {
        return None;
    }
    PHYSICAL_NAMES.get(usize::from(code)).copied()
}

/// The key code of a character: its scalar value when it is printable and
/// in the Basic Multilingual Plane outside the named keys' block. Anything
/// else is sent as text (REQ-INT-002).
#[must_use]
pub fn character_key(character: char) -> Option<u16> {
    let code = u16::try_from(u32::from(character)).ok()?;
    if character.is_control() || is_named(code) {
        return None;
    }
    Some(code)
}

/// Whether a key code is a named key, in or beyond the table.
#[must_use]
pub const fn is_named(code: u16) -> bool {
    code >= NAMED_BASE && code <= NAMED_LAST
}

/// The end of the block the named keys may grow into.
pub const NAMED_LAST: u16 = 0xF0FF;

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    // Tests of REQ-INT-003.
    use super::*;

    #[test]
    fn the_tables_are_frozen() {
        // A change of place would change what every sent record means.
        assert_eq!(named_key("Unidentified"), Some(NAMED_BASE));
        assert_eq!(named_key("Enter"), Some(0xF00F));
        assert_eq!(named_key("ColorF0Red"), Some(0xF086));
        assert_eq!(physical_key("KeyW"), 42);
        assert_eq!(physical_key("Numpad5"), 90);
    }

    #[test]
    fn every_name_round_trips_and_fits_its_block() {
        for name in KEY_NAMES {
            let code = named_key(name).unwrap();
            assert!(is_named(code));
            assert_eq!(key_name(code), Some(name));
        }
        for name in &PHYSICAL_NAMES[1..] {
            assert_eq!(physical_name(physical_key(name)), Some(*name));
        }
        assert_eq!(physical_key("NoSuchKey"), 0);
        assert_eq!(physical_name(0), None);
    }

    #[test]
    fn characters_are_their_own_codes() {
        assert_eq!(character_key('a'), Some(0x61));
        assert_eq!(character_key('£'), Some(0xA3));
        assert_eq!(character_key('\r'), None, "Enter is a named key");
        assert_eq!(character_key('\u{1F600}'), None, "beyond the plane: sent as text");
        assert_eq!(character_key('\u{F00F}'), None, "the named keys' block");
    }
}
