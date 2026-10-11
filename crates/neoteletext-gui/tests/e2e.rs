//! The browser end to end against the local sites of
//! `tests/web-server/sites.py`: real tabs, real fetcher and decoder
//! processes confined as they are in use, and real HTTP. The window is
//! not opened; the tests drive the tabs it drives.
#![allow(missing_docs, clippy::unwrap_used, clippy::panic)]

use neoteletext_app::pipe::UserInput;
use neoteletext_browser::location::parse_input;
use neoteletext_browser::tuning::Confirmed;
use neoteletext_browser::{Location, Why};
use neoteletext_gui::helpers::{Options, Wake};
use neoteletext_gui::picture;
use neoteletext_gui::tab::{Tab, View};
mod common;

use common::sites;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long a page may take to arrive.
const PATIENCE: Duration = Duration::from_secs(20);
/// How often a waiting test looks again.
const POLL: Duration = Duration::from_millis(20);

struct Browser {
    tab: Tab,
    confirmed: Confirmed,
    options: Options,
    wake: Wake,
}

impl Browser {
    fn new() -> Self {
        Self {
            tab: Tab::new(1),
            confirmed: Confirmed::default(),
            options: Options {
                program: PathBuf::from(env!("CARGO_BIN_EXE_neoteletext")),
                allow_local: vec!["127.0.0.1".parse().unwrap()],
                no_sandbox: false,
            },
            wake: Arc::new(|| {}),
        }
    }

    fn type_address(&mut self, address: &str) {
        let destination = parse_input(address).unwrap();
        self.tab.navigate(destination, Why::Typed, &self.confirmed, &self.options, &self.wake);
    }

    fn confirm(&mut self) {
        assert!(matches!(self.tab.view, View::Tuning(_)), "expected the tuning screen, got {:?}", self.tab.view);
        self.tab.confirm(&mut self.confirmed, &self.options, &self.wake);
    }

    fn back(&mut self) {
        self.tab.back(&self.confirmed, &self.options, &self.wake);
    }

    fn forward(&mut self) {
        self.tab.forward(&self.confirmed, &self.options, &self.wake);
    }

    fn keys(&mut self, inputs: &[UserInput]) {
        for &input in inputs {
            self.tab.input(input);
        }
    }

    /// Waits until `done` holds, polling the tab.
    fn wait(&mut self, what: &str, done: impl Fn(&Tab) -> bool) {
        let start = Instant::now();
        while start.elapsed() < PATIENCE {
            self.tab.poll();
            if done(&self.tab) {
                return;
            }
            std::thread::sleep(POLL);
        }
        panic!(
            "timed out waiting for {what}: view {:?}, status {:?}, notice {:?}",
            self.tab.view, self.tab.status, self.tab.notice
        );
    }

    /// Waits for page `mpp` to be on screen with `text` on it.
    fn wait_page(&mut self, mpp: u16, text: &str) {
        self.wait(&format!("page {mpp:03X} with {text:?}"), |tab| {
            tab.view == View::Page
                && tab
                    .status
                    .as_ref()
                    .is_some_and(|status| status.found && status.page == mpp && status.text.contains(text))
        });
    }

    fn page(&self) -> u16 {
        self.tab.navigation.current().map_or(0, |entry| entry.page)
    }
}

#[test]
fn a_typed_address_is_confirmed_then_browsed_with_history() {
    let Some(sites) = sites() else { return };
    let mut browser = Browser::new();
    browser.type_address(&sites.address("news", "/"));
    // REQ: a typed address asks before anything from the site is shown.
    assert!(matches!(&browser.tab.view, View::Tuning(tuning) if tuning.why == Why::Typed));
    assert!(!browser.tab.is_live(), "nothing is fetched before the user confirms");
    browser.confirm();
    browser.wait_page(0x100, "TEST NEWS");
    assert_eq!(browser.tab.name.as_deref(), Some("Test News"));
    assert!(browser.tab.is_unencrypted(), "plain http is marked");
    // Digits choose a page of the same site, with no tuning screen.
    browser.keys(&[UserInput::Digit(2), UserInput::Digit(0), UserInput::Digit(0)]);
    browser.wait_page(0x200, "TOP STORY");
    // A Fastext link of the same site.
    browser.keys(&[UserInput::Colour(0)]);
    browser.wait_page(0x100, "TEST NEWS");
    let pages: Vec<u16> = browser.tab.navigation.entries().0.iter().map(|entry| entry.page).collect();
    assert_eq!(pages, [0x100, 0x200, 0x100]);
    browser.back();
    browser.wait_page(0x200, "TOP STORY");
    browser.back();
    browser.wait_page(0x100, "TEST NEWS");
    browser.forward();
    browser.wait_page(0x200, "TOP STORY");
    assert_eq!(browser.page(), 0x200);
    assert_eq!(browser.tab.navigation.entries().0.len(), 3, "history navigation adds no entries");
}

