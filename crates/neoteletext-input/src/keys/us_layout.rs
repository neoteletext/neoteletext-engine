//! The United States layout's characters by W3C `KeyboardEvent.code` name
//! (`INPUT.md` section 5.7): the `base` field of a KEY.

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
