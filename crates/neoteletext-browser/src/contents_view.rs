//! The contents view (REQ-BRW-009, decision E.70 item 8): a channel's
//! `contents.txt` drawn by the receiver as teletext index screens, or,
//! without one, the pages received so far.
//!
//! The file is read by `neoteletext_web::contents` and drawn here as rows
//! of 7-bit codes, like the receiver's other pages; nothing in it passes
//! through a decoder. Titles are the channel's text and are drawn only
//! with the characters every national option shows the same.

use crate::rows::{ALPHA_CYAN, Activity, Page, blank, line, row0, showable, titled};
use neoteletext_core::consts::{ALPHA_WHITE, ALPHA_YELLOW, COLUMNS, ROWS};
use neoteletext_web::contents::{self, Date, Entry};

/// The first row of entries.
const FIRST_ROW: usize = 4;
/// The last row of entries; row 24 holds the keys.
const LAST_ROW: usize = ROWS - 2;
/// Entries on one screen.
pub const ENTRIES_PER_SCREEN: usize = LAST_ROW - FIRST_ROW + 1;
/// The row for a note about where the list came from.
const NOTE_ROW: usize = 3;
/// The column of the mark: `*` for a changed page, `»` for another
/// channel.
const MARK_COLUMN: usize = 1;
/// The first column of a title.
const TITLE_COLUMN: usize = 2;
/// The last column the dots reach.
const LAST_DOT_COLUMN: usize = 35;
/// The first column of the page number.
const NUMBER_COLUMN: usize = COLUMNS - 3;
/// The column of the screen number on the title rows.
const SCREEN_NUMBER_COLUMN: usize = 33;
/// The mark for a page changed since the previous visit.
const CHANGED: u8 = b'*';
/// The dot leaders.
const LEADER: u8 = b'.';
/// The keys, on row 24.
const KEYS_LINE: &str = "Enter go   Left Right more   Esc back";
/// The file name of a channel's contents, for the address bar.
pub const CONTENTS_FILE_NAME: &str = neoteletext_web::channel::CONTENTS_FILE;
/// What the view says when the channel has no contents file.
pub const NO_CONTENTS: &str = "No contents file: pages received so far";

/// One line of the contents.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Line {
    Section(String),
    Page { page: u16, title: String, changed: bool },
    Channel { target: String, title: String, page: Option<u16> },
}

/// Where an entry leads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Target {
    /// A page of this channel, `0xMPP`.
    Page(u16),
    /// Another channel: `name:mpp` or an address.
    Channel(String),
}

/// A key the contents view acts on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    /// Select the previous entry.
    Up,
    /// Select the next entry.
    Down,
    /// The previous screen.
    Left,
    /// The next screen.
    Right,
    /// Follow the selected entry.
    Enter,
}

/// A channel's contents, ready to draw.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContentsView {
    lines: Vec<Line>,
    /// The line indices on each screen.
    screens: Vec<Vec<usize>>,
    screen: usize,
    selected: Option<usize>,
    note: Option<String>,
    /// Lines of `contents.txt` that were ignored (REQ-WEB-025).
    pub ignored: usize,
}

/// The page number of a target, `name:mpp` or a page file's address.
fn target_page(target: &str) -> Option<u16> {
    let digits = match target.rsplit_once(':') {
        Some((_, digits)) if !target.contains("://") => digits,
        _ => target.rsplit('/').next()?.split('.').next()?,
    };
    crate::location::page_number(digits)
}

impl ContentsView {
    /// The contents of a channel's `contents.txt`. A page entry is marked
    /// changed when its date is later than `previous_visit`. A file over the
    /// limits is listed as `received` instead, with the reason.
    #[must_use]
    pub fn from_file(bytes: &[u8], previous_visit: Option<Date>, received: &[(u16, String)]) -> Self {
        let Ok(entries) = contents::read(bytes) else {
            return Self::from_received(received, "The contents file is too large to use.");
        };
        let mut lines = Vec::new();
        let mut ignored = 0usize;
        for entry in entries {
            match entry {
                Ok(Entry::Section { title }) => lines.push(Line::Section(title.to_string())),
                Ok(Entry::Page { page, title, date }) => lines.push(Line::Page {
                    page: page.mpp(),
                    title: title.to_string(),
                    changed: matches!((date, previous_visit), (Some(date), Some(visit)) if date > visit),
                }),
                Ok(Entry::Channel { target, title }) => lines.push(Line::Channel {
                    target: target.to_string(),
                    title: title.to_string(),
                    page: target_page(target),
                }),
                Err(_) => ignored = ignored.saturating_add(1),
            }
        }
        Self::new(lines, None, ignored)
    }

