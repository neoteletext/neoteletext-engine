//! Leaving a site: the tuning screen (REQ-NAV-014, `docs/web-native-concept.md`
//! section 6, `docs/desktop-browser.md` section 8).
//!
//! Before anything from another site is shown, the tab shows this screen
//! in its page area, with `GO?` and the destination in row 0, and waits
//! for the user. Only going back or forward to a site confirmed earlier in
//! the session skips it.

use std::collections::HashSet;

use crate::location::{Destination, Location};
use crate::rows::{ADDRESS_ROWS, ALPHA_CYAN, ALPHA_GREEN, Activity, Page, blank, keep_right, line, row0, titled, wrap};
use neoteletext_core::consts::{ALPHA_RED, ALPHA_WHITE, ALPHA_YELLOW, COLUMNS, DOUBLE_HEIGHT};

/// Columns for the site's name in double height, after its attributes.
const NAME_COLUMNS: usize = COLUMNS - 2;
/// Rows of the screen's layout.
const NAME_ROW: usize = 4;
const ADDRESS_LABEL_ROW: usize = 7;
const ADDRESS_FIRST_ROW: usize = 8;
const SECURITY_ROW: usize = 13;
const WHY_ROW: usize = 16;
const DECLARED_ROW: usize = 17;
const CONFIRM_ROW: usize = 19;
const CANCEL_ROW: usize = 20;

/// How the tab came to be leaving its site.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Why {
    /// The user typed the address.
    Typed,
    /// The user chose a bookmark or a history entry.
    Chosen,
    /// A tab restored from the last session.
    Restored,
    /// A link on a page of `from`. `declared` is the site's own name for
    /// the destination when its descriptor declares it; an address written
    /// in a page is not declared.
    Link {
        /// The site the link is on.
        from: String,
        /// The name the site's descriptor gives the destination.
        declared: Option<String>,
    },
    /// `from` redirected the request here.
    Redirect {
        /// The site that redirected.
        from: String,
    },
    /// Going back or forward to a site not yet confirmed this session.
    History,
}

/// A tab waiting at the tuning screen.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tuning {
    /// Where to.
    pub destination: Destination,
    /// Why.
    pub why: Why,
}

/// The sites confirmed this session (concept decision 4), by site key.
/// They are not kept between runs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Confirmed(HashSet<String>);

impl Confirmed {
    /// Remembers a confirmed site.
    pub fn confirm(&mut self, location: &Location) {
        self.0.insert(location.site_key());
    }

    /// The site was confirmed this session.
    #[must_use]
    pub fn contains(&self, location: &Location) -> bool {
        self.0.contains(&location.site_key())
    }
}

/// Whether going to `destination` from `current` needs the tuning screen.
/// Staying on the same site never does; going back or forward to a site
/// confirmed this session does not; everything else does.
#[must_use]
pub fn needed(current: Option<&Location>, destination: &Location, why: &Why, confirmed: &Confirmed) -> bool {
    if current.is_some_and(|current| current.site_key() == destination.site_key()) {
        return false;
    }
    !(*why == Why::History && confirmed.contains(destination))
}

impl Tuning {
    /// The name row 0 shows while the screen waits.
    #[must_use]
    pub fn label(&self) -> String {
        self.destination.location.label()
    }

