//! A receiver's side of one input session (`neoteletext-common/INPUT.md`
//! sections 6 to 8): HELLO and WELCOME, the modes, the input count the
//! channel acknowledges, the round trip it gives, and the rate limit.
//! Every receiver uses it alike: NeoTeletext's trusted core, `neottx view`,
//! `neottx keyboard`, a microcontroller.

use crate::control::Control;
use crate::event::{Event, InputError, Item, write};
use crate::modes::{AGENT, ALL_CAPS, Modes};
use crate::{VERSION, WELCOME_TIMEOUT_MS};
mod count;
mod filter;
mod limiter;

pub use count::REMEMBERED;
pub use limiter::Limiter;

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

    /// The input count sent so far.
    #[must_use]
    pub const fn count(&self) -> u16 {
        self.count
    }

    /// The last acknowledgement taken: echo up to it is answered.
    #[must_use]
    pub const fn acked(&self) -> u16 {
        self.acked
    }
}

#[cfg(test)]
mod tests;