    /// The pages received so far, each with its title, when the channel has
    /// no usable contents file; `note` says why.
    #[must_use]
    pub fn from_received(received: &[(u16, String)], note: &str) -> Self {
        let mut pages: Vec<&(u16, String)> = received.iter().collect();
        pages.sort_by_key(|(page, _)| *page);
        pages.dedup_by_key(|(page, _)| *page);
        let lines = pages
            .into_iter()
            .map(|(page, title)| Line::Page {
                page: *page,
                title: title.chars().take(contents::MAX_TITLE).collect(),
                changed: false,
            })
            .collect();
        Self::new(lines, Some(note.to_string()), 0)
    }

    fn new(lines: Vec<Line>, note: Option<String>, ignored: usize) -> Self {
        let mut screens: Vec<Vec<usize>> = vec![Vec::new()];
        for (index, line) in lines.iter().enumerate() {
            let full = screens.last().is_some_and(|screen| screen.len() >= ENTRIES_PER_SCREEN);
            // A heading on the last row would be cut off from its entries.
            let heading_last = matches!(line, Line::Section(_))
                && screens.last().is_some_and(|screen| screen.len() == ENTRIES_PER_SCREEN - 1);
            if full || heading_last {
                screens.push(Vec::new());
            }
            if let Some(screen) = screens.last_mut() {
                screen.push(index);
            }
        }
        Self { lines, screens, screen: 0, selected: None, note, ignored }
    }

    /// The number of screens.
    #[must_use]
    pub fn screens(&self) -> usize {
        self.screens.len()
    }

