//! The key record (REQ-INT-001, REQ-INT-002): one key event, sent by a
//! receiver to a channel as one binary WebSocket message, as a terminal
//! sends a key to its host.
//!
//! ```text
//! byte 0       version (1)
//! byte 1       kind
//! bytes 2-3    sequence number, from 1, wrapping
//! bytes 4-7    the receiver's clock when the key happened, milliseconds, wrapping
//! bytes 8-9    a
//! bytes 10-11  b
//! bytes 12-13  c
//! bytes 14-    text, kind 3 only: UTF-8, 1 to 32 bytes
//! ```
//!
//! Integers are big-endian. A record is refused whole when any field is
//! outside its range.

use neoteletext_core::consts::{COLUMNS, ROWS};

use crate::is_page;
use crate::names::{character_key, is_named, key_name};

/// The version every key record carries.
pub const VERSION: u8 = 1;
/// The fixed part of a record.
pub const HEADER: usize = 14;
/// The longest text of a kind 3 record, in bytes of UTF-8.
pub const MAX_TEXT: usize = 32;
/// The longest record: one WebSocket message.
pub const MAX_RECORD: usize = HEADER + MAX_TEXT;
/// The most records a second a sender sends (REQ-INT-004): qrkeyboard's
/// relay limit, and more than any typist. Beyond it keys are dropped, but
/// never a key-up.
pub const MAX_PER_SECOND: u32 = 120;

/// Kind 1: a key went down.
pub const KIND_DOWN: u8 = 1;
/// Kind 2: a key came up.
pub const KIND_UP: u8 = 2;
/// Kind 3: text an input method or a phone committed.
pub const KIND_TEXT: u8 = 3;
/// Kind 4: the viewer opened a page, or none.
pub const KIND_PAGE: u8 = 4;
/// Kind 5: a click on a cell of the open page.
pub const KIND_CLICK: u8 = 5;
/// Kind 6: the keys left the page or came back to it.
pub const KIND_FOCUS: u8 = 6;
/// Kind 7: the connection was made again; the last sequence number sent.
pub const KIND_RESUME: u8 = 7;

/// Modifier: Shift.
pub const SHIFT: u16 = 1;
/// Modifier: Control.
pub const CONTROL: u16 = 2;
/// Modifier: Alt, which macOS calls Option.
pub const ALT: u16 = 4;
/// Not a modifier: the key went down again because it is held. Value 8 is
/// left for Meta, which a receiver keeps for its own shortcuts and never
/// sends.
pub const REPEAT: u16 = 16;
const ALL_MODIFIERS: u16 = SHIFT | CONTROL | ALT | REPEAT;

/// A key went down or up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Press {
    /// What the key means: a character's scalar value, or a named key
    /// ([`crate::names`]).
    pub key: u16,
    /// [`SHIFT`], [`CONTROL`], [`ALT`] and [`REPEAT`].
    pub modifiers: u16,
    /// Where the key is ([`crate::names::PHYSICAL_NAMES`]), 0 when unknown.
    pub physical: u16,
}

/// What a record says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event<'a> {
    /// A key went down.
    Down(Press),
    /// A key came up.
    Up(Press),
    /// Text committed by an input method or a phone.
    Text(&'a str),
    /// The viewer opened this page, or none.
    Page(Option<u16>),
    /// A click on the cell at this column and row.
    Click {
        /// 0 to 39.
        column: u8,
        /// 0 to 24.
        row: u8,
    },
    /// The keys came to the page (true) or left it (false).
    Focus(bool),
    /// The connection was made again; the last sequence number sent
    /// before. The channel answers by pushing the open page again.
    Resume(u16),
}

/// One key record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Record<'a> {
    /// Counts records of the connection from 1, wrapping.
    pub sequence: u16,
    /// The sender's clock, milliseconds from any start, wrapping: only
    /// differences mean anything, and it is never the time of day.
    pub clock_ms: u32,
    /// What happened.
    pub event: Event<'a>,
}

/// Why a record was refused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyError {
    /// Shorter than [`HEADER`], or than its kind needs.
    Truncated,
    /// Longer than its kind allows.
    TooLong,
    /// A version this receiver does not read.
    Version(u8),
    /// A kind that does not exist.
    UnknownKind(u8),
    /// A field outside its range, and which.
    BadValue(&'static str),
}

