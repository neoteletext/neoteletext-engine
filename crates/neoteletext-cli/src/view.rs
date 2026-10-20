//! The interactive viewer, `neottx view` (REQ-CLI-004, decision 19 of
//! IMPLEMENTATION-LOG.md): the core viewer machine driven by terminal keys,
//! with forms (Appendix C, REQ-NAV-009), the TTI Link Extension (Appendix
//! E, REQ-NAV-011 and 012) and the security confirmations of area SEC.

use crate::output::image::{Frame, Scale, iterm2_inline, png, raster};
use crate::output::text::{ColourDepth, MosaicStyle, ansi};
use crate::terminal::Display;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, execute, queue, terminal};
use neoteletext_app::extras::{self, PageExtras};
use neoteletext_app::interact::{ActiveForm, message_row, origin_of, submission};
use neoteletext_app::pipe::Notice;
use neoteletext_app::session::Session;
use neoteletext_app::source::Source;
use neoteletext_core::acq::EnhancementReport;
use neoteletext_core::config::Config;
use neoteletext_core::consts::{ALPHA_YELLOW, COLUMNS, SPACE, STATUS_COLUMNS};
use neoteletext_core::forms::{FormAction, FormKey, FormModel};
use neoteletext_core::fsm::PageState;
use neoteletext_core::level1::Grid;
use neoteletext_core::nav::PageLinks;
use neoteletext_core::packet::Packet;
use neoteletext_core::service::initial_page;
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_core::viewer::{
    Action, ComposeContext, Composed, Key, KeyContext, SecurityIndicator, Viewer, ViewerEvent, ViewerState,
};
use neoteletext_image::palette::Rgba;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant as StdInstant};

/// What ends the viewer's wait.
enum Wakened {
    /// The terminal reported an event.
    Terminal(Event),
    /// The source has something for its next poll.
    Source,
}

/// Waits up to `wait` for the terminal or the source, and returns the key
/// if a key ended the wait; anything else only ends it early.
fn next_key(wakes: &Receiver<Wakened>, wait: Duration) -> Option<KeyEvent> {
    match wakes.recv_timeout(wait) {
        Ok(Wakened::Terminal(Event::Key(key))) => Some(key),
        Ok(_) | Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
    }
}

/// The longest the viewer waits before it looks at the source again and
/// draws flash and the ticker; a key or a page wakes it at once.
const TICK: Duration = Duration::from_millis(40);
/// Microseconds per step of the scrolling form origin (REQ-SEC-009).
const TICKER_STEP_US: u64 = 300_000;

fn map_key(key: KeyEvent) -> Option<Key> {
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    Some(match key.code {
        KeyCode::Char(c @ '0'..='9') => Key::Digit(c as u8 - b'0'),
        KeyCode::Char('r') | KeyCode::F(1) => Key::Red,
        KeyCode::Char('g') | KeyCode::F(2) => Key::Green,
        KeyCode::Char('y') | KeyCode::F(3) => Key::Yellow,
        KeyCode::Char('c') | KeyCode::F(4) => Key::Cyan,
        KeyCode::Char('i') => Key::Index,
        KeyCode::Char('h') | KeyCode::Home => Key::Home,
        KeyCode::Up | KeyCode::Char('+') => Key::NextPage,
        KeyCode::Down | KeyCode::Char('-') => Key::PreviousPage,
        KeyCode::Left if alt => Key::Back,
        KeyCode::Right if alt => Key::Forward,
        KeyCode::Right => Key::NextSubPage,
        KeyCode::Left => Key::PreviousSubPage,
        KeyCode::Char('?') => Key::Reveal,
        KeyCode::Char('H') => Key::Hold,
        KeyCode::Char('m') => Key::Mix,
        KeyCode::Char('z') => Key::Zoom,
        KeyCode::Backspace => Key::Back,
        _ => return None,
    })
}

