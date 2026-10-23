//! A channel's log lines (decision E.109, REQ-INP-026): `LG,level,text` in
//! a TTI file, for the author of the channel, as a web page's script logs
//! to a browser's console. Teletext runs no scripts; a channel's server
//! writes these lines where it writes its pages, and a receiver shows them
//! only in a developer's view, never on the page.
//!
//! The record is strict: one of three levels, then UTF-8 text of 1 to
//! [`MAX_LOG_TEXT_BYTES`] bytes with no control character and no
//! bidirectional formatting character. Anything else is refused whole;
//! nothing of a refused line is shown.

/// The longest text of a log line, in bytes of UTF-8: a console line, five
/// teletext rows of characters.
pub const MAX_LOG_TEXT_BYTES: usize = 200;
/// The most log lines a page may carry, counted from its `PN` line (and,
/// for lines before any `PN`, from the start); more are refused, so that a
/// channel cannot flood a console from one file.
pub const MAX_LOG_LINES_PER_PAGE: usize = 16;

/// How much a log line matters, as a console sorts them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum LogLevel {
    /// Something failed (`E`).
    Error,
    /// Something is wrong but the page works (`W`).
    Warning,
    /// For the author's information (`I`).
    Information,
}

impl LogLevel {
    /// The level's letter in an `LG` line.
    #[must_use]
    pub const fn letter(self) -> u8 {
        match self {
            Self::Error => b'E',
            Self::Warning => b'W',
            Self::Information => b'I',
        }
    }

    const fn of_letter(letter: u8) -> Option<Self> {
        match letter {
            b'E' => Some(Self::Error),
            b'W' => Some(Self::Warning),
            b'I' => Some(Self::Information),
            _ => None,
        }
    }
}

/// A log line as read: its level and its text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LogLine<'a> {
    /// The level.
    pub level: LogLevel,
    /// The text: UTF-8, 1 to [`MAX_LOG_TEXT_BYTES`] bytes, with no control
    /// or bidirectional formatting character.
    pub text: &'a str,
}

/// Why a log line was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum LogRefused {
    /// The level is not `E`, `W` or `I`, or no comma follows it.
    Level,
    /// The text is empty.
    Empty,
    /// The text is longer than [`MAX_LOG_TEXT_BYTES`].
    TooLong,
    /// The text is not UTF-8.
    NotUtf8,
    /// The text holds a control character (C0, DEL or C1) or a
    /// bidirectional formatting character, which could make a console
    /// show something other than what was sent.
    Character,
}

/// The bidirectional formatting characters (Unicode Standard Annex 9,
/// Table 1: the explicit embeddings, overrides and isolates, and the
/// implicit marks).
const BIDI_CONTROLS: [char; 12] = [
    '\u{061C}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}',
    '\u{2067}', '\u{2068}', '\u{2069}',
];

/// True when `character` may be in a log line's text.
fn allowed(character: char) -> bool {
    !character.is_control() && !BIDI_CONTROLS.contains(&character)
}

/// Reads the arguments of an `LG` line, everything after `LG,`: the level's
/// letter, a comma, and the text to the end of the line.
///
/// # Errors
/// Anything but a level and clean text within the bound ([`LogRefused`]).
pub fn read(arguments: &[u8]) -> Result<LogLine<'_>, LogRefused> {
    let (&letter, rest) = arguments.split_first().ok_or(LogRefused::Level)?;
    let level = LogLevel::of_letter(letter).ok_or(LogRefused::Level)?;
    let text = rest.strip_prefix(b",").ok_or(LogRefused::Level)?;
    check(text).map(|text| LogLine { level, text })
}

/// Checks a log line's text, as [`read`] and the TTI writer do.
///
/// # Errors
/// Empty, too long, not UTF-8, or holding a character not allowed.
pub fn check(text: &[u8]) -> Result<&str, LogRefused> {
    if text.is_empty() {
        return Err(LogRefused::Empty);
    }
    if text.len() > MAX_LOG_TEXT_BYTES {
        return Err(LogRefused::TooLong);
    }
    let text = core::str::from_utf8(text).map_err(|_| LogRefused::NotUtf8)?;
    if text.chars().all(allowed) { Ok(text) } else { Err(LogRefused::Character) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "a test")]
mod tests {
    use super::*;

    #[test]
    fn a_log_line_is_a_level_and_clean_text() {
        assert_eq!(
            read(b"W,Page 200 is late: its feed did not answer"),
            Ok(LogLine { level: LogLevel::Warning, text: "Page 200 is late: its feed did not answer" })
        );
        assert_eq!(read("E,Météo ∅ — no data".as_bytes()).unwrap().level, LogLevel::Error);
        assert_eq!(read(b"I,a,b").unwrap().text, "a,b", "commas belong to the text");
        let longest = [b'x'; MAX_LOG_TEXT_BYTES];
        assert_eq!(read(&[b"I,".as_slice(), &longest].concat()).unwrap().text.len(), MAX_LOG_TEXT_BYTES);
    }

    #[test]
    fn anything_else_is_refused() {
        for (line, why) in [
            (&b""[..], LogRefused::Level),
            (b"X,text", LogRefused::Level),
            (b"w,text", LogRefused::Level),
            (b"W text", LogRefused::Level),
            (b"W", LogRefused::Level),
            (b"W,", LogRefused::Empty),
            (b"W,\xFF\xFE", LogRefused::NotUtf8),
            (b"W,tab\there", LogRefused::Character),
            (b"W,bell\x07", LogRefused::Character),
            (b"W,escape\x1B[31m", LogRefused::Character),
            (b"W,delete\x7F", LogRefused::Character),
            ("W,c1\u{85}".as_bytes(), LogRefused::Character),
            ("W,abc\u{202E}fed".as_bytes(), LogRefused::Character),
            ("W,iso\u{2066}late".as_bytes(), LogRefused::Character),
        ] {
            assert_eq!(read(line), Err(why), "{line:?}");
        }
        let long = [b'x'; MAX_LOG_TEXT_BYTES + 1];
        assert_eq!(read(&[b"I,".as_slice(), &long].concat()), Err(LogRefused::TooLong));
    }
}
