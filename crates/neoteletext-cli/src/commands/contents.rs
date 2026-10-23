//! `neottx contents FOLDER` (REQ-CLI-008, decision E.70 item 9): writes a
//! channel's `contents.txt` from the author's own folder of page files.
//!
//! The folder is listed and its files read and decoded first; then the
//! sandbox is applied, as for every other command (REQ-SEC-017), and the
//! titles are taken from the decoded pages. Nothing is written but
//! standard output and standard error.

use super::io_failure;
use super::sandbox_closed;
use crate::ansi::{MosaicStyle, plain};
use neoteletext_app::input::{self, Options};
use neoteletext_app::session::Session;
use neoteletext_app::{Failure, exit};
use neoteletext_core::types::PageAddress;
use neoteletext_web::channel::{self, ChannelFile};
use neoteletext_web::contents::MAX_TITLE;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The TTI command that gives a page's description (REQ-INP-013).
const DESCRIPTION_COMMAND: &str = "DE,";
/// Seconds in a day.
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

/// A page found, with what titles and dates it.
struct Found {
    title: String,
    modified: Option<SystemTime>,
}

/// A file of the folder, read and decoded before the sandbox.
struct Loaded {
    path: PathBuf,
    /// The page its name gives; `None` for `index.t42`.
    page: Option<PageAddress>,
    description: Option<String>,
    modified: Option<SystemTime>,
    session: Session,
}

/// The civil date of a day count since 1970-01-01 (H. Hinnant's
/// days-from-civil inverse, proleptic Gregorian calendar).
///
/// Every step is exact for any day count a file's time gives; the
/// saturating forms only bound a day count near `u64::MAX`, which no file
/// has, so the algorithm cannot panic on a hostile timestamp.
fn civil_date(days: u64) -> (u64, u64, u64) {
    // Shift the epoch to 0000-03-01, so leap days fall at the end of a
    // year: 719 468 days lie between that day and 1970-01-01.
    let shifted = days.saturating_add(719_468);
    // An era is 400 years of 146 097 days.
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era = day_of_era
        .saturating_sub(day_of_era / 1460)
        .saturating_add(day_of_era / 36_524)
        .saturating_sub(day_of_era / 146_096)
        / 365;
    let day_of_year = day_of_era.saturating_sub(
        year_of_era.saturating_mul(365).saturating_add(year_of_era / 4).saturating_sub(year_of_era / 100),
    );
    // Months counted from March, of 153 days per 5 months.
    let month_index = day_of_year.saturating_mul(5).saturating_add(2) / 153;
    let day = day_of_year.saturating_sub(month_index.saturating_mul(153).saturating_add(2) / 5).saturating_add(1);
    let month = if month_index < 10 { month_index.saturating_add(3) } else { month_index.saturating_sub(9) };
    let year = year_of_era.saturating_add(era.saturating_mul(400)).saturating_add(u64::from(month <= 2));
    (year, month, day)
}

/// A modification time as `YYYY-MM-DD` in UTC.
fn date(time: SystemTime) -> Option<String> {
    let seconds = time.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    let (year, month, day) = civil_date(seconds / SECONDS_PER_DAY);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Text as a title: printable ASCII only, spaces collapsed, at most
/// [`MAX_TITLE`] characters.
fn title_text(text: &str) -> String {
    let printable: String = text.chars().map(|c| if c.is_ascii_graphic() { c } else { ' ' }).collect();
    let words = printable.split_whitespace().collect::<Vec<_>>().join(" ");
    words.chars().take(MAX_TITLE).collect()
}

/// The first row of rows 1 to 23 of a page that is not blank.
fn first_line(session: &mut Session, page: PageAddress) -> Option<String> {
    let key = session.pages().iter().find(|stored| stored.key.address == page).map(|stored| stored.key)?;
    let grid = session.grid(key, None)?;
    let text = plain(&grid, false, MosaicStyle::Unicode16);
    text.lines().skip(1).take(23).map(title_text).find(|line| !line.is_empty())
}

/// The description of a TTI file, from its first `DE` line.
fn description(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().find(|line| line.starts_with(DESCRIPTION_COMMAND))?;
    let title = title_text(line.get(DESCRIPTION_COMMAND.len()..)?);
    (!title.is_empty()).then_some(title)
}

/// Reads and decodes one file of the folder; `None`, with a diagnostic,
/// when it cannot be read or fails the check for its format
/// (REQ-WEB-027).
fn load(options: &Options, path: &Path, file: ChannelFile, modified: Option<SystemTime>) -> Option<Loaded> {
    let report = |why: &str| eprintln!("neottx: {}: skipped: {why}", path.display());
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            report(&error.to_string());
            return None;
        }
    };
    let (page, t42) = match file {
        ChannelFile::Page { page, t42 } => (Some(page), t42),
        _ => (None, true),
    };
    let valid = if t42 { channel::is_t42(&bytes) } else { channel::is_tti(&bytes) };
    if !valid {
        report(if t42 { "not whole T42 packets" } else { "not a TTI page file" });
        return None;
    }
    let file_options = Options { input: path.to_string_lossy().into_owned(), ..options.clone() };
    match input::load(&file_options) {
        Ok(session) => Some(Loaded {
            path: path.to_path_buf(),
            page,
            description: if t42 { None } else { description(&bytes) },
            modified,
            session,
        }),
        Err((_, message)) => {
            report(&message);
            None
        }
    }
}

