//! The framed messages between the fetcher and decoder processes
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md), and between the
//! browser's interface process and both (`docs/desktop-browser.md`
//! section 3.4). Each frame is a kind byte, a 32-bit big-endian length and
//! the payload. Both sides bound the length, parse strictly and reject
//! unknown kinds.

use std::io::{self, Read, Write};

/// The largest payload: one page response (REQ-WEB-008 default).
pub const MAX_FRAME: usize = 256 * 1024;
/// The widest screen a decoder may send: two pages side by side for the
/// glyph comparison, each with both side panels, plus the gap.
pub const MAX_SCREEN_WIDTH: usize = 2048;
/// The tallest screen a decoder may send: 25 rows of 20 lines, with room.
pub const MAX_SCREEN_HEIGHT: usize = 1024;
/// The most palette entries a screen may carry.
pub const MAX_SCREEN_COLOURS: usize = 256;
/// The largest screen payload: the indices, the palette and the sizes.
const MAX_SCREEN_FRAME: usize = MAX_SCREEN_WIDTH * MAX_SCREEN_HEIGHT + MAX_SCREEN_COLOURS * 4 + 16;
/// The longest text of a screen: 25 rows of 40 characters of up to four
/// bytes of UTF-8 each, and their line ends.
pub const MAX_SCREEN_TEXT: usize = 25 * (40 * 4 + 1);

/// Where page bytes came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// A page fetch.
    Page,
    /// The push connection.
    Push,
    /// The response to a form submission (REQ-WEB-011).
    Form,
    /// The front door of a site without a descriptor: any number of the
    /// site's pages.
    FrontDoor,
}

/// Page bytes from the fetcher.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageData {
    /// Came over an encrypted connection.
    pub secure: bool,
    /// Where the bytes came from.
    pub origin: Origin,
    /// The address they came from: the base and origin of a form on them.
    pub url: String,
    /// TTI or T42 bytes.
    pub bytes: Vec<u8>,
}

/// A form submission for the fetcher (REQ-WEB-011).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Submission {
    /// The page's address.
    pub page_url: String,
    /// True for POST, false for GET.
    pub post: bool,
    /// The `FS` address.
    pub target: String,
    /// The `FT` token.
    pub token: Option<String>,
    /// The pressed button's action.
    pub action: Option<String>,
    /// Field names and values.
    pub fields: Vec<(String, String)>,
}

/// A message on the pipe.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Frame {
    /// Fetcher to decoder: one 42-byte packet.
    Packet([u8; neoteletext_core::consts::PACKET_BYTES]),
    /// Fetcher to decoder: TTI page bytes.
    Tti(PageData),
    /// Fetcher to decoder: T42 page bytes.
    T42(PageData),
    /// Fetcher to decoder: a notice for the viewer.
    Notice(Notice),
    /// Decoder to fetcher: request a page (`0xMPP`, sub-code or 0xFFFF).
    Request(u16, u16),
    /// Decoder to fetcher: submit a form.
    Submit(Submission),
    /// Decoder to fetcher: the viewer's answer to a confirmation.
    Confirm(bool),
    /// Decoder to fetcher: follow a link to another service.
    Follow(String, u16, u16),
    /// Interface to decoder: a key or typed character.
    Input(UserInput),
    /// Interface to decoder: show this page (`0xMPP`, sub-code or 0xFFFF),
    /// for the browser's own back and forward.
    Show(u16, u16),
    /// Interface to decoder: the display settings.
    Settings(DisplaySettings),
    /// Interface to decoder: the pointer pressed this cell (row, column),
    /// for page catching and the Fastext prompts.
    Click(u8, u8),
    /// Decoder to interface: the rendered page.
    Screen(Screen),
    /// Decoder to interface: what the page shows and where the viewer is.
    Status(PageStatus),
}

/// A key or character from the interface. Which of them the decoder acts
/// on depends on whether a form has the focus, as in `ttx view`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UserInput {
    /// A digit key, 0 to 9.
    Digit(u8),
    /// The red, green, yellow and cyan keys, 0 to 3.
    Colour(u8),
    /// Index.
    Index,
    /// The next page.
    NextPage,
    /// The previous page.
    PreviousPage,
    /// The next sub-page.
    NextSubPage,
    /// The previous sub-page.
    PreviousSubPage,
    /// Reveal.
    Reveal,
    /// Hold.
    Hold,
    /// Mix.
    Mix,
    /// Zoom: top half, bottom half, off.
    Zoom,
    /// Tab: into or around a form.
    Tab,
    /// Shift-Tab.
    BackTab,
    /// Enter.
    Enter,
    /// Backspace in a form.
    Backspace,
    /// Escape: leaves a form.
    Escape,
    /// The arrow keys in a form: left, right, up, down.
    Arrow(u8),
    /// A typed character for a form.
    Char(char),
    /// The site's initial page.
    Home,
}

