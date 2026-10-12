//! A font library's index, `fonts.txt` (REQ-FNT-004, decision E.72 item
//! 5): a folder, in the form of a channel's folder (REQ-WEB-023), whose
//! index lists fonts by title, each with its SHA-256, so that fonts can be
//! found without knowing their names.
//!
//! Fields are separated by one tab, written `→` here, and comments and
//! empty lines are as `contents.txt` has them (REQ-WEB-025):
//!
//! ```text
//! # comments and empty lines are ignored
//! f→bedstead-teletext.bdf→Bedstead teletext→5ee6…f14d
//! f→https://fonts.example/zvbi.bdf→zvbi 12 x 10→9a1c…07b2
//! c→bedstead-teletext.bdf→G0-Latin-English* G0-Greek*
//! ```
//!
//! A `c` line states the character sets a listed font conforms for
//! (REQ-FNT-011, decision E.75 item 3), each name followed by `*` when the
//! font also meets the SAA5050 cell profile. The names are checked by the
//! caller, which knows them.
//!
//! The reader is strict and bounded and allocates nothing: [`read`] checks
//! the file's size and line count, then [`Entries`] yields each font or a
//! fault. A line of an unknown kind is skipped, for later versions; a
//! malformed line is a fault, which the caller counts. Listing a library
//! fetches no font: a font is fetched only when the viewer chooses it.

use crate::sha256::{Digest, parse_hex};
use crate::url::{Url, UrlError};

/// The largest index read (REQ-FNT-004).
pub const MAX_BYTES: usize = 64 * 1024;
/// The most lines read.
pub const MAX_LINES: usize = 1000;
/// The longest title.
pub const MAX_TITLE: usize = 32;
/// The index's fixed name in the library's folder.
pub const INDEX_NAME: &str = "fonts.txt";
/// The field separator.
const TAB: u8 = b'\t';
/// The first character of a comment line.
const COMMENT: u8 = b'#';
/// What marks a font given by its address rather than its file name.
const ADDRESS_MARK: &str = "://";

/// Why an index was refused whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexError {
    /// More than [`MAX_BYTES`].
    TooLarge,
    /// More than [`MAX_LINES`].
    TooManyLines,
}

/// Why one line was ignored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// The wrong number of fields.
    Fields,
    /// A byte outside printable ASCII, other than the tab.
    Character,
    /// A font that is neither a file name in the folder nor an address.
    Location,
    /// A title that is empty or longer than 32 characters.
    Title,
    /// A checksum that is not 64 hexadecimal digits.
    Checksum,
    /// A `c` line's sets that are not names separated by single spaces.
    Sets,
}

/// One line of an index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry<'a> {
    /// An `f` line: a font.
    Font(Font<'a>),
    /// A `c` line: the character sets a font conforms for.
    Conformance(Conformance<'a>),
}

/// The character sets a listed font conforms for (REQ-FNT-004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Conformance<'a> {
    /// The font, as its `f` line gives it.
    pub location: &'a str,
    /// The sets' names, separated by single spaces, each perhaps ending in
    /// `*`; empty when the font conforms for none.
    pub sets: &'a str,
}

impl<'a> Conformance<'a> {
    /// Each set's name, and whether the font meets the SAA5050 cell profile
    /// for it.
    pub fn sets(&self) -> impl Iterator<Item = (&'a str, bool)> {
        self.sets
            .split(' ')
            .filter(|token| !token.is_empty())
            .map(|token| token.strip_suffix('*').map_or((token, false), |name| (name, true)))
    }
}

/// A `c` line's sets field: names of letters, digits and hyphens, each
/// perhaps ending in `*`, separated by single spaces.
fn sets_field(field: &str) -> Result<&str, Fault> {
    if field.is_empty() {
        return Ok(field);
    }
    let name_ok = |token: &str| {
        let name = token.strip_suffix('*').unwrap_or(token);
        !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    };
    if field.split(' ').all(name_ok) { Ok(field) } else { Err(Fault::Sets) }
}

/// One font of a library.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Font<'a> {
    /// A file name in the library's folder, or an address.
    pub location: &'a str,
    /// The title the viewer chooses by.
    pub title: &'a str,
    /// The font's SHA-256.
    pub sha256: Digest,
}

/// One line that was ignored, by its number from 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineFault {
    /// The line.
    pub line: usize,
    /// Why.
    pub fault: Fault,
}

