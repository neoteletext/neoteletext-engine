//! One turn of the viewer's loop, in the order a turn takes its steps:
//! the source's notices, the action the viewer asked for, the channel's
//! typing, the page shown and its form, the drawing, then the terminal's
//! event and the keys.

use super::terminal_io::{TICK, Wakened, next_event};
use super::{Shown, ViewState};
use crate::clock::now_ms;
use crate::frame::Frame;
use crate::terminal::Display;
use crossterm::event::Event;
use neoteletext_app::interact::message_row;
use neoteletext_app::session::Session;
use neoteletext_app::source::Source;
use neoteletext_core::config::Config;
use neoteletext_core::level1::Grid;
use neoteletext_core::types::{Instant, PageAddress, SubCode};
use neoteletext_core::viewer::{Action, Viewer, ViewerEvent};
use std::sync::mpsc;

impl ViewState {
    /// Runs turns until the user quits. `paint` draws a grid at a flash
    /// phase; a failure to draw ends the loop with the error.
    pub(super) fn run(
        &mut self,
        session: &mut Session,
        source: &mut dyn Source,
        wakes: &mpsc::Receiver<Wakened>,
        display: Display,
        paint: &mut dyn FnMut(&Grid, Frame) -> std::io::Result<()>,
    ) -> std::io::Result<()> {
        loop {
            if let Some(report) = self.unrecorded.take() {
                session.decoder_mut().record_enhancement(report.build, &report.faults);
            }
            let now = Instant(self.start.elapsed().as_micros().try_into().unwrap_or(u64::MAX));
            if self.live {
                self.live = source.poll(session, now);
            }
            for notice in source.notices() {
                self.take_notice(notice, source);
            }
            let config: Config = *session.decoder().config();
            self.act(session, source);
            let (requested, subcode) = self.viewer.requested();
            if self.typing_turn(session, source, requested) {
                continue;
            }
            let session: &Session = session;
            let page = self.page_shown(session, requested, subcode);
            let page_extras = page.and_then(|page| source.extras(page.key.address, page.key.subcode)).cloned();
            // The modes the open page's `IM` line asks for, while it is open
            // (REQ-INT-114); the session adds them to its stack's top.
            if let Some(modes) = page_extras.as_ref().and_then(|extras| extras.input_modes) {
                self.typing.input.modes.set_page_modes(requested.mpp(), modes);
            }
            let shown = Shown { page, extras: page_extras.as_ref(), config: &config, now };
            self.activate_form(session, &shown);
            self.compose(session, requested, &shown);
            let reveal = self.viewer.reveal(now);
            // Images change with the flash phase; text flashes by itself.
            let frame = Frame::at(&config, reveal, now);
            let state = (self.grid, (display == Display::Iterm2).then_some(frame), reveal);
            if self.last_drawn.as_ref() != Some(&state) {
                paint(&self.grid, frame)?;
                self.last_drawn = Some(state);
            }
            let woke = next_event(wakes, TICK);
            let clock_ms = now_ms(self.start);
            self.paste_or_focus(woke.as_ref(), source, clock_ms);
            if let Some(Event::Key(key)) = woke
                && self.key(key, source, session, &shown, clock_ms).is_break()
            {
                return Ok(());
            }
        }
    }

    /// Carries out the action the viewer asked for last, and clears it.
    fn act(&mut self, session: &mut Session, source: &mut dyn Source) {
        match self.action {
            Action::Request(address, subcode) => {
                session.decoder_mut().set_requested(Some(address));
                source.request(address, subcode);
                self.prompt = None;
            }
            Action::FollowService(target) => {
                if let Some(service) = target.service {
                    self.confirmed_services.insert(service);
                    source.follow(service, target.address, target.subcode);
                    self.viewer = Viewer::new(target.address);
                    let _ = self.viewer.event(ViewerEvent::Started);
                }
                self.prompt = None;
            }
            Action::ConfirmService(target) => {
                let name = target.service.and_then(|service| source.service_name(service)).unwrap_or_default();
                self.prompt = Some(message_row(&format!("Leave for {name}? Press the same key again")));
            }
            Action::None => {}
        }
        self.action = Action::None;
    }

    /// The channel's side of the turn: its records taken, its welcome
    /// shown, a page it sends the viewer to, and the page open told to it.
    /// True when the channel sent the viewer to another page, which the
    /// next turn requests.
    fn typing_turn(&mut self, session: &mut Session, source: &mut dyn Source, requested: PageAddress) -> bool {
        let clock_ms = now_ms(self.start);
        let was_on = self.typing.on();
        if self.typing.records(source, session.take_kept().into_iter(), clock_ms) {
            self.prompt = Some(message_row("This channel did not welcome input"));
        }
        if self.typing.on() && !was_on {
            self.prompt = Some(message_row("Keys go to this channel. Ctrl-] leaves"));
        }
        if let Some(address) = self.typing.go_to.take().and_then(PageAddress::from_mpp)
            && address != requested
        {
            self.viewer = Viewer::new(address);
            self.action = Action::Request(address, SubCode::ANY);
            return true;
        }
        // The channel is told which of its pages is open (INPUT.md section
        // 5.4).
        if self.typing.on() && self.typing.told != Some(requested.mpp()) {
            self.typing.told = Some(requested.mpp());
            let page = neoteletext_input::event::Item::Event(neoteletext_input::event::Event::Page(requested.mpp()));
            self.typing.send(source, &page, Some(requested.mpp()), clock_ms);
        }
        false
    }
}