/// Keys while a form has focus (REQ-NAV-009): typing enters text; the
/// coloured keys are F1 to F4; Esc leaves the form.
fn map_form_key(key: KeyEvent) -> Option<FormKey> {
    Some(match key.code {
        KeyCode::Tab => FormKey::Tab,
        KeyCode::BackTab => FormKey::BackTab,
        KeyCode::Enter => FormKey::Enter,
        KeyCode::Backspace => FormKey::Backspace,
        KeyCode::Left => FormKey::Left,
        KeyCode::Right => FormKey::Right,
        KeyCode::Up => FormKey::Up,
        KeyCode::Down => FormKey::Down,
        KeyCode::Char(' ') => FormKey::Space,
        KeyCode::Char(c) => FormKey::Char(c),
        KeyCode::F(1) => FormKey::Red,
        KeyCode::F(2) => FormKey::Green,
        KeyCode::F(3) => FormKey::Yellow,
        KeyCode::F(4) => FormKey::Cyan,
        KeyCode::Esc => FormKey::Index,
        _ => return None,
    })
}

/// The key that leaves a channel's keys, as telnet's escape character
/// leaves its host's: Control and the right square bracket.
const LEAVE_TYPING: char = ']';

/// What `neottx view` offers in HELLO (INPUT.md section 6.2): it shows
/// pages and can send every key as KEY; a terminal reports no releases and
/// no physical keys, and the viewer draws no cursor.
const OFFERED: u32 = neoteletext_input::modes::PAGE_CAP | neoteletext_input::modes::ALL_KEYS;

/// A terminal key as the input stream has it (INPUT.md sections 5.3 and
/// 5.5): printable text as text, or in `ALL_KEYS` a KEY carrying it; a
/// Control or Alt combination and every other key as KEY. `None` for keys
/// a terminal cannot name.
fn input_of(key: KeyEvent, all_keys: bool, buffer: &mut [u8; 4]) -> Option<neoteletext_input::event::Item<'_>> {
    use neoteletext_input::event::{ALT, Action, CONTROL, Event, Item, Key, KeyCode as InputKey, SHIFT, printable};
    let bit = |set: bool, value: u8| if set { value } else { 0 };
    let mut control = key.modifiers.contains(KeyModifiers::CONTROL);
    let mut alt = key.modifiers.contains(KeyModifiers::ALT);
    // AltGr is a level shift, not a modifier (INPUT.md section 5.7 rule 5,
    // REQ-INT-116): a terminal on Windows reports it as Control and Alt
    // held, and a character typed so is text.
    if control && alt && matches!(key.code, KeyCode::Char(character) if printable(character)) {
        (control, alt) = (false, false);
    }
    let mut mods = bit(key.modifiers.contains(KeyModifiers::SHIFT), SHIFT) | bit(control, CONTROL) | bit(alt, ALT);
    let functional = |name: &str| neoteletext_input::keys::code(name).map(InputKey::Functional);
    let (code, text) = match key.code {
        KeyCode::Char(character) if !printable(character) => return None,
        KeyCode::Char(character) => {
            let text: &str = character.encode_utf8(buffer);
            if !control && !alt && !all_keys {
                return Some(Item::Text(text));
            }
            // A terminal gives the character typed; Shift is in `mods`.
            let unshifted = character.to_lowercase().next().unwrap_or(character);
            (InputKey::Character(unshifted), (!control && !alt).then_some(text))
        }
        KeyCode::BackTab => {
            mods |= SHIFT;
            (functional("Tab")?, None)
        }
        KeyCode::F(number) => (functional(&format!("F{number}"))?, None),
        KeyCode::Enter => (functional("Enter")?, None),
        KeyCode::Backspace => (functional("Backspace")?, None),
        KeyCode::Tab => (functional("Tab")?, None),
        KeyCode::Esc => (functional("Escape")?, None),
        KeyCode::Left => (functional("ArrowLeft")?, None),
        KeyCode::Right => (functional("ArrowRight")?, None),
        KeyCode::Up => (functional("ArrowUp")?, None),
        KeyCode::Down => (functional("ArrowDown")?, None),
        KeyCode::Home => (functional("Home")?, None),
        KeyCode::End => (functional("End")?, None),
        KeyCode::PageUp => (functional("PageUp")?, None),
        KeyCode::PageDown => (functional("PageDown")?, None),
        KeyCode::Delete => (functional("Delete")?, None),
        KeyCode::Insert => (functional("Insert")?, None),
        _ => return None,
    };
    Some(Item::Event(Event::Key(Key { action: Action::Press, key: code, mods, text, base: None })))
}

