//! The screen: it asks the relay for an instance, shows the QR that joins
//! it, pairs with the phone that scans it, and reports the phone's keys,
//! as qrkeyboard's screen SDK does (`v1.ts`), without its browser parts.

use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use neoteletext_web::url::Url;

use crate::config::{CONFIG_HASH_BYTES, Layout, ScreenConfig};
use crate::crypto::{Opener, Role, SECRET_BYTES, Sealer};
use crate::json::Value;
use crate::message::{self, FromPhone};
use crate::net::{self, Incoming, Wake, Waker, WebSocket, later};
use crate::pairing::{Outcome, Pairing, Secret};
use crate::{Error, KEEPALIVE, KEEPALIVE_INTERVAL_MS, LEAVE};

/// PIN runs allowed before the screen locks (`MAX_PIN_ATTEMPTS`).
pub const MAX_PIN_ATTEMPTS: u32 = 5;
/// Generated PINs are four digits, 1000 to 9999.
const PIN_FIRST: u32 = 1000;
const PIN_RANGE: u32 = 9000;
/// Acknowledgements kept, so a repeated message is acknowledged again
/// without acting on it twice (`ACKNOWLEDGEMENT_CACHE_SIZE`).
const ACKNOWLEDGED: usize = 256;
/// A held key is released after this long if its "up" never comes
/// (`MAX_HOLD_MS`).
pub const MAX_HOLD: Duration = Duration::from_secs(15);
/// The relay's join codes: 16 symbols of RFC 4648's base-32 alphabet
/// (`CODE_LENGTH`).
const CODE_LENGTH: usize = 16;
/// The longest owner token taken: 32 bytes in base64url are 43 symbols.
const MAX_TOKEN: usize = 64;
/// The relay's close codes that end a session for good: a bad owner token,
/// no such instance, and finished (`CLOSE`).
const CLOSE_AUTH_FAILED: u16 = 4003;
const CLOSE_NO_INSTANCE: u16 = 4004;
const CLOSE_FINISHED: u16 = 4010;
/// Reconnecting after a lost connection waits this long, doubling, up to
/// the maximum (`RECONNECT_BASE_DELAY_MS`, `RECONNECT_MAX_DELAY_MS`).
const RECONNECT_FIRST: Duration = Duration::from_secs(1);
const RECONNECT_MAX: Duration = Duration::from_secs(10);

/// What the screen shows and the phone needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Join {
    /// The instance's join code.
    pub code: String,
    /// The address the QR holds: the relay's phone page, with the code,
    /// the pairing key in no-PIN mode, and the configuration's hash, all
    /// in the fragment, which the phone never sends.
    pub url: String,
    /// The PIN to show beside the QR, in PIN mode.
    pub pin: Option<String>,
}

/// What happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The relay is ready: show the QR.
    Scan,
    /// A phone paired: it is the keyboard now.
    Connected,
    /// A key of the layout went down, by its W3C name.
    Down(String),
    /// A key of the layout came up.
    Up(String),
    /// Text typed on the phone.
    Text(String),
    /// A special key of the phone's text box, by its W3C name, pressed
    /// `count` times.
    Special {
        /// The W3C name.
        key: &'static str,
        /// How many times.
        count: usize,
    },
    /// The phone went away; it may come back within this many
    /// milliseconds and keep the screen.
    Disconnected(Option<u64>),
    /// A PIN run failed; this many remain before the screen locks.
    PinFailed(u32),
    /// Too many wrong PINs: the session must be finished and started again.
    Locked,
    /// The session is over: finished by the phone, the screen or the relay.
    Finished,
}

/// The screen's side of one qrkeyboard instance.
pub struct Screen {
    relay: Url,
    config: ScreenConfig,
    hash: [u8; CONFIG_HASH_BYTES],
    join: Join,
    owner: String,
    socket_address: Url,
    secret: Secret,
    socket: Option<WebSocket>,
    pairing: Option<Pairing>,
    channel: Option<(Sealer, Opener)>,
    pin_attempts: u32,
    locked: bool,
    finished: bool,
    keepalive: Instant,
    reconnect_at: Option<Instant>,
    reconnect_delay: Duration,
    acknowledged: VecDeque<(String, bool)>,
    held: Vec<(String, Instant)>,
    /// Every connection to the relay goes through this proxy (REQ-WEB-032).
    proxy: Option<neoteletext_web::proxy::Proxy>,
    /// What ends a wait: the relay socket's reader and every [`Waker`].
    wakes: mpsc::Receiver<Wake>,
    wake_sender: mpsc::Sender<Wake>,
}

