//! `neottx qr` and `neottx keyboard` (REQ-CLI-012, REQ-CLI-013, decision
//! E.94): a QR code as a page's mosaics, and a phone as the keyboard of a
//! channel that takes input, through qrkeyboard's relay.

use std::fmt::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

use neoteletext_app::exit;
use neoteletext_input::key::{Event, MAX_RECORD, MAX_TEXT, Press, Record};
use neoteletext_input::names::{character_key, named_key};
use neoteletext_qrkeyboard::config::{Layout, ScreenConfig};
use neoteletext_qrkeyboard::net::{self, Incoming, WebSocket};
use neoteletext_qrkeyboard::qr::{Correction, Matrix};
use neoteletext_qrkeyboard::screen::{Event as ScreenEvent, Screen};

// The engine's two names for the input profile's largest message agree.
const _: () = assert!(
    neoteletext_web::consts::INPUT_MESSAGE_MAX == MAX_RECORD && neoteletext_app::pipe::MAX_KEY_RECORD == MAX_RECORD
);

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

/// Numbers and writes key records to the channel.
struct Channel {
    socket: WebSocket,
    sequence: u16,
    started: Instant,
}

impl Channel {
    fn send(&mut self, event: Event<'_>) -> Result<(), Failure> {
        self.sequence = match self.sequence.wrapping_add(1) {
            0 => 1,
            next => next,
        };
        let clock_ms = u32::try_from(self.started.elapsed().as_millis() & u128::from(u32::MAX)).unwrap_or(0);
        let mut out = [0_u8; MAX_RECORD];
        let length = Record { sequence: self.sequence, clock_ms, event }
            .write(&mut out)
            .ok_or((exit::INPUT, "a key record out of range".to_owned()))?;
        self.socket.send_binary(&out[..length]).map_err(|error| (exit::INPUT, error.to_string()))
    }

    fn key(&mut self, name: &str, down: bool) -> Result<(), Failure> {
        let mut characters = name.chars();
        let single = match (characters.next(), characters.next()) {
            (Some(character), None) => character_key(character),
            _ => None,
        };
        let Some(key) = named_key(name).or(single) else { return Ok(()) };
        let press = Press { key, modifiers: 0, physical: 0 };
        self.send(if down { Event::Down(press) } else { Event::Up(press) })
    }

    fn text(&mut self, text: &str) -> Result<(), Failure> {
        let mut start = 0;
        while start < text.len() {
            let mut end = (start + MAX_TEXT).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let piece: String = text[start..end].chars().filter(|character| !character.is_control()).collect();
            start = end;
            if !piece.is_empty() {
                self.send(Event::Text(&piece))?;
            }
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
    let mut channel = Channel { socket, sequence: 0, started: Instant::now() };
    loop {
        // The channel's pushes are for its screen, not for this bridge.
        if let Ok(Some(Incoming::Closed(_))) = channel.socket.receive(Duration::ZERO) {
            return Err((exit::INPUT, "the channel closed its socket".to_owned()));
        }
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
