//! The wire form of the frames: the kind bytes, the strict payload reader,
//! and writing and reading one frame (decision 26 of IMPLEMENTATION-LOG.md).

use super::{Frame, MAX_FRAME, MAX_INPUT_MESSAGE, Notice, Origin, PageData, Submission};
use std::io::{self, Read, Write};

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
// b'Z' carried the glyph choice until decision E.72 removed it, and the
// browser's frames used b'k', b'V', b'Q', b'Y', b'X', b'J', b'v', b'q',
// b'o', b'f', b'g', b'h', b'i', b'j' and b'l' until it left the repository;
// they are not reused.
const REDIRECT: u8 = b'W';
const SERVICE_NAME: u8 = b'M';
const BUNDLE: u8 = b'b';
const ALIASES: u8 = b'a';
const REFRESH: u8 = b'e';
const KEY: u8 = b'y';
const INPUT: u8 = b'n';
/// The most strings one frame may carry.
const MAX_STRINGS: usize = 256;

fn put(out: &mut dyn Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(payload.len()).map_err(|_| io::Error::other("frame too long"))?;
    if payload.len() > MAX_FRAME {
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
    buffer.extend_from_slice(bytes.get(..length as usize).unwrap_or_default());
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

    /// The next `N` bytes.
    fn array<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        self.bytes(N)?.try_into().map_err(|_| io::Error::other("malformed frame"))
    }

    fn byte(&mut self) -> io::Result<u8> {
        Ok(u8::from_be_bytes(self.array()?))
    }

    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> io::Result<usize> {
        Ok(u32::from_be_bytes(self.array()?) as usize)
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
        Frame::Input(record) => {
            if record.len() > MAX_INPUT_MESSAGE {
                return Err(io::Error::other("an input message too long"));
            }
            put(out, KEY, record)
        }
        Frame::Notice(Notice::Input(on)) => put(out, INPUT, &[u8::from(*on)]),
        Frame::Follow(service, mpp, sub) => {
            let mut buffer = [mpp.to_be_bytes(), sub.to_be_bytes()].concat();
            put_string(&mut buffer, service);
            put(out, FOLLOW, &buffer)
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
        Frame::Notice(Notice::Refresh(seconds)) => put(out, REFRESH, &seconds.to_be_bytes()),
    }
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
    if length > MAX_FRAME {
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
        SUBMITTED if payload.len() == 1 => Frame::Notice(Notice::Submitted(payload.first() != Some(&0))),
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
        CONFIRM if payload.len() == 1 => Frame::Confirm(payload.first() != Some(&0)),
        KEY if payload.len() <= MAX_INPUT_MESSAGE => Frame::Input(payload),
        INPUT if payload.len() == 1 => Frame::Notice(Notice::Input(payload.first() != Some(&0))),
        FOLLOW => {
            let mpp = cursor.u16()?;
            let sub = cursor.u16()?;
            let service = cursor.string()?;
            cursor.end()?;
            Frame::Follow(service, mpp, sub)
        }
        kind => notice_frame(kind, &payload)?,
    };
    Ok(Some(frame))
}

/// The fetcher's notices about a channel, read as strictly as the rest.
fn notice_frame(kind: u8, payload: &[u8]) -> io::Result<Frame> {
    let invalid = || io::Error::other("malformed frame");
    let mut cursor = Cursor(payload);
    Ok(match kind {
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
        REFRESH => Frame::Notice(Notice::Refresh(u32::from_be_bytes(payload.try_into().map_err(|_| invalid())?))),
        _ => return Err(invalid()),
    })
}
