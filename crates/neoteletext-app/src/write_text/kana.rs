//! Japanese text as the kana sets hold it (REQ-2188 item 4, DEC-2187 item
//! 5): half-width katakana (JIS X 0201, U+FF61 to U+FF9F) as the full-width
//! characters, and a kana followed by a voicing mark, half-width or
//! combining, as the one voiced kana.

/// The full-width characters of U+FF61 to U+FF9D, in order.
const HALF_WIDTH: &str = "。「」、・ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン";
/// The first half-width character [`HALF_WIDTH`] gives.
const HALF_WIDTH_FIRST: u32 = 0xFF61;
/// The voicing marks: combining (U+3099, U+309A) and half-width.
const VOICED: [char; 2] = ['\u{3099}', '\u{FF9E}'];
const SEMI_VOICED: [char; 2] = ['\u{309A}', '\u{FF9F}'];
/// The kana a voiced mark turns into the next code point, and those a
/// semi-voiced mark turns into the one after it (Unicode's kana blocks).
const VOICES_TO_NEXT: &str = "かきくけこさしすせそたちつてとはひふへほカキクケコサシスセソタチツテトハヒフヘホ";
const SEMI_VOICES: &str = "はひふへほハヒフヘホ";
/// う and ウ voiced are ゔ and ヴ, apart from the rest.
const VOICED_U: [(char, char); 2] = [('う', 'ゔ'), ('ウ', 'ヴ')];

/// `text` with half-width katakana made full-width and voicing marks
/// joined to their kana. A voicing mark after anything else is kept, and
/// is refused by the encoder like any character no set holds.
#[must_use]
pub(super) fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        let character = u32::from(character)
            .checked_sub(HALF_WIDTH_FIRST)
            .and_then(|index| HALF_WIDTH.chars().nth(index as usize))
            .unwrap_or(character);
        if let Some(voiced) = out.chars().last().and_then(|previous| voice(previous, character)) {
            out.pop();
            out.push(voiced);
            continue;
        }
        out.push(character);
    }
    out
}

/// The voiced kana of `kana` with `mark`, if it has one.
fn voice(kana: char, mark: char) -> Option<char> {
    let step = if VOICED.contains(&mark) {
        if let Some(&(_, voiced)) = VOICED_U.iter().find(|(plain, _)| *plain == kana) {
            return Some(voiced);
        }
        VOICES_TO_NEXT.contains(kana).then_some(1)
    } else if SEMI_VOICED.contains(&mark) {
        SEMI_VOICES.contains(kana).then_some(2)
    } else {
        None
    }?;
    char::from_u32(u32::from(kana).checked_add(step)?)
}
