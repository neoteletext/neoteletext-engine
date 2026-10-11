//! One tab: its helpers, its history, what it shows, and the routing of
//! every frame between its fetcher and its decoder
//! (`docs/desktop-browser.md` sections 3.2 and 5): links to other sites
//! (REQ-NAV-013), aliases (REQ-WEB-020), files (REQ-BRW-005) and the
//! routing rules of REQ-SEC-026.
//!
//! The tab is the trusted side. It reads what the helpers send, but trusts
//! none of it: a decoder can ask for pages, submit a form, or ask to
//! follow a link, and each of those is checked here. Confirmations come
//! only from the user, through the interface, never from a helper; a link
//! or redirect to another site always goes through the tuning screen.
//!
//! Nothing here draws or knows about windows, so the end-to-end tests
//! drive tabs directly.

use crate::helpers::{Helpers, Message, Options, Wake};
use neoteletext_app::pipe::{self, DisplaySettings, Frame, Notice, Origin, PageData, PageStatus, Screen, UserInput};
use neoteletext_browser::location::parse_input;
use neoteletext_browser::navigation::clean_title;
use neoteletext_browser::tuning::{self, Confirmed};
use neoteletext_browser::{Destination, Entry, Location, Navigation, Tuning, Why};
use neoteletext_core::consts::PACKET_BYTES;

/// The largest file opened: a long broadcast capture.
const MAX_FILE: u64 = 256 * 1024 * 1024;
/// Lines of a page's text searched for its title, after row 0.
const TITLE_LINES: usize = 4;
/// The sub-code that means "whichever sub-page".
const ANY_SUBCODE: u16 = u16::MAX;
/// The extension of a T42 file.
const T42_EXTENSION: &str = ".t42";

/// What a tab shows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum View {
    /// The start page of a new tab.
    Start,
    /// The keys page.
    Keys,
    /// The tuning screen, waiting for the user.
    Tuning(Tuning),
    /// The decoder's pages.
    Page,
    /// Something went wrong: a title and the details.
    Failed(String, String),
}

/// A form submission waiting for the user's confirmation (REQ-SEC-008,
/// REQ-SEC-009), as the fetcher reported it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PendingSubmission {
    /// The host the values go to.
    pub host: String,
    /// They would travel unencrypted.
    pub plain_http: bool,
    /// The first submission to that origin this session.
    pub first: bool,
}

/// A tab.
pub struct Tab {
    /// Its number, for the interface.
    pub id: u64,
    /// Its history.
    pub navigation: Navigation,
    /// Where its pages come from now.
    pub location: Option<Location>,
    /// What it shows.
    pub view: View,
    helpers: Option<Helpers>,
    /// The decoder's latest screen.
    pub screen: Option<Screen>,
    /// The decoder's latest status.
    pub status: Option<PageStatus>,
    /// The service's name, from its descriptor or its host.
    pub name: Option<String>,
    linked: Vec<(String, String)>,
    aliases: Vec<(u16, String)>,
    /// Some page of this site came over plain HTTP.
    pub unencrypted: bool,
    /// The latest message for the status line.
    pub notice: Option<String>,
    /// A submission waiting for confirmation.
    pub submission: Option<PendingSubmission>,
    /// The page history led to, which is not a new visit when it shows.
    history_target: Option<u16>,
    /// The history entry to return to if the tuning screen is cancelled.
    history_revert: Option<usize>,
    /// Bumped whenever anything shown changes.
    pub version: u64,
    settings: DisplaySettings,
}

/// The title of a page: the first line of its text after row 0 that has
/// letters in it.
#[must_use]
pub fn page_title(text: &str) -> String {
    text.lines()
        .skip(1)
        .take(TITLE_LINES)
        .map(str::trim)
        .find(|line| line.chars().any(char::is_alphabetic))
        .map(clean_title)
        .unwrap_or_default()
}

