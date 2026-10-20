//! A receiver's side of one input session (`neoteletext-common/INPUT.md`
//! sections 6 to 8): HELLO and WELCOME, the modes, the input count the
//! channel acknowledges, the round trip it gives, and the rate limit.
//! Every receiver uses it alike: NeoTeletext's trusted core, `neottx view`,
//! `neottx keyboard`, a microcontroller.

use crate::control::Control;
use crate::event::{self, Action, Event, InputError, Item, KeyCode, MouseAction, units, write};
use crate::keys;
use crate::modes::{
    AGENT, ALL_CAPS, ALL_KEYS, BASE_KEY, COMPOSE, KEY_RELEASE, MOUSE_ALL, MOUSE_DRAG, MOUSE_MODES, MOUSE_PIXELS, Modes,
    PAGE_CAP,
};
use crate::{MAX_PER_SECOND, VERSION, WELCOME_TIMEOUT_MS};

/// Sends remembered for the round trip (REQ-INT-137).
pub const REMEMBERED: usize = 256;
/// RFC 1982 serial arithmetic on 16 bits: a count this far ahead or more
/// is behind.
const SERIAL_HALF: u16 = 0x8000;
/// The window the rate limit counts over.
const SECOND_MS: u32 = 1_000;

/// Where the session is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// No connection.
    Closed,
    /// HELLO sent at this time; nothing else may go yet.
    Waiting(u32),
    /// The channel answered: input goes.
    Welcomed {
        /// The capabilities both sides use.
        caps: u32,
    },
    /// The channel did not answer in time, or answered with another
    /// version: it takes no input.
    Refused,
}

/// Why the channel's WELCOME was not taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WelcomeError {
    /// Not waiting for one.
    Unasked,
    /// Another version.
    Version(u32),
    /// Capabilities the receiver did not offer.
    Caps,
}

/// Why an item was not sent.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SendError {
    /// Before WELCOME, or after the channel refused input.
    NotWelcomed,
    /// The item could not be encoded.
    Input(InputError),
}

impl From<InputError> for SendError {
    fn from(error: InputError) -> Self {
        Self::Input(error)
    }
}

/// Whether `later` is after `earlier` by RFC 1982 serial arithmetic on 16
/// bits.
const fn serial_after(later: u16, earlier: u16) -> bool {
    let ahead = later.wrapping_sub(earlier);
    ahead != 0 && ahead < SERIAL_HALF
}

/// One session.
#[derive(Clone, Copy, Debug)]
pub struct Session {
    state: State,
    offered: u32,
    /// What the session's modes are.
    pub modes: Modes,
    /// The input count sent so far, modulo 2¹⁶ as the cursor record carries
    /// it.
    count: u16,
    /// The count after each unacknowledged send and when it went, oldest
    /// first from `sent_first`, a ring; the oldest goes when it is full.
    sent: [(u16, u32); REMEMBERED],
    sent_first: usize,
    sent_length: usize,
    /// The last acknowledgement taken.
    acked: u16,
    limiter: Limiter,
}

impl Session {
    /// A closed session that will offer `caps`.
    #[must_use]
    pub const fn new(caps: u32) -> Self {
        Self {
            state: State::Closed,
            offered: caps & ALL_CAPS,
            modes: Modes::new(),
            count: 0,
            sent: [(0, 0); REMEMBERED],
            sent_first: 0,
            sent_length: 0,
            acked: 0,
            limiter: Limiter::new(),
        }
    }

