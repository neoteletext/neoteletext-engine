//! The interactive viewer, `ttx view` (REQ-CLI-004, decision 19 of
//! IMPLEMENTATION-LOG.md): the core viewer machine driven by terminal keys,
//! with forms (Appendix C, REQ-NAV-009), the TTI Link Extension (Appendix
//! E, REQ-NAV-011 and 012) and the security confirmations of area SEC.

use crate::output::image::{Frame, Scale, iterm2_inline, png, raster};
use crate::output::text::{ColourDepth, MosaicStyle, ansi};
use crate::terminal::Display;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, execute, queue, terminal};
use std::collections::HashSet;
use std::io::Write;
use std::time::{Duration, Instant as StdInstant};
use tellytxt_app::extras::{self, PageExtras};
use tellytxt_app::pipe::{Notice, Submission};
use tellytxt_app::session::Session;
use tellytxt_app::source::Source;
use tellytxt_core::acq::EnhancementReport;
use tellytxt_core::config::Config;
use tellytxt_core::consts::{ALPHA_YELLOW, COLUMNS, SPACE, STATUS_COLUMNS};
use tellytxt_core::forms::{FormAction, FormKey, FormModel, Method};
use tellytxt_core::fsm::PageState;
use tellytxt_core::level1::Grid;
use tellytxt_core::nav::PageLinks;
use tellytxt_core::service::initial_page;
use tellytxt_core::types::{Instant, PageAddress, SubCode};
use tellytxt_core::viewer::{
    Action, ComposeContext, Composed, Key, KeyContext, SecurityIndicator, Viewer, ViewerEvent, ViewerState,
};
use tellytxt_image::palette::Rgba;

/// How often the viewer wakes to poll the source, check flash and keys.
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

/// Restores the terminal however the viewer exits.
struct RawGuard;

impl Drop for RawGuard {
    fn drop(&mut self) {
        let mut out = std::io::stdout();
        let _ = execute!(out, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

/// A form on the page being shown.
struct ActiveForm {
    page: PageAddress,
    subcode: SubCode,
    model: FormModel,
    url: String,
    focused: bool,
}

/// What the viewer is waiting for the user to confirm.
enum Pending {
    /// A form submission (REQ-SEC-008 and REQ-SEC-009).
    Submission,
}

/// A receiver-drawn row 24 message, in white, as 7-bit codes.
fn message_row(text: &str) -> [u8; COLUMNS] {
    let mut row = [SPACE; COLUMNS];
    row[0] = ALPHA_YELLOW;
    for (slot, byte) in row.iter_mut().skip(1).zip(text.bytes()) {
        *slot = if byte.is_ascii() && byte >= SPACE { byte } else { SPACE };
    }
    row
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
            *cell = tellytxt_core::level1::Cell {
                glyph: if byte == SPACE {
                    tellytxt_core::level1::Glyph::Space
                } else {
                    tellytxt_core::level1::Glyph::Char(tellytxt_core::charset::CharRef::new(
                        tellytxt_core::charset::CharSet::G0(g0),
                        byte,
                    ))
                },
                foreground: tellytxt_core::consts::COLOUR_BLACK,
                // The colour of the Alpha Yellow attribute.
                background: ALPHA_YELLOW & tellytxt_core::consts::COLOUR_ATTRIBUTE_MASK,
                ..reference
            };
        }
    }
}

/// The origin part of an address, for the ticker.
fn origin_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split('/').next().unwrap_or(rest);
    let scheme = url.split_once("://").map_or("", |(scheme, _)| scheme);
    format!("{scheme}://{host}")
}

/// The submission for a form and the button pressed (REQ-WEB-011).
fn submission(form: &ActiveForm, button: Option<usize>) -> Submission {
    let spec = form.model.spec();
    let mut fields = Vec::new();
    form.model.pairs(&mut |name, value| {
        fields.push((String::from_utf8_lossy(name).into_owned(), value.iter().collect::<String>()));
    });
    let action = button
        .and_then(|index| spec.fields.as_slice().get(index))
        .map(|field| String::from_utf8_lossy(field.action.as_slice()).into_owned())
        .filter(|action| !action.is_empty());
    let token = String::from_utf8_lossy(spec.token.as_slice()).into_owned();
    Submission {
        page_url: form.url.clone(),
        post: spec.method == Method::Post,
        target: String::from_utf8_lossy(spec.target.as_slice()).into_owned(),
        token: (!token.is_empty()).then_some(token),
        action,
        fields,
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
    // The report of the last grid built. The page and store stay borrowed
    // until the key is handled, so it is recorded at the start of the next
    // pass, when nothing borrows the decoder.
    let mut unrecorded: Option<EnhancementReport> = None;
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
                Notice::Message(text) => messages.push(text),
                // REQ-WEB-009: a page that is not stored cannot be shown
                // while the network is away.
                Notice::Offline(mpp) if PageAddress::from_mpp(mpp) == Some(viewer.requested().0) => {
                    prompt = Some(message_row("Page not available offline"));
                }
                Notice::Offline(_) | Notice::Current(_) => {}
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
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
        {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
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
                    && viewer.zoom() != tellytxt_core::level1::Zoom::Top
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
        eprintln!("ttx: {message}");
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