/// The numbered lines of an index.
type Lines<'a> = core::iter::Enumerate<core::slice::Split<'a, u8, fn(&u8) -> bool>>;

/// The fonts of an index, in order.
#[derive(Clone, Debug)]
pub struct Entries<'a> {
    lines: Lines<'a>,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "the predicate `slice::split` calls takes a reference")]
fn is_line_end(byte: &u8) -> bool {
    *byte == b'\n'
}

/// Checks an index's size and length, and returns its fonts.
pub fn read(bytes: &[u8]) -> Result<Entries<'_>, IndexError> {
    if bytes.len() > MAX_BYTES {
        return Err(IndexError::TooLarge);
    }
    #[allow(clippy::naive_bytecount, reason = "at most 64 kilobytes, counted once; no crate is added for it")]
    let lines = bytes.iter().filter(|&&byte| byte == b'\n').count();
    let unterminated = usize::from(bytes.last().is_some_and(|&byte| byte != b'\n'));
    if lines.saturating_add(unterminated) > MAX_LINES {
        return Err(IndexError::TooManyLines);
    }
    Ok(Entries { lines: bytes.split(is_line_end as fn(&u8) -> bool).enumerate() })
}

/// The index's address in a library's folder, which ends in `/`.
pub fn index_url(folder: &Url) -> Result<Url, UrlError> {
    folder.join(INDEX_NAME)
}

/// A font's address: its file name resolved in the library's folder, or
/// its own address. The network rules of REQ-FNT-003 apply where it is
/// fetched.
pub fn font_url(folder: &Url, location: &str) -> Result<Url, UrlError> {
    folder.join(location)
}

/// A font's location: an address, or a file name of the folder itself,
/// with no path, no query and no leading dot.
fn location(field: &str) -> Result<&str, Fault> {
    if field.contains(ADDRESS_MARK) {
        let url = Url::parse(field).map_err(|_| Fault::Location)?;
        return if url.scheme().is_websocket() { Err(Fault::Location) } else { Ok(field) };
    }
    let plain = !field.is_empty()
        && !field.starts_with('.')
        && field.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'));
    if plain { Ok(field) } else { Err(Fault::Location) }
}

impl<'a> Iterator for Entries<'a> {
    type Item = Result<Entry<'a>, LineFault>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (index, raw) = self.lines.next()?;
            let line = index.saturating_add(1);
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            if raw.is_empty() || raw.first() == Some(&COMMENT) {
                continue;
            }
            let fail = |fault| Some(Err(LineFault { line, fault }));
            if !raw.iter().all(|&byte| byte == TAB || (0x20..0x7F).contains(&byte)) {
                return fail(Fault::Character);
            }
            // Printable ASCII is UTF-8.
            let Ok(text) = core::str::from_utf8(raw) else { return fail(Fault::Character) };
            let mut fields = text.split(char::from(TAB));
            let entry = match (fields.next(), [fields.next(), fields.next(), fields.next(), fields.next()]) {
                (Some("f"), [Some(place), Some(title), Some(sha256), None]) => location(place).and_then(|location| {
                    if title.is_empty() || title.len() > MAX_TITLE {
                        return Err(Fault::Title);
                    }
                    let sha256 = parse_hex(sha256.as_bytes()).ok_or(Fault::Checksum)?;
                    Ok(Entry::Font(Font { location, title, sha256 }))
                }),
                (Some("c"), [Some(place), Some(sets), None, None]) => location(place)
                    .and_then(|location| Ok(Entry::Conformance(Conformance { location, sets: sets_field(sets)? }))),
                (Some("f" | "c"), _) => Err(Fault::Fields),
                // A kind for a later version.
                _ => continue,
            };
            return Some(entry.map_err(|fault| LineFault { line, fault }));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use std::string::String;
    use std::vec::Vec;
    use std::{format, vec};

    const SHA: &str = "5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d";