    /// Where it is.
    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }

    /// Whether input may go.
    #[must_use]
    pub const fn welcomed(&self) -> bool {
        matches!(self.state, State::Welcomed { .. })
    }

    /// The capabilities both sides use, once welcomed.
    #[must_use]
    pub const fn caps(&self) -> u32 {
        match self.state {
            State::Welcomed { caps } => caps,
            _ => 0,
        }
    }

    /// The push connection opened at `now_ms`: a new session (REQ-INT-106),
    /// whose HELLO is returned to be sent first, counted as the input count
    /// counts every event; with `agent` the AGENT capability is offered.
    pub fn open<'a>(&mut self, now_ms: u32, agent: Option<&'a str>) -> Event<'a> {
        *self = Self::new(self.offered);
        self.state = State::Waiting(now_ms);
        let caps = if agent.is_some() { self.offered | AGENT } else { self.offered & !AGENT };
        self.count = 1;
        Event::Hello { version: VERSION, caps, agent }
    }

    /// The connection closed: nothing is held and nothing goes.
    pub fn close(&mut self) {
        *self = Self::new(self.offered);
    }

    /// Takes the channel's WELCOME.
    ///
    /// # Errors
    /// When none was asked for, or it names another version or capabilities
    /// not offered: the session is then refused.
    pub fn welcome(&mut self, version: u32, caps: u32) -> Result<(), WelcomeError> {
        if !matches!(self.state, State::Waiting(_)) {
            return Err(WelcomeError::Unasked);
        }
        if version != VERSION {
            self.state = State::Refused;
            return Err(WelcomeError::Version(version));
        }
        if caps & !self.offered != 0 {
            self.state = State::Refused;
            return Err(WelcomeError::Caps);
        }
        self.state = State::Welcomed { caps };
        self.modes.agree(caps);
        Ok(())
    }

    /// Takes a control message: WELCOME, or a mode change, which applies
    /// only once welcomed.
    ///
    /// # Errors
    /// As [`Session::welcome`].
    pub fn control(&mut self, control: &Control<'_>) -> Result<(), WelcomeError> {
        match *control {
            Control::Welcome { version, caps } => self.welcome(version, caps),
            Control::PushMode(modes) if self.welcomed() => {
                self.modes.push(modes);
                Ok(())
            }
            Control::PopMode(count) if self.welcomed() => {
                self.modes.pop(count);
                Ok(())
            }
            Control::PageModes { page, modes } if self.welcomed() => {
                self.modes.set_page_modes(page, modes);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// At `now_ms`: refused when WELCOME is overdue (section 6.1).
    pub fn tick(&mut self, now_ms: u32) {
        if let State::Waiting(since) = self.state
            && now_ms.wrapping_sub(since) >= WELCOME_TIMEOUT_MS
        {
            self.state = State::Refused;
        }
    }

    /// When WELCOME is due, while waiting for it.
    #[must_use]
    pub const fn welcome_due(&self) -> Option<u32> {
        match self.state {
            State::Waiting(since) => Some(since.wrapping_add(WELCOME_TIMEOUT_MS)),
            _ => None,
        }
    }

    /// The modes in force while `open` is the page the viewer has open.
    #[must_use]
    pub fn modes_on(&self, open: Option<u16>) -> u32 {
        self.modes.on(open)
    }

    /// Encodes `item` into `out` as section 6.3 and section 8 say, while
    /// `open` is the page the viewer has open, at `now_ms`, and counts it:
    /// what the modes in force do not ask for is not sent, nor is a field
    /// they do not ask for, text over the rate limit is cut and an event
    /// over it dropped. Returns how many bytes to write, 0 for none.
    ///
    /// # Errors
    /// Before WELCOME, or when the item cannot be encoded.
    pub fn send(
        &mut self,
        item: &Item<'_>,
        open: Option<u16>,
        now_ms: u32,
        out: &mut [u8],
    ) -> Result<usize, SendError> {
        if !self.welcomed() {
            return Err(SendError::NotWelcomed);
        }
        let Some(item) = self.filter(item, open) else { return Ok(0) };
        let allowed = self.allow(&item, now_ms);
        let item = match item {
            Item::Text(text) => {
                let end = text
                    .char_indices()
                    .nth(usize::try_from(allowed).unwrap_or(usize::MAX))
                    .map_or(text.len(), |(at, _)| at);
                Item::Text(text.get(..end).unwrap_or(""))
            }
            Item::Event(_) if allowed == 0 => return Ok(0),
            event @ Item::Event(_) => event,
        };
        if matches!(item, Item::Text("")) {
            return Ok(0);
        }
        let length = write(&item, out)?;
        self.sent(&item, now_ms);
        Ok(length)
    }

    /// `item` as the modes in force on `open` have it sent, or `None`: the
    /// mouse and the wheel only in a mouse mode (REQ-INT-115).
    fn filter<'a>(&self, item: &Item<'a>, open: Option<u16>) -> Option<Item<'a>> {
        let modes = self.modes.on(open);
        let has = |mode: u32| modes & mode != 0;
        let event = match *item {
            Item::Text(_) if has(ALL_KEYS) => return None,
            Item::Text(text) => return Some(Item::Text(text)),
            Item::Event(event) => event,
        };
        let event = match event {
            Event::Key(key) if key.action != Action::Press && !has(KEY_RELEASE) => return None,
            // A dead key goes only in ALL_KEYS (section 5.7 rule 4), and a
            // key that only modifies or locks only when keys are asked for
            // whole (section 6.3).
            Event::Key(event::Key { key: KeyCode::Functional(code), .. }) if keys::is_dead(code) && !has(ALL_KEYS) => {
                return None;
            }
            Event::Key(event::Key { key: KeyCode::Functional(code), .. })
                if keys::is_modifier(code) && !has(ALL_KEYS | KEY_RELEASE) =>
            {
                return None;
            }
            Event::Key(mut key) => {
                if !has(ALL_KEYS) {
                    key.text = None;
                }
                if !has(BASE_KEY) || key.base.map(KeyCode::Character) == Some(key.key) {
                    key.base = None;
                }
                Event::Key(key)
            }
            Event::Mouse(mut mouse) => {
                let wanted = match mouse.action {
                    MouseAction::Press | MouseAction::Release => has(MOUSE_MODES),
                    MouseAction::Move => has(MOUSE_ALL) || (has(MOUSE_DRAG) && mouse.button != 0),
                };
                if !wanted {
                    return None;
                }
                if !has(MOUSE_PIXELS) {
                    mouse.at.pixels = None;
                }
                Event::Mouse(mouse)
            }
            Event::Wheel(mut wheel) => {
                if !has(MOUSE_MODES) {
                    return None;
                }
                if !has(MOUSE_PIXELS) {
                    wheel.at.pixels = None;
                }
                Event::Wheel(wheel)
            }
            Event::Compose { .. } if !has(COMPOSE) => return None,
            Event::Page(_) if self.caps() & PAGE_CAP == 0 => return None,
            // HELLO goes once, from `open`.
            Event::Hello { .. } => return None,
            other => other,
        };
        Some(Item::Event(event))
    }

    /// How many of `item`'s units may go at `now_ms` under the rate limit
    /// (REQ-INT-109), counting those that may; a release always goes. Text
    /// that may only partly go is cut to that many characters by the
    /// caller.
    pub fn allow(&mut self, item: &Item<'_>, now_ms: u32) -> u32 {
        let release = matches!(item, Item::Event(Event::Key(key)) if key.action == Action::Release);
        let mut allowed: u32 = 0;
        for _ in 0..units(item) {
            if !self.limiter.allow(now_ms, release) {
                break;
            }
            allowed = allowed.saturating_add(1);
        }
        allowed
    }

    /// Records that `item` went at `now_ms`, for the input count and the
    /// round trip; returns the count after it.
    pub fn sent(&mut self, item: &Item<'_>, now_ms: u32) -> u16 {
        // The count is modulo 2¹⁶, so only the low 16 bits of the units add.
        let units = u16::try_from(units(item) & u32::from(u16::MAX)).unwrap_or(u16::MAX);
        self.count = self.count.wrapping_add(units);
        if self.sent_length == REMEMBERED {
            self.drop_oldest();
        }
        let slot = self.sent_first.saturating_add(self.sent_length).checked_rem(REMEMBERED).unwrap_or(0);
        if let Some(entry) = self.sent.get_mut(slot) {
            *entry = (self.count, now_ms);
            self.sent_length = self.sent_length.saturating_add(1);
        }
        self.count
    }

    fn drop_oldest(&mut self) {
        self.sent_first = self.sent_first.saturating_add(1).checked_rem(REMEMBERED).unwrap_or(0);
        self.sent_length = self.sent_length.saturating_sub(1);
    }

    /// The input count sent so far.
    #[must_use]
    pub const fn count(&self) -> u16 {
        self.count
    }

    /// A cursor record's acknowledgement at `now_ms`: the round trip in
    /// milliseconds to the latest send it covers, when it is newer than the
    /// last taken (RFC 1982 serial arithmetic: an older one changes nothing).
    /// The sends it covers are forgotten.
    pub fn acknowledged(&mut self, acked: u16, now_ms: u32) -> Option<u32> {
        if !serial_after(acked, self.acked) {
            return None;
        }
        self.acked = acked;
        let mut latest = None;
        while self.sent_length > 0 {
            let Some(&(count, at)) = self.sent.get(self.sent_first) else { break };
            if serial_after(count, acked) {
                break;
            }
            latest = Some(at);
            self.drop_oldest();
        }
        latest.map(|at| now_ms.wrapping_sub(at))
    }

    /// The last acknowledgement taken: echo up to it is answered.
    #[must_use]
    pub const fn acked(&self) -> u16 {
        self.acked
    }
}

