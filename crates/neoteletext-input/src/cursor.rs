//! The cursor record (`neoteletext-common/INPUT.md` section 7.1,
//! REQ-INT-005, REQ-INT-006, REQ-INT-110): a channel's answer to the input,
//! saying where the cursor is and how much input it has acted on. It is
//! optional, as the cursor is; a receiver that gets none draws no cursor.
//! A receiver draws nothing of its own for the input: what is typed shows
//! as the channel draws it (decision E.134, which withdrew local echo).
//!
//! It is a record of the audio profile's frame (REQ-AUD-003) of type
//! [`TYPE_CURSOR`], carried on the audio profile's independent data lines
//! (REQ-AUD-007): EN 300 708 Table 16 gives data channel 13 to Datavideo,
//! so the profile's own channel 12 carries it, as a user-defined service of
//! clause 6.7.3.2 (decision E.91). It is always one fragment, so a receiver
//! reads it without reassembly.
//!
//! ```text
//! bytes 0-1  page: the page it is for
//! bytes 2-3  acked: the input count acted on, modulo 65 536, or 0
//! byte 4     row, 1 to 23, or 255 for no cursor
//! byte 5     column, 0 to 39
//! byte 6     reserved: written 0, ignored when read (the echo width
//!            before decision E.134)
//! byte 7     flags: 4 block; bits 1 and 2 reserved, written 0 and
//!            ignored when read (echo and masked before decision E.134)
//! ```

use neoteletext_audio::idl::{FRAGMENT_DATA, packet};
pub use neoteletext_audio::record::TYPE_CURSOR;
use neoteletext_audio::record::{RecordError, record_length, split, write};
use neoteletext_core::consts::{COLUMNS, PACKET_FIRST_ROW};
use neoteletext_core::packet::Packet;

use crate::carriage::input_record;
use crate::is_page;

/// Flag: draw a block cursor rather than a bar.
pub const BLOCK: u8 = 4;
/// The flags that once asked for echo and masked echo (decision E.134):
/// a channel written before their withdrawal may still set them, and its
/// cursor is read all the same, without them.
const WITHDRAWN_FLAGS: u8 = 1 | 2;
/// The row value for no cursor.
pub const NO_CURSOR: u8 = 255;
/// The last row a cursor may be on: row 24 is the Fastext row, which a
/// receiver draws itself, and row 0 is the header.
pub const LAST_ROW: u8 = 23;
/// The body's bytes.
pub const BODY: usize = 8;
/// The whole record's bytes, frame and CRC included.
pub const RECORD: usize = record_length(BODY);
const _: () = assert!(RECORD <= FRAGMENT_DATA, "a cursor record is one fragment");

/// Where the cursor is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Place {
    /// 1 to [`LAST_ROW`].
    pub row: u8,
    /// 0 to 39.
    pub column: u8,
}

/// A cursor record, checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cursor {
    /// The page it is for; a receiver ignores it for any other.
    pub page: u16,
    /// The input count the channel has acted on, modulo 65 536 and
    /// compared as a serial number ([`Session::acknowledged`]), or 0 for
    /// none yet.
    ///
    /// [`Session::acknowledged`]: crate::session::SessionOf::acknowledged
    pub acked: u16,
    /// The cursor, or `None` for no cursor: a game sends that for the
    /// acknowledgement alone.
    pub place: Option<Place>,
    /// [`BLOCK`], or 0.
    pub flags: u8,
}

/// Why a cursor record was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CursorError {
    /// The frame: length, CRC or version (REQ-AUD-003).
    Frame(RecordError),
    /// A record of another type: not a cursor record.
    NotCursor(u8),
    /// A body of the wrong length.
    Length,
    /// A field outside its range, and which.
    BadValue(&'static str),
}

impl Cursor {
    /// The body's bytes.
    #[must_use]
    pub fn body(&self) -> [u8; BODY] {
        let [p0, p1] = self.page.to_be_bytes();
        let [a0, a1] = self.acked.to_be_bytes();
        let (row, column) = self.place.map_or((NO_CURSOR, 0), |place| (place.row, place.column));
        [p0, p1, a0, a1, row, column, 0, self.flags]
    }

    /// The whole record, or `None` when a field is outside its range.
    #[must_use]
    pub fn record(&self) -> Option<[u8; RECORD]> {
        let mut out = [0; RECORD];
        write(TYPE_CURSOR, &self.body(), &mut out)?;
        // The writer checks with the reader, so the two cannot disagree.
        (read(&out).ok()? == *self).then_some(out)
    }

    /// The packet 4/31 that carries the record as record `record_id` of
    /// user-defined audio service `service`, or `None` when a field is
    /// outside its range.
    #[must_use]
    pub fn packet(&self, service: u8, record_id: u8) -> Option<Packet> {
        Some(packet(service, record_id, 0, true, &self.record()?))
    }
}

/// Reads one whole cursor record, frame and CRC included.
///
/// # Errors
/// A frame that fails REQ-AUD-003, a record of another type, or any field
/// outside its range (REQ-INT-005).
pub fn read(bytes: &[u8]) -> Result<Cursor, CursorError> {
    let (kind, body) = split(bytes).map_err(CursorError::Frame)?;
    if kind != TYPE_CURSOR {
        return Err(CursorError::NotCursor(kind));
    }
    let body: [u8; BODY] = body.try_into().map_err(|_| CursorError::Length)?;
    from_body(body)
}

/// Checks a cursor given as its body's fields, as a TTI page's `CU` line
/// gives it (REQ-INT-006).
///
/// # Errors
/// Any field outside its range (REQ-INT-005).
pub fn from_body(body: [u8; BODY]) -> Result<Cursor, CursorError> {
    // Byte 6, the reserved byte, is not read.
    let [p0, p1, a0, a1, row, column, _, flags] = body;
    let page = u16::from_be_bytes([p0, p1]);
    if !is_page(page) {
        return Err(CursorError::BadValue("page"));
    }
    let flags = flags & !WITHDRAWN_FLAGS;
    if flags & !BLOCK != 0 {
        return Err(CursorError::BadValue("flags"));
    }
    let place = if row == NO_CURSOR {
        if column != 0 || flags != 0 {
            return Err(CursorError::BadValue("a cursor's fields with no cursor"));
        }
        None
    } else {
        if !(PACKET_FIRST_ROW..=LAST_ROW).contains(&row) {
            return Err(CursorError::BadValue("row"));
        }
        if usize::from(column) >= COLUMNS {
            return Err(CursorError::BadValue("column"));
        }
        Some(Place { row, column })
    };
    Ok(Cursor { page, acked: u16::from_be_bytes([a0, a1]), place, flags })
}

/// The cursor record `packet` carries, or `None` when the packet is not a
/// one-fragment record of type [`TYPE_CURSOR`] on the audio profile's data
/// channel, which is everything else the channel carries.
///
/// # Errors
/// A cursor record that is damaged or out of range.
#[must_use]
pub fn from_packet(packet: &Packet) -> Option<Result<Cursor, CursorError>> {
    let fragment = input_record(packet, |kind| kind == TYPE_CURSOR)?;
    Some(read(fragment.data.get(..RECORD).unwrap_or(&[])))
}

#[cfg(test)]
mod tests;
