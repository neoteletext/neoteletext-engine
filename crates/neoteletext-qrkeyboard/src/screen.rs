//! The screen: it asks the relay for an instance, shows the QR that joins
//! it, pairs with the phone that scans it, and reports the phone's keys,
//! as qrkeyboard's screen SDK does (`v1.ts`), without its browser parts.

use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use neoteletext_web::url::Url;

use crate::config::{CONFIG_HASH_BYTES, ScreenConfig};
use crate::crypto::{Opener, SECRET_BYTES, Sealer};
use crate::json::Value;
use crate::message;
use crate::net::{self, Wake, Waker, WebSocket};
use crate::pairing::{Pairing, Secret};
use crate::{Error, LEAVE};

mod messages;
mod relay;
mod wait;

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
mod tests;