/// Counts input units against [`MAX_PER_SECOND`] over a sliding second; a
/// release is always allowed, so no key is left held at the channel.
#[derive(Clone, Copy, Debug)]
pub struct Limiter {
    /// When each of the last [`MAX_PER_SECOND`] units went, a ring.
    sent_ms: [u32; MAX_PER_SECOND as usize],
    next: usize,
    count: usize,
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Limiter {
    /// A limiter that has sent nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { sent_ms: [0; MAX_PER_SECOND as usize], next: 0, count: 0 }
    }

    /// Whether one unit may go at `now_ms`; one that may is counted, and a
    /// release always goes and is counted.
    pub fn allow(&mut self, now_ms: u32, release: bool) -> bool {
        let full = self.count == self.sent_ms.len();
        let oldest = if full { self.sent_ms.get(self.next).copied() } else { None };
        let recent = oldest.is_some_and(|oldest| now_ms.wrapping_sub(oldest) < SECOND_MS);
        if recent && !release {
            return false;
        }
        if let Some(slot) = self.sent_ms.get_mut(self.next) {
            *slot = now_ms;
        }
        self.next = self.next.saturating_add(1).checked_rem(self.sent_ms.len()).unwrap_or(0);
        self.count = self.count.saturating_add(1).min(self.sent_ms.len());
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects, reason = "a test")]
mod tests {
    use super::*;
    use crate::event::{Key, KeyCode};
    use crate::modes::{MOUSE_BUTTONS, PAGE_CAP};

