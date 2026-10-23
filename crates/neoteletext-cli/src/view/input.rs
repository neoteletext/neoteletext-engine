//! The terminal's events in a turn: a paste and the focus, and each key,
//! whether the channel's, a confirmation's, the active form's or the
//! viewer's.

use super::keys::{LEAVE_TYPING, map_form_key, map_key};
use super::typing::input_of;
use super::{Pending, Shown, ViewState};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use neoteletext_app::interact::{message_row, submission};
use neoteletext_app::session::Session;
use neoteletext_app::source::Source;
use neoteletext_core::forms::FormAction;
use neoteletext_core::nav::PageLinks;
use neoteletext_core::viewer::{Action, KeyContext, SecurityIndicator};
use std::ops::ControlFlow;

impl ViewState {
    /// A paste, while keys go to the channel, and the terminal's focus.
    pub(super) fn paste_or_focus(&mut self, woke: Option<&Event>, source: &mut dyn Source, clock_ms: u32) {
        match woke {
            // A paste goes as one PASTE while keys go (INPUT.md section 9.6).
            Some(Event::Paste(text)) if self.typing.on() => {
                let mut end = text.len().min(neoteletext_input::MAX_PASTE);
                while !text.is_char_boundary(end) {
                    end = end.saturating_sub(1);
                }
                let open = Some(self.viewer.requested().0.mpp());
                self.typing.send(
                    source,
                    &neoteletext_input::event::Item::Event(neoteletext_input::event::Event::Paste(
                        text.get(..end).unwrap_or_default(),
                    )),
                    open,
                    clock_ms,
                );
            }
            Some(Event::FocusGained | Event::FocusLost) => {
                self.typing.focus_changed(matches!(woke, Some(Event::FocusLost)), source, clock_ms);
            }
            _ => {}
        }
    }

    /// A key: the channel's while it takes them, then a confirmation's, the
    /// active form's, or the viewer's. Breaks when the user quits.
    pub(super) fn key(
        &mut self,
        key: KeyEvent,
        source: &mut dyn Source,
        session: &Session,
        shown: &Shown<'_>,
        clock_ms: u32,
    ) -> ControlFlow<()> {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.typing.may_leave()
            && control
            && key.code == KeyCode::Char(LEAVE_TYPING)
            && key.kind != KeyEventKind::Release
        {
            self.leave_or_return_to_typing(source, clock_ms);
            return ControlFlow::Continue(());
        }
        // Every key is the channel's while it takes them, Control-C
        // included, as a terminal's are its host's.
        if self.typing.on() {
            let open = Some(self.viewer.requested().0.mpp());
            let all_keys = self.typing.all_keys(open);
            let mut buffer = [0_u8; 4];
            if let Some(item) = input_of(key, all_keys, &mut buffer) {
                self.typing.send(source, &item, open, clock_ms);
            }
            return ControlFlow::Continue(());
        }
        // The viewer's own keys act once, as pressed.
        if key.kind == KeyEventKind::Release {
            return ControlFlow::Continue(());
        }
        if key.code == KeyCode::Char('c') && control {
            return ControlFlow::Break(());
        }
        self.last_drawn = None;
        if let Some(Pending::Submission) = &self.pending {
            self.answer_submission(key, source);
            return ControlFlow::Continue(());
        }
        if let Some(active) = self.form.as_mut().filter(|form| form.focused) {
            if let Some(form_key) = map_form_key(key) {
                match active.model.key(form_key, shown.now) {
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
            return ControlFlow::Continue(());
        }
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
            return ControlFlow::Break(());
        }
        if key.code == KeyCode::Tab
            && let Some(active) = self.form.as_mut()
        {
            active.focused = true;
            return ControlFlow::Continue(());
        }
        self.viewer_key(key, session, shown);
        ControlFlow::Continue(())
    }

    /// Control-]: leaving typing is losing the focus, and the channel lets
    /// go of what is held (REQ-INT-108); pressed again, keys go to it again.
    fn leave_or_return_to_typing(&mut self, source: &mut dyn Source, clock_ms: u32) {
        let left = self.typing.leave_or_return(source, clock_ms);
        self.prompt = Some(message_row(if left {
            "Keys are the viewer's. Ctrl-] types again"
        } else {
            "Keys go to this channel. Ctrl-] leaves"
        }));
        self.last_drawn = None;
    }

    /// The answer to a submission's confirmation (REQ-SEC-008, REQ-SEC-009).
    fn answer_submission(&mut self, key: KeyEvent, source: &mut dyn Source) {
        let yes = matches!(key.code, KeyCode::Enter | KeyCode::F(4));
        source.confirm(yes);
        self.pending = None;
        self.prompt = if yes { Some(message_row("Sending...")) } else { None };
        self.viewer.set_indicator(SecurityIndicator::None);
    }

    /// A key for the viewer machine, with the links of the page shown.
    fn viewer_key(&mut self, key: KeyEvent, session: &Session, shown: &Shown<'_>) {
        let Some(key) = map_key(key) else { return };
        let links = shown
            .extras
            .and_then(|extras| extras.links)
            .or_else(|| shown.page.map(PageLinks::from_page))
            .unwrap_or_default();
        let confirmed = |service| self.confirmed_services.contains(&service);
        // REQ-SEC-024: links work only while their prompts are on
        // screen: row 24 drawn, not boxed away, not zoomed off.
        let row24_shown = !self.grid.boxed_mode
            && self.viewer.zoom() != neoteletext_core::level1::Zoom::Top
            && self.grid.cells[24].iter().any(|cell| !cell.glyph.is_blank());
        let key_context = KeyContext {
            links: &links,
            store: session.decoder().store(),
            initial: self.initial,
            config: shown.config,
            now: shown.now,
            confirmed: &confirmed,
            row24_shown,
        };
        self.action = self.viewer.key(key, &key_context);
        if !matches!(self.action, Action::ConfirmService(_)) {
            self.prompt = None;
        }
    }
}