impl Record<'_> {
    /// Writes the record into `out` and returns its length, or `None`
    /// when a field is outside its range: a sender can never write what a
    /// receiver would refuse.
    pub fn write(&self, out: &mut [u8; MAX_RECORD]) -> Option<usize> {
        let (kind, a, b, c, text): (u8, u16, u16, u16, &[u8]) = match self.event {
            Event::Down(press) => (KIND_DOWN, press.key, press.modifiers, press.physical, &[]),
            Event::Up(press) => (KIND_UP, press.key, press.modifiers, press.physical, &[]),
            Event::Text(text) => (KIND_TEXT, 0, 0, 0, text.as_bytes()),
            Event::Page(page) => (KIND_PAGE, page.unwrap_or(0), 0, 0, &[]),
            Event::Click { column, row } => (KIND_CLICK, u16::from(column), u16::from(row), 0, &[]),
            Event::Focus(on) => (KIND_FOCUS, u16::from(on), 0, 0, &[]),
            Event::Resume(last) => (KIND_RESUME, last, 0, 0, &[]),
        };
        let length = HEADER.checked_add(text.len())?;
        let record = out.get_mut(..length)?;
        let (header, tail) = record.split_at_mut(HEADER);
        let [s0, s1] = self.sequence.to_be_bytes();
        let [t0, t1, t2, t3] = self.clock_ms.to_be_bytes();
        let [a0, a1] = a.to_be_bytes();
        let [b0, b1] = b.to_be_bytes();
        let [c0, c1] = c.to_be_bytes();
        header.copy_from_slice(&[VERSION, kind, s0, s1, t0, t1, t2, t3, a0, a1, b0, b1, c0, c1]);
        tail.copy_from_slice(text);
        // The writer checks with the reader, so the two cannot disagree.
        read(record).ok()?;
        Some(length)
    }
}

/// Reads one record, exactly `bytes` long.
///
/// # Errors
/// Any failure of REQ-INT-001 and REQ-INT-002: the length, the version, the
/// kind or any field.
pub fn read(bytes: &[u8]) -> Result<Record<'_>, KeyError> {
    if bytes.len() > MAX_RECORD {
        return Err(KeyError::TooLong);
    }
    let header = bytes.get(..HEADER).ok_or(KeyError::Truncated)?;
    let text = bytes.get(HEADER..).unwrap_or(&[]);
    let byte = |at: usize| header.get(at).copied().unwrap_or(0);
    let word = |at: usize| u16::from_be_bytes([byte(at), byte(at.saturating_add(1))]);
    let version = byte(0);
    if version != VERSION {
        return Err(KeyError::Version(version));
    }
    let kind = byte(1);
    let sequence = word(2);
    let clock_ms = u32::from_be_bytes([byte(4), byte(5), byte(6), byte(7)]);
    let (a, b, c) = (word(8), word(10), word(12));
    if kind != KIND_TEXT && !text.is_empty() {
        return Err(KeyError::TooLong);
    }
    let unused = |count: u8| {
        let fields = [a, b, c];
        if fields.iter().skip(usize::from(count)).all(|&field| field == 0) {
            Ok(())
        } else {
            Err(KeyError::BadValue("unused field"))
        }
    };
    let event = match kind {
        KIND_DOWN | KIND_UP => {
            let press = Press { key: a, modifiers: b, physical: c };
            check_key(press.key)?;
            if press.modifiers & !ALL_MODIFIERS != 0 {
                return Err(KeyError::BadValue("modifiers"));
            }
            if kind == KIND_UP && press.modifiers & REPEAT != 0 {
                return Err(KeyError::BadValue("repeat on a key-up"));
            }
            if press.physical != 0 && crate::names::physical_name(press.physical).is_none() {
                return Err(KeyError::BadValue("physical key"));
            }
            if kind == KIND_DOWN { Event::Down(press) } else { Event::Up(press) }
        }
        KIND_TEXT => {
            unused(0)?;
            if text.is_empty() {
                return Err(KeyError::Truncated);
            }
            let text = core::str::from_utf8(text).map_err(|_| KeyError::BadValue("text"))?;
            if text.chars().any(char::is_control) {
                return Err(KeyError::BadValue("control character in text"));
            }
            Event::Text(text)
        }
        KIND_PAGE => {
            unused(1)?;
            match a {
                0 => Event::Page(None),
                page if is_page(page) => Event::Page(Some(page)),
                _ => return Err(KeyError::BadValue("page")),
            }
        }
        KIND_CLICK => {
            unused(2)?;
            let column = u8::try_from(a).ok().filter(|column| usize::from(*column) < COLUMNS);
            let row = u8::try_from(b).ok().filter(|row| usize::from(*row) < ROWS);
            match (column, row) {
                (Some(column), Some(row)) => Event::Click { column, row },
                _ => return Err(KeyError::BadValue("cell")),
            }
        }
        KIND_FOCUS => {
            unused(1)?;
            match a {
                0 => Event::Focus(false),
                1 => Event::Focus(true),
                _ => return Err(KeyError::BadValue("focus")),
            }
        }
        KIND_RESUME => {
            unused(1)?;
            Event::Resume(a)
        }
        other => return Err(KeyError::UnknownKind(other)),
    };
    Ok(Record { sequence, clock_ms, event })
}

