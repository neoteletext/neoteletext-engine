//! The framed messages between the fetcher and decoder processes
//! (REQ-SEC-018, decision 26 of IMPLEMENTATION-LOG.md). Each frame is a
//! kind byte, a 32-bit big-endian length and the payload. Both sides bound
//! the length and reject unknown kinds.

use std::io::{self, Read, Write};

/// The largest payload: one page response (REQ-WEB-008 default).
pub const MAX_FRAME: usize = 256 * 1024;

/// A message on the pipe.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Frame {
    /// Fetcher to decoder: one 42-byte packet.
    Packet([u8; tellytxt_core::consts::PACKET_BYTES]),
    /// Fetcher to decoder: TTI page bytes; the flag is "came encrypted".
    Tti(bool, Vec<u8>),
    /// Fetcher to decoder: T42 page bytes; the flag is "came encrypted".
    T42(bool, Vec<u8>),
    /// Fetcher to decoder: a notice for the user.
    Notice(Notice),
    /// Decoder to fetcher: request a page (`0xMPP`, sub-code or 0xFFFF).
    Request(u16, u16),
}

/// Notices the fetcher sends.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Notice {
    /// The service's initial page, `0xMPP`.
    InitialPage(u16),
    /// The page on screen came over plain HTTP (REQ-SEC-008).
    Unencrypted,
    /// A message for standard error.
    Message(String),
}

const PACKET: u8 = b'P';
const TTI: u8 = b'T';
const T42: u8 = b'B';
const INITIAL: u8 = b'I';
const UNENCRYPTED: u8 = b'U';
const MESSAGE: u8 = b'N';
const REQUEST: u8 = b'R';

fn put(out: &mut dyn Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    let length = u32::try_from(payload.len()).map_err(|_| io::Error::other("frame too long"))?;
    out.write_all(&[kind])?;
    out.write_all(&length.to_be_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

/// Writes one frame.
pub fn write(out: &mut dyn Write, frame: &Frame) -> io::Result<()> {
    match frame {
        Frame::Packet(bytes) => put(out, PACKET, bytes),
        Frame::Tti(secure, bytes) => put(out, TTI, &[&[u8::from(*secure)], bytes.as_slice()].concat()),
        Frame::T42(secure, bytes) => put(out, T42, &[&[u8::from(*secure)], bytes.as_slice()].concat()),
        Frame::Notice(Notice::InitialPage(mpp)) => put(out, INITIAL, &mpp.to_be_bytes()),
        Frame::Notice(Notice::Unencrypted) => put(out, UNENCRYPTED, &[]),
        Frame::Notice(Notice::Message(text)) => put(out, MESSAGE, text.as_bytes()),
        Frame::Request(mpp, sub) => put(out, REQUEST, &[mpp.to_be_bytes(), sub.to_be_bytes()].concat()),
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
    let frame = match head[0] {
        PACKET => Frame::Packet(payload.as_slice().try_into().map_err(|_| invalid())?),
        TTI | T42 => {
            let (&secure, bytes) = payload.split_first().ok_or_else(invalid)?;
            let secure = secure != 0;
            if head[0] == TTI { Frame::Tti(secure, bytes.to_vec()) } else { Frame::T42(secure, bytes.to_vec()) }
        }
        INITIAL => Frame::Notice(Notice::InitialPage(u16::from_be_bytes(
            payload.as_slice().try_into().map_err(|_| invalid())?,
        ))),
        UNENCRYPTED if payload.is_empty() => Frame::Notice(Notice::Unencrypted),
        MESSAGE => Frame::Notice(Notice::Message(String::from_utf8_lossy(&payload).into_owned())),
        REQUEST => {
            let [a, b, c, d]: [u8; 4] = payload.as_slice().try_into().map_err(|_| invalid())?;
            Frame::Request(u16::from_be_bytes([a, b]), u16::from_be_bytes([c, d]))
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
            Frame::Tti(true, b"PN,10000\n".to_vec()),
            Frame::T42(false, vec![1, 2, 3]),
            Frame::Notice(Notice::InitialPage(0x100)),
            Frame::Notice(Notice::Unencrypted),
            Frame::Notice(Notice::Message("hello".into())),
            Frame::Request(0x123, 0xFFFF),
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
        let oversized = [b'T', 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(read(&mut oversized.as_slice()).is_err());
        let unknown = [b'X', 0, 0, 0, 0];
        assert!(read(&mut unknown.as_slice()).is_err());
        let short_packet = [b'P', 0, 0, 0, 1, 0];
        assert!(read(&mut short_packet.as_slice()).is_err());
    }
}
