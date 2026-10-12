//! The framed messages between the fetcher and decoder processes
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md), and between the
//! browser's interface process and both (`docs/desktop-browser.md`
//! section 3.4). Each frame is a kind byte, a 32-bit big-endian length and
//! the payload. Both sides bound the length, parse strictly and reject
//! unknown kinds.

use neoteletext_core::consts::{CELL_FIELD_LINES, FONT_MAX_CHARACTERS, FONT_MAX_FILE_BYTES};
use neoteletext_core::glyphs::LoadedFont;
use neoteletext_core::types::FontGlyph;
use std::io::{self, Read, Write};

/// The largest payload: one page response (REQ-WEB-008 default).
pub const MAX_FRAME: usize = 256 * 1024;
/// The widest screen a decoder may send: a page with both side panels, 72
/// columns of 12 pixels, with room.
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
/// The longest destination of a caught link: an address.
pub const MAX_LINK_TEXT: usize = 512;
/// Bytes of a SHA-256 on the pipe (REQ-FNT-003).
pub const SHA256_BYTES: usize = 32;
/// The longest font address, or text of a font, carried.
pub const MAX_FONT_TEXT: usize = 512;
/// Bytes of one glyph on the pipe: its code point and ten 16-bit lines.
const GLYPH_WIRE_BYTES: usize = 4 + 2 * CELL_FIELD_LINES;
/// The largest glyph table: 1,024 glyphs (REQ-FNT-002).
const MAX_TABLE_BYTES: usize = 2 + FONT_MAX_CHARACTERS * GLYPH_WIRE_BYTES;
/// The largest font reply: the file, its table and three texts.
const MAX_FONT_FRAME: usize = 1 + 4 + FONT_MAX_FILE_BYTES + MAX_TABLE_BYTES + 3 * (5 + MAX_FONT_TEXT);
/// The most fonts one library listing carries (REQ-FNT-004's 1,000 lines).
pub const MAX_LIBRARY_FONTS: usize = 1000;

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
    /// Interface to decoder: the pointer pressed this cell (row, column),
    /// for page catching and the Fastext prompts.
    Click(u8, u8),
    /// Decoder to interface: the rendered page.
    Screen(Screen),
    /// Decoder to interface: what the page shows and where the viewer is.
    Status(PageStatus),
    /// Decoder to interface: a link on the page leads to this address
    /// (`docs/web-native-concept.md` section 3), for the tuning screen.
    Visit(String),
    /// Interface to fetcher: fetch the channel's `contents.txt`
    /// (REQ-BRW-009).
    FetchContents,
    /// Fetcher to interface, never to a decoder: the answer to
    /// [`Frame::FetchContents`].
    Contents(ContentsReply),
    /// Interface to a font reader: the bytes of a font to read
    /// (REQ-FNT-009), from the cache or a local file.
    FontBytes(Vec<u8>),
    /// Font helper to interface: a font, read in the helper's confinement
    /// (REQ-FNT-003).
    Font(FontReply),
    /// Font helper to interface: a library's index, read in the helper's
    /// confinement (REQ-FNT-004).
    FontLibrary(LibraryReply),
    /// Interface to decoder: the fonts to draw the page with (REQ-FNT-008).
    Fonts(DecoderFonts),
}

/// A font read by a helper. The interface never parses the file: it
/// computes the bytes' SHA-256 itself and checks the table's shape.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FontData {
    /// The file, for the cache.
    pub bytes: Vec<u8>,
    /// `FAMILY_NAME`, made printable.
    pub family: Option<String>,
    /// `COPYRIGHT`, made printable.
    pub copyright: Option<String>,
    /// `NOTICE`, made printable.
    pub notice: Option<String>,
    /// The glyphs, sorted by code point.
    pub glyphs: Vec<FontGlyph>,
}

/// A font helper's answer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FontReply {
    /// The font.
    Loaded(FontData),
    /// It could not be fetched or read, and why.
    Failed(String),
}

/// One font of a library's index.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LibraryFont {
    /// Its address, resolved against the library's folder.
    pub address: String,
    /// The title the viewer chooses by.
    pub title: String,
    /// Its SHA-256.
    pub sha256: [u8; SHA256_BYTES],
}

/// A library helper's answer.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LibraryReply {
    /// The fonts, and how many lines were ignored as malformed.
    Listed(Vec<LibraryFont>, u32),
    /// The index could not be fetched or read, and why.
    Failed(String),
}