    fn welcomed(caps: u32) -> Session {
        let mut session = Session::new(caps);
        session.open(0, None);
        session.welcome(VERSION, caps).unwrap();
        session
    }

    // REQ-INT-106.
    #[test]
    fn nothing_goes_before_welcome_and_a_late_one_refuses() {
        let mut session = Session::new(PAGE_CAP | MOUSE_BUTTONS);
        let hello = session.open(1_000, None);
        assert_eq!(hello, Event::Hello { version: VERSION, caps: PAGE_CAP | MOUSE_BUTTONS, agent: None });
        assert!(!session.welcomed());
        assert_eq!(session.welcome_due(), Some(1_000 + WELCOME_TIMEOUT_MS));
        session.tick(1_000 + WELCOME_TIMEOUT_MS - 1);
        assert!(matches!(session.state(), State::Waiting(_)));
        session.tick(1_000 + WELCOME_TIMEOUT_MS);
        assert_eq!(session.state(), State::Refused);
        assert_eq!(session.welcome(VERSION, PAGE_CAP), Err(WelcomeError::Unasked));
    }

    // REQ-INT-106.
    #[test]
    fn hello_carries_the_agent_only_with_its_capability() {
        let mut session = Session::new(PAGE_CAP | AGENT);
        assert_eq!(session.open(0, None), Event::Hello { version: VERSION, caps: PAGE_CAP, agent: None });
        let agent = Some("NeoTeletext/0.1 (macOS)");
        assert_eq!(session.open(0, agent), Event::Hello { version: VERSION, caps: PAGE_CAP | AGENT, agent });
    }