fn check_key(key: u16) -> Result<(), KeyError> {
    if is_named(key) {
        return key_name(key).map(|_| ()).ok_or(KeyError::BadValue("named key"));
    }
    let printable = char::from_u32(u32::from(key)).and_then(character_key).is_some();
    if printable { Ok(()) } else { Err(KeyError::BadValue("key")) }
}

/// Counts records against [`MAX_PER_SECOND`] over a sliding second: a
/// sender asks before each record, and a key-up is always allowed, so no
/// key is left held at the channel.
#[derive(Clone, Copy, Debug)]
pub struct Limiter {
    /// When each of the last [`MAX_PER_SECOND`] records was sent, a ring.
    sent_ms: [u32; MAX_PER_SECOND as usize],
    next: usize,
    count: usize,
}

/// The window the limiter counts over.
const SECOND_MS: u32 = 1_000;

impl Default for Limiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Limiter {
    /// A limiter that has sent nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { sent_ms: [0; MAX_PER_SECOND as usize], next: 0, count: 0 }
    }

    /// Whether a record may go at `now_ms`; a record that may go is
    /// counted. `key_up` records always go, and are counted.
    pub fn allow(&mut self, now_ms: u32, key_up: bool) -> bool {
        let full = self.count == self.sent_ms.len();
        let oldest = if full { self.sent_ms.get(self.next).copied() } else { None };
        let recent = oldest.is_some_and(|oldest| now_ms.wrapping_sub(oldest) < SECOND_MS);
        if recent && !key_up {
            return false;
        }
        if let Some(slot) = self.sent_ms.get_mut(self.next) {
            *slot = now_ms;
        }
        self.next = self.next.saturating_add(1).checked_rem(self.sent_ms.len()).unwrap_or(0);
        self.count = self.count.saturating_add(1).min(self.sent_ms.len());
        true
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    // Tests of REQ-INT-001, REQ-INT-002 and REQ-INT-004.
    use super::*;
    use crate::names::{NAMED_BASE, named_key, physical_key};

    fn round_trip(event: Event<'_>) -> std::vec::Vec<u8> {
        let record = Record { sequence: 0xFFFF, clock_ms: 0xFFFF_FFF0, event };
        let mut out = [0; MAX_RECORD];
        let length = record.write(&mut out).unwrap();
        assert_eq!(read(&out[..length]), Ok(record));
        out[..length].to_vec()
    }

    #[test]
    fn every_kind_round_trips() {
        let enter = Press { key: named_key("Enter").unwrap(), modifiers: 0, physical: physical_key("Enter") };
        let bytes = round_trip(Event::Down(enter));
        assert_eq!(bytes.len(), HEADER);
        assert_eq!(&bytes[..8], &[VERSION, KIND_DOWN, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xF0]);
        round_trip(Event::Up(Press { key: u16::from(b'w'), modifiers: SHIFT | CONTROL | ALT, physical: 42 }));
        round_trip(Event::Down(Press { key: u16::from(b'w'), modifiers: REPEAT, physical: 0 }));
        assert_eq!(round_trip(Event::Text("héllo wörld, 32 bytes of text!")).len(), HEADER + 32);
        round_trip(Event::Page(Some(0x302)));
        round_trip(Event::Page(None));
        round_trip(Event::Click { column: 39, row: 24 });
        round_trip(Event::Focus(true));
        round_trip(Event::Focus(false));
        round_trip(Event::Resume(77));
    }

    #[test]
    fn the_largest_record_is_46_bytes() {
        assert_eq!(MAX_RECORD, 46);
        let text = "0123456789abcdef0123456789abcdef!";
        let record = Record { sequence: 1, clock_ms: 0, event: Event::Text(text) };
        assert_eq!(record.write(&mut [0; MAX_RECORD]), None, "33 bytes of text");
        let mut long = [0_u8; MAX_RECORD + 1];
        long[0] = VERSION;
        long[1] = KIND_TEXT;
        assert_eq!(read(&long), Err(KeyError::TooLong));
    }

    #[test]
    fn every_field_is_checked() {
        let base = |kind: u8, a: u16, b: u16, c: u16| {
            let mut bytes = [0_u8; HEADER];
            bytes[0] = VERSION;
            bytes[1] = kind;
            bytes[8..10].copy_from_slice(&a.to_be_bytes());
            bytes[10..12].copy_from_slice(&b.to_be_bytes());
            bytes[12..14].copy_from_slice(&c.to_be_bytes());
            bytes
        };
        let refused = |bytes: &[u8]| read(bytes).is_err();
        let mut wrong_version = base(KIND_DOWN, 0x61, 0, 0);
        wrong_version[0] = 2;
        assert_eq!(read(&wrong_version), Err(KeyError::Version(2)));
        assert_eq!(read(&base(8, 0, 0, 0)), Err(KeyError::UnknownKind(8)));
        assert_eq!(read(&base(KIND_DOWN, 0x61, 0, 0)[..13]), Err(KeyError::Truncated));
        assert!(refused(&base(KIND_DOWN, 0x0D, 0, 0)), "a control character is a named key");
        assert!(refused(&base(KIND_DOWN, 0xD800, 0, 0)), "a surrogate");
        assert!(refused(&base(KIND_DOWN, NAMED_BASE + 0xF0, 0, 0)), "a named key beyond the table");
        assert!(refused(&base(KIND_DOWN, 0x61, 8, 0)), "Meta never goes");
        assert!(refused(&base(KIND_UP, 0x61, REPEAT, 0)));
        assert!(refused(&base(KIND_DOWN, 0x61, 0, 5000)), "a physical key beyond the table");
        assert!(refused(&base(KIND_PAGE, 0x099, 0, 0)));
        assert!(refused(&base(KIND_PAGE, 0x900, 0, 0)));
        assert!(refused(&base(KIND_PAGE, 0x100, 1, 0)), "an unused field set");
        assert!(refused(&base(KIND_CLICK, 40, 0, 0)));
        assert!(refused(&base(KIND_CLICK, 0, 25, 0)));
        assert!(refused(&base(KIND_FOCUS, 2, 0, 0)));
        assert!(refused(&base(KIND_TEXT, 0, 0, 0)), "text with no text");
        let mut control = std::vec::Vec::from(base(KIND_TEXT, 0, 0, 0));
        control.extend_from_slice(b"a\nb");
        assert!(refused(&control));
        let mut invalid = std::vec::Vec::from(base(KIND_TEXT, 0, 0, 0));
        invalid.push(0xFF);
        assert!(refused(&invalid));
        let mut trailing = std::vec::Vec::from(base(KIND_DOWN, 0x61, 0, 0));
        trailing.push(b'a');
        assert_eq!(read(&trailing), Err(KeyError::TooLong));
    }

    #[test]
    fn the_limiter_drops_the_121st_key_in_a_second_but_never_a_key_up() {
        let mut limiter = Limiter::new();
        for n in 0..MAX_PER_SECOND {
            assert!(limiter.allow(1_000 + n, false));
        }
        assert!(!limiter.allow(1_500, false));
        assert!(limiter.allow(1_500, true), "a key-up always goes");
        // The first record was sent at 1 000 ms; a second later its slot
        // is free again.
        assert!(limiter.allow(2_001, false));
    }

    #[test]
    fn the_limiter_survives_the_clock_wrapping() {
        let mut limiter = Limiter::new();
        for n in 0..MAX_PER_SECOND {
            assert!(limiter.allow(u32::MAX - 500 + n, false));
        }
        assert!(!limiter.allow(u32::MAX, false));
        assert!(limiter.allow(600, false));
    }
}