/// Typing into a channel that takes input (area INT): the viewer's keys
/// go to it as a terminal's go to its host, in one input session.
struct Typing {
    /// The channel's WebSocket is open and its descriptor declares input.
    open: bool,
    /// The viewer left the keys with Control-].
    left: bool,
    /// The input session (INPUT.md section 6).
    input: neoteletext_input::session::Session,
    /// The session was welcomed and told the size and focus.
    started: bool,
    /// The page the channel was last told is open.
    told: Option<u16>,
    /// A page the channel's cursor record named, to open (REQ-INT-113).
    go_to: Option<u16>,
}

/// The page a cursor record for `cursor_page` opens while `open` is open
/// and keys go to the channel: another page of the channel, as a terminal
/// shows what its host sends (INPUT.md section 8 rule 8, REQ-INT-113).
fn host_page(cursor_page: u16, open: u16, keys_go: bool) -> Option<u16> {
    (keys_go && cursor_page != open).then_some(cursor_page)
}

impl Default for Typing {
    fn default() -> Self {
        Self {
            open: false,
            left: false,
            input: neoteletext_input::session::Session::new(OFFERED),
            started: false,
            told: None,
            go_to: None,
        }
    }
}

impl Typing {
    fn on(&self) -> bool {
        self.open && !self.left && self.input.welcomed()
    }

    /// The push connection opened or closed: a new session (REQ-INT-106).
    fn connection(&mut self, source: &mut dyn Source, open: bool, clock_ms: u32) {
        self.open = open;
        self.started = false;
        self.told = None;
        if open {
            let hello = self.input.open(clock_ms, None);
            let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
            if let Ok(length) = neoteletext_input::event::write(&neoteletext_input::event::Item::Event(hello), &mut out)
            {
                source.send_input(&out[..length]);
            }
        } else {
            self.input.close();
        }
    }

    /// Takes the channel's records; once welcomed, says the size and the
    /// focus (section 6.1). Returns whether the channel refused input.
    fn records(&mut self, source: &mut dyn Source, packets: impl Iterator<Item = Packet>, clock_ms: u32) -> bool {
        use neoteletext_input::event::{Event, Item};
        let waiting = matches!(self.input.state(), neoteletext_input::session::State::Waiting(_));
        for packet in packets {
            let mut buffer = [0; neoteletext_input::control::FRAGMENT_DATA];
            if let Some(Ok(control)) = neoteletext_input::control::from_packet(&packet, &mut buffer) {
                // A refused WELCOME leaves the session refused, said below.
                let _ = self.input.control(&control);
            } else if let Some(Ok(cursor)) = neoteletext_input::cursor::from_packet(&packet) {
                self.input.acknowledged(cursor.acked, clock_ms);
                if let Some(open) = self.told {
                    self.go_to = host_page(cursor.page, open, self.on()).or(self.go_to);
                }
            }
        }
        self.input.tick(clock_ms);
        if self.input.welcomed() && !self.started {
            self.started = true;
            let columns = u32::try_from(COLUMNS).unwrap_or(0);
            let rows = neoteletext_input::MAX_ROWS;
            self.send(source, &Item::Event(Event::Resize { columns, rows, width: 0, height: 0 }), None, clock_ms);
            let focus = if self.left { Event::FocusOut } else { Event::FocusIn };
            self.send(source, &Item::Event(focus), None, clock_ms);
        }
        waiting && self.input.state() == neoteletext_input::session::State::Refused
    }

    /// Writes `item` at once, one message an item (REQ-INT-112).
    fn send(
        &mut self,
        source: &mut dyn Source,
        item: &neoteletext_input::event::Item<'_>,
        open: Option<u16>,
        clock_ms: u32,
    ) {
        let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
        if let Ok(length @ 1..) = self.input.send(item, open, clock_ms, &mut out) {
            source.send_input(&out[..length]);
        }
    }
}

/// Restores the terminal however the viewer exits.
struct RawGuard;