    // REQ-INT-106.
    #[test]
    fn welcome_must_be_this_version_and_a_subset() {
        let mut session = Session::new(PAGE_CAP);
        session.open(0, None);
        assert_eq!(session.welcome(VERSION, PAGE_CAP | MOUSE_BUTTONS), Err(WelcomeError::Caps));
        session.open(0, None);
        assert_eq!(session.welcome(1, PAGE_CAP), Err(WelcomeError::Version(1)));
        session.open(0, None);
        assert_eq!(session.welcome(VERSION, PAGE_CAP), Ok(()));
        assert_eq!(session.caps(), PAGE_CAP);
    }

    // REQ-INT-107.
    #[test]
    fn modes_apply_only_once_welcomed() {
        let mut session = Session::new(MOUSE_BUTTONS);
        session.open(0, None);
        session.control(&Control::PushMode(MOUSE_BUTTONS)).unwrap();
        session.control(&Control::Welcome { version: VERSION, caps: MOUSE_BUTTONS }).unwrap();
        assert_eq!(session.modes.on(None), 0, "the push before WELCOME was not taken");
        session.control(&Control::PageModes { page: 0x300, modes: MOUSE_BUTTONS }).unwrap();
        assert_eq!(session.modes.on(Some(0x300)), MOUSE_BUTTONS);
    }

    // REQ-INT-106.
    #[test]
    fn a_reconnection_is_a_new_session() {
        let mut session = welcomed(MOUSE_BUTTONS);
        session.control(&Control::PushMode(MOUSE_BUTTONS)).unwrap();
        session.sent(&Item::Text("abc"), 5);
        assert_eq!(session.modes.on(None), MOUSE_BUTTONS);
        session.open(10, None);
        assert_eq!(session.modes.on(None), 0, "the mode stack empty");
        assert_eq!(session.count(), 1, "the count from zero, HELLO counted");
        assert_eq!(session.acked(), 0);
        assert!(!session.welcomed());
    }

    // REQ-INT-110.
    #[test]
    fn the_input_count_and_its_acknowledgement_give_the_round_trip() {
        let mut session = welcomed(0);
        assert_eq!(session.sent(&Item::Text("ab"), 100), 3, "HELLO, then two characters");
        assert_eq!(session.sent(&Item::Event(Event::Paste("a long paste")), 110), 4, "a paste counts one");
        assert_eq!(session.sent(&Item::Event(Event::FocusIn), 120), 5);
        assert_eq!(session.acknowledged(3, 130), Some(30), "to the text sent at 100");
        assert_eq!(session.acknowledged(2, 140), None, "an older acknowledgement changes nothing");
        assert_eq!(session.acknowledged(5, 150), Some(30), "to the focus sent at 120");
        assert_eq!(session.acked(), 5);
        assert_eq!(session.acknowledged(6, 160), None, "nothing sent beyond 5: no round trip");
    }

    // REQ-INT-110.
    #[test]
    fn the_count_wraps_and_is_compared_as_a_serial_number() {
        let mut session = welcomed(0);
        for at in 0..u32::from(u16::MAX) {
            session.sent(&Item::Event(Event::FocusIn), at);
            session.acknowledged(session.count(), at);
        }
        assert_eq!(session.count(), 0, "65 536 units: wrapped");
        session.sent(&Item::Text("ab"), 70_000);
        assert_eq!(session.acknowledged(2, 70_005), Some(5), "2 is after 0 across the wrap");
        assert_eq!(session.acknowledged(u16::MAX, 70_006), None, "65 535 is before 2");
    }