    fn all(text: &str) -> Vec<Result<Entry<'_>, LineFault>> {
        read(text.as_bytes()).unwrap().collect()
    }

    fn font(entry: Result<Entry<'_>, LineFault>) -> Font<'_> {
        match entry {
            Ok(Entry::Font(font)) => font,
            other => panic!("not a font: {other:?}"),
        }
    }

    /// REQ-FNT-004: fonts in the folder and elsewhere, with comments, empty
    /// lines, CR LF and a kind for a later version skipped.
    #[test]
    fn fonts_in_the_folder_and_elsewhere() {
        let text = format!(
            "# fonts\r\n\nf\tbedstead.bdf\tBedstead\t{SHA}\r\nz\tsomething\tnew\nf\thttps://fonts.example/z.bdf\tzvbi 12 x 10\t{}\n",
            SHA.to_uppercase()
        );
        let fonts: Vec<Font<'_>> = all(&text).into_iter().map(font).collect();
        assert_eq!(fonts.len(), 2);
        assert_eq!((fonts[0].location, fonts[0].title), ("bedstead.bdf", "Bedstead"));
        assert_eq!(&crate::sha256::to_hex(&fonts[1].sha256)[..], SHA.as_bytes());
        let folder = Url::parse("https://lib.example/teletext/").unwrap();
        assert_eq!(index_url(&folder).unwrap().as_str(), "https://lib.example/teletext/fonts.txt");
        assert_eq!(font_url(&folder, fonts[0].location).unwrap().as_str(), "https://lib.example/teletext/bedstead.bdf");
        assert_eq!(font_url(&folder, fonts[1].location).unwrap().as_str(), "https://fonts.example/z.bdf");
    }

    /// REQ-FNT-004: each fault ignores its line and is reported with it.
    #[test]
    fn each_fault() {
        let long = "t".repeat(MAX_TITLE + 1);
        let cases: Vec<(String, Fault)> = vec![
            (String::from("f\tf.bdf\tTitle"), Fault::Fields),
            (format!("f\tf.bdf\tTitle\t{SHA}\textra"), Fault::Fields),
            (format!("f\tf.bdf\tTitle\u{e9}\t{SHA}"), Fault::Character),
            (format!("f\t../f.bdf\tTitle\t{SHA}"), Fault::Location),
            (format!("f\tsub/f.bdf\tTitle\t{SHA}"), Fault::Location),
            (format!("f\t.hidden\tTitle\t{SHA}"), Fault::Location),
            (format!("f\tf.bdf?x=1\tTitle\t{SHA}"), Fault::Location),
            (format!("f\twss://fonts.example/f\tTitle\t{SHA}"), Fault::Location),
            (format!("f\thttps://\tTitle\t{SHA}"), Fault::Location),
            (format!("f\tf.bdf\t\t{SHA}"), Fault::Title),
            (format!("f\tf.bdf\t{long}\t{SHA}"), Fault::Title),
            (format!("f\tf.bdf\tTitle\t{}", &SHA[1..]), Fault::Checksum),
            (String::from("c\tf.bdf"), Fault::Fields),
            (String::from("c\t../f.bdf\tG0-Greek"), Fault::Location),
            (String::from("c\tf.bdf\tG0-Greek  G2-Greek"), Fault::Sets),
            (String::from("c\tf.bdf\tG0_Greek"), Fault::Sets),
            (String::from("c\tf.bdf\t*"), Fault::Sets),
        ];
        for (line, fault) in cases {
            let text = format!("# first\n{line}\n");
            assert_eq!(all(&text), [Err(LineFault { line: 2, fault })], "{line}");
        }
        assert_eq!(all(&format!("f\tf.bdf\t{}\t{SHA}", "t".repeat(MAX_TITLE))).len(), 1, "32 characters fit");
    }

    /// REQ-FNT-004, decision E.75 item 3: a `c` line's sets and profile.
    #[test]
    fn conformance_lines() {
        let entries = all("c\tf.bdf\tG0-Latin-English* G0-Greek\nc\tnone.bdf\t\n");
        let Ok(Entry::Conformance(first)) = entries[0] else { panic!("{entries:?}") };
        assert_eq!(first.location, "f.bdf");
        assert_eq!(first.sets().collect::<Vec<_>>(), [("G0-Latin-English", true), ("G0-Greek", false)]);
        let Ok(Entry::Conformance(none)) = entries[1] else { panic!("{entries:?}") };
        assert_eq!(none.sets().count(), 0, "a font conforming for no set");
    }

    #[test]
    fn size_limits() {
        assert_eq!(read(&vec![b'#'; MAX_BYTES + 1]).err(), Some(IndexError::TooLarge));
        assert_eq!(read("\n".repeat(MAX_LINES + 1).as_bytes()).err(), Some(IndexError::TooManyLines));
        assert!(read("\n".repeat(MAX_LINES).as_bytes()).is_ok());
    }
}