impl Drop for RawGuard {
    fn drop(&mut self) {
        let mut out = std::io::stdout();
        let _ = execute!(out, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

/// What the viewer is waiting for the user to confirm.
enum Pending {
    /// A form submission (REQ-SEC-008 and REQ-SEC-009).
    Submission,
}

/// Writes the form's origin into the status area as a ticker (REQ-SEC-009:
/// while a form has focus, the origin it submits to is shown there).
fn origin_ticker(grid: &mut Grid, origin: &str, now: Instant) {
    let text: Vec<u8> = format!("> {origin}   ").bytes().filter(|b| b.is_ascii_graphic() || *b == b' ').collect();
    if text.is_empty() {
        return;
    }
    let offset = usize::try_from(now.0 / TICKER_STEP_US).unwrap_or(0) % text.len();
    let reference = grid.cells[0][STATUS_COLUMNS];
    let g0 = grid.designation.g0;
    for column in 0..STATUS_COLUMNS {
        let byte = text[(offset + column) % text.len()];
        if let Some(cell) = grid.cell_mut(0, column) {
            *cell = neoteletext_core::level1::Cell {
                glyph: if byte == SPACE {
                    neoteletext_core::level1::Glyph::Space
                } else {
                    neoteletext_core::level1::Glyph::Char(neoteletext_core::charset::CharRef::new(
                        neoteletext_core::charset::CharSet::G0(g0),
                        byte,
                    ))
                },
                foreground: neoteletext_core::consts::COLOUR_BLACK,
                // The colour of the Alpha Yellow attribute.
                background: ALPHA_YELLOW & neoteletext_core::consts::COLOUR_ATTRIBUTE_MASK,
                ..reference
            };
        }
    }
}

/// Runs the viewer until the user quits.
#[allow(clippy::too_many_lines)] // One event loop; splitting it scatters its state.
pub fn run(
    session: &mut Session,
    source: &mut dyn Source,
    display: Display,
    scale: Scale,
    depth: ColourDepth,
    mosaics: MosaicStyle,
) -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    let guard = RawGuard;
    let mut out = std::io::stdout();
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;

    let start = StdInstant::now();
    let mut initial = initial_page(session.service().as_ref());
    let mut viewer = Viewer::new(initial);
    let mut messages: Vec<String> = Vec::new();
    let mut live = true;
    let mut grid = Grid::EMPTY;
    let mut last_drawn: Option<(Grid, Option<Frame>, bool)> = None;
    let mut action = viewer.event(ViewerEvent::Started);
    let mut form: Option<ActiveForm> = None;
    let mut pending: Option<Pending> = None;
    let mut prompt: Option<[u8; COLUMNS]> = None;
    let mut confirmed_services: HashSet<u8> = HashSet::new();
    let mut typing = Typing::default();
    session.keep(neoteletext_input::is_input_packet);
    // The report of the last grid built. The page and store stay borrowed
    // until the key is handled, so it is recorded at the start of the next
    // pass, when nothing borrows the decoder.
    let mut unrecorded: Option<EnhancementReport> = None;
    // The viewer waits for the terminal and the source together: a key,
    // and a page the source has for it, are taken at once; the tick is
    // only the longest wait, for flash and the ticker (REQ-INT-008).
    let (woken, wakes) = mpsc::channel();
    let terminal = woken.clone();
    // Held for as long as the viewer runs, so the wait never ends early for
    // want of a sender: a file source drops the wake it is given.
    let _held = woken.clone();
    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            if terminal.send(Wakened::Terminal(event)).is_err() {
                return;
            }
        }
    });
    source.set_wake(Arc::new(move || {
        let _ = woken.send(Wakened::Source);
    }));
    let result = loop {
        if let Some(report) = unrecorded.take() {
            session.decoder_mut().record_enhancement(report.build, &report.faults);
        }
        let now = Instant(start.elapsed().as_micros().try_into().unwrap_or(u64::MAX));
        if live {
            live = source.poll(session, now);
        }
        for notice in source.notices() {
            match notice {
                Notice::Unencrypted => viewer.set_indicator(SecurityIndicator::Unencrypted),
                Notice::InitialPage(mpp) => {
                    if let Some(address) = PageAddress::from_mpp(mpp) {
                        let untouched = viewer.requested().0 == initial && !viewer.history().can_go_back();
                        initial = address;
                        if untouched && address != viewer.requested().0 {
                            viewer = Viewer::new(address);
                            action = viewer.event(ViewerEvent::Started);
                        }
                    }
                }
                Notice::ShowPage(mpp) => {
                    if let Some(address) = PageAddress::from_mpp(mpp) {
                        viewer = Viewer::new(address);
                        action = viewer.event(ViewerEvent::Started);
                    }
                }
                Notice::Confirm(host, plain, _first) => {
                    viewer.set_indicator(if plain {
                        SecurityIndicator::PlainHttpWarning
                    } else {
                        SecurityIndicator::ConfirmSubmission
                    });
                    let warning = if plain { "UNENCRYPTED " } else { "" };
                    prompt = Some(message_row(&format!("{warning}Send to {host}? Enter=yes Esc=no")));
                    pending = Some(Pending::Submission);
                }
                Notice::Submitted(ok) => {
                    let pages = form.as_ref().map(|form| (form.model.spec().success, form.model.spec().error));
                    if let Some((success, error)) = pages
                        && let Some(page) = if ok { success } else { error }
                    {
                        action = Action::Request(page, SubCode::ANY);
                        viewer = Viewer::new(page);
                        let _ = viewer.event(ViewerEvent::Started);
                    }
                    prompt = Some(message_row(if ok { "Sent." } else { "Not sent: the service refused it." }));
                }
                Notice::Service(_) => confirmed_services.clear(),
                Notice::Input(open) => {
                    let clock_ms = u32::try_from(start.elapsed().as_millis() & u128::from(u32::MAX)).unwrap_or(0);
                    typing.connection(source, open, clock_ms);
                }
                Notice::Message(text) => messages.push(text),
                // REQ-WEB-009: a page that is not stored cannot be shown
                // while the network is away.
                Notice::Offline(mpp) if PageAddress::from_mpp(mpp) == Some(viewer.requested().0) => {
                    prompt = Some(message_row("Page not available offline"));
                }
                Notice::Offline(_)
                | Notice::Current(_)
                | Notice::ServiceName(_)
                | Notice::Bundle(_)
                | Notice::Aliases(_)
                | Notice::Refresh(_) => {}
                Notice::RedirectToOtherChannel(address) => messages.push(format!("redirected to {address}")),
            }
        }
        let config: Config = *session.decoder().config();
        match action {
            Action::Request(address, subcode) => {
                session.decoder_mut().set_requested(Some(address));
                source.request(address, subcode);
                prompt = None;
            }
            Action::FollowService(target) => {
                if let Some(service) = target.service {
                    confirmed_services.insert(service);
                    source.follow(service, target.address, target.subcode);
                    viewer = Viewer::new(target.address);
                    let _ = viewer.event(ViewerEvent::Started);
                }
                prompt = None;
            }
            Action::ConfirmService(target) => {
                let name = target.service.and_then(|service| source.service_name(service)).unwrap_or_default();
                prompt = Some(message_row(&format!("Leave for {name}? Press the same key again")));
            }
            Action::None => {}
        }
        action = Action::None;
        let (requested, subcode) = viewer.requested();
        let clock_ms = u32::try_from(start.elapsed().as_millis() & u128::from(u32::MAX)).unwrap_or(0);
        let was_on = typing.on();
        if typing.records(source, session.take_kept().into_iter(), clock_ms) {
            prompt = Some(message_row("This channel did not welcome input"));
        }
        if typing.on() && !was_on {
            prompt = Some(message_row("Keys go to this channel. Ctrl-] leaves"));
        }
        if let Some(address) = typing.go_to.take().and_then(PageAddress::from_mpp)
            && address != requested
        {
            viewer = Viewer::new(address);
            action = Action::Request(address, SubCode::ANY);
            continue;
        }
        // The channel is told which of its pages is open (INPUT.md section
        // 5.4).
        if typing.on() && typing.told != Some(requested.mpp()) {
            typing.told = Some(requested.mpp());
            let page = neoteletext_input::event::Item::Event(neoteletext_input::event::Event::Page(requested.mpp()));
            typing.send(source, &page, Some(requested.mpp()), clock_ms);
        }
        if viewer.state() == ViewerState::Searching && session.page(requested, subcode).is_some() {
            viewer.event(ViewerEvent::PagePresent);
        }
        let page = match viewer.state() {
            ViewerState::Searching | ViewerState::Starting => None,
            _ => session.page(requested, subcode),
        };
        // REQ-FSM-005: the status area says when the page shown has not
        // been received, or confirmed current, recently.
        match page.map(|page| page.state) {
            Some(PageState::Stale) => {
                let _ = viewer.event(ViewerEvent::PageStale);
            }
            Some(_) => {
                let _ = viewer.event(ViewerEvent::PageUpdated);
            }
            None => {}
        }
        let page_extras = page.and_then(|page| source.extras(page.key.address, page.key.subcode)).cloned();
        // The modes the open page's `IM` line asks for, while it is open
        // (REQ-INT-114); the session adds them to its stack's top.
        if let Some(modes) = page_extras.as_ref().and_then(|extras| extras.input_modes) {
            typing.input.modes.set_page_modes(requested.mpp(), modes);
        }
        // A form on this page becomes active, keeping values when returning.
        match (&page_extras, page) {
            (Some(PageExtras { form: Some(spec), url, .. }), Some(page)) => {
                let same =
                    form.as_ref().is_some_and(|form| form.page == page.key.address && form.subcode == page.key.subcode);
                if !same {
                    let mut probe = Grid::EMPTY;
                    let magazine = session.decoder().magazine_data(page.key.address.magazine);
                    let store = session.decoder().store();
                    let rolling = session.rolling_header();
                    let context = ComposeContext {
                        magazine: &magazine,
                        config: &config,
                        rolling_header: &rolling,
                        store,
                        row24: None,
                        lookup: session.decoder().enhancement_lookup(),
                    };
                    let _ = viewer.compose(&mut probe, Some(page), &context);
                    form = FormModel::new(*spec, probe.designation.g0).ok().map(|model| ActiveForm {
                        page: page.key.address,
                        subcode: page.key.subcode,
                        model,
                        url: url.clone(),
                        focused: true,
                    });
                }
            }
            (_, Some(page)) if form.as_ref().is_some_and(|form| form.page != page.key.address) => form = None,
            _ => {}
        }
        let rolling = session.rolling_header();
        let magazine = session.decoder().magazine_data(requested.magazine);
        let store = session.decoder().store();
        let row24 =
            prompt.or_else(|| page_extras.as_ref().and_then(|extras| extras.row24.as_ref()).map(|(row, _)| *row));
        let lookup = session.decoder().enhancement_lookup();
        let context =
            ComposeContext { magazine: &magazine, config: &config, rolling_header: &rolling, store, row24, lookup };
        if let Composed::Built(report) = viewer.compose(&mut grid, page, &context) {
            unrecorded = report;
        }
        if prompt.is_none()
            && let Some((_, marks)) = page_extras.as_ref().and_then(|extras| extras.row24.as_ref())
        {
            extras::draw_marks(&mut grid, marks);
        }
        if let Some(active) = &form {
            active.model.draw(&mut grid);
            if active.focused && pending.is_none() {
                origin_ticker(&mut grid, &origin_of(&active.url), now);
            }
        }
        let reveal = viewer.reveal(now);
        // Images change with the flash phase; text flashes by itself.
        let frame = Frame::at(&config, reveal, now);
        let state = (grid, (display == Display::Iterm2).then_some(frame), reveal);
        if last_drawn.as_ref() != Some(&state) {
            draw(&mut out, &grid, display, scale, depth, mosaics, frame)?;
            last_drawn = Some(state);
        }
        if let Some(key) = next_key(&wakes, TICK) {
            let control = key.modifiers.contains(KeyModifiers::CONTROL);
            if typing.open && typing.input.welcomed() && control && key.code == KeyCode::Char(LEAVE_TYPING) {
                // Leaving is losing the focus: the channel lets go of what
                // is held (REQ-INT-108).
                let clock_ms = u32::try_from(start.elapsed().as_millis() & u128::from(u32::MAX)).unwrap_or(0);
                let focus = if typing.left {
                    neoteletext_input::event::Event::FocusIn
                } else {
                    neoteletext_input::event::Event::FocusOut
                };
                typing.send(source, &neoteletext_input::event::Item::Event(focus), None, clock_ms);
                typing.left = !typing.left;
                typing.told = None;
                prompt = Some(message_row(if typing.left {
                    "Keys are the viewer's. Ctrl-] types again"
                } else {
                    "Keys go to this channel. Ctrl-] leaves"
                }));
                last_drawn = None;
                continue;
            }
            // Every key is the channel's while it takes them, Control-C
            // included, as a terminal's are its host's.
            if typing.on() {
                let open = Some(viewer.requested().0.mpp());
                let all_keys = typing.input.modes_on(open) & neoteletext_input::modes::ALL_KEYS != 0;
                let mut buffer = [0_u8; 4];
                if let Some(item) = input_of(key, all_keys, &mut buffer) {
                    let clock_ms = u32::try_from(start.elapsed().as_millis() & u128::from(u32::MAX)).unwrap_or(0);
                    typing.send(source, &item, open, clock_ms);
                }
                continue;
            }
            if key.code == KeyCode::Char('c') && control {
                break Ok(());
            }
            last_drawn = None;
            if let Some(Pending::Submission) = &pending {
                let yes = matches!(key.code, KeyCode::Enter | KeyCode::F(4));
                source.confirm(yes);
                pending = None;
                prompt = if yes { Some(message_row("Sending...")) } else { None };
                viewer.set_indicator(SecurityIndicator::None);
                continue;
            }
            if let Some(active) = form.as_mut().filter(|form| form.focused) {
                if let Some(form_key) = map_form_key(key) {
                    match active.model.key(form_key, now) {
                        FormAction::Submit(button) => {
                            source.submit(submission(active, button));
                            prompt = Some(message_row("Submitting..."));
                        }
                        FormAction::Invalid(_, reason) => {
                            prompt = Some(message_row(&format!("Check the field: {reason:?}")));
                        }
                        FormAction::Leave => active.focused = false,
                        FormAction::Refused => prompt = Some(message_row("That character cannot be shown here")),
                        FormAction::None => {}
                    }
                }
                continue;
            }
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                break Ok(());
            }
            if key.code == KeyCode::Tab
                && let Some(active) = form.as_mut()
            {
                active.focused = true;
                continue;
            }
            if let Some(key) = map_key(key) {
                let links = page_extras
                    .as_ref()
                    .and_then(|extras| extras.links)
                    .or_else(|| page.map(PageLinks::from_page))
                    .unwrap_or_default();
                let confirmed = |service| confirmed_services.contains(&service);
                // REQ-SEC-024: links work only while their prompts are on
                // screen: row 24 drawn, not boxed away, not zoomed off.
                let row24_shown = !grid.boxed_mode
                    && viewer.zoom() != neoteletext_core::level1::Zoom::Top
                    && grid.cells[24].iter().any(|cell| !cell.glyph.is_blank());
                let key_context = KeyContext {
                    links: &links,
                    store,
                    initial,
                    config: &config,
                    now,
                    confirmed: &confirmed,
                    row24_shown,
                };
                action = viewer.key(key, &key_context);
                if !matches!(action, Action::ConfirmService(_)) {
                    prompt = None;
                }
            }
        }
    };
    drop(guard);
    for message in messages {
        eprintln!("neottx: {message}");
    }
    result
}