impl Tab {
    /// A new tab at its start page.
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self {
            id,
            navigation: Navigation::new(),
            location: None,
            view: View::Start,
            helpers: None,
            screen: None,
            status: None,
            name: None,
            linked: Vec::new(),
            aliases: Vec::new(),
            unencrypted: false,
            notice: None,
            submission: None,
            history_target: None,
            history_revert: None,
            version: 0,
            settings: DisplaySettings::default(),
        }
    }

    /// The tab's title for the tab strip.
    #[must_use]
    pub fn title(&self) -> String {
        match &self.view {
            View::Start => "New tab".to_string(),
            View::Keys => "Keys".to_string(),
            View::Tuning(tuning) => format!("{}?", tuning.label()),
            View::Failed(title, _) => title.clone(),
            View::Page => {
                let label = self.location.as_ref().map(Location::label).unwrap_or_default();
                match self.navigation.current() {
                    Some(entry) if !entry.title.is_empty() => format!("{} {:03X}", entry.title, entry.page),
                    Some(entry) => format!("{label} {:03X}", entry.page),
                    None => label,
                }
            }
        }
    }

    /// The pages shown came over an unencrypted connection.
    #[must_use]
    pub fn is_unencrypted(&self) -> bool {
        self.location.as_ref().is_some_and(|location| !location.is_secure()) || self.unencrypted
    }

    /// The tab has helpers running.
    #[must_use]
    pub const fn is_live(&self) -> bool {
        self.helpers.is_some()
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    fn send_decoder(&mut self, frame: &Frame) {
        if let Some(helpers) = &mut self.helpers {
            // A decoder that cannot be written to has ended; its reader
            // reports that.
            let _ = helpers.send_decoder(frame);
        }
    }

    fn send_fetcher(&mut self, frame: &Frame) {
        if let Some(helpers) = &mut self.helpers {
            let _ = helpers.send_fetcher(frame);
        }
    }

    fn same_site(&self, location: &Location) -> bool {
        self.helpers.is_some() && self.location.as_ref().is_some_and(|here| here.site_key() == location.site_key())
    }

    /// Goes to a destination: on this site directly, elsewhere through the
    /// tuning screen unless the rules of the concept say otherwise.
    pub fn navigate(
        &mut self,
        destination: Destination,
        why: Why,
        confirmed: &Confirmed,
        options: &Options,
        wake: &Wake,
    ) {
        if self.same_site(&destination.location) {
            self.view = View::Page;
            if let Some(page) = destination.page {
                self.send_decoder(&Frame::Show(page, ANY_SUBCODE));
            }
            self.changed();
            return;
        }
        if tuning::needed(self.location.as_ref(), &destination.location, &why, confirmed) {
            self.view = View::Tuning(Tuning { destination, why });
            self.changed();
            return;
        }
        self.open(&destination, options, wake);
    }

    /// The user confirmed the tuning screen: the site is confirmed for the
    /// session, and opened.
    pub fn confirm(&mut self, confirmed: &mut Confirmed, options: &Options, wake: &Wake) {
        let View::Tuning(tuning) = &self.view else { return };
        let destination = tuning.destination.clone();
        confirmed.confirm(&destination.location);
        self.history_revert = None;
        self.open(&destination, options, wake);
    }

    /// The user cancelled the tuning screen: back to what was shown.
    pub fn cancel(&mut self) {
        if !matches!(self.view, View::Tuning(_) | View::Keys) {
            return;
        }
        if let Some(index) = self.history_revert.take() {
            self.history_target = None;
            let _ = self.navigation.go_to(index);
        }
        self.view = if self.helpers.is_some() { View::Page } else { View::Start };
        self.changed();
    }

    /// Shows the keys page, or leaves it.
    pub fn toggle_keys(&mut self) {
        self.view = match self.view {
            View::Keys if self.helpers.is_some() => View::Page,
            View::Keys => View::Start,
            _ => View::Keys,
        };
        self.changed();
    }

    /// Starts the helpers for a destination and shows its page.
    pub fn open(&mut self, destination: &Destination, options: &Options, wake: &Wake) {
        self.helpers = None;
        self.screen = None;
        self.status = None;
        self.name = None;
        self.linked.clear();
        self.aliases.clear();
        self.unencrypted = false;
        self.submission = None;
        self.notice = None;
        self.location = Some(destination.location.clone());
        match Helpers::start(options, &destination.location, wake) {
            Ok(helpers) => {
                self.helpers = Some(helpers);
                self.view = View::Page;
                self.send_decoder(&Frame::Settings(self.settings));
                if let Some(page) = destination.page {
                    self.send_decoder(&Frame::Show(page, ANY_SUBCODE));
                }
                if let Location::File { path } = &destination.location {
                    let path = path.clone();
                    if let Err(problem) = self.feed_file(&path) {
                        self.view = View::Failed("Cannot open the file".to_string(), problem);
                    }
                }
            }
            Err(problem) => self.view = View::Failed("Cannot start".to_string(), problem),
        }
        self.changed();
    }

    /// Reads a file and gives its pages to the decoder: T42 as packets,
    /// anything else as TTI. Only the bytes are read here; the decoder
    /// parses them.
    fn feed_file(&mut self, path: &str) -> Result<(), String> {
        let metadata = std::fs::metadata(path).map_err(|error| format!("{path}: {error}"))?;
        if metadata.len() > MAX_FILE {
            return Err(format!("{path}: larger than {MAX_FILE} bytes"));
        }
        let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
        let t42 = path.to_ascii_lowercase().ends_with(T42_EXTENSION);
        if t42 {
            if bytes.len() % PACKET_BYTES != 0 {
                return Err(format!("{path}: not whole 42-byte packets"));
            }
            for packet in bytes.as_chunks::<PACKET_BYTES>().0 {
                self.send_decoder(&Frame::Packet(*packet));
            }
        } else {
            if bytes.len() > pipe::MAX_FRAME {
                return Err(format!("{path}: a TTI file larger than {} bytes", pipe::MAX_FRAME));
            }
            let page = PageData { secure: true, origin: Origin::FrontDoor, url: format!("file://{path}"), bytes };
            self.send_decoder(&Frame::Tti(page));
        }
        Ok(())
    }

    /// A key or character for the page.
    pub fn input(&mut self, input: UserInput) {
        if self.view == View::Page {
            self.send_decoder(&Frame::Input(input));
        }
    }

    /// The pointer pressed a cell of the page.
    pub fn click(&mut self, row: u8, column: u8) {
        if self.view == View::Page {
            self.send_decoder(&Frame::Click(row, column));
        }
    }

    /// The display settings changed.
    pub fn set_settings(&mut self, settings: DisplaySettings) {
        self.settings = settings;
        self.send_decoder(&Frame::Settings(settings));
    }

    /// Asks again for the page shown.
    pub fn reload(&mut self) {
        if let Some(status) = self.status.clone() {
            self.history_target = Some(status.page);
            self.send_decoder(&Frame::Show(status.page, ANY_SUBCODE));
        }
    }

    /// Back through the history.
    pub fn back(&mut self, confirmed: &Confirmed, options: &Options, wake: &Wake) {
        let previous = self.navigation.entries().1;
        if let Some(entry) = self.navigation.back().cloned() {
            self.go_history(entry, previous, confirmed, options, wake);
        }
    }

    /// Forward through the history.
    pub fn forward(&mut self, confirmed: &Confirmed, options: &Options, wake: &Wake) {
        let previous = self.navigation.entries().1;
        if let Some(entry) = self.navigation.forward().cloned() {
            self.go_history(entry, previous, confirmed, options, wake);
        }
    }

    fn go_history(
        &mut self,
        entry: Entry,
        previous: Option<usize>,
        confirmed: &Confirmed,
        options: &Options,
        wake: &Wake,
    ) {
        self.history_target = Some(entry.page);
        let destination = Destination { location: entry.location, page: Some(entry.page) };
        if self.same_site(&destination.location) {
            self.view = View::Page;
            self.send_decoder(&Frame::Show(entry.page, ANY_SUBCODE));
            self.changed();
            return;
        }
        if tuning::needed(self.location.as_ref(), &destination.location, &Why::History, confirmed) {
            self.history_revert = previous;
            self.view = View::Tuning(Tuning { destination, why: Why::History });
            self.changed();
            return;
        }
        self.open(&destination, options, wake);
    }

    /// The user answered a form confirmation.
    pub fn answer_submission(&mut self, send: bool) {
        if self.submission.take().is_some() {
            self.send_fetcher(&Frame::Confirm(send));
            self.notice = Some(if send { "Sending..." } else { "Not sent." }.to_string());
            self.changed();
        }
    }

    /// Takes in what the helpers sent. Returns true when something shown
    /// changed.
    pub fn poll(&mut self) -> bool {
        let Some(helpers) = &self.helpers else { return false };
        let messages: Vec<Message> = helpers.messages.try_iter().collect();
        let before = self.version;
        for message in messages {
            match message {
                Message::Fetcher(frame) => self.route_fetcher(frame),
                Message::Decoder(frame) => self.route_decoder(frame),
                Message::FetcherEnded => {
                    self.notice = Some("The connection process stopped.".to_string());
                    self.changed();
                }
                Message::DecoderEnded => {
                    self.helpers = None;
                    self.view = View::Failed(
                        "The page stopped".to_string(),
                        "The process decoding this site's pages stopped. Reload to try again.".to_string(),
                    );
                    self.changed();
                    return true;
                }
            }
        }
        self.version != before
    }

    fn route_fetcher(&mut self, frame: Frame) {
        match frame {
            Frame::Notice(Notice::Confirm(host, plain_http, first)) => {
                self.submission = Some(PendingSubmission { host: clean_title(&host), plain_http, first });
                self.changed();
            }
            Frame::Notice(Notice::LinkedSites(sites)) => self.linked = sites,
            Frame::Notice(Notice::Aliases(aliases)) => self.aliases = aliases,
            Frame::Notice(Notice::ServiceName(name)) => {
                self.name = Some(clean_title(&name));
                self.changed();
            }
            Frame::Notice(Notice::RedirectToOtherSite(address)) => {
                let from = self.location.as_ref().map(Location::label).unwrap_or_default();
                match parse_input(&address) {
                    Ok(destination) => {
                        self.view = View::Tuning(Tuning { destination, why: Why::Redirect { from } });
                    }
                    Err(_) => self.notice = Some("The site redirected to an address that is not allowed.".into()),
                }
                self.changed();
            }
            Frame::Notice(Notice::Message(text)) => {
                self.notice = Some(clean_title(&text));
                self.changed();
            }
            Frame::Notice(Notice::Unencrypted) => {
                self.unencrypted = true;
                self.send_decoder(&Frame::Notice(Notice::Unencrypted));
            }
            frame @ (Frame::Tti(_) | Frame::T42(_)) => {
                if let Frame::Tti(page) | Frame::T42(page) = &frame
                    && !page.secure
                    && page.origin != Origin::Push
                {
                    self.unencrypted = true;
                }
                self.send_decoder(&frame);
            }
            frame @ (Frame::Packet(_) | Frame::Notice(_)) => self.send_decoder(&frame),
            // A fetcher never sends anything else; it is not passed on.
            _ => {}
        }
    }

    fn route_decoder(&mut self, frame: Frame) {
        match frame {
            Frame::Screen(screen) => {
                self.screen = Some(screen);
                self.changed();
            }
            Frame::Status(status) => {
                if status.found
                    && let Some(location) = self.location.clone()
                {
                    let title = page_title(&status.text);
                    if self.history_target == Some(status.page) {
                        self.history_target = None;
                    } else {
                        self.navigation.visit(location, status.page, &title);
                    }
                }
                self.status = Some(status);
                self.changed();
            }
            Frame::Request(mpp, sub) => {
                // The descriptor's alias for a number leads elsewhere on
                // every page of the site (concept section 5 item 4); the
                // decoder has already preferred the page's own link.
                match self.aliases.iter().find(|(page, _)| *page == mpp).map(|(_, target)| target.clone()) {
                    Some(target) => self.alias(&target),
                    None => self.send_fetcher(&Frame::Request(mpp, sub)),
                }
            }
            frame @ Frame::Submit(_) => self.send_fetcher(&frame),
            Frame::Follow(service, mpp, _) => self.follow(&service, mpp),
            Frame::Visit(address) => self.visit(&address),
            // A decoder never confirms anything, and never sends the rest.
            _ => {}
        }
    }

    /// An address written in a page (concept section 3): through the
    /// tuning screen, marked as not declared by the site.
    fn visit(&mut self, address: &str) {
        let from = self.location.as_ref().map(Location::label).unwrap_or_default();
        match parse_input(address) {
            Ok(destination)
                if self.location.as_ref().is_some_and(|here| here.site_key() == destination.location.site_key()) =>
            {
                if let Some(page) = destination.page {
                    self.send_decoder(&Frame::Show(page, ANY_SUBCODE));
                }
            }
            Ok(destination) => {
                self.view = View::Tuning(Tuning { destination, why: Why::Link { from, declared: None } });
            }
            Err(_) => self.notice = Some("That link's address is not allowed.".to_string()),
        }
        self.changed();
    }

    /// A number the descriptor aliases: an address, or `name:mpp`. The
    /// decoder was searching for the number; it goes back to the page
    /// shown while the tuning screen waits.
    fn alias(&mut self, target: &str) {
        if let Some(page) = self.navigation.current().map(|entry| entry.page) {
            self.send_decoder(&Frame::Show(page, ANY_SUBCODE));
            self.history_target = Some(page);
        }
        if target.contains("://") {
            let from = self.location.as_ref().map(Location::label).unwrap_or_default();
            match parse_input(target) {
                Ok(destination) => {
                    let declared = Some(destination.location.label());
                    self.view = View::Tuning(Tuning { destination, why: Why::Link { from, declared } });
                }
                Err(_) => self.notice = Some("The site's alias names an address that is not allowed.".into()),
            }
            self.changed();
            return;
        }
        let named = target.split_once(':').and_then(|(service, page)| {
            neoteletext_browser::location::page_number(page).map(|page| (service.to_string(), page))
        });
        if let Some((service, page)) = named {
            self.follow(&service, page);
        } else {
            self.notice = Some("The site's alias is not a link.".to_string());
            self.changed();
        }
    }

    /// A page's link to a service its descriptor names: through the tuning
    /// screen, naming the service as the site declares it.
    fn follow(&mut self, service: &str, mpp: u16) {
        let Some((name, address)) = self.linked.iter().find(|(name, _)| name == service).cloned() else {
            self.notice = Some("That link leads to a service this site does not name.".to_string());
            self.changed();
            return;
        };
        let from = self.location.as_ref().map(Location::label).unwrap_or_default();
        match parse_input(&address) {
            Ok(mut destination) => {
                destination.page = Some(mpp);
                self.view = View::Tuning(Tuning { destination, why: Why::Link { from, declared: Some(name) } });
            }
            Err(_) => self.notice = Some("That link's address is not allowed.".to_string()),
        }
        self.changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_come_from_the_first_line_with_words() {
        let text = "P100 example.org 12:00\n\n  NEOTELETEXT TEST SERVICE \nWeather 200\n";
        assert_eq!(page_title(text), "NEOTELETEXT TEST SERVICE");
        assert_eq!(page_title("P100\n123\n"), "");
    }
}