    // REQ-INT-137.
    #[test]
    fn at_most_256_sends_are_remembered() {
        let mut session = welcomed(0);
        for at in 0..300 {
            session.sent(&Item::Text("a"), at);
        }
        assert_eq!(session.acknowledged(45, 400), None, "the sends to 45, the 44 oldest, were forgotten");
        assert_eq!(session.acknowledged(301, 400), Some(101), "the last went at 299");
    }

    // REQ-INT-109.
    #[test]
    fn the_rate_is_120_units_a_second_and_releases_always_go() {
        let mut session = Session::new(0);
        assert_eq!(session.allow(&Item::Text("ab"), 5), 2);
        let mut allowed = 0;
        for _ in 0..119 {
            allowed += session.allow(&Item::Text("a"), 5);
        }
        assert_eq!(allowed, 118, "120 units in all");
        let release = Item::Event(Event::Key(Key {
            action: Action::Release,
            key: KeyCode::Character('a'),
            mods: 0,
            text: None,
            base: None,
        }));
        assert_eq!(session.allow(&release, 5), 1, "a release beyond the rate");
        assert_eq!(session.allow(&Item::Text("a"), 5 + 1_000), 1, "a second later");
        let mut limiter = Limiter::new();
        for _ in 0..MAX_PER_SECOND {
            assert!(limiter.allow(u32::MAX - 10, false));
        }
        assert!(!limiter.allow(u32::MAX, false), "the 121st unit");
        assert!(limiter.allow(u32::MAX, true), "a release across the clock's wrap");
        assert!(limiter.allow(1_000 - 11, false), "a second after, past the wrap");
    }