fn draw(
    out: &mut std::io::Stdout,
    grid: &Grid,
    display: Display,
    scale: Scale,
    depth: ColourDepth,
    mosaics: MosaicStyle,
    frame: Frame,
) -> std::io::Result<()> {
    queue!(out, cursor::MoveTo(0, 0))?;
    match display {
        Display::Iterm2 => {
            let raster = raster(grid, frame, scale, Rgba::opaque(0, 0, 0));
            let encoded = png(&raster)?;
            out.write_all(iterm2_inline(&encoded, raster.width, raster.height).as_bytes())?;
        }
        Display::Text => {
            let text = ansi(grid, frame.options.reveal, mosaics, depth).replace('\n', "\r\n");
            out.write_all(text.as_bytes())?;
        }
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "a test")]
    use super::*;

    #[test]
    fn a_page_from_the_source_ends_the_wait_at_once() {
        // The viewer looked at the source only between 40 ms waits for the
        // terminal, so a page the fetcher had in 0.3 ms waited up to 40 ms
        // more to be drawn (REQ-INT-008).
        const LONG: Duration = Duration::from_secs(5);
        let (woken, wakes) = mpsc::channel();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(5));
            let _ = woken.send(Wakened::Source);
        });
        let started = StdInstant::now();
        assert!(next_key(&wakes, LONG).is_none());
        assert!(started.elapsed() < LONG / 10, "waited {:?}", started.elapsed());
    }

    fn sent(code: KeyCode, modifiers: KeyModifiers, all_keys: bool) -> Option<Vec<u8>> {
        let mut buffer = [0_u8; 4];
        let item = input_of(KeyEvent::new(code, modifiers), all_keys, &mut buffer)?;
        let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
        let length = neoteletext_input::event::write(&item, &mut out).unwrap();
        Some(out[..length].to_vec())
    }

    /// REQ-INT-104: printable text as text, a Control combination and a
    /// named key as KEY; REQ-INT-116: a non-United-States layout's letters
    /// as the characters typed, in any script, and AltGr as text.
    #[test]
    fn a_terminal_key_is_sent_as_the_input_stream_has_it() {
        let none = KeyModifiers::NONE;
        assert_eq!(sent(KeyCode::Char('a'), none, false), Some(b"a".to_vec()));
        assert_eq!(sent(KeyCode::Char('A'), KeyModifiers::SHIFT, false), Some(b"A".to_vec()));
        assert_eq!(sent(KeyCode::Char('c'), KeyModifiers::CONTROL, false), Some(vec![0xFF, 0x01, 0x08, b'c', 0x04]));
        assert_eq!(sent(KeyCode::Enter, none, false), Some(vec![0xFF, 0x01, 0x04, 0x02]));
        assert_eq!(sent(KeyCode::Up, KeyModifiers::SHIFT, false), Some(vec![0xFF, 0x01, 0x0C, 0x09, 0x01]));
        assert_eq!(sent(KeyCode::BackTab, none, false), Some(vec![0xFF, 0x01, 0x0C, 0x03, 0x01]), "Shift-Tab");
        assert_eq!(sent(KeyCode::Char('й'), none, false), Some("й".as_bytes().to_vec()), "Russian");
        assert_eq!(sent(KeyCode::Char('ש'), none, false), Some("ש".as_bytes().to_vec()), "Hebrew");
        assert_eq!(
            sent(KeyCode::Char('é'), none, false),
            Some("é".as_bytes().to_vec()),
            "French, composed by the terminal"
        );
        let alt_gr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        assert_eq!(
            sent(KeyCode::Char('@'), alt_gr, false),
            Some(b"@".to_vec()),
            "German AltGr-Q, as Windows reports it"
        );
        assert_eq!(sent(KeyCode::Char('\u{1b}'), none, false), None, "a control character is not text");
    }

    /// In `ALL_KEYS` every key is KEY, a character key's text with it.
    #[test]
    fn in_all_keys_a_character_is_a_key_with_its_text() {
        assert_eq!(sent(KeyCode::Char('a'), KeyModifiers::NONE, true), Some(vec![0xFF, 0x01, 0x10, b'a', 1, b'a']));
        assert_eq!(
            sent(KeyCode::Char('A'), KeyModifiers::SHIFT, true),
            Some(vec![0xFF, 0x01, 0x18, b'a', 0x01, 1, b'A']),
            "the key unshifted, Shift in mods, the text as typed"
        );
    }

    /// REQ-INT-113: a cursor record naming another page opens it, only
    /// while keys go to the channel.
    #[test]
    fn a_host_goes_to_a_page_by_naming_it() {
        assert_eq!(host_page(0x301, 0x300, true), Some(0x301));
        assert_eq!(host_page(0x300, 0x300, true), None, "the page already open");
        assert_eq!(host_page(0x301, 0x300, false), None, "keys do not go to the channel");
    }
}
