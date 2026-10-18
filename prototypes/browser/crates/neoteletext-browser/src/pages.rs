//! The pages the receiver draws itself (REQ-SEC-025, REQ-BRW-004, `prototypes/browser/docs/desktop-browser.md`
//! section 9.5): the start page of a new tab, the keys page, and error
//! pages. They are teletext pages, so the browser keeps one look.

use crate::rows::{ALPHA_CYAN, ALPHA_GREEN, Activity, Page, line, row0, titled};
use neoteletext_core::consts::{ALPHA_RED, ALPHA_WHITE, ALPHA_YELLOW, ROWS};

/// The first row of the start page's list of bookmarks.
pub const START_LIST_ROW: usize = 6;
/// The most bookmarks the start page lists.
pub const START_LIST_LENGTH: usize = ROWS - 1 - START_LIST_ROW - 1;
/// The receiver's own name in row 0.
const RECEIVER_LABEL: &str = "neoteletext";

/// The start page: how to begin, and the first bookmarks, one per row
/// from [`START_LIST_ROW`], so a click on a row opens that bookmark.
#[must_use]
pub fn start(bookmarks: &[String], clock: Option<&str>) -> Page {
    let mut page = titled("neoteletext", ALPHA_CYAN);
    page[0] = row0(Activity::Receiver, RECEIVER_LABEL, clock);
    page[3] = line(&[ALPHA_WHITE], "Type an address above, or drop a file.");
    page[4] = line(&[ALPHA_WHITE], "Press K for the keys.");
    if bookmarks.is_empty() {
        page[START_LIST_ROW] = line(&[ALPHA_GREEN], "No bookmarks yet: press Cmd-D on a");
        page[START_LIST_ROW + 1] = line(&[ALPHA_GREEN], "page to keep it here.");
    }
    for (index, title) in bookmarks.iter().take(START_LIST_LENGTH).enumerate() {
        page[START_LIST_ROW + index] = line(&[ALPHA_YELLOW], title);
    }
    page
}

/// The keys page: the letter for each function first, so it is right on
/// every keyboard (`prototypes/browser/docs/web-native-concept.md` section 7).
#[must_use]
pub fn keys(clock: Option<&str>) -> Page {
    let mut page = titled("Keys", ALPHA_CYAN);
    page[0] = row0(Activity::Receiver, RECEIVER_LABEL, clock);
    for (index, (key, function)) in KEYS.iter().enumerate() {
        page[3 + index] = line(&[ALPHA_YELLOW], &format!("{key:<KEY_COLUMNS$}{function}"));
    }
    page
}

/// Columns for the key on the keys page.
const KEY_COLUMNS: usize = 12;
/// The keys page's lines: the key, then what it does. Each fits in a row,
/// and none uses a character that differs between national options.
const KEYS: [(&str, &str); 21] = [
    ("0-9", "Page number"),
    ("r g y c", "Colours (F1 to F4)"),
    ("i", "Index (End)"),
    ("h", "First page (Home)"),
    ("+ -", "Next, previous page"),
    ("Left Right", "Sub-pages"),
    ("Backspace f", "Back, forward"),
    ("H", "Hold (Insert)"),
    ("? m z", "Reveal, mix, zoom"),
    ("u", "Reload (F5)"),
    ("Enter l", "Catch links, show links"),
    ("Tab", "Into the page's form"),
    ("Enter", "Go, on the tuning screen"),
    ("Escape", "Cancel (Delete)"),
    ("Cmd-L", "Address bar (F6)"),
    ("Cmd-T Cmd-W", "New tab, close tab"),
    ("Cmd-D", "Bookmark this page"),
    ("Cmd-Left", "Back (Page Up)"),
    ("Cmd-Right", "Forward (Page Down)"),
    ("C", "Contents"),
    ("K", "This page"),
];

/// An error page: what went wrong, in the receiver's colours.
#[must_use]
pub fn error(title: &str, detail: &str, clock: Option<&str>) -> Page {
    let mut page = titled(title, ALPHA_RED);
    page[0] = row0(Activity::Receiver, RECEIVER_LABEL, clock);
    crate::rows::wrap(&mut page, 4, 8, ALPHA_WHITE, detail);
    page
}

/// A page shown while a site's first page loads.
#[must_use]
pub fn loading(label: &str, clock: Option<&str>) -> Page {
    let mut page = titled("Tuning in", ALPHA_GREEN);
    page[0] = row0(Activity::Receiver, label, clock);
    page[4] = line(&[ALPHA_WHITE], label);
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(page: &Page) -> String {
        page.iter().map(|row| String::from_utf8_lossy(row).into_owned() + "\n").collect()
    }

    #[test]
    fn every_line_of_the_keys_page_fits_and_shows_the_same_everywhere() {
        for (key, function) in KEYS {
            let text = format!("{key:<KEY_COLUMNS$}{function}");
            assert!(text.len() < neoteletext_core::consts::COLUMNS, "{text}");
            assert!(!crate::rows::showable(&text).contains(&b'?') || text.contains('?'), "{text}");
        }
    }

    #[test]
    fn pages_say_what_they_are_for() {
        let first = text(&start(&["News".to_string(), "Teefax".to_string()], None));
        assert!(first.contains("Type an address") && first.contains("Teefax"), "{first}");
        let all_keys = text(&keys(None));
        assert!(all_keys.contains("Index") && all_keys.contains("Bookmark"), "{all_keys}");
        let failure = text(&error("Cannot connect", "The site did not answer.", None));
        assert!(failure.contains("did not answer"), "{failure}");
        let bookmarks: Vec<String> = (0..40).map(|n| format!("Bookmark {n}")).collect();
        let full = text(&start(&bookmarks, None));
        assert!(full.contains("Bookmark 16") && !full.contains("Bookmark 17"), "the list stops before row 24");
    }
}