/// `neottx contents FOLDER`.
pub fn contents(options: &Options) -> Result<(), Failure> {
    let folder = Path::new(&options.input);
    let listing = std::fs::read_dir(folder).map_err(|error| io_failure(folder, error))?;
    let mut files: Vec<(PathBuf, ChannelFile, Option<SystemTime>)> = Vec::new();
    for entry in listing.flatten() {
        // Only the folder's own files: a subfolder is another channel.
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = entry.file_name();
        let file = match name.to_str().and_then(ChannelFile::from_name) {
            Some(file @ ChannelFile::Page { page, .. }) if page.page.is_decimal() => file,
            Some(file @ ChannelFile::Bundle) => file,
            _ => continue,
        };
        let modified = entry.metadata().and_then(|metadata| metadata.modified()).ok();
        files.push((entry.path(), file, modified));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut loaded = Vec::new();
    let mut skipped = 0usize;
    for (path, file, modified) in &files {
        match load(options, path, *file, *modified) {
            Some(file) => loaded.push(file),
            None => skipped = skipped.saturating_add(1),
        }
    }
    sandbox_closed(options.no_sandbox)?;
    let mut pages: BTreeMap<PageAddress, Found> = BTreeMap::new();
    for file in &mut loaded {
        let wanted: Vec<PageAddress> = if let Some(page) = file.page {
            vec![page]
        } else {
            let mut all: Vec<PageAddress> = file.session.pages().iter().map(|stored| stored.key.address).collect();
            all.dedup();
            all.into_iter().filter(|page| page.page.is_decimal()).collect()
        };
        for page in wanted {
            if !file.session.pages().iter().any(|stored| stored.key.address == page) {
                eprintln!("neottx: {}: skipped: it does not hold its page", file.path.display());
                skipped = skipped.saturating_add(1);
                continue;
            }
            let line = first_line(&mut file.session, page);
            // A page file wins over the bundle for its own page.
            if file.page.is_none() && pages.contains_key(&page) {
                continue;
            }
            let title = file
                .description
                .clone()
                .or(line)
                .unwrap_or_else(|| format!("Page {}", String::from_utf8_lossy(&page.ascii())));
            pages.insert(page, Found { title, modified: file.modified });
        }
    }
    let mut out = std::io::stdout().lock();
    let mut magazine = None;
    for (page, found) in &pages {
        if magazine != Some(page.magazine) {
            magazine = Some(page.magazine);
            let heading = PageAddress::from_mpp(u16::from(page.magazine.number()) << 8)
                .and_then(|first| pages.get(&first))
                .map_or_else(|| format!("Magazine {}", page.magazine.number()), |first| first.title.clone());
            let _ = writeln!(out, "s\t{heading}");
        }
        let number = String::from_utf8_lossy(&page.ascii()).into_owned();
        let dated = found.modified.and_then(date).map(|date| format!("\t{date}")).unwrap_or_default();
        let _ = writeln!(out, "p\t{number}\t{}{dated}", found.title);
    }
    if skipped > 0 {
        return Err((exit::CHECK_ERRORS, format!("{skipped} file(s) skipped")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        // 2000-02-29, a leap day in a year divisible by 400.
        assert_eq!(civil_date(11_016), (2000, 2, 29));
        assert_eq!(civil_date(20_738), (2026, 10, 12));
    }

    #[test]
    fn titles_are_printable_and_short() {
        assert_eq!(title_text("  NEWS \u{1fb00} INDEX  "), "NEWS INDEX");
        assert_eq!(title_text(&"x".repeat(40)).len(), MAX_TITLE);
        assert_eq!(description(b"DE,  Weather today \r\nPN,10000\r\n").as_deref(), Some("Weather today"));
    }
}