/// What the interface asks the decoder to draw with.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DisplaySettings {
    /// The glyphs: 0 original, 1 Bedstead, 2 both side by side.
    pub glyphs: u8,
}

/// A rendered page: palette indices row by row and the palette, opaque
/// red, green, blue and alpha.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Screen {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// The palette.
    pub palette: Vec<[u8; 4]>,
    /// `width` times `height` indices into the palette.
    pub indices: Vec<u8>,
}

/// What the decoder reports about the page it shows.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageStatus {
    /// The page shown or searched for, `0xMPP`.
    pub page: u16,
    /// Its sub-code, or 0xFFFF.
    pub subcode: u16,
    /// The page is on screen, not being searched for.
    pub found: bool,
    /// Row 0 columns 0 to 7 as the viewer would show them, 7-bit codes.
    pub status: [u8; 8],
    /// A form on the page has the focus.
    pub form: bool,
    /// The page's text, one line per row, for accessibility, copying and
    /// tests.
    pub text: String,
}

/// Notices the fetcher sends.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notice {
    /// The service's initial page, `0xMPP`.
    InitialPage(u16),
    /// The page on screen came over plain HTTP (REQ-SEC-008).
    Unencrypted,
    /// A message for the user.
    Message(String),
    /// A new service is ready; its linked services' short names in
    /// descriptor order (Appendix E).
    Service(Vec<String>),
    /// Confirm a submission (REQ-SEC-008, REQ-SEC-009): host, plain HTTP,
    /// first submission to this origin.
    Confirm(String, bool, bool),
    /// A submission finished: true for success.
    Submitted(bool),
    /// Show this page, `0xMPP`: a form response carried it (REQ-WEB-011).
    ShowPage(u16),
    /// The server confirmed page `0xMPP` unchanged (REQ-WEB-004): the
    /// stored copy is current (REQ-WEB-009).
    Current(u16),
    /// Fetching page `0xMPP` failed for want of a network (REQ-WEB-009).
    Offline(u16),
    /// The descriptor's linked services: short name and descriptor
    /// address, for the browser's tuning screen.
    LinkedSites(Vec<(String, String)>),
    /// A request was redirected to another site and not followed: its
    /// address.
    RedirectToOtherSite(String),
    /// The service's name, from its descriptor or its host.
    ServiceName(String),
    /// A bundle of pages arrived (a file, or a site's front door); its
    /// first page, `0xMPP`, is shown if the page asked for is not in it
    /// and the user has not moved.
    Bundle(u16),
}

const PACKET: u8 = b'P';
const TTI: u8 = b'T';
const T42: u8 = b'B';
const INITIAL: u8 = b'I';
const UNENCRYPTED: u8 = b'U';
const MESSAGE: u8 = b'N';
const SERVICE: u8 = b'S';
const CONFIRM_ASK: u8 = b'A';
const SUBMITTED: u8 = b'D';
const SHOW: u8 = b'G';
const CURRENT: u8 = b'K';
const OFFLINE: u8 = b'O';
const REQUEST: u8 = b'R';
const SUBMIT: u8 = b'F';
const CONFIRM: u8 = b'C';
const FOLLOW: u8 = b'L';
const INPUT: u8 = b'k';
const SHOW_PAGE: u8 = b'V';
const SETTINGS: u8 = b'Z';
const CLICK: u8 = b'Q';
const SCREEN: u8 = b'Y';
const STATUS: u8 = b'X';
const LINKED_SITES: u8 = b'J';
const REDIRECT: u8 = b'W';
const SERVICE_NAME: u8 = b'M';
const BUNDLE: u8 = b'b';
/// The most strings one frame may carry.
const MAX_STRINGS: usize = 256;

fn put(out: &mut dyn Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    put_limited(out, kind, payload, MAX_FRAME)
}

