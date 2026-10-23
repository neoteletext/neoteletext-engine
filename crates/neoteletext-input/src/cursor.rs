//! The cursor record (`neoteletext-common/INPUT.md` section 7.1,
//! REQ-INT-005, REQ-INT-006, REQ-INT-110): a channel's answer to the input,
//! saying where the cursor is, whether to echo, and how much input it has
//! acted on. It is optional, as the cursor is; a receiver that gets none
//! draws no cursor and echoes nothing.
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
//! byte 6     width: how far echo may run from the column, 0 to 40 cells
//! byte 7     flags: 1 echo, 2 masked, 4 block
//! ```

use neoteletext_audio::idl::{FRAGMENT_DATA, packet};
pub use neoteletext_audio::record::TYPE_CURSOR;
use neoteletext_audio::record::{RecordError, record_length, split, write};
use neoteletext_core::consts::{COLUMNS, PACKET_FIRST_ROW};
use neoteletext_core::packet::Packet;

use crate::carriage::input_record;
use crate::is_page;

/// Flag: echo printable keys at the cursor until the channel answers.
pub const ECHO: u8 = 1;
/// Flag: echo `*` in place of each character, for a password.
pub const MASKED: u8 = 2;
/// Flag: draw a block cursor rather than a bar.
pub const BLOCK: u8 = 4;
const ALL_FLAGS: u8 = ECHO | MASKED | BLOCK;
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
    /// [`Session::acknowledged`]: crate::session::Session::acknowledged
    pub acked: u16,
    /// The cursor, or `None` for no cursor: a game sends that for the
    /// acknowledgement alone.
    pub place: Option<Place>,
    /// How many cells echo may run from the cursor's column, 0 for none.
    pub width: u8,
    /// [`ECHO`], [`MASKED`] and [`BLOCK`].
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
        [p0, p1, a0, a1, row, column, self.width, self.flags]
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
    let [p0, p1, a0, a1, row, column, width, flags] = body;
    let page = u16::from_be_bytes([p0, p1]);
    if !is_page(page) {
        return Err(CursorError::BadValue("page"));
    }
    if flags & !ALL_FLAGS != 0 {
        return Err(CursorError::BadValue("flags"));
    }
    let place = if row == NO_CURSOR {
        if column != 0 || width != 0 || flags != 0 {
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
        if usize::from(column).saturating_add(usize::from(width)) > COLUMNS {
            return Err(CursorError::BadValue("width"));
        }
        Some(Place { row, column })
    };
    if flags & ECHO != 0 && width == 0 {
        return Err(CursorError::BadValue("echo with no width"));
    }
    if flags & MASKED != 0 && flags & ECHO == 0 {
        return Err(CursorError::BadValue("masked with no echo"));
    }
    Ok(Cursor { page, acked: u16::from_be_bytes([a0, a1]), place, width, flags })
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
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    // Tests of REQ-INT-005 and REQ-INT-006.
    use super::*;
    use neoteletext_audio::record::TYPE_CONTROL;

    fn cursor() -> Cursor {
        Cursor { page: 0x100, acked: 7, place: Some(Place { row: 5, column: 10 }), width: 20, flags: ECHO | BLOCK }
    }

    // REQ-INT-005 and REQ-INT-110.
    #[test]
    fn a_cursor_round_trips_through_a_packet() {
        let packet = cursor().packet(3, 200).unwrap();
        assert_eq!(from_packet(&packet), Some(Ok(cursor())));
        let address = neoteletext_core::packet::decode_address(&packet).unwrap();
        assert_eq!((address.magazine.number(), address.number), (4, 31));
        assert_eq!(RECORD, 16);
    }

    #[test]
    fn no_cursor_carries_the_acknowledgement_alone() {
        let game = Cursor { page: 0x8FF, acked: 65_535, place: None, width: 0, flags: 0 };
        assert_eq!(read(&game.record().unwrap()), Ok(game));
    }

    #[test]
    fn every_field_is_checked() {
        let refused = |change: fn(&mut Cursor)| {
            let mut cursor = cursor();
            change(&mut cursor);
            let mut out = [0; RECORD];
            write(TYPE_CURSOR, &cursor.body(), &mut out).unwrap();
            assert!(read(&out).is_err(), "{cursor:?}");
            assert!(cursor.record().is_none());
        };
        refused(|cursor| cursor.page = 0x0FF);
        refused(|cursor| cursor.page = 0x900);
        refused(|cursor| cursor.place = Some(Place { row: 0, column: 0 }));
        refused(|cursor| cursor.place = Some(Place { row: 24, column: 0 }));
        refused(|cursor| cursor.place = Some(Place { row: 1, column: 40 }));
        refused(|cursor| cursor.width = 31);
        refused(|cursor| cursor.flags = 8);
        refused(|cursor| cursor.flags = MASKED);
        refused(|cursor| {
            cursor.width = 0;
        });
        refused(|cursor| {
            cursor.place = None;
        });
    }

    #[test]
    fn other_records_and_damage_are_not_cursors() {
        let mut out = [0; RECORD];
        write(TYPE_CONTROL, &cursor().body(), &mut out).unwrap();
        assert_eq!(read(&out), Err(CursorError::NotCursor(TYPE_CONTROL)));
        let mut record = cursor().record().unwrap();
        record[6] ^= 1;
        assert!(matches!(read(&record), Err(CursorError::Frame(RecordError::Crc))));
    }

    #[test]
    fn a_sound_on_the_same_channel_is_not_a_cursor() {
        // A CONTROL record is one fragment too.
        let body = [0_u8; 14];
        let mut out = [0; 22];
        let length = write(TYPE_CONTROL, &body, &mut out).unwrap();
        let packet = packet(0, 1, 0, true, &out[..length]);
        assert_eq!(from_packet(&packet), None);
    }
}