/// The fonts a decoder draws with (REQ-FNT-005, REQ-FNT-008): the font in
/// use, then the user's font for what it lacks; an empty table is none.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DecoderFonts {
    /// The font in use.
    pub in_use: Vec<FontGlyph>,
    /// The user's font, for characters the font in use lacks.
    pub fallback: Vec<FontGlyph>,
}

/// The fetcher's answer to [`Frame::FetchContents`] (REQ-WEB-025).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ContentsReply {
    /// The file, unchecked; the interface reads it.
    File(Vec<u8>),
    /// The channel has none (404 or 410).
    Missing,
    /// It could not be fetched or used, and why.
    Failed(String),
}

/// The largest contents file carried, the limit of REQ-WEB-025; the
/// fetcher asserts it is the web module's.
pub const MAX_CONTENTS: usize = 64 * 1024;

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
    /// Underline every link on the page, or stop (concept decision 11).
    ShowLinks,
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
    /// Page catching is on (`docs/web-native-concept.md` section 5), and
    /// this is where the selected link leads, as the decoder sees it;
    /// empty when catching is off.
    pub link: String,
    /// The page's text, one line per row, for accessibility, copying and
    /// tests.
    pub text: String,
    /// The font the page names with `TF` (REQ-FNT-007): its address,
    /// resolved against the page's, and its SHA-256. The interface parses
    /// the address again and decides whether to fetch it.
    pub font: Option<(String, [u8; SHA256_BYTES])>,
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
    /// A request was redirected out of the channel and not followed: its
    /// address (REQ-WEB-021).
    RedirectToOtherChannel(String),
    /// The service's name, from its descriptor or its host.
    ServiceName(String),
    /// The descriptor's aliases: page numbers, `0xMPP`, and their targets,
    /// `name:mpp` or an address (concept section 5 item 4).
    Aliases(Vec<(u16, String)>),
    /// Ask for the page on screen again this often, in seconds: the
    /// descriptor's refresh interval for a service without push.
    Refresh(u32),
    /// A bundle of pages arrived (a file, or a site's front door); its
    /// first page, `0xMPP`, is shown if the page asked for is not in it
    /// and the user has not moved.
    Bundle(u16),
    /// The channel names a font in `channel.json` (REQ-FNT-006): its
    /// address and SHA-256. The interface decides whether to fetch it.
    ChannelFont(String, [u8; SHA256_BYTES]),
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
// b'Z' carried the glyph choice until decision E.72 removed it; it is not reused.
const CLICK: u8 = b'Q';
const SCREEN: u8 = b'Y';
const STATUS: u8 = b'X';
const LINKED_SITES: u8 = b'J';
const REDIRECT: u8 = b'W';
const SERVICE_NAME: u8 = b'M';
const BUNDLE: u8 = b'b';
const ALIASES: u8 = b'a';
const VISIT: u8 = b'v';
const REFRESH: u8 = b'e';
const FETCH_CONTENTS: u8 = b'q';
const CONTENTS: u8 = b'o';
const FONT_BYTES: u8 = b'f';
const FONT: u8 = b'g';
const FONT_LIBRARY: u8 = b'h';
const FONTS: u8 = b'i';
const CHANNEL_FONT: u8 = b'j';
/// The kinds of [`FontReply`] and [`LibraryReply`] on the wire.
const REPLY_OK: u8 = 0;
const REPLY_FAILED: u8 = 1;
/// The kinds of [`ContentsReply`] on the wire.
const CONTENTS_FILE: u8 = 0;
const CONTENTS_MISSING: u8 = 1;
const CONTENTS_FAILED: u8 = 2;
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

    /// A glyph table of the shape every receiver checks (REQ-FNT-003,
    /// REQ-FNT-008): at most 1,024 glyphs, strictly increasing scalar code
    /// points, twelve dots a line.
    fn table(&mut self) -> io::Result<Vec<FontGlyph>> {
        let count = usize::from(self.u16()?);
        if count > FONT_MAX_CHARACTERS {
            return Err(io::Error::other("malformed frame"));
        }
        let mut glyphs = Vec::with_capacity(count);
        for _ in 0..count {
            let code = self.bytes(4)?;
            let code_point = u32::from_be_bytes([code[0], code[1], code[2], code[3]]);
            let mut rows = [0u16; CELL_FIELD_LINES];
            for row in &mut rows {
                *row = self.u16()?;
            }
            glyphs.push(FontGlyph { code_point, rows });
        }
        LoadedFont::new(&glyphs).map_err(|_| io::Error::other("malformed frame"))?;
        Ok(glyphs)
    }

    /// A string of at most [`MAX_FONT_TEXT`] bytes.
    fn short_string(&mut self) -> io::Result<String> {
        let text = self.string()?;
        if text.len() > MAX_FONT_TEXT { Err(io::Error::other("malformed frame")) } else { Ok(text) }
    }

    fn sha256(&mut self) -> io::Result<[u8; SHA256_BYTES]> {
        self.bytes(SHA256_BYTES)?.try_into().map_err(|_| io::Error::other("malformed frame"))
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
        Frame::Notice(Notice::RedirectToOtherChannel(address)) => put(out, REDIRECT, address.as_bytes()),
        Frame::Notice(Notice::ServiceName(name)) => put(out, SERVICE_NAME, name.as_bytes()),
        Frame::Notice(Notice::Bundle(mpp)) => put(out, BUNDLE, &mpp.to_be_bytes()),
        Frame::Notice(Notice::Aliases(aliases)) => {
            let mut buffer = Vec::new();
            buffer.extend_from_slice(&u16::try_from(aliases.len().min(MAX_STRINGS)).unwrap_or(0).to_be_bytes());
            for (mpp, target) in aliases.iter().take(MAX_STRINGS) {
                buffer.extend_from_slice(&mpp.to_be_bytes());
                put_string(&mut buffer, target);
            }
            put(out, ALIASES, &buffer)
        }
        Frame::Visit(address) => put(out, VISIT, address.as_bytes()),
        Frame::FetchContents => put(out, FETCH_CONTENTS, &[]),
        Frame::Contents(reply) => put(out, CONTENTS, &contents_payload(reply)),
        Frame::Notice(Notice::Refresh(seconds)) => put(out, REFRESH, &seconds.to_be_bytes()),
        Frame::Input(input) => {
            let (code, value) = input_code(*input);
            let mut buffer = vec![code];
            buffer.extend_from_slice(&value.to_be_bytes());
            put(out, INPUT, &buffer)
        }
        Frame::Show(mpp, sub) => put(out, SHOW_PAGE, &[mpp.to_be_bytes(), sub.to_be_bytes()].concat()),
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
        Frame::Status(status) => put(out, STATUS, &status_payload(status)),
        Frame::Notice(Notice::ChannelFont(..))
        | Frame::FontBytes(_)
        | Frame::Font(_)
        | Frame::FontLibrary(_)
        | Frame::Fonts(_) => write_font(out, frame),
    }
}

