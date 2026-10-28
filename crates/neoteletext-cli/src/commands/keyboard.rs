//! `neottx qr` and `neottx keyboard` (REQ-CLI-012, REQ-CLI-013, decision
//! E.94): a QR code as a page's mosaics, and a phone as the keyboard of a
//! channel that takes input, through qrkeyboard's relay.

use super::qr::qr;
use crate::clock::now_ms;
use neoteletext_app::{Failure, exit};
use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::packet::Packet;
use neoteletext_input::event::{Action, Event, Item, Key, KeyCode};
use neoteletext_input::session::{SendError, Session, State};
use neoteletext_input::{MAX_MESSAGE, MAX_ROWS, control, keys};
use neoteletext_qrkeyboard::config::{Layout, ScreenConfig};
use neoteletext_qrkeyboard::net::{self, Incoming, WebSocket};
use neoteletext_qrkeyboard::qr::{Correction, Matrix};
use neoteletext_qrkeyboard::screen::{Event as ScreenEvent, Screen};
use std::time::{Duration, Instant};

/// What the bridge offers in HELLO: it draws no page, no cursor and no
/// echo, so no capability (INPUT.md section 6.2).
const OFFERED: u32 = 0;
/// A dropped phone keeps the channel's screen this long.
const REJOIN_SECONDS: u32 = 30;

/// The options of `neottx keyboard`.
pub struct KeyboardOptions {
    /// The channel's push WebSocket, which takes its input.
    pub channel: String,
    /// The qrkeyboard relay.
    pub relay: String,
    /// Pair with a PIN.
    pub pin: bool,
    /// The screen's name, shown on the phone.
    pub name: String,
    /// The page whose QR to write, and where.
    pub qr_page: Option<(String, std::path::PathBuf)>,
    /// The proxy every connection goes through (REQ-WEB-032).
    pub proxy: Option<String>,
}

/// One input session with the channel (INPUT.md section 6): HELLO, its
/// WELCOME, then the phone's input.
struct Channel {
    socket: WebSocket,
    session: Session,
    started: Instant,
}

impl Channel {
    fn now_ms(&self) -> u32 {
        now_ms(self.started)
    }

    /// Sends HELLO and waits for WELCOME, then says the bridge's size and
    /// focus (section 6.1).
    fn open(socket: WebSocket) -> Result<Self, Failure> {
        let mut channel = Self { socket, session: Session::new(OFFERED), started: Instant::now() };
        let mut out = [0_u8; MAX_MESSAGE];
        let now_ms = channel.now_ms();
        let length =
            channel.session.open(now_ms, None, &mut out).map_err(|error| (exit::INPUT, format!("HELLO: {error:?}")))?;
        channel.write(out.get(..length).ok_or((exit::INPUT, "HELLO: longer than a message".to_owned()))?)?;
        let wait = Duration::from_millis(u64::from(neoteletext_input::WELCOME_TIMEOUT_MS));
        let deadline = Instant::now().checked_add(wait).unwrap_or_else(Instant::now);
        while let State::Waiting(_) = channel.session.state() {
            channel.receive(deadline.saturating_duration_since(Instant::now()))?;
            channel.session.tick(channel.now_ms());
        }
        if !channel.session.welcomed() {
            return Err((exit::INPUT, "the channel did not welcome input".to_owned()));
        }
        // The bridge draws nothing: a teletext page's cells, pixels unknown.
        let columns = u32::try_from(neoteletext_core::consts::COLUMNS).unwrap_or(0);
        channel.send(&Item::Event(Event::Resize { columns, rows: MAX_ROWS, width: 0, height: 0 }))?;
        channel.send(&Item::Event(Event::FocusIn))?;
        Ok(channel)
    }

    fn write(&mut self, message: &[u8]) -> Result<(), Failure> {
        self.socket.send_binary(message).map_err(|error| (exit::INPUT, error.to_string()))
    }

