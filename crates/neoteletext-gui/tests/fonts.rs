//! Fonts end to end (area FNT, issue #177) against the local sites of
//! `tests/web-server/sites.py`: real tabs, real fetcher, decoder and font
//! helper processes, confined as in use, and real HTTP. The window is not
//! opened; the tests drive the tab and the font choosing it drives
//! (`neoteletext_gui::fonts::update_tab`), and read what the decoder drew.
//! They cover REQ-FNT-003 to REQ-FNT-009.
#![allow(missing_docs, clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use neoteletext_app::pipe::UserInput;
use neoteletext_browser::Why;
use neoteletext_browser::fonts::{Chooser, Scope};
use neoteletext_browser::location::parse_input;
use neoteletext_browser::saved::{Settings, UserFont};
use neoteletext_browser::tuning::Confirmed;
use neoteletext_gui::fonts::{Fonts, Library, Request, State, Work, update_tab, user_request};
use neoteletext_gui::helpers::{Options, Wake};
use neoteletext_gui::picture;
use neoteletext_gui::storage::Storage;
use neoteletext_gui::tab::{Tab, View};
use neoteletext_web::sha256::to_hex;
mod common;

use common::{Sites, sites};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long a page or a font may take to arrive.
const PATIENCE: Duration = Duration::from_secs(20);
/// How often a waiting test looks again.
const POLL: Duration = Duration::from_millis(20);
/// The stripes of the sites' pattern fonts (`sites.py`).
const CHANNEL_STRIPE: u16 = 0xF00;
const PAGE_STRIPE: u16 = 0x00F;
const LIBRARY_STRIPE: u16 = 0x0F0;
/// The pixels of a cell in the 12-by-20 raster.
const CELL_WIDTH: usize = 12;
const CELL_HEIGHT: usize = 20;

struct Run {
    tab: Tab,
    fonts: Fonts,
    settings: Settings,
    storage: Storage,
    profile: PathBuf,
    confirmed: Confirmed,
    options: Options,
    wake: Wake,
}

impl Run {
    fn new(profile: PathBuf) -> Self {
        let storage = Storage::new(Some(profile.clone()));
        Self {
            tab: Tab::new(1),
            fonts: Fonts::new(&storage),
            settings: Settings::default(),
            storage,
            profile,
            confirmed: Confirmed::default(),
            options: Options {
                program: PathBuf::from(env!("CARGO_BIN_EXE_neoteletext")),
                allow_local: vec!["127.0.0.1".parse().unwrap()],
                no_sandbox: false,
            },
            wake: Arc::new(|| {}),
        }
    }

    fn step(&mut self) {
        let now = neoteletext_gui::now_seconds();
        self.tab.poll();
        self.fonts.poll(&self.storage, &self.options, &self.wake, now);
        let user = self.settings.font.as_ref().and_then(user_request);
        let work = Work { storage: &self.storage, options: &self.options, wake: &self.wake, now };
        update_tab(&mut self.tab, &mut self.fonts, &self.settings, user.as_ref(), &work);
    }

    fn wait(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        let start = Instant::now();
        while start.elapsed() < PATIENCE {
            self.step();
            if done(self) {
                return;
            }
            std::thread::sleep(POLL);
        }
        panic!("timed out waiting for {what}: font {:?}, notice {:?}", self.tab.font_shown, self.tab.notice);
    }

    fn open(&mut self, address: &str) {
        let destination = parse_input(address).unwrap();
        self.tab.navigate(destination, Why::Typed, &self.confirmed, &self.options, &self.wake);
        assert!(matches!(self.tab.view, View::Tuning(_)));
        self.tab.confirm(&mut self.confirmed, &self.options, &self.wake);
    }

    fn page(&mut self, digits: [u8; 3], title: &str) {
        for digit in digits {
            self.tab.input(UserInput::Digit(digit));
        }
        self.wait(title, |run| {
            run.tab.status.as_ref().is_some_and(|status| status.found && status.text.contains(title))
        });
    }