/// The payload of a [`Frame::Status`].
fn status_payload(status: &PageStatus) -> Vec<u8> {
    let mut buffer = [status.page.to_be_bytes(), status.subcode.to_be_bytes()].concat();
    buffer.push(u8::from(status.found));
    buffer.extend_from_slice(&status.status);
    buffer.push(u8::from(status.form));
    put_string(&mut buffer, &status.link);
    put_string(&mut buffer, &status.text);
    buffer.push(u8::from(status.font.is_some()));
    if let Some((address, sha256)) = &status.font {
        put_string(&mut buffer, address);
        buffer.extend_from_slice(sha256);
    }
    buffer
}

/// Writes the frames of fonts (REQ-FNT-003, REQ-FNT-004, REQ-FNT-008).
fn write_font(out: &mut dyn Write, frame: &Frame) -> io::Result<()> {
    match frame {
        Frame::Notice(Notice::ChannelFont(address, sha256)) => {
            let mut buffer = Vec::new();
            put_string(&mut buffer, address);
            buffer.extend_from_slice(sha256);
            put(out, CHANNEL_FONT, &buffer)
        }
        Frame::FontBytes(bytes) => put(out, FONT_BYTES, bytes),
        Frame::Font(reply) => {
            let mut buffer = Vec::new();
            match reply {
                FontReply::Loaded(font) => {
                    buffer.push(REPLY_OK);
                    buffer.extend_from_slice(&u32::try_from(font.bytes.len()).unwrap_or(0).to_be_bytes());
                    buffer.extend_from_slice(&font.bytes);
                    for text in [&font.family, &font.copyright, &font.notice] {
                        put_option(&mut buffer, text.as_ref());
                    }
                    put_table(&mut buffer, &font.glyphs);
                }
                FontReply::Failed(why) => {
                    buffer.push(REPLY_FAILED);
                    put_string(&mut buffer, why);
                }
            }
            put_limited(out, FONT, &buffer, MAX_FONT_FRAME)
        }
        Frame::FontLibrary(reply) => {
            let mut buffer = Vec::new();
            match reply {
                LibraryReply::Listed(fonts, ignored) => {
                    buffer.push(REPLY_OK);
                    buffer.extend_from_slice(&ignored.to_be_bytes());
                    buffer.extend_from_slice(&u16::try_from(fonts.len()).unwrap_or(0).to_be_bytes());
                    for font in fonts {
                        put_string(&mut buffer, &font.address);
                        put_string(&mut buffer, &font.title);
                        buffer.extend_from_slice(&font.sha256);
                    }
                }
                LibraryReply::Failed(why) => {
                    buffer.push(REPLY_FAILED);
                    put_string(&mut buffer, why);
                }
            }
            put(out, FONT_LIBRARY, &buffer)
        }
        Frame::Fonts(fonts) => {
            let mut buffer = Vec::new();
            put_table(&mut buffer, &fonts.in_use);
            put_table(&mut buffer, &fonts.fallback);
            put(out, FONTS, &buffer)
        }
        _ => Err(io::Error::other("not a font frame")),
    }
}

