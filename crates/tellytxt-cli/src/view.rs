//! The interactive viewer, `ttx view` (REQ-CLI-004, decision 19 of
//! IMPLEMENTATION-LOG.md): the core viewer machine driven by terminal keys.

use crate::output::image::{Scale, iterm2_inline, png, raster};
use crate::output::text::{ColourDepth, MosaicStyle, ansi};
use crate::pipe::Notice;
use crate::session::Session;
use crate::terminal::Display;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, execute, queue, terminal};
use std::io::Write;
use std::time::{Duration, Instant as StdInstant};
use tellytxt_core::config::Config;
use tellytxt_core::level1::Grid;
use tellytxt_core::nav::PageLinks;
use tellytxt_core::render::{RenderOptions, flash_visible};
use tellytxt_core::service::initial_page;
use tellytxt_core::types::{Instant, PageAddress, SubCode};
use tellytxt_core::viewer::SecurityIndicator;
use tellytxt_core::viewer::{Action, ComposeContext, Key, KeyContext, Viewer, ViewerEvent, ViewerState};
use tellytxt_image::palette::{CLUT0, Rgba};

/// Something that feeds packets into the session while the viewer runs.
pub trait Source {
    /// Feeds whatever is available now. Returns false at the end of input.
    fn poll(&mut self, session: &mut Session, now: Instant) -> bool;

    /// Asks for a page, for sources that fetch on demand.
    fn request(&mut self, _page: PageAddress, _subcode: SubCode) {}

    /// Notices for the viewer since the last call.
    fn notices(&mut self) -> Vec<Notice> {
        Vec::new()
    }
}

/// How often the viewer wakes to poll the source, check flash and keys.
const TICK: Duration = Duration::from_millis(40);

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

/// Restores the terminal however the viewer exits.
struct RawGuard;

impl Drop for RawGuard {
    fn drop(&mut self) {
        let mut out = std::io::stdout();
        let _ = execute!(out, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

/// Runs the viewer until the user quits.
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
    let mut last_drawn: Option<(Grid, bool, bool)> = None;
    let mut action = viewer.event(ViewerEvent::Started);
    let result = loop {
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
                            action = Action::Request(address, SubCode::ANY);
                        }
                    }
                }
                Notice::Message(text) => messages.push(text),
            }
        }
        let config: Config = *session.decoder().config();
        if let Action::Request(address, subcode) = action {
            session.decoder_mut().set_requested(Some(address));
            source.request(address, subcode);
            if viewer.requested().0 != address {
                // A notice moved the start page: follow it without a key.
                viewer = Viewer::new(address);
                let _ = viewer.event(ViewerEvent::Started);
            }
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
        let rolling = session.rolling_header();
        let magazine = session.decoder().magazine_data(requested.magazine);
        let store = session.decoder().store();
        let context =
            ComposeContext { magazine: &magazine, config: &config, rolling_header: &rolling, store, row24: None };
        let _ = viewer.compose(&mut grid, page, &context);
        let reveal = viewer.reveal(now);
        let flash = flash_visible(now, &config);
        let state = (grid, flash, reveal);
        if last_drawn.as_ref() != Some(&state) {
            draw(&mut out, &grid, display, scale, depth, mosaics, &config, flash, reveal)?;
            last_drawn = Some(state);
        }
        if event::poll(TICK)?
            && let Event::Key(key) = event::read()?
        {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                break Ok(());
            }
            if let Some(key) = map_key(key) {
                let links = page.map(PageLinks::from_page).unwrap_or_default();
                let no = |_| false;
                let key_context = KeyContext { links: &links, store, initial, config: &config, now, confirmed: &no };
                action = viewer.key(key, &key_context);
                last_drawn = None;
            }
        }
    };
    drop(guard);
    for message in messages {
        eprintln!("ttx: {message}");
    }
    result
}

#[allow(clippy::too_many_arguments)] // Drawing settings of one frame.
fn draw(
    out: &mut std::io::Stdout,
    grid: &Grid,
    display: Display,
    scale: Scale,
    depth: ColourDepth,
    mosaics: MosaicStyle,
    config: &Config,
    flash: bool,
    reveal: bool,
) -> std::io::Result<()> {
    queue!(out, cursor::MoveTo(0, 0))?;
    match display {
        Display::Iterm2 => {
            let raster =
                raster(grid, RenderOptions::from_config(config, reveal), scale, &CLUT0, Rgba::opaque(0, 0, 0), flash);
            let encoded = png(&raster)?;
            out.write_all(iterm2_inline(&encoded, raster.width, raster.height).as_bytes())?;
        }
        Display::Text => {
            let text = ansi(grid, reveal, mosaics, depth, &CLUT0).replace('\n', "\r\n");
            out.write_all(text.as_bytes())?;
        }
    }
    out.flush()
}
