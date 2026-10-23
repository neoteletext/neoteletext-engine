//! The interactive viewer, `neottx view` (REQ-CLI-004, decision 19 of
//! IMPLEMENTATION-LOG.md): the core viewer machine driven by terminal keys,
//! with forms (Appendix C, REQ-NAV-009), the TTI Link Extension (Appendix
//! E, REQ-NAV-011 and 012) and the security confirmations of area SEC.

use crate::ansi::{ColourDepth, MosaicStyle, ansi};
use crate::clock::now_ms;
use crate::frame::Frame;
use crate::image::{Scale, iterm2_inline, png, raster};
use crate::terminal::Display;
use crossterm::event::{
    self, EnableBracketedPaste, EnableFocusChange, Event, KeyCode, KeyEventKind, KeyModifiers,
    KeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::{cursor, execute, queue, terminal};
use keys::{LEAVE_TYPING, map_form_key, map_key};
use neoteletext_app::extras::{self, PageExtras};
use neoteletext_app::interact::{ActiveForm, message_row, origin_of, submission};
use neoteletext_app::pipe::Notice;
use neoteletext_app::session::Session;
use neoteletext_app::source::Source;
use neoteletext_core::acq::EnhancementReport;
use neoteletext_core::config::Config;
use neoteletext_core::consts::COLUMNS;
use neoteletext_core::forms::{FormAction, FormModel};
use neoteletext_core::fsm::PageState;
use neoteletext_core::level1::Grid;
use neoteletext_core::nav::PageLinks;
use neoteletext_core::service::initial_page;
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_core::viewer::{
    Action, ComposeContext, Composed, KeyContext, SecurityIndicator, Viewer, ViewerEvent, ViewerState,
};
use neoteletext_image::palette::Rgba;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant as StdInstant;
use terminal_io::{RawGuard, TICK, Wakened, next_event};
use ticker::origin_ticker;
use typing::{Typing, input_of};

mod keys;
mod terminal_io;
mod ticker;
mod typing;

/// What the viewer is waiting for the user to confirm.
enum Pending {
    /// A form submission (REQ-SEC-008 and REQ-SEC-009).
    Submission,
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
    // Releases and repeats where the terminal has the Kitty keyboard
    // protocol, a paste as one event, and the focus (INPUT.md section 9.6).
    let enhanced = terminal::supports_keyboard_enhancement().unwrap_or(false);
    let guard = RawGuard { enhanced };
    let mut out = std::io::stdout();
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide, EnableBracketedPaste, EnableFocusChange)?;
    if enhanced {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        execute!(out, PushKeyboardEnhancementFlags(flags))?;
    }

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
    let mut typing = Typing::new(enhanced);
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
                    let clock_ms = now_ms(start);
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
        let clock_ms = now_ms(start);
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
        let woke = next_event(&wakes, TICK);
        let clock_ms = now_ms(start);
        match &woke {
            // A paste goes as one PASTE while keys go (INPUT.md section 9.6).
            Some(Event::Paste(text)) if typing.on() => {
                let mut end = text.len().min(neoteletext_input::MAX_PASTE);
                while !text.is_char_boundary(end) {
                    end = end.saturating_sub(1);
                }
                let open = Some(viewer.requested().0.mpp());
                typing.send(
                    source,
                    &neoteletext_input::event::Item::Event(neoteletext_input::event::Event::Paste(&text[..end])),
                    open,
                    clock_ms,
                );
            }
            Some(Event::FocusGained | Event::FocusLost) => {
                typing.unfocused = matches!(woke, Some(Event::FocusLost));
                typing.tell_focus(source, clock_ms);
            }
            _ => {}
        }
        if let Some(Event::Key(key)) = woke {
            let control = key.modifiers.contains(KeyModifiers::CONTROL);
            if typing.open
                && typing.input.welcomed()
                && control
                && key.code == KeyCode::Char(LEAVE_TYPING)
                && key.kind != KeyEventKind::Release
            {
                // Leaving is losing the focus: the channel lets go of what
                // is held (REQ-INT-108).
                typing.left = !typing.left;
                typing.tell_focus(source, clock_ms);
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
                    typing.send(source, &item, open, clock_ms);
                }
                continue;
            }
            // The viewer's own keys act once, as pressed.
            if key.kind == KeyEventKind::Release {
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
mod tests;