fn put_limited(out: &mut dyn Write, kind: u8, payload: &[u8], limit: usize) -> io::Result<()> {
    let length = u32::try_from(payload.len()).map_err(|_| io::Error::other("frame too long"))?;
    if payload.len() > limit {
        return Err(io::Error::other("frame longer than the limit"));
    }
    out.write_all(&[kind])?;
    out.write_all(&length.to_be_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

fn put_string(buffer: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    let length = u32::try_from(bytes.len()).unwrap_or(0);
    buffer.extend_from_slice(&length.to_be_bytes());
    buffer.extend_from_slice(&bytes[..length as usize]);
}

fn put_option(buffer: &mut Vec<u8>, text: Option<&String>) {
    buffer.push(u8::from(text.is_some()));
    put_string(buffer, text.map_or("", String::as_str));
}

/// A strict reader over a payload.
struct Cursor<'a>(&'a [u8]);

impl Cursor<'_> {
    fn bytes(&mut self, count: usize) -> io::Result<&[u8]> {
        if count > self.0.len() {
            return Err(io::Error::other("malformed frame"));
        }
        let (head, rest) = self.0.split_at(count);
        self.0 = rest;
        Ok(head)
    }

    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    fn u16(&mut self) -> io::Result<u16> {
        let bytes = self.bytes(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> io::Result<usize> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    }

    fn string(&mut self) -> io::Result<String> {
        let length = self.u32()?;
        let bytes = self.bytes(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| io::Error::other("malformed frame"))
    }

    fn option(&mut self) -> io::Result<Option<String>> {
        let present = self.byte()? != 0;
        let text = self.string()?;
        Ok(present.then_some(text))
    }

    fn end(&self) -> io::Result<()> {
        if self.0.is_empty() { Ok(()) } else { Err(io::Error::other("malformed frame")) }
    }
}

fn page_payload(page: &PageData) -> Vec<u8> {
    let mut buffer = vec![
        u8::from(page.secure),
        match page.origin {
            Origin::Page => 0,
            Origin::Push => 1,
            Origin::Form => 2,
            Origin::FrontDoor => 3,
        },
    ];
    put_string(&mut buffer, &page.url);
    buffer.extend_from_slice(&page.bytes);
    buffer
}

fn page_data(payload: &[u8]) -> io::Result<PageData> {
    let mut cursor = Cursor(payload);
    let secure = cursor.byte()? != 0;
    let origin = match cursor.byte()? {
        0 => Origin::Page,
        1 => Origin::Push,
        2 => Origin::Form,
        3 => Origin::FrontDoor,
        _ => return Err(io::Error::other("malformed frame")),
    };
    let url = cursor.string()?;
    Ok(PageData { secure, origin, url, bytes: cursor.0.to_vec() })
}

/// Writes one frame.
pub fn write(out: &mut dyn Write, frame: &Frame) -> io::Result<()> {
    match frame {
        Frame::Packet(bytes) => put(out, PACKET, bytes),
        Frame::Tti(page) => put(out, TTI, &page_payload(page)),
        Frame::T42(page) => put(out, T42, &page_payload(page)),
        Frame::Notice(Notice::InitialPage(mpp)) => put(out, INITIAL, &mpp.to_be_bytes()),
        Frame::Notice(Notice::Unencrypted) => put(out, UNENCRYPTED, &[]),
        Frame::Notice(Notice::Message(text)) => put(out, MESSAGE, text.as_bytes()),
        Frame::Notice(Notice::Service(names)) => {
            let mut buffer = Vec::new();
            buffer.extend_from_slice(&u16::try_from(names.len().min(MAX_STRINGS)).unwrap_or(0).to_be_bytes());
            for name in names.iter().take(MAX_STRINGS) {
                put_string(&mut buffer, name);
            }
            put(out, SERVICE, &buffer)
        }
        Frame::Notice(Notice::Confirm(host, plain, first)) => {
            let mut buffer = vec![u8::from(*plain), u8::from(*first)];
            put_string(&mut buffer, host);
            put(out, CONFIRM_ASK, &buffer)
        }
        Frame::Notice(Notice::Submitted(ok)) => put(out, SUBMITTED, &[u8::from(*ok)]),
        Frame::Notice(Notice::ShowPage(mpp)) => put(out, SHOW, &mpp.to_be_bytes()),
        Frame::Notice(Notice::Current(mpp)) => put(out, CURRENT, &mpp.to_be_bytes()),
        Frame::Notice(Notice::Offline(mpp)) => put(out, OFFLINE, &mpp.to_be_bytes()),
        Frame::Request(mpp, sub) => put(out, REQUEST, &[mpp.to_be_bytes(), sub.to_be_bytes()].concat()),
        Frame::Submit(submission) => {
            let mut buffer = vec![u8::from(submission.post)];
            put_string(&mut buffer, &submission.page_url);
            put_string(&mut buffer, &submission.target);
            put_option(&mut buffer, submission.token.as_ref());
            put_option(&mut buffer, submission.action.as_ref());
            buffer
                .extend_from_slice(&u16::try_from(submission.fields.len().min(MAX_STRINGS)).unwrap_or(0).to_be_bytes());
            for (name, value) in submission.fields.iter().take(MAX_STRINGS) {
                put_string(&mut buffer, name);
                put_string(&mut buffer, value);
            }
            put(out, SUBMIT, &buffer)
        }
        Frame::Confirm(yes) => put(out, CONFIRM, &[u8::from(*yes)]),
        Frame::Follow(service, mpp, sub) => {
            let mut buffer = [mpp.to_be_bytes(), sub.to_be_bytes()].concat();
            put_string(&mut buffer, service);
            put(out, FOLLOW, &buffer)
        }
        Frame::Notice(Notice::LinkedSites(sites)) => {
            let mut buffer = Vec::new();
            buffer.extend_from_slice(&u16::try_from(sites.len().min(MAX_STRINGS)).unwrap_or(0).to_be_bytes());
            for (name, address) in sites.iter().take(MAX_STRINGS) {
                put_string(&mut buffer, name);
                put_string(&mut buffer, address);
            }
            put(out, LINKED_SITES, &buffer)
        }
        Frame::Notice(Notice::RedirectToOtherSite(address)) => put(out, REDIRECT, address.as_bytes()),
        Frame::Notice(Notice::ServiceName(name)) => put(out, SERVICE_NAME, name.as_bytes()),
        Frame::Notice(Notice::Bundle(mpp)) => put(out, BUNDLE, &mpp.to_be_bytes()),
        Frame::Input(input) => {
            let (code, value) = input_code(*input);
            let mut buffer = vec![code];
            buffer.extend_from_slice(&value.to_be_bytes());
            put(out, INPUT, &buffer)
        }
        Frame::Show(mpp, sub) => put(out, SHOW_PAGE, &[mpp.to_be_bytes(), sub.to_be_bytes()].concat()),
        Frame::Settings(settings) => put(out, SETTINGS, &[settings.glyphs]),
        Frame::Click(row, column) => put(out, CLICK, &[*row, *column]),
        Frame::Screen(screen) => {
            let mut buffer = Vec::with_capacity(screen.indices.len() + screen.palette.len() * 4 + 6);
            buffer.extend_from_slice(&screen.width.to_be_bytes());
            buffer.extend_from_slice(&screen.height.to_be_bytes());
            buffer.extend_from_slice(&u16::try_from(screen.palette.len()).unwrap_or(u16::MAX).to_be_bytes());
            for colour in &screen.palette {
                buffer.extend_from_slice(colour);
            }
            buffer.extend_from_slice(&screen.indices);
            put_limited(out, SCREEN, &buffer, MAX_SCREEN_FRAME)
        }
        Frame::Status(status) => {
            let mut buffer = [status.page.to_be_bytes(), status.subcode.to_be_bytes()].concat();
            buffer.push(u8::from(status.found));
            buffer.extend_from_slice(&status.status);
            buffer.push(u8::from(status.form));
            put_string(&mut buffer, &status.text);
            put(out, STATUS, &buffer)
        }
    }
}

/// The wire form of a user input: a code and a value.
fn input_code(input: UserInput) -> (u8, u32) {
    match input {
        UserInput::Digit(digit) => (0, u32::from(digit)),
        UserInput::Colour(colour) => (1, u32::from(colour)),
        UserInput::Index => (2, 0),
        UserInput::NextPage => (3, 0),
        UserInput::PreviousPage => (4, 0),
        UserInput::NextSubPage => (5, 0),
        UserInput::PreviousSubPage => (6, 0),
        UserInput::Reveal => (7, 0),
        UserInput::Hold => (8, 0),
        UserInput::Mix => (9, 0),
        UserInput::Zoom => (10, 0),
        UserInput::Tab => (11, 0),
        UserInput::BackTab => (12, 0),
        UserInput::Enter => (13, 0),
        UserInput::Backspace => (14, 0),
        UserInput::Escape => (15, 0),
        UserInput::Arrow(direction) => (16, u32::from(direction)),
        UserInput::Char(character) => (17, u32::from(character)),
        UserInput::Home => (18, 0),
    }
}

/// A user input from its wire form; `None` for anything out of range.
fn user_input(code: u8, value: u32) -> Option<UserInput> {
    let small = u8::try_from(value).ok();
    Some(match code {
        0 => UserInput::Digit(small.filter(|digit| *digit <= 9)?),
        1 => UserInput::Colour(small.filter(|colour| *colour <= 3)?),
        2 => UserInput::Index,
        3 => UserInput::NextPage,
        4 => UserInput::PreviousPage,
        5 => UserInput::NextSubPage,
        6 => UserInput::PreviousSubPage,
        7 => UserInput::Reveal,
        8 => UserInput::Hold,
        9 => UserInput::Mix,
        10 => UserInput::Zoom,
        11 => UserInput::Tab,
        12 => UserInput::BackTab,
        13 => UserInput::Enter,
        14 => UserInput::Backspace,
        15 => UserInput::Escape,
        16 => UserInput::Arrow(small.filter(|direction| *direction <= 3)?),
        17 => UserInput::Char(char::from_u32(value).filter(|character| !character.is_control())?),
        18 => UserInput::Home,
        _ => return None,
    })
}

fn screen(payload: &[u8]) -> io::Result<Screen> {
    let invalid = || io::Error::other("malformed frame");
    let mut cursor = Cursor(payload);
    let width = cursor.u16()?;
    let height = cursor.u16()?;
    let colours = usize::from(cursor.u16()?);
    if usize::from(width) > MAX_SCREEN_WIDTH || usize::from(height) > MAX_SCREEN_HEIGHT || colours > MAX_SCREEN_COLOURS
    {
        return Err(invalid());
    }
    let palette: Vec<[u8; 4]> = cursor.bytes(colours * 4)?.as_chunks::<4>().0.to_vec();
    let indices = cursor.bytes(usize::from(width) * usize::from(height))?.to_vec();
    cursor.end()?;
    // Every index must name a colour the palette has.
    if indices.iter().any(|&index| usize::from(index) >= colours) {
        return Err(invalid());
    }
    Ok(Screen { width, height, palette, indices })
}

/// Reads one frame; `None` at the end of the stream.
pub fn read(input: &mut dyn Read) -> io::Result<Option<Frame>> {
    let mut head = [0u8; 5];
    match input.read_exact(&mut head) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = u32::from_be_bytes([head[1], head[2], head[3], head[4]]) as usize;
    let limit = if head[0] == SCREEN { MAX_SCREEN_FRAME } else { MAX_FRAME };
    if length > limit {
        return Err(io::Error::other("frame longer than the limit"));
    }
    let mut payload = vec![0u8; length];
    input.read_exact(&mut payload)?;
    let invalid = || io::Error::other("malformed frame");
    let mut cursor = Cursor(&payload);
    let frame = match head[0] {
        PACKET => Frame::Packet(payload.as_slice().try_into().map_err(|_| invalid())?),
        TTI => Frame::Tti(page_data(&payload)?),
        T42 => Frame::T42(page_data(&payload)?),
        INITIAL => Frame::Notice(Notice::InitialPage(u16::from_be_bytes(
            payload.as_slice().try_into().map_err(|_| invalid())?,
        ))),
        UNENCRYPTED if payload.is_empty() => Frame::Notice(Notice::Unencrypted),
        MESSAGE => Frame::Notice(Notice::Message(String::from_utf8_lossy(&payload).into_owned())),
        SERVICE => {
            let count = usize::from(cursor.u16()?);
            let names = (0..count.min(MAX_STRINGS)).map(|_| cursor.string()).collect::<io::Result<Vec<_>>>()?;
            cursor.end()?;
            Frame::Notice(Notice::Service(names))
        }
        CONFIRM_ASK => {
            let plain = cursor.byte()? != 0;
            let first = cursor.byte()? != 0;
            let host = cursor.string()?;
            cursor.end()?;
            Frame::Notice(Notice::Confirm(host, plain, first))
        }
        SUBMITTED if payload.len() == 1 => Frame::Notice(Notice::Submitted(payload[0] != 0)),
        SHOW => {
            Frame::Notice(Notice::ShowPage(u16::from_be_bytes(payload.as_slice().try_into().map_err(|_| invalid())?)))
        }
        CURRENT => {
            Frame::Notice(Notice::Current(u16::from_be_bytes(payload.as_slice().try_into().map_err(|_| invalid())?)))
        }
        OFFLINE => {
            Frame::Notice(Notice::Offline(u16::from_be_bytes(payload.as_slice().try_into().map_err(|_| invalid())?)))
        }
        REQUEST => {
            let [a, b, c, d]: [u8; 4] = payload.as_slice().try_into().map_err(|_| invalid())?;
            Frame::Request(u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d]))
        }
        SUBMIT => {
            let post = cursor.byte()? != 0;
            let page_url = cursor.string()?;
            let target = cursor.string()?;
            let token = cursor.option()?;
            let action = cursor.option()?;
            let count = usize::from(cursor.u16()?);
            let fields = (0..count.min(MAX_STRINGS))
                .map(|_| Ok((cursor.string()?, cursor.string()?)))
                .collect::<io::Result<Vec<_>>>()?;
            cursor.end()?;
            Frame::Submit(Submission { page_url, post, target, token, action, fields })
        }
        CONFIRM if payload.len() == 1 => Frame::Confirm(payload[0] != 0),
        FOLLOW => {
            let mpp = cursor.u16()?;
            let sub = cursor.u16()?;
            let service = cursor.string()?;
            cursor.end()?;
            Frame::Follow(service, mpp, sub)
        }
        kind => browser_frame(kind, &payload)?,
    };
    Ok(Some(frame))
}

