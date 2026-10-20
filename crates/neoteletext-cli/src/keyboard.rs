//! `neottx qr` and `neottx keyboard` (REQ-CLI-012, REQ-CLI-013, decision
//! E.94): a QR code as a page's mosaics, and a phone as the keyboard of a
//! channel that takes input, through qrkeyboard's relay.

use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use neoteletext_app::exit;
use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::packet::Packet;
use neoteletext_input::event::{Action, Event, Item, Key, KeyCode};
use neoteletext_input::modes::{ALL_KEYS, KEY_RELEASE};
use neoteletext_input::session::{SendError, Session, State};
use neoteletext_input::{MAX_MESSAGE, MAX_ROWS, control, keys};
use neoteletext_qrkeyboard::config::{Layout, ScreenConfig};
use neoteletext_qrkeyboard::net::{self, Incoming, WebSocket};
use neoteletext_qrkeyboard::qr::{Correction, Matrix};
use neoteletext_qrkeyboard::screen::{Event as ScreenEvent, Screen};

// The engine's three names for the input system's largest message agree.
const _: () = assert!(
    neoteletext_web::consts::INPUT_MESSAGE_MAX == MAX_MESSAGE
        && neoteletext_app::pipe::MAX_INPUT_MESSAGE == MAX_MESSAGE
);

// A page's `IM` line takes exactly the modes the input system defines.
const _: () = assert!(neoteletext_formats::page::INPUT_MODE_BITS == neoteletext_input::modes::ALL_MODES);

/// What the bridge offers in HELLO: it draws no page, so only the key
/// modes a phone can give (INPUT.md section 6.2).
const OFFERED: u32 = ALL_KEYS | KEY_RELEASE;

/// TTI writes a code below 0x20 as Escape and the code plus 0x40.
const ESCAPE: char = '\x1b';
const ESCAPE_OFFSET: u8 = 0x40;
/// The last row a page's text may use; row 24 is the Fastext row.
const LAST_ROW: usize = 23;
/// One wait on the relay or the channel.
const POLL: Duration = Duration::from_millis(50);
/// A dropped phone keeps the channel's screen this long.
const REJOIN_SECONDS: u32 = 30;

type Failure = (u8, String);

fn page_number(text: &str) -> Result<u16, Failure> {
    u16::from_str_radix(text, 16)
        .ok()
        .filter(|page| (0x100..=0x8FF).contains(page))
        .ok_or_else(|| (exit::USAGE, format!("{text}: a page is 100 to 8FF")))
}

/// A page whose rows from `row` hold the QR of `text` in mosaics, from
/// `column`, as TTI.
fn qr_page(text: &str, page: u16, row: usize, column: usize) -> Result<String, Failure> {
    let matrix = Matrix::new(text, Correction::Low).map_err(|error| (exit::USAGE, error.to_string()))?;
    let rows = matrix.mosaic_rows();
    let width = rows.first().map_or(0, Vec::len);
    if row.saturating_add(rows.len()).saturating_sub(1) > LAST_ROW
        || column.saturating_add(width) > neoteletext_core::consts::COLUMNS
    {
        return Err((
            exit::USAGE,
            format!("the code is {width} columns by {} rows: it does not fit there", rows.len()),
        ));
    }
    let mut tti = format!("PN,{page:03X}00\r\nSC,0000\r\nPS,8000\r\n");
    for (index, line) in rows.iter().enumerate() {
        let _ = write!(tti, "OL,{},", row + index);
        tti.push_str(&" ".repeat(column));
        for &code in line {
            if code < 0x20 {
                tti.push(ESCAPE);
                tti.push(char::from(code + ESCAPE_OFFSET));
            } else {
                tti.push(char::from(code));
            }
        }
        tti.push_str("\r\n");
    }
    Ok(tti)
}