/// A glyph table: its length, then each glyph's code point and lines.
fn put_table(buffer: &mut Vec<u8>, glyphs: &[FontGlyph]) {
    buffer.extend_from_slice(&u16::try_from(glyphs.len()).unwrap_or(0).to_be_bytes());
    for glyph in glyphs {
        buffer.extend_from_slice(&glyph.code_point.to_be_bytes());
        for row in glyph.rows {
            buffer.extend_from_slice(&row.to_be_bytes());
        }
    }
}

/// The payload of a [`Frame::Contents`]: its kind, then the file or the
/// reason.
fn contents_payload(reply: &ContentsReply) -> Vec<u8> {
    let (kind, body) = match reply {
        ContentsReply::File(bytes) => (CONTENTS_FILE, bytes.as_slice()),
        ContentsReply::Missing => (CONTENTS_MISSING, &[][..]),
        ContentsReply::Failed(why) => (CONTENTS_FAILED, why.as_bytes()),
    };
    [&[kind][..], body].concat()
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
        UserInput::ShowLinks => (19, 0),
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
        19 => UserInput::ShowLinks,
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
    let limit = match head[0] {
        SCREEN => MAX_SCREEN_FRAME,
        FONT => MAX_FONT_FRAME,
        _ => MAX_FRAME,
    };
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
            Frame::Notice(Notice::RedirectToOtherChannel(String::from_utf8(payload.to_vec()).map_err(|_| invalid())?))
        }
        SERVICE_NAME => Frame::Notice(Notice::ServiceName(String::from_utf8(payload.to_vec()).map_err(|_| invalid())?)),
        BUNDLE => Frame::Notice(Notice::Bundle(u16::from_be_bytes(payload.try_into().map_err(|_| invalid())?))),
        ALIASES => {
            let count = usize::from(cursor.u16()?);
            let aliases = (0..count.min(MAX_STRINGS))
                .map(|_| Ok((cursor.u16()?, cursor.string()?)))
                .collect::<io::Result<Vec<_>>>()?;
            cursor.end()?;
            Frame::Notice(Notice::Aliases(aliases))
        }
        VISIT => Frame::Visit(String::from_utf8(payload.to_vec()).map_err(|_| invalid())?),
        FETCH_CONTENTS if payload.is_empty() => Frame::FetchContents,
        CONTENTS => {
            let kind = cursor.byte()?;
            let body = payload.get(1..).unwrap_or(&[]);
            Frame::Contents(match kind {
                CONTENTS_FILE if body.len() <= MAX_CONTENTS => ContentsReply::File(body.to_vec()),
                CONTENTS_MISSING if body.is_empty() => ContentsReply::Missing,
                CONTENTS_FAILED if body.len() <= MAX_LINK_TEXT => {
                    ContentsReply::Failed(String::from_utf8(body.to_vec()).map_err(|_| invalid())?)
                }
                _ => return Err(invalid()),
            })
        }
        REFRESH => Frame::Notice(Notice::Refresh(u32::from_be_bytes(payload.try_into().map_err(|_| invalid())?))),
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
            let link = cursor.string()?;
            let text = cursor.string()?;
            let font = match cursor.byte()? {
                0 => None,
                1 => Some((cursor.short_string()?, cursor.sha256()?)),
                _ => return Err(invalid()),
            };
            cursor.end()?;
            if text.len() > MAX_SCREEN_TEXT || link.len() > MAX_LINK_TEXT {
                return Err(invalid());
            }
            Frame::Status(PageStatus { page, subcode, found, status, form, link, text, font })
        }
        CHANNEL_FONT | FONT_BYTES | FONT | FONT_LIBRARY | FONTS => font_frame(kind, payload)?,
        _ => return Err(invalid()),
    })
}