#[test]
fn a_link_to_another_site_goes_through_the_tuning_screen() {
    let Some(sites) = sites() else { return };
    let mut browser = Browser::new();
    browser.type_address(&sites.address("news", "/"));
    browser.confirm();
    browser.wait_page(0x100, "TEST NEWS");
    // Yellow is `weather:300`, a service the descriptor names.
    browser.keys(&[UserInput::Colour(2)]);
    browser.wait("the tuning screen for the weather site", |tab| matches!(tab.view, View::Tuning(_)));
    let View::Tuning(tuning) = browser.tab.view.clone() else { unreachable!() };
    assert_eq!(
        tuning.why,
        Why::Link { from: format!("127.0.0.1:{}", sites.ports["news"]), declared: Some("weather".into()) }
    );
    assert_eq!(tuning.destination.page, Some(0x300));
    // The old site stays on screen underneath until the user decides.
    assert!(browser.tab.is_live());
    browser.tab.cancel();
    assert_eq!(browser.tab.view, View::Page);
    browser.keys(&[UserInput::Colour(2)]);
    browser.wait("the tuning screen again", |tab| matches!(tab.view, View::Tuning(_)));
    browser.confirm();
    browser.wait_page(0x300, "WEATHER FORECAST");
    assert_eq!(browser.tab.location, Some(Location::Site { origin: sites.address("weather", ""), secure: false }));
    // Back to the confirmed news site needs no tuning screen.
    browser.back();
    browser.wait_page(0x100, "TEST NEWS");
}

#[test]
fn a_site_without_a_descriptor_is_its_front_door_and_page_files() {
    let Some(sites) = sites() else { return };
    let mut browser = Browser::new();
    browser.type_address(&sites.address("weather", "/"));
    browser.confirm();
    // Page 100 comes from /index.t42, the front door.
    browser.wait_page(0x100, "WEATHER SITE");
    // 300 was in the front door too; 200 is fetched from /200.t42.
    browser.keys(&[UserInput::Digit(2), UserInput::Digit(0), UserInput::Digit(0)]);
    browser.wait_page(0x200, "RAIN RADAR");
    // A page file's address shows that page.
    let mut direct = Browser::new();
    direct.type_address(&sites.address("weather", "/300.t42"));
    direct.confirm();
    direct.wait_page(0x300, "WEATHER FORECAST");
}

#[test]
fn a_redirect_to_another_site_is_confirmed_first() {
    let Some(sites) = sites() else { return };
    let mut browser = Browser::new();
    browser.type_address(&sites.address("redirect", "/"));
    browser.confirm();
    browser.wait(
        "the tuning screen for the redirect",
        |tab| matches!(&tab.view, View::Tuning(tuning) if matches!(tuning.why, Why::Redirect { .. })),
    );
    browser.confirm();
    browser.wait_page(0x100, "WEATHER SITE");
}

#[test]
fn a_form_is_sent_only_after_the_user_confirms() {
    let Some(sites) = sites() else { return };
    let mut browser = Browser::new();
    browser.type_address(&sites.address("news", "/300.t42"));
    browser.confirm();
    browser.wait_page(0x300, "CONTACT US");
    browser.wait("the form to take the keys", |tab| tab.status.as_ref().is_some_and(|status| status.form));
    for character in "Ann".chars() {
        browser.keys(&[UserInput::Char(character)]);
    }
    // Down to the button, then Enter presses it.
    browser.keys(&[UserInput::Tab, UserInput::Enter]);
    browser.wait("the confirmation of the submission", |tab| tab.submission.is_some());
    let submission = browser.tab.submission.clone().unwrap();
    assert_eq!(submission.host, "127.0.0.1");
    assert!(submission.plain_http, "the warning of REQ-SEC-008");
    browser.tab.answer_submission(true);
    browser.wait_page(0x301, "THANK YOU");
}

