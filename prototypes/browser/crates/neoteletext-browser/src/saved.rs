//! What the browser keeps between runs (REQ-BRW-006, `prototypes/browser/docs/desktop-browser.md`
//! section 11), as text the program reads and writes: bookmarks, the
//! history of visits, the open tabs and the settings. Reading is strict and
//! bounded, because the files could have been changed by anything.
//!
//! Each record is one line of tab-separated fields. Addresses are stored
//! in the address bar's own form and read back through its parser, so a
//! stored address can never mean more than a typed one.

use crate::location::{Destination, parse_input};
use neoteletext_core::glyphs::conformance::{FONT_SET_COUNT, FONT_SETS, font_set_index};
use std::fmt::Write as _;

/// The most records read from one file.
pub const MAX_RECORDS: usize = 10_000;
/// The longest line read.
const MAX_LINE: usize = 4096;
/// Seconds of history kept (decision 10 of the browser design: 30 days).
pub const HISTORY_SECONDS: u64 = 30 * 24 * 60 * 60;
/// The field separator.
const SEPARATOR: char = '\t';

/// A saved page: a bookmark, a visit, or an open tab.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Saved {
    /// When, in seconds since 1970; 0 for bookmarks and tabs.
    pub time: u64,
    /// The title shown for it.
    pub title: String,
    /// Where.
    pub destination: Destination,
}

/// A field made safe to store: no separators or line ends.
fn field(text: &str) -> String {
    text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// Writes records, newest last.
#[must_use]
pub fn write(records: &[Saved]) -> String {
    let mut out = String::new();
    for record in records {
        let address = record.destination.location.address(record.destination.page, false);
        // Writing to a String cannot fail.
        let _ = writeln!(out, "{}{SEPARATOR}{}{SEPARATOR}{}", record.time, field(&record.title), field(&address));
    }
    out
}

/// Reads records, skipping any line that is not exactly one. At most
/// [`MAX_RECORDS`] are read.
#[must_use]
pub fn read(text: &str) -> Vec<Saved> {
    text.lines()
        .filter(|line| line.len() <= MAX_LINE)
        .filter_map(|line| {
            let mut fields = line.split(SEPARATOR);
            let (Some(time), Some(title), Some(address), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                return None;
            };
            let time = time.parse().ok()?;
            let destination = parse_input(address).ok()?;
            Some(Saved { time, title: title.to_string(), destination })
        })
        .take(MAX_RECORDS)
        .collect()
}

/// History without visits older than [`HISTORY_SECONDS`] before `now`,
/// and at most [`MAX_RECORDS`] of the newest.
#[must_use]
pub fn prune_history(mut visits: Vec<Saved>, now: u64) -> Vec<Saved> {
    let oldest = now.saturating_sub(HISTORY_SECONDS);
    visits.retain(|visit| visit.time >= oldest);
    let excess = visits.len().saturating_sub(MAX_RECORDS);
    visits.drain(..excess);
    visits
}

/// The settings kept between runs, for every tab (REQ-BRW-007).
#[derive(Clone, PartialEq, Eq, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each is an independent switch the user sets, saved as its own key; no two form a state machine"
)]
pub struct Settings {
    /// Stretch pages to the 4:3 picture of a television (REQ-RND-004).
    pub aspect: bool,
    /// The font the user chose for each character set, by its place in
    /// `FONT_SETS`, or none: Bedstead (REQ-FNT-005, decision E.75 item 6).
    pub fonts: [Option<UserFont>; FONT_SET_COUNT],
    /// "Always use my fonts": the user's font for each set, or Bedstead,
    /// whatever the channel or page names (REQ-FNT-005).
    pub always_my_font: bool,
    /// Allow fonts from other sites (REQ-FNT-003); allowed by default.
    pub fonts_from_other_sites: bool,
    /// The font libraries the user added, by their folders' addresses
    /// (REQ-FNT-004), at most [`MAX_LIBRARIES`].
    pub libraries: Vec<String>,
    /// The font browser panel is shown (REQ-BRW-010).
    pub font_browser: bool,
    /// The font browser lists fonts that draw the same as one listed
    /// before them (REQ-BRW-010); off by default.
    pub show_duplicate_fonts: bool,
    /// The page area shows the chosen font's sample instead of the page
    /// while the font browser is open (REQ-BRW-010); off by default.
    pub show_font_sample: bool,
    /// Character rounding (REQ-RND-012); on by default.
    pub rounding: bool,
    /// The one font every page is drawn with: an embedded font's id, or
    /// empty for Bedstead, the bundled font (owner, 14 October 2026).
    pub embedded_font: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            aspect: true,
            fonts: [const { None }; FONT_SET_COUNT],
            always_my_font: false,
            fonts_from_other_sites: true,
            libraries: Vec::new(),
            font_browser: false,
            show_duplicate_fonts: false,
            show_font_sample: false,
            rounding: true,
            embedded_font: String::new(),
        }
    }
}