    /// Takes what the channel pushed within `wait`: its control records; the
    /// rest is for its screen, not for this bridge.
    /// Whether a message came.
    fn receive(&mut self, wait: Duration) -> Result<bool, Failure> {
        match self.socket.receive(wait).map_err(|error| (exit::INPUT, error.to_string()))? {
            Some(Incoming::Closed(_)) => Err((exit::INPUT, "the channel closed its socket".to_owned())),
            Some(Incoming::Binary(bytes)) => {
                let (packets, _) = bytes.as_chunks::<PACKET_BYTES>();
                for bytes in packets {
                    let mut buffer = [0; control::FRAGMENT_DATA];
                    if let Some(Ok(control)) = control::from_packet(&Packet { bytes: *bytes }, &mut buffer) {
                        // A refused WELCOME leaves the session refused.
                        let _ = self.session.control(&control);
                    }
                }
                Ok(true)
            }
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    /// Takes everything the channel has pushed, without waiting.
    fn drain(&mut self) -> Result<(), Failure> {
        while self.receive(Duration::ZERO)? {}
        Ok(())
    }

    fn send(&mut self, item: &Item<'_>) -> Result<(), Failure> {
        let mut out = [0_u8; MAX_MESSAGE];
        let now_ms = self.now_ms();
        match crate::nfc::with_nfc(item, |item| self.session.send(item, now_ms, &mut out)) {
            Ok(0) => Ok(()),
            Ok(length) => self.write(out.get(..length).ok_or((exit::INPUT, "input longer than a message".to_owned()))?),
            // A session that keeps nothing is never resumed, so never asks
            // for what it sent to be sent again first.
            Err(SendError::NotWelcomed | SendError::ResendFirst) => {
                Err((exit::INPUT, "the channel does not take input".to_owned()))
            }
            Err(SendError::Input(error)) => Err((exit::INPUT, format!("input not sent: {error:?}"))),
        }
    }

    /// A key the phone pressed or let go, by its W3C name or its character
    /// (INPUT.md sections 5.5 and 6.3): a character as KEY with its text,
    /// any other key as KEY with its functional code.
    fn key(&mut self, name: &str, down: bool) -> Result<(), Failure> {
        let mut characters = name.chars();
        let character = match (characters.next(), characters.next()) {
            (Some(character), None) if neoteletext_input::event::printable(character) => Some(character),
            _ => None,
        };
        let action = if down { Action::Press } else { Action::Release };
        let item = if let Some(character) = character {
            // The phone gives the character typed; its key unshifted.
            let unshifted = character.to_lowercase().next().unwrap_or(character);
            let text = down.then_some(name);
            Item::Event(Event::Key(Key { action, key: KeyCode::Character(unshifted), mods: 0, text, base: None }))
        } else {
            let code = keys::code(name).unwrap_or_else(keys::unidentified);
            Item::Event(Event::Key(Key { action, key: KeyCode::Functional(code), mods: 0, text: None, base: None }))
        };
        self.send(&item)
    }

    /// Text the phone committed, without control characters (section 5.3),
    /// in messages of whole characters.
    fn text(&mut self, text: &str) -> Result<(), Failure> {
        let clean: String = text.chars().filter(|&character| neoteletext_input::event::printable(character)).collect();
        let mut start = 0;
        while start < clean.len() {
            let mut end = start.saturating_add(MAX_MESSAGE).min(clean.len());
            while !clean.is_char_boundary(end) {
                end = end.saturating_sub(1);
            }
            self.send(&Item::Text(clean.get(start..end).unwrap_or_default()))?;
            start = end;
        }
        Ok(())
    }
}

/// The QR as the terminal draws it: two characters a module, dark on light.
fn terminal_qr(matrix: &Matrix) -> String {
    let side = matrix.with_quiet_zone();
    let mut out = String::new();
    for y in 0..side {
        for x in 0..side {
            out.push_str(if matrix.dark_with_quiet_zone(x, y) { "  " } else { "\u{2588}\u{2588}" });
        }
        out.push('\n');
    }
    out
}

/// `neottx keyboard`: a phone through qrkeyboard types into a channel's
/// input socket, as a browser would.
pub fn keyboard(options: &KeyboardOptions) -> Result<(), Failure> {
    let url =
        net::parse(&options.channel).map_err(|_| (exit::USAGE, format!("{}: a ws or wss address", options.channel)))?;
    let proxy = options
        .proxy
        .as_deref()
        .map(neoteletext_web::proxy::Proxy::parse)
        .transpose()
        .map_err(|message| (exit::USAGE, message))?;
    let config = ScreenConfig {
        name: options.name.clone(),
        layout: Layout::Terminal,
        pin: options.pin,
        rejoin_seconds: Some(REJOIN_SECONDS),
    };
    let mut screen = Screen::create(&options.relay, config, proxy.clone())
        .map_err(|error| (exit::INPUT, format!("{}: {error}", options.relay)))?;
    let join = screen.join().clone();
    let matrix = Matrix::new(&join.url, Correction::Medium).map_err(|error| (exit::INPUT, error.to_string()))?;
    println!("{}", terminal_qr(&matrix));
    println!("Scan with your phone camera to type into {}. Anyone who can see this code can type.", options.channel);
    if let Some(pin) = &join.pin {
        println!("PIN: {pin}");
    }
    if let Some((page, output)) = &options.qr_page {
        qr(&join.url, page, 2, 0, output)?;
        println!("The QR is on page {page} in {}.", output.display());
    }
    // The channel's bytes end the screen's wait as the relay's do, so one
    // wait serves both sockets (decision E.106).
    let socket = WebSocket::connect(&url, proxy.as_ref(), Some(screen.waker()))
        .map_err(|error| (exit::INPUT, format!("{}: {error}", options.channel)))?;
    let mut channel = Channel::open(socket)?;
    loop {
        channel.drain()?;
        for event in screen.wait(None).map_err(|error| (exit::INPUT, error.to_string()))? {
            match event {
                ScreenEvent::Connected => println!("Phone connected."),
                ScreenEvent::Disconnected(_) => println!("Phone gone; it may scan again."),
                ScreenEvent::PinFailed(left) => println!("Wrong PIN: {left} tries left."),
                ScreenEvent::Locked => println!("Too many wrong PINs: start again."),
                ScreenEvent::Down(name) => channel.key(&name, true)?,
                ScreenEvent::Up(name) => channel.key(&name, false)?,
                ScreenEvent::Text(text) => channel.text(&text)?,
                ScreenEvent::Special { key, count } => {
                    for _ in 0..count {
                        channel.key(key, true)?;
                        channel.key(key, false)?;
                    }
                }
                ScreenEvent::Finished => {
                    println!("Session finished.");
                    channel.socket.close();
                    return Ok(());
                }
                ScreenEvent::Scan => {}
            }
        }
    }
}
