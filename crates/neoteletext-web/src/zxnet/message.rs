//! The server's messages (REQ-INP-022): JSON arrays named by their first
//! element, read and checked, and the packets they carry rebuilt with a
//! Hamming-coded address and cross-checked against the packet decoder.

use neoteletext_core::consts::{CONTROL_MASK, DATA_BYTES, DATA_START};
use neoteletext_core::packet::{self, Decoded, Packet};
use neoteletext_core::types::{Magazine, PageAddress, PageNumber, SubCode};

use super::{CLOCK_BYTES, HeaderField, LiveEvent, LiveSink, MAX_PACKET_NUMBER, Malformed, event};
use crate::base64;
use crate::json::{JsonReader, Token, integer};

/// Room for the decoded `initialpage` status message.
const STATUS_BYTES: usize = neoteletext_core::consts::STATUS_MESSAGE_BYTES + 2;

/// A server message, parsed and checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Message {
    ApiVersion(i64),
    WrongName,
    Header { slot: u8, data: [u8; DATA_BYTES], control: u16, magazine: Magazine, subcode: u16, page: u8 },
    Row { slot: u8, magazine: Magazine, number: u8, data: [u8; DATA_BYTES] },
    Bsdp([u8; DATA_BYTES]),
    Clock([u8; CLOCK_BYTES], usize),
    PageExists(PageAddress),
    InitialPage { page: PageAddress, subcode: SubCode, status: [u8; STATUS_BYTES], len: usize },
}

/// A magazine as the feed writes it: 1 to 8, or 0 for 8.
fn magazine(value: i64) -> Option<Magazine> {
    match value {
        0 => Some(Magazine::from_bits(0)),
        _ => Magazine::new(u8::try_from(value).ok()?),
    }
}

/// Reads the next token as an integer.
fn next_integer(reader: &mut JsonReader<'_>) -> Option<i64> {
    match reader.next_token().ok()? {
        Some(Token::Number(number)) => integer(number),
        _ => None,
    }
}

/// Decodes the next token as base64 packet data of exactly 40 bytes.
fn next_packet_data(reader: &mut JsonReader<'_>) -> Result<[u8; DATA_BYTES], Malformed> {
    let Ok(Some(Token::Str(value))) = reader.next_token() else { return Err(Malformed::Fields) };
    let text = value.plain().ok_or(Malformed::PacketData)?;
    let mut data = [0u8; DATA_BYTES + 2];
    match base64::decode(text, &mut data) {
        Ok(DATA_BYTES) => {
            let mut exact = [0u8; DATA_BYTES];
            exact.copy_from_slice(data.get(..DATA_BYTES).unwrap_or(&[0; DATA_BYTES]));
            Ok(exact)
        }
        _ => Err(Malformed::PacketData),
    }
}

fn next_u8(reader: &mut JsonReader<'_>) -> Result<u8, Malformed> {
    next_integer(reader).and_then(|v| u8::try_from(v).ok()).ok_or(Malformed::Fields)
}

fn next_u16(reader: &mut JsonReader<'_>) -> Result<u16, Malformed> {
    next_integer(reader).and_then(|v| u16::try_from(v).ok()).ok_or(Malformed::Fields)
}

fn next_magazine(reader: &mut JsonReader<'_>) -> Result<Magazine, Malformed> {
    next_integer(reader).and_then(magazine).ok_or(Malformed::Fields)
}

/// Decodes the next token as base64 into `out`, returning the length.
fn next_base64(reader: &mut JsonReader<'_>, out: &mut [u8]) -> Result<usize, Malformed> {
    let Ok(Some(Token::Str(value))) = reader.next_token() else { return Err(Malformed::Fields) };
    let text = value.plain().ok_or(Malformed::PacketData)?;
    base64::decode(text, out).map_err(|_| Malformed::PacketData)
}