fn is_code(code: &str) -> bool {
    code.len() == CODE_LENGTH && code.bytes().all(|byte| byte.is_ascii_uppercase() || (b'2'..=b'7').contains(&byte))
}

impl Screen {
    /// Asks the relay at `relay` (an `https` origin, or `http` for one on
    /// this computer) for an instance, and connects to it.
    ///
    /// # Errors
    /// A configuration outside the relay's limits, a relay that cannot be
    /// reached, refuses, or answers out of protocol.
    pub fn create(
        relay: &str,
        config: ScreenConfig,
        proxy: Option<neoteletext_web::proxy::Proxy>,
    ) -> Result<Self, Error> {
        config.check()?;
        let relay = net::parse(relay.trim_end_matches('/'))?;
        let origin = relay.as_str().trim_end_matches('/').to_owned();
        let (status, body) = net::request(
            &net::parse(&format!("{origin}/i"))?,
            "POST",
            None,
            Some(&config.value().text()),
            proxy.as_ref(),
        )?;
        if status != neoteletext_web::consts::STATUS_OK {
            return Err(Error::Status(status));
        }
        let answer = Value::parse(&body).map_err(|_| Error::Protocol("the instance is not JSON"))?;
        let code = answer
            .get("code")
            .and_then(Value::as_str)
            .filter(|code| is_code(code))
            .ok_or(Error::Protocol("a join code"))?;
        let owner = answer
            .get("ownerToken")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty() && token.len() <= MAX_TOKEN)
            .ok_or(Error::Protocol("an owner token"))?;
        let socket_address =
            net::parse(answer.get("relay").and_then(Value::as_str).ok_or(Error::Protocol("a relay address"))?)?;
        // The relay names its own WebSocket, on its own host, never another.
        if socket_address.host_text() != relay.host_text() || socket_address.is_secure() != relay.is_secure() {
            return Err(Error::Protocol("a WebSocket on another host"));
        }
        let hash = config.hash();
        let (secret, fragment_key, pin) = if config.pin {
            let mut bytes = [0_u8; 4];
            crate::random(&mut bytes)?;
            let pin = PIN_FIRST.saturating_add(u32::from_be_bytes(bytes) % PIN_RANGE).to_string();
            (Secret::Pin(pin.clone()), String::new(), Some(pin))
        } else {
            let mut key = [0_u8; SECRET_BYTES];
            crate::random(&mut key)?;
            (Secret::Key(key), format!("&k={}", message::base64url(&key)), None)
        };
        let url = format!("{origin}/app#c={code}{fragment_key}&h={}", message::base64url(&hash));
        let join = Join { code: code.to_owned(), url, pin };
        let (wake_sender, wakes) = mpsc::channel();
        let mut screen = Self {
            relay,
            config,
            hash,
            join,
            owner: owner.to_owned(),
            socket_address,
            secret,
            socket: None,
            pairing: None,
            channel: None,
            pin_attempts: 0,
            locked: false,
            finished: false,
            keepalive: Instant::now(),
            reconnect_at: None,
            reconnect_delay: RECONNECT_FIRST,
            acknowledged: VecDeque::new(),
            held: Vec::new(),
            proxy,
            wakes,
            wake_sender,
        };
        screen.connect()?;
        Ok(screen)
    }

    fn connect(&mut self) -> Result<(), Error> {
        let waker = Waker::new(self.wake_sender.clone(), Wake::Network);
        let mut socket = WebSocket::connect(&self.socket_address, self.proxy.as_ref(), Some(waker))?;
        let auth = Value::object(&[("t", Value::string("auth")), ("owner", Value::string(&self.owner))]);
        socket.send_text(&auth.text())?;
        self.socket = Some(socket);
        self.reconnect_at = None;
        Ok(())
    }

    /// The QR's address, the code and the PIN.
    #[must_use]
    pub const fn join(&self) -> &Join {
        &self.join
    }

    /// Whether a phone is paired now.
    #[must_use]
    pub const fn connected(&self) -> bool {
        self.channel.is_some()
    }

    /// A waker that ends this screen's [`wait`](Self::wait) from another
    /// thread, such as one that reads its owner's "finish". A
    /// [`WebSocket`] connected with it ends the wait when its bytes arrive.
    #[must_use]
    pub fn waker(&self) -> Waker {
        Waker::new(self.wake_sender.clone(), Wake::Asked)
    }

    /// Waits until there is something to report, a [`Waker`] wakes it, or
    /// `limit` passes (`None` waits for one of the first two), and acts on
    /// what the relay sends: the keepalive, pairing, the phone's messages
    /// and their acknowledgements, and reconnection. It sleeps on nothing
    /// but its socket, its wakers and its own next deadline (the keepalive,
    /// a reconnection, a held key's release; REQ-INT-009, decision E.106).
    /// A woken wait may return no events.
    ///
    /// # Errors
    /// Only a failure to reconnect is not an error; anything else that
    /// breaks the session is reported as [`Event::Finished`].
    pub fn wait(&mut self, limit: Option<Duration>) -> Result<Vec<Event>, Error> {
        let give_up = limit.map(later);
        let mut events = Vec::new();
        let mut woken = false;
        loop {
            if self.finished {
                return Ok(events);
            }
            self.turn(&mut events);
            let now = Instant::now();
            if !events.is_empty() || woken || give_up.is_some_and(|at| now >= at) {
                return Ok(events);
            }
            let until = [give_up, self.next_deadline()].into_iter().flatten().min();
            let wake = match until {
                Some(at) => match self.wakes.recv_timeout(at.saturating_duration_since(now)) {
                    Ok(wake) => Some(wake),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    // The screen holds a sender: never.
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(events),
                },
                None => match self.wakes.recv() {
                    Ok(wake) => Some(wake),
                    Err(mpsc::RecvError) => return Ok(events),
                },
            };
            // Every wake already sent is answered by this turn.
            woken |= wake == Some(Wake::Asked);
            while let Ok(wake) = self.wakes.try_recv() {
                woken |= wake == Wake::Asked;
            }
        }
    }

    /// The soonest moment this screen has work without the relay: the
    /// keepalive while paired, a reconnection, and a held key's release.
    fn next_deadline(&self) -> Option<Instant> {
        let keepalive = self
            .channel
            .as_ref()
            .and_then(|_| self.keepalive.checked_add(Duration::from_millis(KEEPALIVE_INTERVAL_MS)));
        let reconnect = if self.socket.is_none() { Some(self.reconnect_at.unwrap_or_else(Instant::now)) } else { None };
        let release = self.held.iter().filter_map(|(_, since)| since.checked_add(MAX_HOLD)).min();
        [keepalive, reconnect, release].into_iter().flatten().min()
    }

    /// One turn without waiting: what is due, and everything the relay has
    /// sent.
    fn turn(&mut self, events: &mut Vec<Event>) {
        self.release_stale(events);
        if self.socket.is_none() {
            if self.reconnect_at.is_some_and(|at| Instant::now() < at) {
                return;
            }
            if self.connect().is_err() {
                self.reconnect_delay = self.reconnect_delay.saturating_mul(2).min(RECONNECT_MAX);
                self.reconnect_at = Some(later(self.reconnect_delay));
                return;
            }
        }
        if self.channel.is_some() && self.keepalive.elapsed() >= Duration::from_millis(KEEPALIVE_INTERVAL_MS) {
            self.keepalive = Instant::now();
            self.send_text(KEEPALIVE);
        }
        while !self.finished {
            let incoming = match self.socket.as_mut().map(|socket| socket.receive(Duration::ZERO)) {
                Some(Ok(Some(incoming))) => incoming,
                Some(Ok(None)) | None => return,
                Some(Err(_)) => Incoming::Closed(None),
            };
            match incoming {
                Incoming::Text(text) => self.control(&text, events),
                Incoming::Binary(frame) => self.binary(&frame, events),
                Incoming::Closed(code) => {
                    self.socket = None;
                    self.clear_channel(events);
                    if matches!(code, Some(CLOSE_AUTH_FAILED | CLOSE_NO_INSTANCE | CLOSE_FINISHED)) {
                        self.finished = true;
                        events.push(Event::Finished);
                    } else {
                        self.reconnect_at = Some(later(self.reconnect_delay));
                    }
                    return;
                }
            }
        }
    }

    fn send_text(&mut self, text: &str) {
        if let Some(socket) = &mut self.socket
            && socket.send_text(text).is_err()
        {
            self.socket = None;
        }
    }

    fn send_binary(&mut self, frame: &[u8]) {
        if let Some(socket) = &mut self.socket
            && socket.send_binary(frame).is_err()
        {
            self.socket = None;
        }
    }

    /// Text from the relay: `ready`, and `peer` up or down. Anything else
    /// is ignored, so the relay can add messages ahead of screens.
    fn control(&mut self, text: &str, events: &mut Vec<Event>) {
        if text == KEEPALIVE {
            return;
        }
        let Ok(message) = Value::parse(text.as_bytes()) else { return };
        match message.get("type").and_then(Value::as_str) {
            Some("ready") => {
                self.reconnect_delay = RECONNECT_FIRST;
                if self.channel.is_none() && !self.locked {
                    events.push(Event::Scan);
                }
            }
            Some("peer") if message.get("up").and_then(Value::as_bool) == Some(true) => self.peer_up(events),
            Some("peer") => {
                let abandoned = self.pairing.as_ref().is_some_and(Pairing::uses_pin);
                self.clear_channel(events);
                if abandoned {
                    self.pin_failed(events);
                }
                events.push(Event::Disconnected(message.get("reservedForMs").and_then(Value::as_u64)));
            }
            _ => {}
        }
    }

    fn peer_up(&mut self, events: &mut Vec<Event>) {
        self.clear_channel(events);
        if self.locked {
            return;
        }
        if self.config.pin {
            // Counted as the share goes: a phone that abandons the run has
            // still tested one PIN.
            if self.pin_attempts >= MAX_PIN_ATTEMPTS {
                self.lock(events);
                return;
            }
            self.pin_attempts = self.pin_attempts.saturating_add(1);
        }
        let Ok(pairing) = Pairing::new(Role::Device, &self.join.code, &self.secret, &self.hash) else { return };
        for frame in pairing.start() {
            self.send_binary(&frame);
        }
        self.pairing = Some(pairing);
    }

    fn pin_failed(&mut self, events: &mut Vec<Event>) {
        events.push(Event::PinFailed(MAX_PIN_ATTEMPTS.saturating_sub(self.pin_attempts)));
        if self.pin_attempts >= MAX_PIN_ATTEMPTS {
            self.lock(events);
        }
    }

    fn lock(&mut self, events: &mut Vec<Event>) {
        if !self.locked {
            self.locked = true;
            events.push(Event::Locked);
        }
    }

    fn clear_channel(&mut self, events: &mut Vec<Event>) {
        for (key, _) in core::mem::take(&mut self.held) {
            events.push(Event::Up(key));
        }
        self.pairing = None;
        self.channel = None;
    }

    fn binary(&mut self, frame: &[u8], events: &mut Vec<Event>) {
        if let Some(pairing) = &mut self.pairing {
            let (out, outcome) = pairing.receive(frame);
            for frame in out {
                self.send_binary(&frame);
            }
            match outcome {
                None => {}
                Some(Outcome::WrongPin) => {
                    self.pairing = None;
                    self.pin_failed(events);
                }
                Some(Outcome::Paired(channel)) => {
                    self.pairing = None;
                    self.channel = Some(*channel);
                    self.pin_attempts = 0;
                    self.keepalive = Instant::now();
                    events.push(Event::Connected);
                }
            }
            return;
        }
        let Some(plaintext) = self.channel.as_mut().and_then(|(_, opener)| opener.open(frame)) else { return };
        let Some(message) = message::read(&plaintext) else { return };
        if let Some((_, ok)) = self.acknowledged.iter().find(|(id, _)| *id == message.id).cloned() {
            self.acknowledge(&message.id, ok);
            return;
        }
        let ok = match message.body {
            FromPhone::Finish => {
                let _ = self.finish();
                events.push(Event::Finished);
                return;
            }
            FromPhone::Text(text) if self.has_text() => {
                events.push(Event::Text(text));
                true
            }
            FromPhone::Special { key, count } if self.has_text() => {
                events.push(Event::Special { key, count });
                true
            }
            FromPhone::Down(key) if self.has_key(&key) => {
                self.held.retain(|(held, _)| *held != key);
                self.held.push((key.clone(), Instant::now()));
                events.push(Event::Down(key));
                true
            }
            FromPhone::Up(key) if self.has_key(&key) => {
                self.held.retain(|(held, _)| *held != key);
                events.push(Event::Up(key));
                true
            }
            // Keys outside the layout are ignored, as the SDK ignores them.
            FromPhone::Down(_) | FromPhone::Up(_) => return,
            FromPhone::Text(_) | FromPhone::Special { .. } | FromPhone::Other => false,
        };
        self.acknowledged.push_back((message.id.clone(), ok));
        if self.acknowledged.len() > ACKNOWLEDGED {
            self.acknowledged.pop_front();
        }
        self.acknowledge(&message.id, ok);
    }

    fn acknowledge(&mut self, id: &str, ok: bool) {
        let Ok(plaintext) = message::pack(&message::ack(id, ok)) else { return };
        let Some(frame) = self.channel.as_mut().and_then(|(sealer, _)| sealer.seal(&plaintext)) else { return };
        self.send_binary(&frame);
    }

    fn has_text(&self) -> bool {
        matches!(self.config.layout, Layout::Terminal | Layout::Preset("keyboard"))
    }

    /// Whether `key` is a key of the layout (`controls.ts` `PRESETS`).
    fn has_key(&self, key: &str) -> bool {
        const DPAD: [&str; 5] = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Enter"];
        const COLOURS: [&str; 4] = ["ColorF0Red", "ColorF1Green", "ColorF2Yellow", "ColorF3Blue"];
        const TELETEXT: [&str; 16] = [
            "1",
            "2",
            "3",
            "4",
            "5",
            "6",
            "7",
            "8",
            "9",
            "0",
            "Teletext",
            "Exit",
            "ArrowLeft",
            "ChannelDown",
            "ChannelUp",
            "ArrowRight",
        ];
        const REMOTE: [&str; 9] = [
            "BrowserBack",
            "BrowserHome",
            "ContextMenu",
            "AudioVolumeDown",
            "AudioVolumeMute",
            "AudioVolumeUp",
            "ChannelDown",
            "MediaPlayPause",
            "ChannelUp",
        ];
        const GAMEPAD: [&str; 2] = ["a", "b"];
        match self.config.layout {
            Layout::Terminal => DPAD.contains(&key) || COLOURS.contains(&key),
            Layout::Preset("teletext") => COLOURS.contains(&key) || TELETEXT.contains(&key),
            Layout::Preset("remote") => DPAD.contains(&key) || REMOTE.contains(&key) || COLOURS.contains(&key),
            Layout::Preset("gamepad") => DPAD.contains(&key) || GAMEPAD.contains(&key),
            Layout::Preset(_) => false,
        }
    }

    /// Releases keys held past [`MAX_HOLD`], whose "up" was lost.
    fn release_stale(&mut self, events: &mut Vec<Event>) {
        let (stale, held): (Vec<_>, Vec<_>) =
            core::mem::take(&mut self.held).into_iter().partition(|(_, since)| since.elapsed() >= MAX_HOLD);
        self.held = held;
        events.extend(stale.into_iter().map(|(key, _)| Event::Up(key)));
    }

    /// Ends the instance: the QR and the phone's credential stop working.
    ///
    /// # Errors
    /// When the relay cannot be told; the session is closed here anyway.
    pub fn finish(&mut self) -> Result<(), Error> {
        self.finished = true;
        if let Some(mut socket) = self.socket.take() {
            socket.close();
        }
        self.channel = None;
        self.pairing = None;
        let origin = self.relay.as_str().trim_end_matches('/');
        let end = net::parse(&format!("{origin}/i/{}/end", self.join.code))?;
        let (status, _) = net::request(&end, "POST", Some(&self.owner), None, self.proxy.as_ref())?;
        if status == neoteletext_web::consts::STATUS_OK || status == neoteletext_web::consts::STATUS_NOT_FOUND {
            Ok(())
        } else {
            Err(Error::Status(status))
        }
    }

    /// Leaves for good, as a closing screen does: the relay ends the
    /// session and asks the phone to scan again.
    pub fn close(mut self) {
        self.send_text(LEAVE);
        if let Some(mut socket) = self.socket.take() {
            socket.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_codes_are_the_relays() {
        assert!(is_code("ABCDEFGHIJKLMN27"));
        assert!(!is_code("ABCDEFGHIJKLMN28"), "8 is not base-32");
        assert!(!is_code("abcdefghijklmn27"));
        assert!(!is_code("ABC"));
    }
}