    /// The entries that lead somewhere, on the screen shown, as line
    /// indices.
    fn links(&self) -> Vec<usize> {
        self.screens
            .get(self.screen)
            .map(|screen| {
                screen
                    .iter()
                    .copied()
                    .filter(|&index| !matches!(self.lines.get(index), Some(Line::Section(_))))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn target(&self, index: usize) -> Option<Target> {
        match self.lines.get(index)? {
            Line::Section(_) => None,
            Line::Page { page, .. } => Some(Target::Page(*page)),
            Line::Channel { target, .. } => Some(Target::Channel(target.clone())),
        }
    }

    /// Acts on a key; returns where to go when the key follows an entry.
    pub fn key(&mut self, key: Key) -> Option<Target> {
        let links = self.links();
        let position = self.selected.and_then(|selected| links.iter().position(|&index| index == selected));
        match key {
            Key::Down => self.selected = links.get(position.map_or(0, |at| (at + 1).min(links.len() - 1))).copied(),
            Key::Up => self.selected = links.get(position.map_or(0, |at| at.saturating_sub(1))).copied(),
            Key::Left => {
                self.screen = self.screen.saturating_sub(1);
                self.selected = None;
            }
            Key::Right => {
                self.screen = (self.screen + 1).min(self.screens.len().saturating_sub(1));
                self.selected = None;
            }
            Key::Enter => return self.selected.and_then(|index| self.target(index)),
        }
        None
    }

    /// The entry drawn on `row` of the screen shown, for the pointer.
    #[must_use]
    pub fn at_row(&self, row: usize) -> Option<Target> {
        let slot = row.checked_sub(FIRST_ROW)?;
        let index = *self.screens.get(self.screen)?.get(slot)?;
        self.target(index)
    }

    /// The screen shown, with row 0 naming the channel by `label`, and the
    /// cells where the receiver's `»` goes, as (row, column): the rows are
    /// 7-bit codes, and the mark is a G2 character.
    #[must_use]
    pub fn page(&self, label: &str, clock: Option<&str>) -> (Page, Vec<(usize, usize)>) {
        let mut page = titled("Contents", ALPHA_CYAN);
        page[0] = row0(Activity::Contents, label, clock);
        let number = format!("{}/{}", self.screen + 1, self.screens.len());
        for (offset, byte) in showable(&number).into_iter().enumerate() {
            if let Some(cell) = page[1].get_mut(SCREEN_NUMBER_COLUMN + offset) {
                *cell = byte;
            }
        }
        if let Some(note) = &self.note {
            page[NOTE_ROW] = line(&[ALPHA_WHITE], note);
        }
        let mut marks = Vec::new();
        let shown = self.screens.get(self.screen).cloned().unwrap_or_default();
        for (slot, index) in shown.into_iter().enumerate() {
            let row = FIRST_ROW + slot;
            let Some(entry) = self.lines.get(index) else { continue };
            let mut cells = blank();
            let (colour, mark, title, number) = match entry {
                Line::Section(title) => (ALPHA_YELLOW, None, title, None),
                Line::Page { page, title, changed } => {
                    (ALPHA_WHITE, Some(if *changed { CHANGED } else { b' ' }), title, Some(*page))
                }
                Line::Channel { title, page, .. } => {
                    marks.push((row, MARK_COLUMN));
                    (ALPHA_CYAN, Some(b' '), title, *page)
                }
            };
            cells[0] = colour;
            let start = if let Some(mark) = mark {
                cells[MARK_COLUMN] = mark;
                TITLE_COLUMN
            } else {
                MARK_COLUMN
            };
            let text = showable(title);
            for (offset, &byte) in text.iter().take(contents::MAX_TITLE).enumerate() {
                cells[start + offset] = byte;
            }
            if let Some(number) = number {
                let end = start + text.len().min(contents::MAX_TITLE);
                for cell in cells.iter_mut().take(LAST_DOT_COLUMN + 1).skip(end) {
                    *cell = LEADER;
                }
                for (offset, byte) in format!("{number:03X}").bytes().enumerate() {
                    cells[NUMBER_COLUMN + offset] = byte;
                }
            }
            page[row] = cells;
        }
        page[ROWS - 1] = line(&[ALPHA_WHITE], KEYS_LINE);
        (page, marks)
    }

    /// The row of the selected entry on the screen shown, which the window
    /// draws with its colours swapped, as page catching does
    /// (REQ-NAV-015).
    #[must_use]
    pub fn selected_row(&self) -> Option<usize> {
        let selected = self.selected?;
        let slot = self.screens.get(self.screen)?.iter().position(|&index| index == selected)?;
        Some(FIRST_ROW + slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn text(page: &Page) -> String {
        page.iter().map(|row| String::from_utf8_lossy(row).into_owned() + "\n").collect()
    }

    const FILE: &[u8] =
        b"s\tNews\np\t101\tHeadlines\t2026-10-12\np\t102\tWorld\t2026-01-01\nx\tweather:300\tWeather\nq\tlater\n";

    #[test]
    fn a_contents_file_is_an_index() {
        let visit = Date { year: 2026, month: 6, day: 1 };
        let view = ContentsView::from_file(FILE, Some(visit), &[]);
        assert_eq!(view.ignored, 1);
        let (page, marks) = view.page("example.org", None);
        let all = text(&page);
        assert!(all.contains("LIST") && all.contains("Contents") && all.contains("1/1"), "{all}");
        assert!(all.contains("*Headlines....") && all.contains("101"), "{all}");
        assert!(all.contains(" World....") && !all.contains("*World"), "unchanged since the visit: {all}");
        assert!(all.contains("Weather") && all.contains("300"), "{all}");
        assert_eq!(marks, [(FIRST_ROW + 3, MARK_COLUMN)]);
        assert_eq!(page[FIRST_ROW][0], ALPHA_YELLOW, "the heading");
    }

    #[test]
    fn keys_select_and_follow_only_entries_that_lead_somewhere() {
        let mut view = ContentsView::from_file(FILE, None, &[]);
        assert_eq!(view.key(Key::Enter), None, "nothing is selected yet");
        view.key(Key::Down);
        assert_eq!(view.key(Key::Enter), Some(Target::Page(0x101)), "the heading is skipped");
        view.key(Key::Down);
        view.key(Key::Down);
        view.key(Key::Down);
        assert_eq!(view.key(Key::Enter), Some(Target::Channel("weather:300".into())));
        assert_eq!(view.at_row(FIRST_ROW + 2), Some(Target::Page(0x102)));
        assert_eq!(view.at_row(FIRST_ROW), None, "a heading leads nowhere");
    }

    #[test]
    fn long_contents_take_several_screens_and_headings_never_end_one() {
        let mut file = String::new();
        for number in 0..(ENTRIES_PER_SCREEN - 1) {
            let _ = writeln!(file, "p\t1{number:02}\tPage");
        }
        file.push_str("s\tSport\np\t300\tSport\n");
        let mut view = ContentsView::from_file(file.as_bytes(), None, &[]);
        assert_eq!(view.screens(), 2);
        view.key(Key::Right);
        let all = text(&view.page("x", None).0);
        assert!(all.contains("2/2") && all.contains("Sport"), "{all}");
    }

    #[test]
    fn without_a_file_the_received_pages_are_listed() {
        let received = [(0x200, "Second".to_string()), (0x100, "First".to_string())];
        let view = ContentsView::from_received(&received, NO_CONTENTS);
        let all = text(&view.page("x", None).0);
        assert!(all.contains(NO_CONTENTS), "{all}");
        assert!(all.find("First").unwrap() < all.find("Second").unwrap(), "in page order: {all}");
    }
}