    /// The whole screen: row 0 announcing it, and rows 1 to 24.
    #[must_use]
    pub fn page(&self, clock: Option<&str>) -> Page {
        let location = &self.destination.location;
        let mut page = titled("Going to another site", ALPHA_YELLOW);
        page[0] = row0(Activity::Tuning, &self.label(), clock);
        let name = String::from_utf8_lossy(&keep_right(&location.label(), NAME_COLUMNS)).into_owned();
        page[NAME_ROW] = line(&[ALPHA_CYAN, DOUBLE_HEIGHT], &name);
        page[ADDRESS_LABEL_ROW] = line(&[ALPHA_WHITE], "The full address:");
        wrap(&mut page, ADDRESS_FIRST_ROW, ADDRESS_ROWS, ALPHA_WHITE, &location.address(self.destination.page));
        page[SECURITY_ROW] = match location {
            Location::File { .. } => line(&[ALPHA_GREEN], "A file on this computer."),
            Location::Live { .. } => line(&[ALPHA_GREEN], "A live feed, encrypted."),
            _ if location.is_secure() => line(&[ALPHA_GREEN], "Encrypted (https)."),
            _ => line(&[ALPHA_RED], "NOT ENCRYPTED: plain http. Others can"),
        };
        if !location.is_secure() {
            page[SECURITY_ROW + 1] = line(&[ALPHA_RED], "read and change what is sent.");
        }
        let (why, declared) = match &self.why {
            Why::Typed => ("You typed this address.".to_string(), None),
            Why::Chosen => ("You chose this from your list.".to_string(), None),
            Why::Restored => ("This tab was open last time.".to_string(), None),
            Why::History => ("Going back to this site.".to_string(), None),
            Why::Redirect { from } => (format!("{from} sent you here."), None),
            Why::Link { from, declared: Some(name) } => {
                (format!("A link on {from}."), Some(format!("{from} calls it: {name}")))
            }
            Why::Link { from, declared: None } => {
                (format!("A link on {from}."), Some("Written in the page, not declared.".to_string()))
            }
        };
        page[WHY_ROW] = line(&[ALPHA_WHITE], &why);
        if let Some(declared) = declared {
            page[DECLARED_ROW] = line(&[ALPHA_WHITE], &declared);
        }
        page[CONFIRM_ROW] = line(&[ALPHA_GREEN], "Enter or Go: go there.");
        page[CANCEL_ROW] = line(&[ALPHA_YELLOW], "Escape or Cancel: stay.");
        page[page.len() - 1] = blank();
        page
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::parse_input;

    fn tuning(typed: &str, why: Why) -> Tuning {
        Tuning { destination: parse_input(typed).unwrap(), why }
    }

    fn text(page: &Page) -> String {
        page.iter().map(|row| String::from_utf8_lossy(row).into_owned() + "\n").collect()
    }

    #[test]
    fn the_screen_names_the_site_and_says_how_it_is_reached() {
        let screen = tuning("http://bank.example/100.t42", Why::Link { from: "news.example".into(), declared: None });
        let page = screen.page(Some("12:00:00"));
        assert_eq!(&page[0][..4], &[ALPHA_YELLOW, b'G', b'O', b'?'], "row 0 announces the screen");
        let all = text(&page);
        assert!(all.contains("bank.example"), "{all}");
        assert!(all.contains("http://bank.example/100.t42"), "{all}");
        assert!(all.contains("NOT ENCRYPTED"), "{all}");
        assert!(all.contains("not declared"), "{all}");
        let secure = tuning("https://w.example/", Why::Link { from: "a".into(), declared: Some("Weather".into()) });
        let all = text(&secure.page(None));
        assert!(all.contains("Encrypted (https)") && all.contains("calls it: Weather"), "{all}");
    }

    #[test]
    fn only_history_to_a_confirmed_site_skips_the_screen() {
        let here = parse_input("https://a.example/").unwrap().location;
        let there = parse_input("https://b.example/").unwrap().location;
        let mut confirmed = Confirmed::default();
        assert!(!needed(Some(&here), &here, &Why::Typed, &confirmed), "same site");
        assert!(needed(Some(&here), &there, &Why::History, &confirmed));
        confirmed.confirm(&there);
        assert!(!needed(Some(&here), &there, &Why::History, &confirmed));
        for why in [Why::Typed, Why::Chosen, Why::Redirect { from: "a".into() }] {
            assert!(needed(Some(&here), &there, &why, &confirmed), "{why:?}");
        }
        assert!(needed(None, &there, &Why::Typed, &confirmed), "a new tab still confirms a typed address");
    }
}