/// The font the user chose: its SHA-256, which finds it in the cache, where
/// it came from, by address or local file, and its title.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UserFont {
    /// Its SHA-256, as 64 hexadecimal digits.
    pub sha256: String,
    /// Where it came from: an address, or a local file's path.
    pub address: String,
    /// Its title: the library's, or its name.
    pub title: String,
    /// The user named it by address alone, so it is checked daily
    /// (REQ-FNT-009).
    pub by_address: bool,
}

/// The most libraries kept.
pub const MAX_LIBRARIES: usize = 64;
const ASPECT_KEY: &str = "aspect";
/// A set's font is saved under this prefix and the set's name of
/// REQ-FNT-011, such as `font.G0-Latin-German`.
const FONT_KEY_PREFIX: &str = "font.";
const ALWAYS_KEY: &str = "always_my_font";
const OTHER_SITES_KEY: &str = "fonts_from_other_sites";
const LIBRARY_KEY: &str = "library";
const FONT_BROWSER_KEY: &str = "font_browser";
const DUPLICATES_KEY: &str = "show_duplicate_fonts";
const SAMPLE_KEY: &str = "show_font_sample";
const ROUNDING_KEY: &str = "rounding";
const EMBEDDED_FONT_KEY: &str = "embedded_font";
/// SHA-256 in hexadecimal.
const SHA256_DIGITS: usize = 64;

impl Settings {
    /// The settings as `key=value` lines.
    #[must_use]
    pub fn write(&self) -> String {
        let mut out = format!(
            "{ASPECT_KEY}={}\n{ALWAYS_KEY}={}\n{OTHER_SITES_KEY}={}\n{FONT_BROWSER_KEY}={}\n{DUPLICATES_KEY}={}\n{SAMPLE_KEY}={}\n{ROUNDING_KEY}={}\n",
            u8::from(self.aspect),
            u8::from(self.always_my_font),
            u8::from(self.fonts_from_other_sites),
            u8::from(self.font_browser),
            u8::from(self.show_duplicate_fonts),
            u8::from(self.show_font_sample),
            u8::from(self.rounding)
        );
        if !self.embedded_font.is_empty() {
            let _ = writeln!(out, "{EMBEDDED_FONT_KEY}={}", field(&self.embedded_font));
        }
        for ((name, _), font) in FONT_SETS.iter().zip(&self.fonts) {
            let Some(font) = font else { continue };
            let _ = writeln!(
                out,
                "{FONT_KEY_PREFIX}{name}={}{SEPARATOR}{}{SEPARATOR}{}{SEPARATOR}{}",
                font.sha256,
                u8::from(font.by_address),
                field(&font.address),
                field(&font.title)
            );
        }
        for library in self.libraries.iter().take(MAX_LIBRARIES) {
            let _ = writeln!(out, "{LIBRARY_KEY}={}", field(library));
        }
        out
    }

    /// Reads settings; unknown keys and bad values keep the defaults.
    #[must_use]
    pub fn read(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines().filter(|line| line.len() <= MAX_LINE).take(MAX_RECORDS) {
            match line.split_once('=') {
                Some((ASPECT_KEY, value)) => settings.aspect = value.trim() != "0",
                Some((ALWAYS_KEY, value)) => settings.always_my_font = value.trim() == "1",
                Some((OTHER_SITES_KEY, value)) => settings.fonts_from_other_sites = value.trim() != "0",
                Some((FONT_BROWSER_KEY, value)) => settings.font_browser = value.trim() == "1",
                Some((DUPLICATES_KEY, value)) => settings.show_duplicate_fonts = value.trim() == "1",
                Some((SAMPLE_KEY, value)) => settings.show_font_sample = value.trim() == "1",
                Some((ROUNDING_KEY, value)) => settings.rounding = value.trim() != "0",
                Some((EMBEDDED_FONT_KEY, value)) => settings.embedded_font = value.trim().to_string(),
                Some((key, value)) if key.starts_with(FONT_KEY_PREFIX) => {
                    let place = font_set_index(key.trim_start_matches(FONT_KEY_PREFIX));
                    if let Some(slot) = place.and_then(|place| settings.fonts.get_mut(place)) {
                        *slot = user_font(value);
                    }
                }
                Some((LIBRARY_KEY, value)) if !value.is_empty() && settings.libraries.len() < MAX_LIBRARIES => {
                    settings.libraries.push(value.to_string());
                }
                _ => {}
            }
        }
        settings
    }
}