/// The frames of the browser's interface process (`docs/desktop-browser.md`
/// section 3.4), read as strictly as the rest.
fn browser_frame(kind: u8, payload: &[u8]) -> io::Result<Frame> {
    let invalid = || io::Error::other("malformed frame");
    let mut cursor = Cursor(payload);
    Ok(match kind {
        LINKED_SITES => {
            let count = usize::from(cursor.u16()?);
            let sites = (0..count.min(MAX_STRINGS))
                .map(|_| Ok((cursor.string()?, cursor.string()?)))
                .collect::<io::Result<Vec<_>>>()?;
            cursor.end()?;
            Frame::Notice(Notice::LinkedSites(sites))
        }
        REDIRECT => {
            Frame::Notice(Notice::RedirectToOtherSite(String::from_utf8(payload.to_vec()).map_err(|_| invalid())?))
        }
        SERVICE_NAME => Frame::Notice(Notice::ServiceName(String::from_utf8(payload.to_vec()).map_err(|_| invalid())?)),
        BUNDLE => Frame::Notice(Notice::Bundle(u16::from_be_bytes(payload.try_into().map_err(|_| invalid())?))),
        INPUT => {
            let code = cursor.byte()?;
            let value = cursor.u32()?;
            cursor.end()?;
            Frame::Input(user_input(code, u32::try_from(value).map_err(|_| invalid())?).ok_or_else(invalid)?)
        }
        SHOW_PAGE => {
            let [a, b, c, d]: [u8; 4] = payload.try_into().map_err(|_| invalid())?;
            Frame::Show(u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d]))
        }
        SETTINGS => {
            let [glyphs]: [u8; 1] = payload.try_into().map_err(|_| invalid())?;
            if glyphs > 2 {
                return Err(invalid());
            }
            Frame::Settings(DisplaySettings { glyphs })
        }
        CLICK => {
            let [row, column]: [u8; 2] = payload.try_into().map_err(|_| invalid())?;
            Frame::Click(row, column)
        }
        SCREEN => Frame::Screen(screen(payload)?),
        STATUS => {
            let page = cursor.u16()?;
            let subcode = cursor.u16()?;
            let found = cursor.byte()? != 0;
            let status: [u8; 8] = cursor.bytes(8)?.try_into().map_err(|_| invalid())?;
            let form = cursor.byte()? != 0;
            let text = cursor.string()?;
            cursor.end()?;
            if text.len() > MAX_SCREEN_TEXT {
                return Err(invalid());
            }
            Frame::Status(PageStatus { page, subcode, found, status, form, text })
        }
        _ => return Err(invalid()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_bad_frames_are_rejected() {
        let frames = [
            Frame::Packet([7; 42]),
            Frame::Tti(PageData {
                secure: true,
                origin: Origin::Form,
                url: "https://a/b".into(),
                bytes: b"PN,10000\n".to_vec(),
            }),
            Frame::T42(PageData { secure: false, origin: Origin::Push, url: String::new(), bytes: vec![1, 2, 3] }),
            Frame::Notice(Notice::InitialPage(0x100)),
            Frame::Notice(Notice::Unencrypted),
            Frame::Notice(Notice::Message("hello".into())),
            Frame::Notice(Notice::Service(vec!["other".into(), "weather".into()])),
            Frame::Notice(Notice::Confirm("example.org".into(), true, false)),
            Frame::Notice(Notice::Submitted(true)),
            Frame::Notice(Notice::ShowPage(0x301)),
            Frame::Notice(Notice::Current(0x100)),
            Frame::Notice(Notice::Offline(0x8FF)),
            Frame::Request(0x123, 0xFFFF),
            Frame::Submit(Submission {
                page_url: "http://x/p".into(),
                post: true,
                target: "/contact".into(),
                token: Some("t".into()),
                action: None,
                fields: vec![("name".into(), "Ann".into())],
            }),
            Frame::Confirm(false),
            Frame::Follow("other".into(), 0x400, 0xFFFF),
            Frame::Tti(PageData { secure: true, origin: Origin::FrontDoor, url: "h".into(), bytes: vec![] }),
            Frame::Notice(Notice::LinkedSites(vec![("weather".into(), "https://w.example/".into())])),
            Frame::Notice(Notice::RedirectToOtherSite("https://elsewhere.example/".into())),
            Frame::Notice(Notice::ServiceName("Test".into())),
            Frame::Notice(Notice::Bundle(0x199)),
            Frame::Input(UserInput::Digit(7)),
            Frame::Input(UserInput::Colour(3)),
            Frame::Input(UserInput::Char('é')),
            Frame::Input(UserInput::Arrow(2)),
            Frame::Input(UserInput::Home),
            Frame::Show(0x140, 0xFFFF),
            Frame::Settings(DisplaySettings { glyphs: 2 }),
            Frame::Click(3, 17),
            Frame::Screen(Screen {
                width: 2,
                height: 1,
                palette: vec![[0, 0, 0, 255], [9, 9, 9, 255]],
                indices: vec![1, 0],
            }),
            Frame::Status(PageStatus {
                page: 0x100,
                subcode: 0xFFFF,
                found: true,
                status: *b"P100    ",
                form: false,
                text: "HELLO\n".into(),
            }),
        ];
        let mut buffer = Vec::new();
        for frame in &frames {
            write(&mut buffer, frame).unwrap();
        }
        let mut reader = buffer.as_slice();
        for frame in &frames {
            assert_eq!(read(&mut reader).unwrap().as_ref(), Some(frame));
        }
        assert_eq!(read(&mut reader).unwrap(), None);
        for bad in [
            &[b'T', 0xFF, 0xFF, 0xFF, 0xFF][..],
            &[b'X', 0, 0, 0, 0],
            &[b'P', 0, 0, 0, 1, 0],
            &[b'S', 0, 0, 0, 2, 0, 9],
            &[b'L', 0, 0, 0, 8, 1, 0, 0xFF, 0xFF, 0, 0, 0, 9],
            &[b'C', 0, 0, 0, 2, 1, 1],
            // A digit out of range, a control character, an unknown input.
            &[b'K', 0, 0, 0, 5, 0, 0, 0, 0, 10],
            &[b'K', 0, 0, 0, 5, 17, 0, 0, 0, 7],
            &[b'K', 0, 0, 0, 5, 99, 0, 0, 0, 0],
            // A glyph choice that does not exist.
            &[b'Z', 0, 0, 0, 1, 3],
            // A screen whose index names a colour the palette lacks.
            &[b'Y', 0, 0, 0, 11, 0, 1, 0, 1, 0, 1, 0, 0, 0, 255, 1],
            // A screen wider than the limit.
            &[b'Y', 0, 0, 0, 6, 0xFF, 0xFF, 0, 1, 0, 0],
        ] {
            assert!(read(&mut &bad[..]).is_err(), "{bad:?}");
        }
    }
}
