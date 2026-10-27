//! Typing into a channel that takes input (area INT): a terminal key as the
//! input stream has it, and the one input session with the channel.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use neoteletext_app::source::Source;
use neoteletext_core::consts::COLUMNS;
use neoteletext_core::packet::Packet;

/// What `neottx view` offers in HELLO (INPUT.md section 6.2): it shows
/// pages; it draws no cursor and no echo.
const OFFERED: u32 = neoteletext_input::caps::PAGE_CAP;

/// A terminal key as the input stream has it (INPUT.md sections 5.3, 5.5
/// and 6.3): every key as KEY, a character key's text with it unless
/// Control or Alt is held or it is a release. `None` for keys a terminal
/// cannot name.
pub(super) fn input_of(key: KeyEvent, buffer: &mut [u8; 4]) -> Option<neoteletext_input::event::Item<'_>> {
    use neoteletext_input::event::{ALT, Action, CONTROL, Event, Item, Key, KeyCode as InputKey, SHIFT, printable};
    // A terminal with the Kitty keyboard protocol reports repeats and
    // releases; any other reports every key once, as a press.
    let action = match key.kind {
        KeyEventKind::Press => Action::Press,
        KeyEventKind::Repeat => Action::Repeat,
        KeyEventKind::Release => Action::Release,
    };
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
            // A terminal gives the character typed; Shift is in `mods`.
            let unshifted = character.to_lowercase().next().unwrap_or(character);
            (InputKey::Character(unshifted), (!control && !alt && action != Action::Release).then_some(text))
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
    Some(Item::Event(Event::Key(Key { action, key: code, mods, text, base: None })))
}

/// Typing into a channel that takes input (area INT): the viewer's keys
/// go to it as a terminal's go to its host, in one input session.
// Each flag is a separate fact: the socket, the viewer's choice, the
// terminal's focus and the session's start.
#[allow(clippy::struct_excessive_bools)]
pub(super) struct Typing {
    /// The channel's WebSocket is open and its descriptor declares input.
    open: bool,
    /// The viewer left the keys with Control-].
    left: bool,
    /// The terminal lost the focus, where it reports that.
    unfocused: bool,
    /// The focus the channel was last told.
    focus_told: Option<bool>,
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
pub(super) fn host_page(cursor_page: u16, open: u16, keys_go: bool) -> Option<u16> {
    (keys_go && cursor_page != open).then_some(cursor_page)
}

impl Typing {
    /// A session not yet open.
    pub(super) fn new() -> Self {
        Self {
            open: false,
            left: false,
            unfocused: false,
            focus_told: None,
            input: neoteletext_input::session::Session::new(OFFERED),
            started: false,
            told: None,
            go_to: None,
        }
    }

    /// Tells the channel the focus when it changed: away when the viewer
    /// left with Control-] or the terminal lost the focus, so the channel
    /// lets go of what is held (REQ-INT-108).
    pub(super) fn tell_focus(&mut self, source: &mut dyn Source, clock_ms: u32) {
        use neoteletext_input::event::{Event, Item};
        let focus = !(self.left || self.unfocused);
        if self.started && self.focus_told != Some(focus) {
            self.focus_told = Some(focus);
            let event = if focus { Event::FocusIn } else { Event::FocusOut };
            self.send(source, &Item::Event(event), clock_ms);
        }
    }

    pub(super) fn on(&self) -> bool {
        self.open && !self.left && self.input.welcomed()
    }

    /// Whether Control-] may leave the keys, or take them back: the
    /// channel's connection is open and its session welcomed.
    pub(super) fn may_leave(&self) -> bool {
        self.open && self.input.welcomed()
    }

    /// Leaves the keys with Control-], or takes them back, and tells the
    /// channel the focus; the open page is told again on return. Returns
    /// whether the keys are now left.
    pub(super) fn leave_or_return(&mut self, source: &mut dyn Source, clock_ms: u32) -> bool {
        self.left = !self.left;
        self.tell_focus(source, clock_ms);
        self.told = None;
        self.left
    }

    /// The terminal gained or lost the focus.
    pub(super) fn focus_changed(&mut self, unfocused: bool, source: &mut dyn Source, clock_ms: u32) {
        self.unfocused = unfocused;
        self.tell_focus(source, clock_ms);
    }

    /// The page the channel's cursor record named, to open, once.
    pub(super) fn take_go_to(&mut self) -> Option<u16> {
        self.go_to.take()
    }

    /// The channel is told which of its pages is open (INPUT.md section
    /// 5.4), when the keys go to it and the page changed.
    pub(super) fn tell_open_page(&mut self, source: &mut dyn Source, page: u16, clock_ms: u32) {
        if self.on() && self.told != Some(page) {
            self.told = Some(page);
            let item = neoteletext_input::event::Item::Event(neoteletext_input::event::Event::Page(page));
            self.send(source, &item, clock_ms);
        }
    }

    /// The push connection opened or closed: a new session (REQ-INT-106).
    pub(super) fn connection(&mut self, source: &mut dyn Source, open: bool, clock_ms: u32) {
        self.open = open;
        self.started = false;
        self.told = None;
        self.focus_told = None;
        if open {
            let hello = self.input.open(clock_ms, None);
            let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
            if let Ok(length) = neoteletext_input::event::write(&neoteletext_input::event::Item::Event(hello), &mut out)
                && let Some(message) = out.get(..length)
            {
                source.send_input(message);
            }
        } else {
            self.input.close();
        }
    }

    /// Takes the channel's records; once welcomed, says the size and the
    /// focus (section 6.1). Returns whether the channel refused input.
    pub(super) fn records(
        &mut self,
        source: &mut dyn Source,
        packets: impl Iterator<Item = Packet>,
        clock_ms: u32,
    ) -> bool {
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
            self.send(source, &Item::Event(Event::Resize { columns, rows, width: 0, height: 0 }), clock_ms);
            self.tell_focus(source, clock_ms);
        }
        waiting && self.input.state() == neoteletext_input::session::State::Refused
    }

    /// Writes `item` at once, one message an item (REQ-INT-112).
    pub(super) fn send(&mut self, source: &mut dyn Source, item: &neoteletext_input::event::Item<'_>, clock_ms: u32) {
        let mut out = [0_u8; neoteletext_input::MAX_MESSAGE];
        let sent = crate::nfc::with_nfc(item, |item| self.input.send(item, clock_ms, &mut out));
        if let Ok(length @ 1..) = sent
            && let Some(message) = out.get(..length)
        {
            source.send_input(message);
        }
    }
}
