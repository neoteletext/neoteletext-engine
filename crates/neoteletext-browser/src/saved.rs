//! What the browser keeps between runs (REQ-BRW-006, `docs/desktop-browser.md`
//! section 11), as text the program reads and writes: bookmarks, the
//! history of visits, the open tabs and the settings. Reading is strict and
//! bounded, because the files could have been changed by anything.
//!
//! Each record is one line of tab-separated fields. Addresses are stored
//! in the address bar's own form and read back through its parser, so a
//! stored address can never mean more than a typed one.

use crate::location::{Destination, parse_input};
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

/// The settings kept between runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Settings {
    /// Stretch pages to the 4:3 picture of a television (REQ-RND-004).
    pub aspect: bool,
}

const ASPECT_KEY: &str = "aspect";

impl Settings {
    /// The settings as `key=value` lines.
    #[must_use]
    pub fn write(&self) -> String {
        format!("{ASPECT_KEY}={}\n", u8::from(self.aspect))
    }

    /// Reads settings; unknown keys and bad values keep the defaults.
    #[must_use]
    pub fn read(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines().take(MAX_RECORDS) {
            if let Some((ASPECT_KEY, value)) = line.split_once('=') {
                settings.aspect = value.trim() == "1";
            }
        }
        settings
    }
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
        let settings = Settings { aspect: true };
        assert_eq!(Settings::read(&settings.write()), settings);
        // The retired glyph choice is an unknown key now (decision E.72).
        assert_eq!(Settings::read("glyphs=2\naspect=yes\nother=1"), Settings::default());
    }
}