/// `neottx qr`: writes the QR of `text` as a TTI page.
pub fn qr(text: &str, page: &str, row: usize, column: usize, output: &Path) -> Result<(), Failure> {
    let tti = qr_page(text, page_number(page)?, row, column)?;
    std::fs::write(output, tti).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))
}

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
        u32::try_from(self.started.elapsed().as_millis() & u128::from(u32::MAX)).unwrap_or(0)
    }

    /// Sends HELLO and waits for WELCOME, then says the bridge's size and
    /// focus (section 6.1).
    fn open(socket: WebSocket) -> Result<Self, Failure> {
        let mut channel = Self { socket, session: Session::new(OFFERED), started: Instant::now() };
        let hello = channel.session.open(channel.now_ms(), None);
        let mut out = [0_u8; MAX_MESSAGE];
        let length = neoteletext_input::event::write(&Item::Event(hello), &mut out)
            .map_err(|error| (exit::INPUT, format!("HELLO: {error:?}")))?;
        channel.write(&out[..length])?;
        while let State::Waiting(_) = channel.session.state() {
            channel.receive(POLL)?;
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
    fn receive(&mut self, wait: Duration) -> Result<(), Failure> {
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
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn send(&mut self, item: &Item<'_>) -> Result<(), Failure> {
        let mut out = [0_u8; MAX_MESSAGE];
        match self.session.send(item, None, self.now_ms(), &mut out) {
            Ok(0) => Ok(()),
            Ok(length) => self.write(&out[..length]),
            Err(SendError::NotWelcomed) => Err((exit::INPUT, "the channel does not take input".to_owned())),
            Err(SendError::Input(error)) => Err((exit::INPUT, format!("input not sent: {error:?}"))),
        }
    }

    /// A key the phone pressed or let go, by its W3C name or its character
    /// (section 5.3, 5.5): a character as text, or in `ALL_KEYS` a KEY with
    /// it; any other key as KEY with its functional code.
    fn key(&mut self, name: &str, down: bool) -> Result<(), Failure> {
        let mut characters = name.chars();
        let character = match (characters.next(), characters.next()) {
            (Some(character), None) if neoteletext_input::event::printable(character) => Some(character),
            _ => None,
        };
        let action = if down { Action::Press } else { Action::Release };
        let item = match character {
            Some(_) if down && self.session.modes_on(None) & ALL_KEYS == 0 => Item::Text(name),
            Some(character) => {
                // The phone gives the character typed; its key unshifted.
                let unshifted = character.to_lowercase().next().unwrap_or(character);
                let text = down.then_some(name);
                Item::Event(Event::Key(Key { action, key: KeyCode::Character(unshifted), mods: 0, text, base: None }))
            }
            None => {
                let code = keys::code(name).unwrap_or_else(keys::unidentified);
                Item::Event(Event::Key(Key { action, key: KeyCode::Functional(code), mods: 0, text: None, base: None }))
            }
        };
        self.send(&item)
    }

    /// Text the phone committed, without control characters (section 5.3),
    /// in messages of whole characters.
    fn text(&mut self, text: &str) -> Result<(), Failure> {
        let clean: String = text.chars().filter(|&character| neoteletext_input::event::printable(character)).collect();
        if self.session.modes_on(None) & ALL_KEYS != 0 {
            let mut buffer = [0_u8; 4];
            for character in clean.chars() {
                self.key(character.encode_utf8(&mut buffer), true)?;
            }
            return Ok(());
        }
        let mut start = 0;
        while start < clean.len() {
            let mut end = (start + MAX_MESSAGE).min(clean.len());
            while !clean.is_char_boundary(end) {
                end -= 1;
            }
            self.send(&Item::Text(&clean[start..end]))?;
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
    let config = ScreenConfig {
        name: options.name.clone(),
        layout: Layout::Terminal,
        pin: options.pin,
        rejoin_seconds: Some(REJOIN_SECONDS),
    };
    let mut screen =
        Screen::create(&options.relay, config).map_err(|error| (exit::INPUT, format!("{}: {error}", options.relay)))?;
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
    let socket = WebSocket::connect(&url).map_err(|error| (exit::INPUT, format!("{}: {error}", options.channel)))?;
    let mut channel = Channel::open(socket)?;
    loop {
        channel.receive(Duration::ZERO)?;
        for event in screen.poll(POLL).map_err(|error| (exit::INPUT, error.to_string()))? {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_join_address_is_written_as_a_page() {
        let join = "https://qrkeyboard.com/app#c=ABCDEFGHIJKLMNOP&k=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&h=AAAAAAAAAAAAAAAAAAAAAA";
        let tti = qr_page(join, 0x600, 2, 0).unwrap();
        assert!(tti.starts_with("PN,60000\r\n"));
        // The engine's own reader takes the page back.
        let mut pages = Vec::new();
        let mut reader = neoteletext_formats::tti::TtiReader::new();
        neoteletext_formats::Reader::feed(&mut reader, tti.as_bytes(), &mut |item| {
            if let neoteletext_formats::Item::Page(page) = item {
                pages.push(page.address.mpp());
            }
        });
        neoteletext_formats::Reader::finish(&mut reader, &mut |item| {
            if let neoteletext_formats::Item::Page(page) = item {
                pages.push(page.address.mpp());
            }
        });
        assert_eq!(pages, [0x600]);
        assert!(qr_page(join, 0x600, 20, 0).is_err(), "too low on the page");
    }
}