#[test]
fn a_hostile_page_cannot_forge_row_0_or_the_tuning_screen() {
    let Some(sites) = sites() else { return };
    let mut browser = Browser::new();
    browser.type_address(&sites.address("hostile", "/"));
    browser.confirm();
    browser.wait_page(0x100, "bank.example");
    assert_eq!(browser.tab.view, View::Page, "a page that draws a tuning screen is still just a page");
    let sets = neoteletext_gui::GLYPHS[0];
    let shown = picture::for_tab(&browser.tab, &[], Some("12:00:00"), sets);
    let row0 = picture::tab_row0(&browser.tab, Some("12:00:00"));
    let header = String::from_utf8_lossy(&row0).into_owned();
    assert!(header.contains(&format!("127.0.0.1:{}", sites.ports["hostile"])), "row 0 names the real site: {header}");
    assert!(!header.contains("bank"), "the page's own header never reaches row 0: {header}");
    let mut page = [neoteletext_browser::rows::blank(); 25];
    page[0] = row0;
    let honest = picture::receiver(&page, sets);
    let row_bytes = shown.row_height() * shown.width * 4;
    assert_eq!(&shown.rgba[..row_bytes], &honest.rgba[..row_bytes]);
    // A page that never finishes does not stop the tab.
    browser.keys(&[UserInput::Digit(2), UserInput::Digit(0), UserInput::Digit(0)]);
    std::thread::sleep(Duration::from_secs(1));
    browser.tab.poll();
    browser.keys(&[UserInput::Digit(1), UserInput::Digit(0), UserInput::Digit(0)]);
    browser.wait_page(0x100, "bank.example");
}

#[test]
fn files_open_without_a_fetcher() {
    let mut browser = Browser::new();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/pages/testcard.tti");
    let path = path.canonicalize().unwrap();
    browser.type_address(path.to_str().unwrap());
    browser.confirm();
    browser.wait("the test card", |tab| tab.status.as_ref().is_some_and(|status| status.found));
}

/// The helpers' confinement on macOS: a decoder can read no files, reach
/// no network and start no program; the fetcher can reach the network and
/// nothing else.
#[cfg(target_os = "macos")]
#[test]
fn helpers_are_confined() {
    use neoteletext_app::sandbox::{Helper, helper_command};
    use std::process::Command;
    let Some(sites) = sites() else { return };
    let program = PathBuf::from(env!("CARGO_BIN_EXE_neoteletext"));
    let address = format!("127.0.0.1:{}", sites.ports["news"]);
    let run = |helper| {
        let (mut command, by_parent) = helper_command(&program, helper).unwrap();
        assert!(by_parent);
        let output = command.arg("sandbox-test").arg(&address).output().unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };
    assert_eq!(run(Helper::Decoder), "file=false network=false program=false");
    assert_eq!(run(Helper::Fetcher), "file=false network=true program=false");
    // Unconfined, the same test can do all three: the checks are real.
    let output = Command::new(&program).arg("sandbox-test").arg(&address).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "file=true network=true program=true");
}

/// Opens the news site and waits for page `mpp` with `text` on it.
fn news_at(sites: &common::Sites, mpp: u16, text: &str) -> Browser {
    let mut browser = Browser::new();
    browser.type_address(&sites.address("news", &format!("/{mpp:03X}.t42")));
    browser.confirm();
    browser.wait_page(mpp, text);
    browser
}

fn tuning(browser: &Browser) -> neoteletext_browser::Tuning {
    match &browser.tab.view {
        View::Tuning(tuning) => tuning.clone(),
        other => panic!("expected the tuning screen, got {other:?}"),
    }
}