    /// The dots of the first glyph line of the cell at row 2, column 0
    /// (the "A" of every styled page), as drawn by the decoder.
    fn drawn_a(&self) -> Option<u16> {
        let screen = self.tab.screen.as_ref()?;
        let width = usize::from(screen.width);
        // Glyph line 1, the second of the cell's 20 lines... any line will
        // do for a stripe; take the cell's fourth output line.
        let y = 2 * CELL_HEIGHT + 3;
        let row = screen.indices.get(y * width..(y + 1) * width)?;
        // Column 30 of the row is a space: its colour is the background.
        let background = *row.get(30 * CELL_WIDTH)?;
        Some((0..CELL_WIDTH).fold(0u16, |mask, dot| (mask << 1) | u16::from(row[dot] != background)))
    }

    fn wait_drawn(&mut self, what: &str, name: &str, chooser: Option<Chooser>, stripe: u16) {
        let name = name.to_string();
        self.wait(what, |run| {
            let shown = match (&run.tab.font_shown, chooser) {
                (Some((shown, by)), Some(chooser)) => *shown == name && *by == chooser,
                (None, None) => true,
                _ => false,
            };
            shown && run.drawn_a() == Some(stripe)
        });
    }
}

fn profile(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&path);
    path
}

/// The paths the font site was asked for, from its `/log`.
fn log(sites: &Sites) -> Vec<String> {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", sites.ports["fontsite"])).unwrap();
    stream.write_all(b"GET /log HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    let body = answer.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    body.lines().map(str::to_string).collect()
}

/// REQ-FNT-003 to REQ-FNT-009: a channel's font, a page's own font at
/// another site and at the channel's origin, a font failing its checksum,
/// fonts from other sites refused, a library listed and chosen from with
/// "always use my font", and the cache used again with no request.
#[test]
fn fonts_named_by_the_channel_the_page_the_user_and_a_library() {
    let Some(sites) = sites() else { return };
    let mut run = Run::new(profile("fonts-e2e"));
    run.open(&sites.address("styled", "/"));
    run.wait("the channel's front page", |run| {
        run.tab.status.as_ref().is_some_and(|status| status.found && status.text.contains("CHANNEL FONT"))
    });
    // REQ-FNT-006: the channel's font draws its pages.
    run.wait_drawn("the channel's font", "Channel stripe", Some(Chooser::Channel), CHANNEL_STRIPE);
    let index = std::fs::read_to_string(run.profile.join("fonts").join("index.tsv")).unwrap();
    assert!(index.contains("Channel stripe") && index.contains("channel http://127.0.0.1:"), "{index}");
    // REQ-FNT-007: a page's own font at another site, with its checksum.
    run.page([2, 0, 0], "PAGE FONT ELSEWHERE");
    run.wait_drawn("the page's font", "Page stripe", Some(Chooser::Page), PAGE_STRIPE);
    // REQ-FNT-005: row 0 is the receiver's, in Bedstead, whatever the page
    // names.
    let shown = picture::for_tab(&run.tab, &[], Some("12:00:00"));
    let mut page = [neoteletext_browser::rows::blank(); 25];
    page[0] = picture::tab_row0(&run.tab, Some("12:00:00"));
    let honest = picture::receiver(&page);
    let row_bytes = shown.row_height() * shown.width * 4;
    assert_eq!(shown.rgba[..row_bytes], honest.rgba[..row_bytes], "row 0 in Bedstead");
    // A font that fails its checksum is not used; the channel's draws.
    run.page([3, 0, 0], "WRONG CHECKSUM");
    run.wait("the page to name its font", |run| run.tab.page_font().is_some());
    let (address, sha256) = run.tab.page_font().unwrap();
    let scope = Scope::Channel(run.tab.channel_folder().unwrap());
    let request = Request { address, sha256: Some(sha256), chooser: Chooser::Page, scope };
    run.wait("the checksum to fail", |run| matches!(run.fonts.state(&request), Some(State::Failed(_))));
    let failed = run.fonts.state(&request);
    assert!(matches!(failed, Some(State::Failed(why)) if why.contains("SHA-256")), "{failed:?}");
    run.wait_drawn("the channel's font again", "Channel stripe", Some(Chooser::Channel), CHANNEL_STRIPE);
    // REQ-FNT-003: fonts from other sites refused in the settings.
    run.settings.fonts_from_other_sites = false;
    run.page([2, 0, 0], "PAGE FONT ELSEWHERE");
    run.wait_drawn("the refusal", "Channel stripe", Some(Chooser::Channel), CHANNEL_STRIPE);
    assert!(run.tab.notice.as_deref().is_some_and(|notice| notice.contains("other sites")), "{:?}", run.tab.notice);
    // A page font at the channel's own origin needs no permission.
    run.page([4, 0, 0], "PAGE FONT HERE");
    run.wait_drawn("the page's own font", "Page stripe", Some(Chooser::Page), PAGE_STRIPE);
    run.settings.fonts_from_other_sites = true;
    // REQ-FNT-004: a library is listed without fetching any of its fonts.
    let library = sites.address("fontsite", "/lib/");
    run.fonts.list_library(&library, &run.options, &run.wake);
    run.wait("the library's listing", |run| matches!(run.fonts.library(&library), Some(Library::Listed(..))));
    let Some(Library::Listed(fonts, ignored)) = run.fonts.library(&library).cloned() else { unreachable!() };
    assert_eq!(fonts.iter().map(|font| font.title.as_str()).collect::<Vec<_>>(), ["Library stripe", "Page stripe"]);
    assert_eq!(ignored, 1, "the entry that climbs out of the folder");
    assert_eq!(fonts[0].address, sites.address("fontsite", "/lib/library.bdf"));
    assert!(!log(&sites).contains(&"/lib/library.bdf".to_string()), "listing fetched no font");
    // REQ-FNT-005: the user's font from the library, with "always use my
    // font", wins over the channel and the page.
    run.settings.font = Some(UserFont {
        sha256: String::from_utf8_lossy(&to_hex(&fonts[0].sha256)).into_owned(),
        address: fonts[0].address.clone(),
        title: fonts[0].title.clone(),
        by_address: false,
    });
    run.settings.always_my_font = true;
    run.wait_drawn("the user's font", "Library stripe", Some(Chooser::User), LIBRARY_STRIPE);
    // Without the override the page's font draws again.
    run.settings.always_my_font = false;
    run.wait_drawn("the page's font over the user's", "Page stripe", Some(Chooser::Page), PAGE_STRIPE);
    // REQ-FNT-009: the next run uses the cache, with no request.
    let fetched = log(&sites).iter().filter(|path| *path == "/page.bdf").count();
    drop(run);
    let mut again = Run::new(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fonts-e2e"));
    again.open(&sites.address("styled", "/200.tti"));
    again.wait_drawn("the page's font from the cache", "Page stripe", Some(Chooser::Page), PAGE_STRIPE);
    assert_eq!(log(&sites).iter().filter(|path| *path == "/page.bdf").count(), fetched, "no new request");
}

/// REQ-FNT-009: a font the user named by address alone is checked again
/// after a day with a conditional request; when the server says it is
/// current, the cached copy is used.
#[test]
fn a_font_named_by_address_is_checked_daily() {
    let Some(sites) = sites() else { return };
    let path = profile("fonts-daily");
    let request = Request {
        address: sites.address("fontsite", "/page.bdf"),
        sha256: None,
        chooser: Chooser::User,
        scope: Scope::Shared,
    };
    let mut run = Run::new(path.clone());
    let now = neoteletext_gui::now_seconds();
    run.fonts.want(&request, &run.storage, &run.options, &run.wake, now);
    run.wait("the font by address", |run| matches!(run.fonts.state(&request), Some(State::Ready(_))));
    let mut index = run.storage.font_index();
    let entry = index.find_address(&request.address).cloned().unwrap();
    assert!(!entry.named_by_checksum);
    assert_eq!(entry.etag, "\"page-v1\"", "the validator is kept");
    // A day later.
    index.mark_checked(&entry.sha256, now - neoteletext_browser::fonts::RECHECK_SECONDS);
    run.storage.save_font_index(&index);
    drop(run);
    let mut later = Run::new(path);
    let before = log(&sites).len();
    later.fonts.want(&request, &later.storage, &later.options, &later.wake, now);
    later.wait("the checked font", |run| matches!(run.fonts.state(&request), Some(State::Ready(_))));
    let asked: Vec<String> = log(&sites).into_iter().skip(before).collect();
    // The site logs each request's path, then a 304 it answered.
    assert_eq!(asked, ["/page.bdf", "/page.bdf 304"], "one conditional request, answered 304");
    assert!(later.storage.font_index().find_address(&request.address).is_some_and(|entry| entry.checked >= now));
}