    fn key(action: Action, key: KeyCode) -> Item<'static> {
        Item::Event(Event::Key(Key { action, key, mods: 0, text: Some("a"), base: Some('q') }))
    }

    fn sent_item(session: &mut Session, item: &Item<'_>, open: Option<u16>) -> Option<std::vec::Vec<u8>> {
        let mut out = [0; crate::MAX_MESSAGE];
        let length = session.send(item, open, 0, &mut out).unwrap();
        (length > 0).then(|| out.get(..length).unwrap().to_vec())
    }

    // REQ-INT-106.
    #[test]
    fn nothing_is_sent_before_welcome() {
        let mut session = Session::new(0);
        session.open(0, None);
        assert_eq!(session.send(&Item::Text("a"), None, 0, &mut [0; 8]), Err(SendError::NotWelcomed));
    }

    // REQ-INT-104 and REQ-INT-107.
    #[test]
    fn keys_send_what_the_modes_ask_for() {
        let mut session = welcomed(ALL_KEYS | KEY_RELEASE | BASE_KEY);
        let press = key(Action::Press, KeyCode::Character('a'));
        let release = key(Action::Release, KeyCode::Character('a'));
        assert_eq!(sent_item(&mut session, &press, None), Some(std::vec![0xFF, 0x01, 0x00, b'a']), "no text, no base");
        assert_eq!(sent_item(&mut session, &release, None), None, "no release without KEY_RELEASE");
        assert_eq!(sent_item(&mut session, &Item::Text("a"), None), Some(std::vec![b'a']));
        session.control(&Control::PushMode(ALL_KEYS | KEY_RELEASE | BASE_KEY)).unwrap();
        assert_eq!(sent_item(&mut session, &Item::Text("a"), None), None, "no raw text in ALL_KEYS");
        assert_eq!(
            sent_item(&mut session, &press, None),
            Some(std::vec![0xFF, 0x01, 0x30, b'a', 1, b'a', b'q']),
            "text and base"
        );
        assert!(sent_item(&mut session, &release, None).is_some());
    }

    // REQ-INT-116.
    #[test]
    fn a_dead_key_and_a_modifier_alone_go_only_when_asked_for() {
        let functional = |name: &str| key(Action::Press, KeyCode::Functional(keys::code(name).unwrap()));
        let mut session = welcomed(ALL_KEYS | KEY_RELEASE);
        for quiet in ["Dead", "ShiftLeft", "AltGraph", "CapsLock"] {
            assert_eq!(sent_item(&mut session, &functional(quiet), None), None, "{quiet}");
        }
        assert!(sent_item(&mut session, &functional("Enter"), None).is_some(), "a non-text key always");
        session.control(&Control::PushMode(KEY_RELEASE)).unwrap();
        assert!(sent_item(&mut session, &functional("ShiftLeft"), None).is_some(), "in KEY_RELEASE");
        assert_eq!(sent_item(&mut session, &functional("Dead"), None), None, "the dead key is ALL_KEYS's");
        session.control(&Control::PushMode(ALL_KEYS)).unwrap();
        assert!(sent_item(&mut session, &functional("Dead"), None).is_some());
    }

    // REQ-INT-115.
    #[test]
    fn the_mouse_goes_only_in_a_mouse_mode_or_on_its_page() {
        let mut session = welcomed(MOUSE_BUTTONS | MOUSE_DRAG | MOUSE_PIXELS);
        let at = event::Point { column: 10, row: 5, pixels: Some((120, 100)), mods: 0 };
        let click = Item::Event(Event::Mouse(event::Mouse { action: MouseAction::Press, button: 0, at }));
        let wheel = Item::Event(Event::Wheel(event::Wheel { dx: 0, dy: 120, at }));
        let drag = Item::Event(Event::Mouse(event::Mouse { action: MouseAction::Move, button: 1, at }));
        let hover = Item::Event(Event::Mouse(event::Mouse { action: MouseAction::Move, button: 0, at }));
        assert_eq!(sent_item(&mut session, &click, Some(0x300)), None, "REQ-INT-115: not asked for");
        assert_eq!(sent_item(&mut session, &wheel, Some(0x300)), None);
        session.control(&Control::PageModes { page: 0x300, modes: MOUSE_BUTTONS }).unwrap();
        assert_eq!(sent_item(&mut session, &click, Some(0x301)), None, "another page did not ask");
        assert_eq!(
            sent_item(&mut session, &click, Some(0x300)),
            Some(std::vec![0xFF, 0x02, 0x00, 0x00, 0x0A, 0x05]),
            "the cell alone"
        );
        assert!(sent_item(&mut session, &wheel, Some(0x300)).is_some());
        assert_eq!(sent_item(&mut session, &drag, Some(0x300)), None, "no moves in MOUSE_BUTTONS");
        session.control(&Control::PushMode(MOUSE_DRAG | MOUSE_PIXELS)).unwrap();
        assert!(sent_item(&mut session, &drag, None).is_some(), "a drag");
        assert_eq!(sent_item(&mut session, &hover, None), None, "a move with no button held is MOUSE_ALL's");
        assert_eq!(sent_item(&mut session, &click, None).map(|bytes| bytes.len()), Some(8), "with pixels");
    }

    // REQ-INT-107.
    #[test]
    fn page_goes_only_with_its_capability_and_compose_only_in_its_mode() {
        let mut session = welcomed(PAGE_CAP);
        assert!(sent_item(&mut session, &Item::Event(Event::Page(0x100)), None).is_some());
        let compose = Item::Event(Event::Compose { cursor: 1, text: "か" });
        assert_eq!(sent_item(&mut session, &compose, None), None);
        let mut session = welcomed(COMPOSE);
        assert_eq!(sent_item(&mut session, &Item::Event(Event::Page(0x100)), None), None);
        session.control(&Control::PushMode(COMPOSE)).unwrap();
        assert!(sent_item(&mut session, &compose, None).is_some());
        assert!(sent_item(&mut session, &Item::Event(Event::FocusOut), None).is_some(), "always on");
    }

    // REQ-INT-109.
    #[test]
    fn text_over_the_rate_is_cut_and_counted() {
        let mut session = welcomed(0);
        let long: std::string::String = core::iter::repeat_n('é', 130).collect();
        let sent = sent_item(&mut session, &Item::Text(&long), None).unwrap();
        assert_eq!(sent.len(), 240, "120 characters of 2 bytes");
        assert_eq!(session.count(), 121, "HELLO and 120 characters");
        assert_eq!(sent_item(&mut session, &Item::Event(Event::FocusIn), None), None, "an event over the rate");
    }
}