/// Parses one server message. `Ok(None)` for messages that are ignored:
/// `channelSettings`, `secondTick` and unknown names. Fields after the
/// known ones are ignored (REQ-INP-022).
pub(super) fn parse_message(text: &[u8]) -> Result<Option<Message>, Malformed> {
    let mut reader = JsonReader::new(text).map_err(|_| Malformed::Json)?;
    if reader.next_token() != Ok(Some(Token::BeginArray)) {
        return Err(Malformed::Json);
    }
    let Ok(Some(Token::Str(name))) = reader.next_token() else { return Err(Malformed::Json) };
    // No message name has escapes; one that does is unknown.
    let Some(name) = name.plain() else { return Ok(None) };
    let message = match name {
        b"apiver" => Message::ApiVersion(next_integer(&mut reader).ok_or(Malformed::Fields)?),
        b"wrongname" => Message::WrongName,
        b"header" => {
            let slot = next_u8(&mut reader)?;
            let data = next_packet_data(&mut reader)?;
            let Ok(Some(Token::Bool(_))) = reader.next_token() else { return Err(Malformed::Fields) };
            let control = next_u16(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let subcode = next_u16(&mut reader)?;
            let page = next_u8(&mut reader)?;
            Message::Header { slot, data, control, magazine, subcode, page }
        }
        b"row" => {
            let slot = next_u8(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let number = next_u8(&mut reader)?;
            if !(1..=MAX_PACKET_NUMBER).contains(&number) {
                return Err(Malformed::Fields);
            }
            Message::Row { slot, magazine, number, data: next_packet_data(&mut reader)? }
        }
        b"bsdp" => Message::Bsdp(next_packet_data(&mut reader)?),
        b"clock" => {
            next_u8(&mut reader)?;
            let mut decoded = [0u8; CLOCK_BYTES + 2];
            let len = next_base64(&mut reader, &mut decoded)?;
            let mut clock = [0u8; CLOCK_BYTES];
            let len = len.min(CLOCK_BYTES);
            clock.get_mut(..len).unwrap_or(&mut []).copy_from_slice(decoded.get(..len).unwrap_or(&[]));
            Message::Clock(clock, len)
        }
        b"pageExists" => {
            next_u8(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let page = next_u8(&mut reader)?;
            Message::PageExists(PageAddress { magazine, page: PageNumber(page) })
        }
        b"initialpage" => {
            next_u8(&mut reader)?;
            let magazine = next_magazine(&mut reader)?;
            let page = next_u8(&mut reader)?;
            let subcode = SubCode::new(next_u16(&mut reader)?);
            let mut status = [0u8; STATUS_BYTES];
            let len = next_base64(&mut reader, &mut status)?;
            Message::InitialPage { page: PageAddress { magazine, page: PageNumber(page) }, subcode, status, len }
        }
        _ => return Ok(None),
    };
    Ok(Some(message))
}

/// Compares the header fields the server decoded with the packet decoder's
/// reading of the same bytes, reporting each disagreement (REQ-INP-022).
pub(super) fn cross_check(packet: &Packet, control: u16, subcode: u16, page: u8, sink: &mut LiveSink<'_>) {
    let Ok(Decoded::Header(header)) = packet::decode(packet) else { return };
    let checks = [
        (HeaderField::Page, u16::from(page), header.page.map(|p| u16::from(p.0))),
        (HeaderField::SubCode, SubCode::new(subcode).value(), header.subcode.map(SubCode::value)),
        (HeaderField::Control, control & CONTROL_MASK, header.control.map(|c| c.0 & CONTROL_MASK)),
    ];
    for (field, server, decoded) in checks {
        if decoded != Some(server) {
            event(sink, LiveEvent::HeaderMismatch { field, server, decoded });
        }
    }
}

/// Builds a complete packet: the Hamming-coded address, then the 40 bytes.
pub(super) fn rebuild(magazine: Magazine, number: u8, data: &[u8; DATA_BYTES]) -> Packet {
    let mut packet = packet::encode::address(magazine, number);
    if let Some(target) = packet.bytes.get_mut(DATA_START..) {
        target.copy_from_slice(data);
    }
    packet
}
