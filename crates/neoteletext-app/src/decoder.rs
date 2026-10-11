//! The browser's decoder process (`docs/desktop-browser.md` sections 3.1
//! and 3.4): one site's pages, decoded and rendered with no network and no
//! files.
//!
//! The interface process starts it, forwards the fetcher's frames to it on
//! standard input with the user's keys, and reads back on standard output
//! the rendered screen, the page's status, and the fetcher requests it
//! makes (page requests, form submissions, links to other services), which
//! the interface checks before passing them on. Everything a decoder sends
//! is untrusted: the interface draws row 0 itself and confirms every other
//! site.

use crate::extras::{self, Destination, INDEX_KEY, PageExtras};
use crate::interact::{ActiveForm, message_row, submission};
use crate::pipe::{self, DisplaySettings, Frame, Notice, PageStatus, Screen, UserInput};
use crate::present::{self, MosaicStyle, Scale};
use crate::session::{DEFAULT_PAGES, Session};
use crate::source::Source;
use crate::{Failure, exit, live};
use neoteletext_core::acq::EnhancementReport;
use neoteletext_core::config::Config;
use neoteletext_core::consts::{COLUMNS, ROWS};
use neoteletext_core::forms::{FormAction, FormKey, FormModel};
use neoteletext_core::fsm::PageState;
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::{Grid, Zoom};
use neoteletext_core::nav::PageLinks;
use neoteletext_core::render::{ColourMode, FlashPhase, RenderOptions, flash_visible};
use neoteletext_core::types::{Instant, PageAddress, PageNumber};
use neoteletext_core::viewer::{
    Action, ComposeContext, Composed, Key, KeyContext, SecurityIndicator, Viewer, ViewerEvent, ViewerState,
};
use neoteletext_image::palette::Rgba;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant as StdInstant};

/// How often the decoder wakes when nothing arrives: often enough for
/// flash (decision E.12) and the page timeout.
const TICK: Duration = Duration::from_millis(40);
/// Columns of one Fastext prompt in row 24: four prompts across the row.
const PROMPT_COLUMNS: usize = COLUMNS / 4;
/// Digits in a page number.
const PAGE_DIGITS: usize = 3;
/// The page number of a magazine's Organization Table, xFE (clause 10.6).
const MOT_PAGE: u8 = neoteletext_core::consts::PAGE_MOT;
/// The row of the Fastext prompts.
const FASTEXT_ROW: usize = ROWS - 1;

