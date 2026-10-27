//! The interactive viewer, `neottx view` (REQ-CLI-004, decision 19 of
//! IMPLEMENTATION-LOG.md): the core viewer machine driven by terminal keys,
//! with the TTI Link Extension (Appendix E, REQ-NAV-011 and 012) and the
//! security confirmations of area SEC.

use crate::ansi::{ColourDepth, MosaicStyle, ansi};
use crate::frame::Frame;
use crate::image::{Scale, iterm2_inline, png, raster};
use crate::terminal::Display;
use crossterm::event::{
    self, EnableBracketedPaste, EnableFocusChange, KeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::{cursor, execute, queue, terminal};
use neoteletext_app::extras::PageExtras;
use neoteletext_app::session::Session;
use neoteletext_app::source::Source;
use neoteletext_core::acq::{EnhancementReport, StoredPage};
use neoteletext_core::config::Config;
use neoteletext_core::consts::COLUMNS;
use neoteletext_core::level1::Grid;
use neoteletext_core::service::initial_page;
use neoteletext_core::types::{Instant, PageAddress};
use neoteletext_core::viewer::{Action, Viewer, ViewerEvent};
use neoteletext_image::palette::Rgba;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant as StdInstant;
use terminal_io::{RawGuard, Wakened};
use typing::Typing;

mod input;
mod keys;
mod notices;
mod page;
mod terminal_io;
mod turn;
mod typing;

/// What the viewer keeps from one turn of its loop to the next.
struct ViewState {
    start: StdInstant,
    initial: PageAddress,
    viewer: Viewer,
    messages: Vec<String>,
    live: bool,
    grid: Grid,
    last_drawn: Option<(Grid, Option<Frame>, bool)>,
    action: Action,
    prompt: Option<[u8; COLUMNS]>,
    confirmed_services: HashSet<u8>,
    typing: Typing,
    // The report of the last grid built. The page and store stay borrowed
    // until the key is handled, so it is recorded at the start of the next
    // pass, when nothing borrows the decoder.
    unrecorded: Option<EnhancementReport>,
}

/// What a turn knows about the page it shows, for the steps after the
/// page is found.
struct Shown<'a> {
    page: Option<&'a StoredPage>,
    extras: Option<&'a PageExtras>,
    config: &'a Config,
    now: Instant,
}

impl ViewState {
    /// The viewer at the start of `session`'s service, started at `start`.
    fn new(session: &Session, start: StdInstant, enhanced: bool) -> Self {
        let initial = initial_page(session.service().as_ref());
        let mut viewer = Viewer::new(initial);
        let action = viewer.event(ViewerEvent::Started);
        Self {
            start,
            initial,
            viewer,
            messages: Vec::new(),
            live: true,
            grid: Grid::EMPTY,
            last_drawn: None,
            action,
            prompt: None,
            confirmed_services: HashSet::new(),
            typing: Typing::new(enhanced),
            unrecorded: None,
        }
    }

    /// The messages the source gave, for after the terminal is restored.
    fn into_messages(self) -> Vec<String> {
        self.messages
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
    // Releases and repeats where the terminal has the Kitty keyboard
    // protocol, a paste as one event, and the focus (INPUT.md section 9.6).
    let enhanced = terminal::supports_keyboard_enhancement().unwrap_or(false);
    let guard = RawGuard::new(enhanced);
    let mut out = std::io::stdout();
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide, EnableBracketedPaste, EnableFocusChange)?;
    if enhanced {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES | KeyboardEnhancementFlags::REPORT_EVENT_TYPES;
        execute!(out, PushKeyboardEnhancementFlags(flags))?;
    }

    let start = StdInstant::now();
    let mut state = ViewState::new(session, start, enhanced);
    session.keep(neoteletext_input::is_input_packet);
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
    state.run(session, source, &wakes, display, &mut |grid, frame| {
        draw(&mut out, grid, display, scale, depth, mosaics, frame)
    })?;
    drop(guard);
    for message in state.into_messages() {
        eprintln!("neottx: {message}");
    }
    Ok(())
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