/// `docs/web-native-concept.md` sections 3 and 5: a Fastext key, a page
/// number and a run of text can each lead to an address, always through
/// the tuning screen, which says the site did not declare it.
#[test]
fn links_to_addresses_go_through_the_tuning_screen() {
    let Some(sites) = sites() else { return };
    let weather = sites.address("weather", "");
    let mut browser = news_at(&sites, 0x100, "TEST NEWS");
    // Cyan is an address in an LN line.
    browser.keys(&[UserInput::Colour(3)]);
    browser.wait("the tuning screen for cyan", |tab| matches!(tab.view, View::Tuning(_)));
    let screen = tuning(&browser);
    assert_eq!(screen.destination.location, Location::Site { origin: weather.clone(), secure: false });
    assert_eq!(screen.destination.page, Some(0x200));
    assert!(matches!(screen.why, Why::Link { declared: None, .. }), "an address in a page is not declared");
    browser.confirm();
    browser.wait_page(0x200, "RAIN RADAR");

    // On news page 200, 400 is linked by LK to the weather site's 300.
    let mut browser = news_at(&sites, 0x200, "TOP STORY");
    browser.keys(&[UserInput::Digit(4), UserInput::Digit(0), UserInput::Digit(0)]);
    browser.wait("the tuning screen for 400", |tab| matches!(tab.view, View::Tuning(_)));
    assert_eq!(tuning(&browser).destination.page, Some(0x300));
    browser.tab.cancel();
    // The page shown is still news 200: the number did not move the tab.
    browser.wait_page(0x200, "TOP STORY");

    // An LT run of text: row 8, "Somewhere".
    browser.tab.click(8, 3);
    browser.wait("the tuning screen for the text link", |tab| matches!(tab.view, View::Tuning(_)));
    assert_eq!(
        tuning(&browser).destination.location,
        Location::Site { origin: sites.address("hostile", ""), secure: false }
    );
}

/// Concept section 5 item 4: the descriptor's aliases send a number to
/// another site on every page.
#[test]
fn aliases_lead_numbers_to_other_sites() {
    let Some(sites) = sites() else { return };
    let mut browser = news_at(&sites, 0x100, "TEST NEWS");
    browser.keys(&[UserInput::Digit(5), UserInput::Digit(0), UserInput::Digit(0)]);
    browser.wait("the tuning screen for alias 500", |tab| matches!(tab.view, View::Tuning(_)));
    let screen = tuning(&browser);
    assert_eq!(screen.destination.page, Some(0x200));
    assert!(matches!(&screen.why, Why::Link { declared: Some(name), .. } if name == "weather"), "{:?}", screen.why);
    browser.tab.cancel();
    browser.wait_page(0x100, "TEST NEWS");
    browser.keys(&[UserInput::Digit(6), UserInput::Digit(0), UserInput::Digit(0)]);
    browser.wait("the tuning screen for alias 600", |tab| matches!(tab.view, View::Tuning(_)));
    assert_eq!(tuning(&browser).destination.page, Some(0x300));
    browser.confirm();
    browser.wait_page(0x300, "WEATHER FORECAST");
}

/// Concept decision 11: Enter turns page catching on, the arrows move
/// between the page's links, row 0 says where the selected one leads, and
/// Enter follows it.
#[test]
fn page_catching_selects_and_follows_links() {
    let Some(sites) = sites() else { return };
    let mut browser = news_at(&sites, 0x100, "TEST NEWS");
    let link = |tab: &Tab| tab.status.as_ref().map(|status| status.link.clone()).unwrap_or_default();
    browser.keys(&[UserInput::Enter]);
    browser.wait("the first link selected", |tab| link(tab) == "P200");
    let row0 = picture::tab_row0(&browser.tab, None);
    assert_eq!(&row0[1..5], b"LINK", "row 0 says page catching is on");
    browser.keys(&[UserInput::NextSubPage]);
    browser.wait("the second link selected", |tab| link(tab) == "P300");
    browser.keys(&[UserInput::Escape]);
    browser.wait("catching off", |tab| link(tab).is_empty());
    browser.keys(&[UserInput::Enter, UserInput::NextSubPage, UserInput::Enter]);
    browser.wait_page(0x300, "CONTACT US");

    // A caught link to an address goes through the tuning screen.
    let mut browser = news_at(&sites, 0x200, "TOP STORY");
    browser.keys(&[UserInput::Enter]);
    browser.wait("the first link", |tab| link(tab) == "P100");
    browser.keys(&[UserInput::NextSubPage]);
    let weather = sites.address("weather", "/300.t42");
    browser.wait("the linked number 400", |tab| link(tab) == weather);
    browser.keys(&[UserInput::Enter]);
    browser.wait("the tuning screen", |tab| matches!(tab.view, View::Tuning(_)));
}
