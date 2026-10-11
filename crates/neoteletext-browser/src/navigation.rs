//! Each tab's history (REQ-BRW-003, `docs/desktop-browser.md` section 6): a flat list of
//! entries and the index of the current one, as Chromium's navigation
//! controller keeps it. A new page after going back drops the entries
//! ahead of it.

use crate::location::Location;

/// The most entries a tab keeps; the oldest go first.
pub const MAX_ENTRIES: usize = 100;
/// The longest title kept for an entry, in characters.
const MAX_TITLE: usize = 80;

/// One page a tab has shown.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Entry {
    /// Where.
    pub location: Location,
    /// The page, `0xMPP`.
    pub page: u16,
    /// The page's title, for menus and the tab: its first line of text.
    pub title: String,
}

/// A tab's history.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Navigation {
    entries: Vec<Entry>,
    current: Option<usize>,
}

/// A title made safe and short: printable characters only, trimmed.
#[must_use]
pub fn clean_title(text: &str) -> String {
    let cleaned: String =
        text.chars().map(|c| if c.is_control() { ' ' } else { c }).take(MAX_TITLE).collect::<String>();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Navigation {
    /// An empty history.
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: Vec::new(), current: None }
    }

    /// The entry shown now.
    #[must_use]
    pub fn current(&self) -> Option<&Entry> {
        self.current.and_then(|index| self.entries.get(index))
    }

    /// Every entry, oldest first, and the index of the current one.
    #[must_use]
    pub fn entries(&self) -> (&[Entry], Option<usize>) {
        (&self.entries, self.current)
    }

    /// A page was shown that history did not lead to: it becomes the
    /// current entry, and the entries ahead are dropped. Showing the
    /// current entry's page again only refreshes its title.
    pub fn visit(&mut self, location: Location, page: u16, title: &str) {
        let title = clean_title(title);
        if let Some(current) = self.current.and_then(|index| self.entries.get_mut(index))
            && current.location == location
            && current.page == page
        {
            if !title.is_empty() {
                current.title = title;
            }
            return;
        }
        let keep = self.current.map_or(0, |index| index + 1);
        self.entries.truncate(keep);
        self.entries.push(Entry { location, page, title });
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
        self.current = Some(self.entries.len() - 1);
    }

    /// Can go back.
    #[must_use]
    pub fn can_go_back(&self) -> bool {
        self.current.is_some_and(|index| index > 0)
    }

    /// Can go forward.
    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        self.current.is_some_and(|index| index + 1 < self.entries.len())
    }

    /// Moves back one entry and returns it.
    pub fn back(&mut self) -> Option<&Entry> {
        let index = self.current.filter(|&index| index > 0)? - 1;
        self.current = Some(index);
        self.entries.get(index)
    }

    /// Moves forward one entry and returns it.
    pub fn forward(&mut self) -> Option<&Entry> {
        let index = self.current? + 1;
        if index >= self.entries.len() {
            return None;
        }
        self.current = Some(index);
        self.entries.get(index)
    }

    /// Moves to the entry at `index`, as a history menu does.
    pub fn go_to(&mut self, index: usize) -> Option<&Entry> {
        if index >= self.entries.len() {
            return None;
        }
        self.current = Some(index);
        self.entries.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn here() -> Location {
        Location::Site { origin: "https://a.example".into(), secure: true }
    }

    fn there() -> Location {
        Location::Site { origin: "https://b.example".into(), secure: true }
    }

    /// REQ-BRW-003.
    #[test]
    fn back_forward_and_pruning() {
        let mut history = Navigation::new();
        assert!(!history.can_go_back());
        history.visit(here(), 0x100, "Index");
        history.visit(here(), 0x140, "News");
        history.visit(there(), 0x300, "Weather");
        assert_eq!(history.back().map(|e| e.page), Some(0x140));
        assert_eq!(history.back().map(|e| e.page), Some(0x100));
        assert_eq!(history.back(), None);
        assert!(history.can_go_forward());
        assert_eq!(history.forward().map(|e| e.page), Some(0x140));
        // A new page after going back drops what was ahead.
        history.visit(here(), 0x150, "Sport");
        assert!(!history.can_go_forward());
        let (entries, current) = history.entries();
        let pages: Vec<u16> = entries.iter().map(|entry| entry.page).collect();
        assert_eq!(pages, [0x100, 0x140, 0x150]);
        assert_eq!(current, Some(2));
    }

    #[test]
    fn the_same_page_again_is_not_a_new_entry() {
        let mut history = Navigation::new();
        history.visit(here(), 0x100, "");
        history.visit(here(), 0x100, "Index\u{1b}[31m  page");
        let (entries, _) = history.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Index [31m page", "control characters never reach a title");
    }

    /// REQ-BRW-003: at most `MAX_ENTRIES`, the oldest dropped first.
    #[test]
    fn history_is_bounded() {
        let mut history = Navigation::new();
        for page in 0..(MAX_ENTRIES as u16 + 20) {
            history.visit(here(), 0x100 + page % 0x700, "");
        }
        assert_eq!(history.entries().0.len(), MAX_ENTRIES);
        assert!(history.can_go_back());
    }
}
