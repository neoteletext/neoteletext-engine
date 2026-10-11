//! The framed messages between the fetcher and decoder processes
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md). Each frame is a
//! kind byte, a 32-bit big-endian length and the payload. Both sides bound
//! the length, parse strictly and reject unknown kinds.

use std::io::{self, Read, Write};

/// The largest payload: one page response (REQ-WEB-008 default).
pub const MAX_FRAME: usize = 256 * 1024;

/// Where page bytes came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Origin {
    /// A page fetch.
    Page,
    /// The push connection.
    Push,
    /// The response to a form submission (REQ-WEB-011).
    Form,
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
        _ => return Err(invalid()),
    };
    Ok(Some(frame))
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
        ] {
            assert!(read(&mut &bad[..]).is_err(), "{bad:?}");
        }
    }
}