fn user_font(value: &str) -> Option<UserFont> {
    let mut fields = value.split(SEPARATOR);
    let (Some(sha256), Some(by_address), Some(address), Some(title), None) =
        (fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    if sha256.len() != SHA256_DIGITS || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) || address.is_empty() {
        return None;
    }
    let by_address = match by_address {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    Some(UserFont {
        sha256: sha256.to_ascii_lowercase(),
        address: address.to_string(),
        title: title.to_string(),
        by_address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(time: u64, title: &str, address: &str) -> Saved {
        Saved { time, title: title.to_string(), destination: parse_input(address).unwrap() }
    }

    #[test]
    fn records_round_trip() {
        let records = vec![
            saved(0, "News\tand\nweather", "https://news.example/140.t42"),
            saved(5, "Teefax", "zxnet:teefax"),
            saved(9, "Capture", "file:///tmp/a b.t42"),
        ];
        let text = write(&records);
        assert_eq!(text.lines().count(), 3, "separators in titles are replaced");
        let back = read(&text);
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].title, "News and weather");
        assert_eq!(back[0].destination, records[0].destination);
        assert_eq!(back[2].destination, records[2].destination);
    }

    #[test]
    fn bad_lines_are_skipped() {
        let text =
            "x\ty\tz\n1\tok\thttps://a.example/\n2\tbad\tjavascript:alert(1)\n3\textra\thttps://a.example/\tmore\n";
        let back = read(text);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].title, "ok");
    }

    #[test]
    fn history_keeps_thirty_days() {
        let now = HISTORY_SECONDS * 2;
        let visits = vec![saved(1, "old", "https://a.example/"), saved(now - 10, "new", "https://a.example/")];
        let kept = prune_history(visits, now);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].title, "new");
    }

    #[test]
    fn settings_round_trip_and_ignore_nonsense() {
        let font = UserFont {
            sha256: "5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d".into(),
            address: "https://fonts.example/a.bdf".into(),
            title: "A\tfont".into(),
            by_address: true,
        };
        let german = font_set_index("G0-Latin-German").unwrap();
        let greek = font_set_index("G2-Greek").unwrap();
        let mut fonts = [const { None }; FONT_SET_COUNT];
        fonts[german] = Some(font.clone());
        fonts[greek] = Some(UserFont { by_address: false, ..font.clone() });
        let settings = Settings {
            aspect: true,
            fonts,
            always_my_font: true,
            fonts_from_other_sites: false,
            libraries: vec!["https://lib.example/teletext/".into(), "https://other.example/".into()],
            font_browser: true,
            show_duplicate_fonts: true,
            show_font_sample: true,
            rounding: false,
            embedded_font: "philips-complete".into(),
        };
        let written = settings.write();
        assert!(written.contains("font.G0-Latin-German=") && written.contains("font.G2-Greek="), "{written}");
        let again = Settings::read(&written);
        assert_eq!(again.fonts[german].as_ref().map(|font| font.title.as_str()), Some("A font"), "no tab in a field");
        let mut expected = settings.clone();
        for slot in expected.fonts.iter_mut().flatten() {
            slot.title = "A font".into();
        }
        assert_eq!(expected, again);
        // The retired glyph choice and the single font of earlier settings are
        // unknown keys now (decisions E.72 and E.75 item 6).
        assert_eq!(Settings::read("glyphs=2\naspect=yes\nother=1"), Settings::default());
        assert_eq!(
            Settings::read("font=5ee6246b7f9bd197f47ce0741ea799a55ac08af775869a08c085803e02a3f14d\t1\thttps://x/\tX"),
            Settings::default()
        );
        assert!(Settings::default().fonts_from_other_sites, "allowed by default (REQ-BRW-007)");
        assert!(Settings::read("font.G0-Latin-German=abc\t1\thttps://x/\tX").fonts[german].is_none(), "a bad checksum");
        assert_eq!(Settings::read("font.G0-Klingon=x").fonts, [const { None }; FONT_SET_COUNT], "an unknown set");
    }
}
