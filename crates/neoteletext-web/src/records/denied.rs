//! The DENIED record (DOC-1961, DEC-1730,
//! REQ-1481): a selection refused, or a program that did not answer in time.
//! Its status is a real HTTP status number, so that a selection has an
//! answer as a web request has.
//!
//! ```text
//! bytes 0-1    status: 403, 404, 429 or 503
//! bytes 2-end  place: the place refused, as SELECT carried it, or nothing
//! ```

use neoteletext_core::packet::Packet;

use super::{RecordFault, TYPE_DENIED, take_u16, write_packets};

/// Not allowed (RFC 9110 section 15.5.4).
pub const STATUS_FORBIDDEN: u16 = 403;
/// No such page (RFC 9110 section 15.5.5).
pub const STATUS_NOT_FOUND: u16 = 404;
/// A limit was reached (RFC 6585 section 4).
pub const STATUS_TOO_MANY: u16 = 429;
/// The channel's program is not answering (RFC 9110 section 15.6.4).
pub const STATUS_UNAVAILABLE: u16 = 503;
/// The status's length.
const STATUS_BYTES: usize = 2;
/// The longest place: a page, a sub-page and no parameters, `302/0002`,
/// since a receiver matches it against what it selected and needs no more.
pub const MAX_PLACE: usize = 8;

/// A DENIED record, checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeniedRecord<'a> {
    /// The status, one of the four this record carries.
    pub status: u16,
    /// The place refused, or empty when the whole channel is unavailable.
    pub place: &'a str,
}

/// Reads a DENIED record's body.
///
/// # Errors
/// A status it does not carry, or a place that is too long or not text.
pub fn read(body: &[u8]) -> Result<DeniedRecord<'_>, RecordFault> {
    let (status, place) = take_u16(body)?;
    if !matches!(status, STATUS_FORBIDDEN | STATUS_NOT_FOUND | STATUS_TOO_MANY | STATUS_UNAVAILABLE) {
        return Err(RecordFault::BadValue("denied status"));
    }
    if place.len() > MAX_PLACE || !place.iter().all(u8::is_ascii_graphic) {
        return Err(RecordFault::BadValue("denied place"));
    }
    let place = core::str::from_utf8(place).map_err(|_| RecordFault::BadValue("denied place"))?;
    Ok(DeniedRecord { status, place })
}

/// Writes a DENIED record as packets 4/31 of audio service `service`,
/// record id `record_id`. Returns the packets written.
///
/// # Errors
/// What the reader would refuse.
pub fn encode(
    denied: DeniedRecord<'_>,
    service: u8,
    record_id: u8,
    out: &mut dyn FnMut(Packet),
) -> Result<usize, RecordFault> {
    let mut body = [0_u8; STATUS_BYTES + MAX_PLACE];
    let length = STATUS_BYTES.checked_add(denied.place.len()).filter(|&length| length <= body.len());
    let length = length.ok_or(RecordFault::Length)?;
    let target = body.get_mut(..length).ok_or(RecordFault::Length)?;
    let (status, place) = target.split_at_mut(STATUS_BYTES);
    status.copy_from_slice(&denied.status.to_be_bytes());
    place.copy_from_slice(denied.place.as_bytes());
    let body = body.get(..length).ok_or(RecordFault::Length)?;
    // The writer checks with the reader, so the two cannot disagree.
    if read(body)? != denied {
        return Err(RecordFault::BadValue("does not read back"));
    }
    write_packets(TYPE_DENIED, body, service, record_id, out).ok_or(RecordFault::Length)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, reason = "a test")]

    use super::*;
    use crate::records::{ChannelRecord, Read, RecordReader};

    #[test]
    fn a_denied_record_reaches_a_receiver() {
        for denied in [
            DeniedRecord { status: STATUS_NOT_FOUND, place: "302/0002" },
            DeniedRecord { status: STATUS_FORBIDDEN, place: "302" },
            DeniedRecord { status: STATUS_TOO_MANY, place: "302" },
            DeniedRecord { status: STATUS_UNAVAILABLE, place: "" },
        ] {
            let mut reader = RecordReader::new();
            let mut seen = None;
            encode(denied, 3, 7, &mut |packet| {
                if let Read::Record(ChannelRecord::Denied(read)) = reader.push(&packet) {
                    seen = Some((read.status, std::string::String::from(read.place)));
                }
            })
            .unwrap();
            assert_eq!(seen, Some((denied.status, denied.place.into())));
        }
    }

    /// DEC-1730 item 5: only the four statuses, and a place that is short text.
    #[test]
    fn anything_else_is_refused() {
        assert_eq!(read(&[]), Err(RecordFault::Length));
        assert!(read(&200u16.to_be_bytes()).is_err());
        assert!(read(&500u16.to_be_bytes()).is_err());
        let mut long = std::vec::Vec::from(404u16.to_be_bytes());
        long.extend_from_slice(b"302/00020");
        assert!(read(&long).is_err());
        let mut spaced = std::vec::Vec::from(404u16.to_be_bytes());
        spaced.extend_from_slice(b"30 2");
        assert!(read(&spaced).is_err());
    }
}