/// Reads the frames of fonts, checking every glyph table's shape
/// (REQ-FNT-003, REQ-FNT-008).
fn font_frame(kind: u8, payload: &[u8]) -> io::Result<Frame> {
    let invalid = || io::Error::other("malformed frame");
    let mut cursor = Cursor(payload);
    Ok(match kind {
        CHANNEL_FONT => {
            let address = cursor.short_string()?;
            let sha256 = cursor.sha256()?;
            cursor.end()?;
            Frame::Notice(Notice::ChannelFont(address, sha256))
        }
        FONT_BYTES if payload.len() <= FONT_MAX_FILE_BYTES => Frame::FontBytes(payload.to_vec()),
        FONT => Frame::Font(match cursor.byte()? {
            REPLY_OK => {
                let length = cursor.u32()?;
                if length > FONT_MAX_FILE_BYTES {
                    return Err(invalid());
                }
                let bytes = cursor.bytes(length)?.to_vec();
                let mut texts = [None, None, None];
                for text in &mut texts {
                    *text = cursor.option()?;
                    if text.as_ref().is_some_and(|text| text.len() > MAX_FONT_TEXT) {
                        return Err(invalid());
                    }
                }
                let glyphs = cursor.table()?;
                cursor.end()?;
                let [family, copyright, notice] = texts;
                FontReply::Loaded(FontData { bytes, family, copyright, notice, glyphs })
            }
            REPLY_FAILED => {
                let why = cursor.short_string()?;
                cursor.end()?;
                FontReply::Failed(why)
            }
            _ => return Err(invalid()),
        }),
        FONT_LIBRARY => Frame::FontLibrary(match cursor.byte()? {
            REPLY_OK => {
                let ignored = u32::try_from(cursor.u32()?).map_err(|_| invalid())?;
                let count = usize::from(cursor.u16()?);
                if count > MAX_LIBRARY_FONTS {
                    return Err(invalid());
                }
                let fonts = (0..count)
                    .map(|_| {
                        Ok(LibraryFont {
                            address: cursor.short_string()?,
                            title: cursor.short_string()?,
                            sha256: cursor.sha256()?,
                        })
                    })
                    .collect::<io::Result<Vec<_>>>()?;
                cursor.end()?;
                LibraryReply::Listed(fonts, ignored)
            }
            REPLY_FAILED => {
                let why = cursor.short_string()?;
                cursor.end()?;
                LibraryReply::Failed(why)
            }
            _ => return Err(invalid()),
        }),
        FONTS => {
            let in_use = cursor.table()?;
            let fallback = cursor.table()?;
            cursor.end()?;
            Frame::Fonts(DecoderFonts { in_use, fallback })
        }
        _ => return Err(invalid()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(code_point: u32, row: u16) -> FontGlyph {
        FontGlyph { code_point, rows: [row; CELL_FIELD_LINES] }
    }

    /// The bytes of a frame.
    fn wire(frame: &Frame) -> Vec<u8> {
        let mut buffer = Vec::new();
        write(&mut buffer, frame).unwrap();
        buffer
    }

    /// REQ-FNT-003, REQ-FNT-008: a glyph table of the wrong shape is refused
    /// when a frame is read, whoever reads it; so is an oversized font.
    #[test]
    fn bad_glyph_tables_and_oversized_fonts_are_refused() {
        let fonts = |in_use: Vec<FontGlyph>| Frame::Fonts(DecoderFonts { in_use, fallback: Vec::new() });
        for (table, what) in [
            (vec![glyph(0x42, 0), glyph(0x41, 0)], "unsorted"),
            (vec![glyph(0x41, 0), glyph(0x41, 0)], "repeated"),
            (vec![glyph(0xD800, 0)], "not a scalar value"),
            (vec![glyph(0x41, 0x1000)], "a thirteenth dot"),
            ((0..1025).map(|code| glyph(0x100 + code, 0)).collect(), "1,025 glyphs"),
        ] {
            let bytes = wire(&fonts(table));
            assert!(read(&mut bytes.as_slice()).is_err(), "{what}");
        }
        let full: Vec<FontGlyph> = (0..1024).map(|code| glyph(0x100 + code, 0)).collect();
        assert!(read(&mut wire(&fonts(full)).as_slice()).is_ok(), "1,024 glyphs fit");
        // A font file larger than REQ-FNT-002 allows, in each frame.
        let large = vec![b'x'; FONT_MAX_FILE_BYTES + 1];
        let mut bytes = vec![FONT_BYTES];
        bytes.extend_from_slice(&u32::try_from(large.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(&large);
        assert!(read(&mut bytes.as_slice()).is_err(), "font bytes too large");
        let reply = Frame::Font(FontReply::Loaded(FontData {
            bytes: large,
            family: None,
            copyright: None,
            notice: None,
            glyphs: Vec::new(),
        }));
        assert!(write(&mut Vec::new(), &reply).is_err() || read(&mut wire(&reply).as_slice()).is_err());
        // A reply of an unknown kind, and a status whose font flag is 2.
        assert!(read(&mut &[FONT, 0, 0, 0, 1, 7][..]).is_err());
        assert!(read(&mut &[FONT_LIBRARY, 0, 0, 0, 1, 7][..]).is_err());
    }

    /// The frames of fonts survive the pipe (REQ-FNT-003, REQ-FNT-008).
    #[test]
    fn font_frames_round_trip() {
        let frames = [
            Frame::Status(PageStatus {
                page: 0x100,
                subcode: 0xFFFF,
                found: true,
                status: *b"P100    ",
                form: false,
                link: String::new(),
                text: String::new(),
                font: Some(("https://fonts.example/f.bdf".into(), [7; SHA256_BYTES])),
            }),
            Frame::Notice(Notice::ChannelFont("https://a.example/f.bdf".into(), [9; SHA256_BYTES])),
            Frame::FontBytes(b"STARTFONT 2.1\n".to_vec()),
            Frame::Font(FontReply::Loaded(FontData {
                bytes: b"STARTFONT".to_vec(),
                family: Some("Family".into()),
                copyright: None,
                notice: Some("CC0".into()),
                glyphs: vec![glyph(0x41, 0x0F0), glyph(0x42, 0xFFF)],
            })),
            Frame::Font(FontReply::Failed("line 3: Spacing".into())),
            Frame::FontLibrary(LibraryReply::Listed(
                vec![LibraryFont { address: "https://l.example/f.bdf".into(), title: "F".into(), sha256: [1; 32] }],
                2,
            )),
            Frame::FontLibrary(LibraryReply::Failed("TooLarge".into())),
            Frame::Fonts(DecoderFonts { in_use: vec![glyph(0x41, 1)], fallback: Vec::new() }),
        ];
        for frame in &frames {
            assert_eq!(read(&mut wire(frame).as_slice()).unwrap().as_ref(), Some(frame));
        }
    }

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
            Frame::Notice(Notice::RedirectToOtherChannel("https://elsewhere.example/".into())),
            Frame::Notice(Notice::ServiceName("Test".into())),
            Frame::Notice(Notice::Bundle(0x199)),
            Frame::Notice(Notice::Aliases(vec![(0x300, "weather:300".into())])),
            Frame::Visit("https://w.example/300.t42".into()),
            Frame::FetchContents,
            Frame::Contents(ContentsReply::File(b"p\t100\tIndex\n".to_vec())),
            Frame::Contents(ContentsReply::Missing),
            Frame::Contents(ContentsReply::Failed("MediaType".into())),
            Frame::Notice(Notice::Refresh(60)),
            Frame::Input(UserInput::Digit(7)),
            Frame::Input(UserInput::Colour(3)),
            Frame::Input(UserInput::Char('é')),
            Frame::Input(UserInput::Arrow(2)),
            Frame::Input(UserInput::Home),
            Frame::Input(UserInput::ShowLinks),
            Frame::Show(0x140, 0xFFFF),
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
                link: "P200".into(),
                text: "HELLO\n".into(),
                font: None,
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
            // The retired glyph-choice kind is unknown now.
            &[b'Z', 0, 0, 0, 1, 0],
            // A contents reply of an unknown kind, and "missing" with a body.
            &[b'o', 0, 0, 0, 1, 3],
            &[b'o', 0, 0, 0, 2, 1, 7],
            // A screen whose index names a colour the palette lacks.
            &[b'Y', 0, 0, 0, 11, 0, 1, 0, 1, 0, 1, 0, 0, 0, 255, 1],
            // A screen wider than the limit.
            &[b'Y', 0, 0, 0, 6, 0xFF, 0xFF, 0, 1, 0, 0],
        ] {
            assert!(read(&mut &bad[..]).is_err(), "{bad:?}");
        }
    }
}