/// The glyph sets a decoder can draw with, indexed by
/// [`DisplaySettings::glyphs`]: the project's own, Bedstead, and both
/// side by side (decision E.62). The program supplies them, so this
/// module builds with whichever sets the program's core has.
pub type GlyphChoices = [&'static [GlyphSet]; 3];

/// Runs the decoder process until the interface closes its input.
/// `sandboxed_by_parent` says the interface already confined this process
/// (macOS, `docs/desktop-browser.md` section 12.3).
pub fn run(no_sandbox: bool, sandboxed_by_parent: bool, glyphs: GlyphChoices) -> Result<(), Failure> {
    let (fetched, fetched_frames) = mpsc::channel();
    let (commands, command_frames) = mpsc::channel();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        // Every frame is validated by pipe::read; a bad one ends the link.
        while let Ok(Some(frame)) = pipe::read(&mut input) {
            let delivered = match frame {
                Frame::Packet(_) | Frame::Tti(_) | Frame::T42(_) | Frame::Notice(_) => fetched.send(frame).is_ok(),
                Frame::Input(_) | Frame::Show(..) | Frame::Settings(_) | Frame::Click(..) => {
                    commands.send(frame).is_ok()
                }
                // Frames only a decoder sends are never taken in.
                _ => true,
            };
            if !delivered {
                break;
            }
        }
    });
    if !sandboxed_by_parent {
        crate::sandbox::apply(
            crate::sandbox::Policy { writable_directory: None, network: false, readable: &[] },
            no_sandbox,
        )
        .map_err(|message| (exit::INPUT, message))?;
    }
    let mut source = live::routed(fetched_frames, Box::new(std::io::stdout()));
    let mut session = Session::new(Config::default(), DEFAULT_PAGES);
    let mut engine = Engine::new(glyphs);
    let start = StdInstant::now();
    let clock = || Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX));
    let mut out = std::io::stdout();
    loop {
        let now = clock();
        for frame in engine.step(&mut session, &mut source, now) {
            if pipe::write(&mut out, &frame).is_err() {
                return Ok(());
            }
        }
        match command_frames.recv_timeout(TICK) {
            Ok(frame) => {
                engine.command(&frame, &mut session, &mut source, clock());
                if !drain(&command_frames, &mut engine, &mut session, &mut source, clock()) {
                    return Ok(());
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

/// Handles every command waiting. Returns false when the interface has
/// gone.
fn drain(
    commands: &Receiver<Frame>,
    engine: &mut Engine,
    session: &mut Session,
    source: &mut dyn Source,
    now: Instant,
) -> bool {
    loop {
        match commands.try_recv() {
            Ok(frame) => engine.command(&frame, session, source, now),
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => return false,
        }
    }
}

/// The decoder's view of one tab: the viewer machine, the form, the
/// receiver's row 24 message, and what was last sent to the interface.
pub struct Engine {
    viewer: Viewer,
    initial: PageAddress,
    action: Action,
    form: Option<ActiveForm>,
    prompt: Option<[u8; COLUMNS]>,
    grid: Grid,
    glyphs: GlyphChoices,
    settings: DisplaySettings,
    unrecorded: Option<EnhancementReport>,
    last_screen: Option<Screen>,
    last_status: Option<PageStatus>,
    /// The user has chosen a page since the decoder started.
    moved: bool,
    /// Frames for the interface made while handling its commands.
    outgoing: Vec<Frame>,
    /// Page catching: the selected link among [`Engine::catchable`].
    catching: Option<usize>,
    /// The extension data of the page last composed.
    last_extras: Option<PageExtras>,
    /// Every link is underlined (concept decision 11).
    show_links: bool,
    /// Enhancement pages already asked for, `0xMPP`.
    prefetched: std::collections::HashSet<u16>,
}

/// A link page catching can select: a page number printed on the page,
/// or an `LT` run of text.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Catchable {
    row: usize,
    column: usize,
    length: usize,
    destination: Destination,
}

fn viewer_key(input: UserInput) -> Option<Key> {
    Some(match input {
        UserInput::Digit(digit) => Key::Digit(digit),
        UserInput::Colour(0) => Key::Red,
        UserInput::Colour(1) => Key::Green,
        UserInput::Colour(2) => Key::Yellow,
        UserInput::Colour(_) => Key::Cyan,
        UserInput::Index => Key::Index,
        UserInput::NextPage => Key::NextPage,
        UserInput::PreviousPage => Key::PreviousPage,
        UserInput::NextSubPage => Key::NextSubPage,
        UserInput::PreviousSubPage => Key::PreviousSubPage,
        UserInput::Reveal => Key::Reveal,
        UserInput::Hold => Key::Hold,
        UserInput::Mix => Key::Mix,
        UserInput::Zoom => Key::Zoom,
        UserInput::Home => Key::Home,
        _ => return None,
    })
}

fn form_key(input: UserInput) -> Option<FormKey> {
    Some(match input {
        UserInput::Tab => FormKey::Tab,
        UserInput::BackTab => FormKey::BackTab,
        UserInput::Enter => FormKey::Enter,
        UserInput::Backspace => FormKey::Backspace,
        UserInput::Arrow(0) => FormKey::Left,
        UserInput::Arrow(1) => FormKey::Right,
        UserInput::Arrow(2) => FormKey::Up,
        UserInput::Arrow(_) => FormKey::Down,
        UserInput::Char(' ') => FormKey::Space,
        UserInput::Char(character) => FormKey::Char(character),
        UserInput::Digit(digit) => FormKey::Char(char::from(b'0' + digit)),
        UserInput::Colour(0) => FormKey::Red,
        UserInput::Colour(1) => FormKey::Green,
        UserInput::Colour(2) => FormKey::Yellow,
        UserInput::Colour(_) => FormKey::Cyan,
        UserInput::Escape => FormKey::Index,
        _ => return None,
    })
}

/// The three-digit page number printed at `column` of a row of text, if
/// the column is on one (page catching, `docs/web-native-concept.md`
/// section 5). A run of more than three digits is not a page number.
#[must_use]
pub fn page_number_at(line: &str, column: usize) -> Option<PageAddress> {
    let characters: Vec<char> = line.chars().collect();
    if !characters.get(column)?.is_ascii_digit() {
        return None;
    }
    let mut first = column;
    while first > 0 && characters[first - 1].is_ascii_digit() {
        first -= 1;
    }
    let mut end = column;
    while end < characters.len() && characters[end].is_ascii_digit() {
        end += 1;
    }
    if end - first != PAGE_DIGITS {
        return None;
    }
    let digits: String = characters[first..end].iter().collect();
    let mpp = u16::from_str_radix(&digits, 16).ok()?;
    PageAddress::from_mpp(mpp)
}

impl Engine {
    /// A decoder showing nothing yet, drawing with `glyphs`.
    #[must_use]
    pub fn new(glyphs: GlyphChoices) -> Self {
        let initial = neoteletext_core::service::initial_page(None);
        let mut viewer = Viewer::new(initial);
        let action = viewer.event(ViewerEvent::Started);
        Self {
            viewer,
            initial,
            action,
            form: None,
            prompt: None,
            grid: Grid::EMPTY,
            glyphs,
            settings: DisplaySettings::default(),
            unrecorded: None,
            last_screen: None,
            last_status: None,
            moved: false,
            outgoing: Vec::new(),
            catching: None,
            last_extras: None,
            show_links: false,
            prefetched: std::collections::HashSet::new(),
        }
    }

    fn show(&mut self, address: PageAddress) {
        self.catching = None;
        self.viewer = Viewer::new(address);
        self.action = self.viewer.event(ViewerEvent::Started);
    }

    /// Handles a command from the interface.
    pub fn command(&mut self, frame: &Frame, session: &mut Session, source: &mut dyn Source, now: Instant) {
        if matches!(frame, Frame::Input(_) | Frame::Show(..) | Frame::Click(..)) {
            self.moved = true;
        }
        match *frame {
            Frame::Input(input) => self.input(input, session, source, now),
            Frame::Show(mpp, _) => {
                if let Some(address) = PageAddress::from_mpp(mpp) {
                    self.show(address);
                }
            }
            Frame::Settings(settings) => {
                self.settings = settings;
                self.last_screen = None;
            }
            Frame::Click(row, column) => self.click(usize::from(row), usize::from(column), session, source, now),
            _ => {}
        }
    }

    /// Asks once for the enhancement pages the shown page uses: its
    /// magazine's Organization Table, then the object and DRCS pages the
    /// table and the page name (concept section 2 item 6). A page's own
    /// file may already have brought them; stored pages are not asked for.
    fn prefetch_dependencies(&mut self, session: &Session, source: &mut dyn Source) {
        let (requested, subcode) = self.viewer.requested();
        let Some(page) = session.page(requested, subcode) else { return };
        let config = session.decoder().config();
        if !config.level.is_enhanced() {
            return;
        }
        let table = PageAddress { magazine: requested.magazine, page: PageNumber(MOT_PAGE) };
        let lookup = session.decoder().enhancement_lookup();
        let links = neoteletext_core::objects::links(page, &lookup, config.level);
        let named = [links.gpop, links.pop, links.gdrcs, links.drcs]
            .into_iter()
            .flatten()
            .map(|link| PageAddress { magazine: link.magazine, page: link.page });
        for address in std::iter::once(table).chain(named) {
            let stored = session.page(address, neoteletext_core::types::SubCode::ANY).is_some();
            if !stored && self.prefetched.insert(address.mpp()) {
                source.prefetch(address);
            }
        }
    }

    /// The extension data of the page on screen.
    fn shown_extras(&self, session: &Session, source: &dyn Source) -> Option<PageExtras> {
        let (requested, subcode) = self.viewer.requested();
        let page = session.page(requested, subcode)?;
        source.extras(page.key.address, page.key.subcode).cloned()
    }

    /// Follows one of the page's own links (`docs/web-native-concept.md`
    /// section 5): an address goes to the interface, which shows the tuning
    /// screen; a named service's page goes the same way through the
    /// fetcher's descriptor; a page of this service is shown here. The
    /// viewer stays on the page shown until the interface moves the tab.
    fn follow(&mut self, destination: &Destination, shown: PageAddress, source: &mut dyn Source) {
        match destination {
            Destination::Address(address) => {
                self.outgoing.push(Frame::Visit(address.clone()));
                self.show(shown);
            }
            Destination::Page(target) => match target.service {
                Some(service) => {
                    source.follow(service, target.address, target.subcode);
                    self.show(shown);
                }
                None => self.show(target.address),
            },
        }
    }

    fn click(&mut self, row: usize, column: usize, session: &mut Session, source: &mut dyn Source, now: Instant) {
        let shown = self.viewer.requested().0;
        if let Some(destination) =
            self.shown_extras(session, source).and_then(|extras| extras.text_link_at(row, column).cloned())
        {
            self.follow(&destination, shown, source);
            return;
        }
        if row == FASTEXT_ROW {
            let colour = u8::try_from(column / PROMPT_COLUMNS).unwrap_or(u8::MAX);
            if colour <= 3 {
                self.input(UserInput::Colour(colour), session, source, now);
            }
            return;
        }
        let text = present::plain(&self.grid, false, MosaicStyle::Unicode16);
        let Some(address) = text.lines().nth(row).and_then(|line| page_number_at(line, column)) else { return };
        for digit in address.ascii() {
            let value = digit.wrapping_sub(b'0');
            self.input(UserInput::Digit(value), session, source, now);
        }
    }

    /// The links page catching can stop at, in reading order: the page's
    /// text links and the page numbers printed on it, rows 1 to 23. Pages
    /// with side panels are not caught, because the text's columns are not
    /// the grid's.
    fn catchable(&self, extras: Option<&PageExtras>) -> Vec<Catchable> {
        if neoteletext_core::render::row_columns(&self.grid) != COLUMNS {
            return Vec::new();
        }
        let text = present::plain(&self.grid, false, MosaicStyle::Unicode16);
        let mut found = Vec::new();
        for (row, line) in text.lines().enumerate().take(FASTEXT_ROW).skip(1) {
            let characters: Vec<char> = line.chars().collect();
            let mut column = 0;
            while column < characters.len() {
                if let Some(link) = extras.and_then(|extras| {
                    extras
                        .text_links
                        .iter()
                        .find(|link| usize::from(link.row) == row && usize::from(link.column) == column)
                }) {
                    found.push(Catchable {
                        row,
                        column,
                        length: usize::from(link.length),
                        destination: link.destination.clone(),
                    });
                    column += usize::from(link.length);
                } else if let Some(address) = page_number_at(line, column) {
                    let destination = extras.and_then(|extras| extras.number_link(address).cloned()).unwrap_or(
                        Destination::Page(neoteletext_core::nav::LinkTarget {
                            address,
                            subcode: neoteletext_core::types::SubCode::ANY,
                            service: None,
                        }),
                    );
                    found.push(Catchable { row, column, length: PAGE_DIGITS, destination });
                    column += PAGE_DIGITS;
                } else {
                    column += 1;
                }
            }
        }
        found
    }

    /// Keys while page catching is on: the arrows move between links,
    /// Enter follows the selected one, Escape turns catching off. Returns
    /// true when the key was used.
    fn catch_key(&mut self, input: UserInput, session: &mut Session, source: &mut dyn Source) -> bool {
        let extras = self.shown_extras(session, source);
        let links = self.catchable(extras.as_ref());
        let Some(selected) = self.catching else {
            if input == UserInput::Enter && !links.is_empty() {
                self.catching = Some(0);
                return true;
            }
            return false;
        };
        let count = links.len().max(1);
        match input {
            UserInput::NextSubPage | UserInput::PreviousPage => self.catching = Some((selected + 1) % count),
            UserInput::PreviousSubPage | UserInput::NextPage => self.catching = Some((selected + count - 1) % count),
            UserInput::Escape => self.catching = None,
            UserInput::Enter => {
                self.catching = None;
                if let Some(link) = links.get(selected) {
                    let shown = self.viewer.requested().0;
                    match &link.destination {
                        // A page of this site goes through the viewer, so
                        // the descriptor's aliases still apply.
                        Destination::Page(target) if target.service.is_none() => {
                            self.action = Action::Request(target.address, target.subcode);
                            self.viewer = Viewer::new(target.address);
                            let _ = self.viewer.event(ViewerEvent::Started);
                        }
                        destination => {
                            let destination = destination.clone();
                            self.follow(&destination, shown, source);
                        }
                    }
                }
            }
            // Every other key leaves catching and does what it does.
            _ => {
                self.catching = None;
                return false;
            }
        }
        true
    }

    fn input(&mut self, input: UserInput, session: &mut Session, source: &mut dyn Source, now: Instant) {
        let form_focused = self.form.as_ref().is_some_and(|form| form.focused);
        if !form_focused && input == UserInput::ShowLinks {
            self.show_links = !self.show_links;
            return;
        }
        if !form_focused && self.catch_key(input, session, source) {
            return;
        }
        if let Some(active) = self.form.as_mut().filter(|form| form.focused) {
            if let Some(key) = form_key(input) {
                match active.model.key(key, now) {
                    FormAction::Submit(button) => {
                        source.submit(submission(active, button));
                        self.prompt = Some(message_row("Submitting..."));
                    }
                    FormAction::Invalid(_, reason) => {
                        self.prompt = Some(message_row(&format!("Check the field: {reason:?}")));
                    }
                    FormAction::Leave => active.focused = false,
                    FormAction::Refused => self.prompt = Some(message_row("That character cannot be shown here")),
                    FormAction::None => {}
                }
            }
            return;
        }
        if input == UserInput::Tab
            && let Some(active) = self.form.as_mut()
        {
            active.focused = true;
            return;
        }
        let Some(key) = viewer_key(input) else { return };
        let (requested, subcode) = self.viewer.requested();
        let page = session.page(requested, subcode);
        let page_extras = page.and_then(|page| source.extras(page.key.address, page.key.subcode)).cloned();
        let links = page_extras
            .as_ref()
            .and_then(|extras| extras.links)
            .or_else(|| page.map(PageLinks::from_page))
            .unwrap_or_default();
        // The interface confirms every other site with its tuning screen,
        // so the decoder never asks twice.
        let confirmed = |_service| true;
        // REQ-SEC-024: links work only while their prompts are on screen.
        let row24_shown = !self.grid.boxed_mode
            && self.viewer.zoom() != Zoom::Top
            && self.grid.cells[FASTEXT_ROW].iter().any(|cell| !cell.glyph.is_blank());
        // A coloured key or Index whose link is an address.
        let key_slot = match input {
            UserInput::Colour(colour) => Some(usize::from(colour)),
            UserInput::Index => Some(INDEX_KEY),
            _ => None,
        };
        let key_address = key_slot
            .and_then(|slot| page_extras.as_ref().and_then(|extras| extras.key_addresses.get(slot).cloned().flatten()));
        if let Some(address) = key_address {
            if row24_shown {
                self.follow(&Destination::Address(address), requested, source);
            }
            return;
        }
        let config = *session.decoder().config();
        let context = KeyContext {
            links: &links,
            store: session.decoder().store(),
            initial: self.initial,
            config: &config,
            now,
            confirmed: &confirmed,
            row24_shown,
        };
        self.action = self.viewer.key(key, &context);
        self.prompt = None;
        // A number typed on a page that links it goes where the page says,
        // before the descriptor's aliases and the site's own page
        // (concept section 5, "Which link a typed number follows").
        if let (UserInput::Digit(_), Action::Request(number, _)) = (input, self.action)
            && let Some(destination) = page_extras.as_ref().and_then(|extras| extras.number_link(number)).cloned()
        {
            self.action = Action::None;
            self.follow(&destination, requested, source);
        }
    }

    fn notices(&mut self, session: &Session, source: &mut dyn Source) {
        for notice in source.notices() {
            match notice {
                // A file or front door without the page asked for opens at
                // its own first page, unless the user has moved.
                Notice::Bundle(mpp) => {
                    let (requested, subcode) = self.viewer.requested();
                    if !self.moved
                        && session.page(requested, subcode).is_none()
                        && let Some(address) = PageAddress::from_mpp(mpp)
                    {
                        self.show(address);
                    }
                }
                Notice::Unencrypted => self.viewer.set_indicator(SecurityIndicator::Unencrypted),
                Notice::InitialPage(mpp) => {
                    if let Some(address) = PageAddress::from_mpp(mpp) {
                        let untouched =
                            self.viewer.requested().0 == self.initial && !self.viewer.history().can_go_back();
                        self.initial = address;
                        if untouched && address != self.viewer.requested().0 {
                            self.show(address);
                        }
                    }
                }
                Notice::ShowPage(mpp) => {
                    if let Some(address) = PageAddress::from_mpp(mpp) {
                        self.show(address);
                    }
                }
                Notice::Submitted(ok) => {
                    let pages = self.form.as_ref().map(|form| (form.model.spec().success, form.model.spec().error));
                    if let Some((success, error)) = pages
                        && let Some(page) = if ok { success } else { error }
                    {
                        self.show(page);
                    }
                    self.prompt = Some(message_row(if ok { "Sent." } else { "Not sent: the service refused it." }));
                }
                // REQ-WEB-009: a page that is not stored cannot be shown
                // while the network is away.
                Notice::Offline(mpp) if PageAddress::from_mpp(mpp) == Some(self.viewer.requested().0) => {
                    self.prompt = Some(message_row("Page not available offline"));
                }
                // The interface handles the rest: confirmations, other
                // sites, names and messages.
                _ => {}
            }
        }
    }

    /// Lets time pass and takes in what arrived; returns the frames for the
    /// interface: requests already went to the fetcher through `source`.
    pub fn step(&mut self, session: &mut Session, source: &mut dyn Source, now: Instant) -> Vec<Frame> {
        if let Some(report) = self.unrecorded.take() {
            session.decoder_mut().record_enhancement(report.build, &report.faults);
        }
        source.poll(session, now);
        self.notices(session, source);
        match std::mem::replace(&mut self.action, Action::None) {
            Action::Request(address, subcode) => {
                session.decoder_mut().set_requested(Some(address));
                source.request(address, subcode);
            }
            Action::FollowService(target) | Action::ConfirmService(target) => {
                if let Some(service) = target.service {
                    source.follow(service, target.address, target.subcode);
                }
            }
            Action::None => {}
        }
        let (requested, subcode) = self.viewer.requested();
        if self.viewer.state() == ViewerState::Searching && session.page(requested, subcode).is_some() {
            self.viewer.event(ViewerEvent::PagePresent);
        }
        self.compose(session, source, now);
        self.prefetch_dependencies(session, source);
        let mut frames = std::mem::take(&mut self.outgoing);
        let screen = self.screen(session.decoder().config(), now);
        if self.last_screen.as_ref() != Some(&screen) {
            self.last_screen = Some(screen.clone());
            frames.push(Frame::Screen(screen));
        }
        let status = self.status(session, now);
        if self.last_status.as_ref() != Some(&status) {
            self.last_status = Some(status.clone());
            frames.push(Frame::Status(status));
        }
        frames
    }

    fn compose(&mut self, session: &mut Session, source: &mut dyn Source, _now: Instant) {
        let (requested, subcode) = self.viewer.requested();
        let page = match self.viewer.state() {
            ViewerState::Searching | ViewerState::Starting => None,
            _ => session.page(requested, subcode),
        };
        // REQ-FSM-005: the status area says when the page shown has not
        // been received, or confirmed current, recently.
        match page.map(|page| page.state) {
            Some(PageState::Stale) => {
                let _ = self.viewer.event(ViewerEvent::PageStale);
            }
            Some(_) => {
                let _ = self.viewer.event(ViewerEvent::PageUpdated);
            }
            None => {}
        }
        let page_extras: Option<PageExtras> =
            page.and_then(|page| source.extras(page.key.address, page.key.subcode)).cloned();
        let config = *session.decoder().config();
        let rolling = session.rolling_header();
        let magazine = session.decoder().magazine_data(requested.magazine);
        let store = session.decoder().store();
        // A form on this page becomes active, keeping values when returning.
        match (&page_extras, page) {
            (Some(PageExtras { form: Some(spec), url, .. }), Some(page)) => {
                let same = self
                    .form
                    .as_ref()
                    .is_some_and(|form| form.page == page.key.address && form.subcode == page.key.subcode);
                if !same {
                    let mut probe = Grid::EMPTY;
                    let context = ComposeContext {
                        magazine: &magazine,
                        config: &config,
                        rolling_header: &rolling,
                        store,
                        row24: None,
                        lookup: session.decoder().enhancement_lookup(),
                    };
                    let _ = self.viewer.compose(&mut probe, Some(page), &context);
                    self.form = FormModel::new(*spec, probe.designation.g0).ok().map(|model| ActiveForm {
                        page: page.key.address,
                        subcode: page.key.subcode,
                        model,
                        url: url.clone(),
                        focused: true,
                    });
                }
            }
            (_, Some(page)) if self.form.as_ref().is_some_and(|form| form.page != page.key.address) => {
                self.form = None;
            }
            _ => {}
        }
        let row24 =
            self.prompt.or_else(|| page_extras.as_ref().and_then(|extras| extras.row24.as_ref()).map(|(row, _)| *row));
        let lookup = session.decoder().enhancement_lookup();
        let context =
            ComposeContext { magazine: &magazine, config: &config, rolling_header: &rolling, store, row24, lookup };
        if let Composed::Built(report) = self.viewer.compose(&mut self.grid, page, &context) {
            self.unrecorded = report;
        }
        if self.prompt.is_none()
            && let Some((_, marks)) = page_extras.as_ref().and_then(|extras| extras.row24.as_ref())
        {
            extras::draw_marks(&mut self.grid, marks);
        }
        if let Some(active) = &self.form {
            active.model.draw(&mut self.grid);
        }
        self.last_extras.clone_from(&page_extras);
        // "Show links" underlines every link with the receiver's own
        // underline, so no page can hide one.
        if self.show_links {
            for link in self.catchable(page_extras.as_ref()) {
                for column in link.column..link.column + link.length {
                    if let Some(cell) = self.grid.cell_mut(link.row, column) {
                        cell.attributes = cell.attributes.with_underline(true);
                    }
                }
            }
        }
        // The selected link is shown with its colours swapped, drawn by
        // the receiver (concept decision 11).
        if let Some(selected) = self.catching {
            let links = self.catchable(page_extras.as_ref());
            match links.get(selected) {
                Some(link) => {
                    for column in link.column..link.column + link.length {
                        if let Some(cell) = self.grid.cell_mut(link.row, column) {
                            core::mem::swap(&mut cell.foreground, &mut cell.background);
                        }
                    }
                }
                None => self.catching = None,
            }
        }
    }

    fn screen(&self, config: &Config, now: Instant) -> Screen {
        let reveal = self.viewer.reveal(now);
        let options = RenderOptions::for_level(config, reveal, FlashPhase::at(now));
        let visible = options.colour != ColourMode::Palette || flash_visible(now, config);
        let sets = self.glyphs.get(usize::from(self.settings.glyphs)).copied().unwrap_or(self.glyphs[0]);
        let raster = present::raster(
            &self.grid,
            options,
            visible,
            sets,
            Scale { x: 1, y: 1, aspect: false },
            Rgba::opaque(0, 0, 0),
        );
        Screen {
            width: u16::try_from(raster.width).unwrap_or(0),
            height: u16::try_from(raster.height).unwrap_or(0),
            palette: raster.palette.iter().map(|colour| [colour.r, colour.g, colour.b, colour.a]).collect(),
            indices: raster.indices,
        }
    }

    fn status(&self, session: &Session, now: Instant) -> PageStatus {
        let (page, subcode) = self.viewer.requested();
        let mut status = [b' '; 8];
        status[..neoteletext_core::consts::STATUS_COLUMNS]
            .copy_from_slice(&self.viewer.status(session.decoder().store()));
        let mut text = present::plain(&self.grid, self.viewer.reveal(now), MosaicStyle::Unicode16);
        if text.len() > pipe::MAX_SCREEN_TEXT {
            let mut end = pipe::MAX_SCREEN_TEXT;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        PageStatus {
            page: page.mpp(),
            subcode: if subcode.is_any() { u16::MAX } else { subcode.value() },
            found: matches!(self.viewer.state(), ViewerState::Displaying | ViewerState::Holding),
            status,
            form: self.form.as_ref().is_some_and(|form| form.focused),
            link: self.caught_destination(),
            text,
        }
    }

    /// Where the selected link leads, for row 0: `P` and the page for this
    /// service, `name:mpp` for a named service, or the address.
    fn caught_destination(&self) -> String {
        let Some(selected) = self.catching else { return String::new() };
        match self.catchable(self.last_extras.as_ref()).get(selected).map(|link| &link.destination) {
            Some(Destination::Address(address)) => address.chars().take(pipe::MAX_LINK_TEXT).collect(),
            Some(Destination::Page(target)) => match target.service {
                Some(_) => format!("another service {:03X}", target.address.mpp()),
                None => format!("P{:03X}", target.address.mpp()),
            },
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_numbers_are_caught_only_as_three_digits() {
        let line = "Weather 200   Phone 0800 000   Sport 302.";
        assert_eq!(page_number_at(line, 8).map(PageAddress::mpp), Some(0x200));
        assert_eq!(page_number_at(line, 10).map(PageAddress::mpp), Some(0x200));
        assert_eq!(page_number_at(line, 20), None, "four digits are not a page number");
        assert_eq!(page_number_at(line, 38).map(PageAddress::mpp), Some(0x302));
        assert_eq!(page_number_at(line, 3), None);
        assert_eq!(page_number_at("099", 1), None, "magazine 0 is not a page");
        assert_eq!(page_number_at("", 0), None);
    }

    /// A source that has nothing to give and records what is asked of it.
    #[derive(Default)]
    struct Recorder {
        requested: Vec<u16>,
        prefetched: Vec<u16>,
    }

    impl Source for Recorder {
        fn poll(&mut self, _session: &mut Session, _now: Instant) -> bool {
            true
        }

        fn request(&mut self, page: PageAddress, _subcode: neoteletext_core::types::SubCode) {
            self.requested.push(page.mpp());
        }

        fn prefetch(&mut self, page: PageAddress) {
            self.prefetched.push(page.mpp());
        }
    }

    /// A session holding the pages of a TTI text.
    fn session_with(tti: &[u8]) -> Session {
        use neoteletext_formats::Item;
        use neoteletext_formats::detect::{AutoConfig, AutoReader, Format};
        use neoteletext_formats::item::Reader;
        let mut session = Session::new(Config::default(), 64);
        let mut reader =
            AutoReader::new(AutoConfig { format: Some(Format::Tti), encode_pages: true, ..AutoConfig::default() });
        let mut packets = Vec::new();
        let mut collect = |item: Item<'_>| {
            if let Item::Packet { packet, meta } = item {
                packets.push((packet, meta));
            }
        };
        reader.feed(tti, &mut collect);
        reader.finish(&mut collect);
        for (packet, meta) in &packets {
            session.push(packet, meta);
        }
        session.finish();
        session
    }

    const GLYPHS: GlyphChoices = [&[], &[], &[]];

    /// `docs/web-native-concept.md` section 2 item 6: a shown page brings
    /// its magazine's Organization Table, once.
    #[test]
    fn the_organization_table_is_asked_for_once() {
        let mut session = session_with(b"PN,10000\nOL,1,HELLO\n");
        let mut source = Recorder::default();
        let mut engine = Engine::new(GLYPHS);
        for tick in 0..3 {
            let _ = engine.step(&mut session, &mut source, Instant(tick));
        }
        assert_eq!(source.requested, [0x100]);
        assert_eq!(source.prefetched, [0x1FE]);
    }

    /// Concept decision 11: "show links" underlines every link, and only
    /// links.
    #[test]
    fn show_links_underlines_the_page_numbers() {
        let mut session = session_with(b"PN,10000\nOL,4,Sport 200 and 1234\n");
        let mut source = Recorder::default();
        let mut engine = Engine::new(GLYPHS);
        let _ = engine.step(&mut session, &mut source, Instant(0));
        let underlined = |engine: &Engine| -> Vec<usize> {
            (0..COLUMNS).filter(|&column| engine.grid.cells[4][column].attributes.underline()).collect()
        };
        assert_eq!(underlined(&engine), Vec::<usize>::new());
        engine.command(&Frame::Input(UserInput::ShowLinks), &mut session, &mut source, Instant(1));
        let _ = engine.step(&mut session, &mut source, Instant(2));
        assert_eq!(underlined(&engine), [6, 7, 8], "200, and not the four digits of 1234");
        engine.command(&Frame::Input(UserInput::ShowLinks), &mut session, &mut source, Instant(3));
        let _ = engine.step(&mut session, &mut source, Instant(4));
        assert_eq!(underlined(&engine), Vec::<usize>::new());
    }
}
