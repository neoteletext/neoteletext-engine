//! The server-sent events stream parser (HTML Living Standard section
//! 9.2.6, "Interpreting an event stream"), for push over `text/event-stream`
//! (REQ-WEB-005).
//!
//! The parser streams: it never holds a whole line. Field names are
//! matched as they arrive, `data` values are appended to the caller's
//! message buffer (whose length is the push message limit), and `event` and
//! `id` values go to small fixed buffers. Lines may end in CR LF, LF or CR,
//! split anywhere across pieces. The `retry` field is ignored: REQ-WEB-005
//! fixes the reconnection delays.

use crate::bytes::TextBuf;
use crate::consts::{EVENT_PACKETS, EVENT_PAGE, MAX_EVENT_ID, MAX_EVENT_TYPE};

/// The byte order mark the stream may start with.
const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
/// The longest field name this parser recognises ("event" is the longest).
const MAX_FIELD_NAME: usize = 5;

/// A dispatched event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SseEvent {
    /// A `page` event: a TTI page of this length in the message buffer.
    Page(usize),
    /// A `packets` event: base64 T42 packets of this length in the message
    /// buffer, line feeds included where the data had several lines.
    Packets(usize),
    /// An event of another type, ignored.
    Other,
}

/// Why the stream was abandoned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SseError {
    /// One event's data exceeded the push message limit.
    TooBig,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Data,
    Event,
    Id,
    /// A comment, `retry`, or an unknown field.
    Ignored,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    /// At the start of the stream, possibly before a byte order mark.
    Bom(usize),
    /// At the start of a line.
    LineStart,
    /// Reading a field name.
    Name,
    /// After the colon, where one space is skipped.
    ValueStart(Field),
    /// Reading a value.
    Value(Field),
}

/// The event stream parser.
#[derive(Clone, Copy, Debug)]
// Each flag is one piece of parser state named by HTML section 9.2.6.
#[allow(clippy::struct_excessive_bools)]
pub struct SseParser {
    state: State,
    name: [u8; MAX_FIELD_NAME],
    name_len: usize,
    /// True when the name is longer than any recognised field.
    name_overflow: bool,
    /// True after a CR, so that a following LF is not a second line end.
    after_cr: bool,
    data_len: usize,
    /// True when the data had at least one `data` line.
    has_data: bool,
    event_type: TextBuf<MAX_EVENT_TYPE>,
    event_type_overflow: bool,
    id_buffer: TextBuf<MAX_EVENT_ID>,
    id_pending: TextBuf<MAX_EVENT_ID>,
    id_invalid: bool,
    last_event_id: TextBuf<MAX_EVENT_ID>,
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SseParser {
    /// A parser at the start of a stream.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: State::Bom(0),
            name: [0; MAX_FIELD_NAME],
            name_len: 0,
            name_overflow: false,
            after_cr: false,
            data_len: 0,
            has_data: false,
            event_type: TextBuf::new(),
            event_type_overflow: false,
            id_buffer: TextBuf::new(),
            id_pending: TextBuf::new(),
            id_invalid: false,
            last_event_id: TextBuf::new(),
        }
    }

    /// Starts a new stream after a reconnection, keeping the last event
    /// identifier for the `Last-Event-ID` header (REQ-WEB-005).
    pub fn restart(&mut self) {
        let last = self.last_event_id;
        *self = Self::new();
        self.last_event_id = last;
        self.id_buffer = last;
    }

    /// The last event identifier, empty when none was given.
    #[must_use]
    pub fn last_event_id(&self) -> &[u8] {
        self.last_event_id.as_bytes()
    }

    /// Feeds bytes. Returns the bytes used and, when an event was
    /// dispatched, the event; its data is in `message`.
    pub fn feed(&mut self, message: &mut [u8], input: &[u8]) -> Result<(usize, Option<SseEvent>), SseError> {
        for (index, &byte) in input.iter().enumerate() {
            if let Some(event) = self.byte(message, byte)? {
                return Ok((index.saturating_add(1), Some(event)));
            }
        }
        Ok((input.len(), None))
    }

    fn byte(&mut self, message: &mut [u8], byte: u8) -> Result<Option<SseEvent>, SseError> {
        if let State::Bom(seen) = self.state {
            if BOM.get(seen) == Some(&byte) {
                self.state = if seen.saturating_add(1) == BOM.len() {
                    State::LineStart
                } else {
                    State::Bom(seen.saturating_add(1))
                };
                return Ok(None);
            }
            // Not a byte order mark: the bytes seen so far were the start
            // of a line, which a BOM prefix can never be in a valid stream.
            self.state = State::LineStart;
        }
        if byte == b'\n' && self.after_cr {
            self.after_cr = false;
            return Ok(None);
        }
        self.after_cr = byte == b'\r';
        if byte == b'\r' || byte == b'\n' {
            return self.end_line(message);
        }
        match self.state {
            State::Bom(_) | State::LineStart => {
                if byte == b':' {
                    self.state = State::Value(Field::Ignored);
                } else {
                    self.name_len = 0;
                    self.name_overflow = false;
                    self.state = State::Name;
                    self.name_byte(byte);
                }
            }
            State::Name => {
                if byte == b':' {
                    let field = self.field();
                    self.begin_value(field);
                    self.state = State::ValueStart(field);
                } else {
                    self.name_byte(byte);
                }
            }
            State::ValueStart(field) => {
                self.state = State::Value(field);
                if byte != b' ' {
                    self.value_byte(message, field, byte)?;
                }
            }
            State::Value(field) => self.value_byte(message, field, byte)?,
        }
        Ok(None)
    }

    fn name_byte(&mut self, byte: u8) {
        match self.name.get_mut(self.name_len) {
            Some(slot) => {
                *slot = byte;
                self.name_len = self.name_len.saturating_add(1);
            }
            None => self.name_overflow = true,
        }
    }

    fn field(&self) -> Field {
        if self.name_overflow {
            return Field::Ignored;
        }
        match self.name.get(..self.name_len).unwrap_or(&[]) {
            b"data" => Field::Data,
            b"event" => Field::Event,
            b"id" => Field::Id,
            _ => Field::Ignored,
        }
    }

    /// Prepares for a field's value (a field with no colon has an empty
    /// value, handled at the end of the line).
    fn begin_value(&mut self, field: Field) {
        match field {
            Field::Data => {
                // Section 9.2.6: each data line adds a line feed before the
                // next one; it is added here, before the second and later.
                self.has_data = true;
            }
            Field::Event => {
                self.event_type.clear();
                self.event_type_overflow = false;
            }
            Field::Id => {
                self.id_pending.clear();
                self.id_invalid = false;
            }
            Field::Ignored => {}
        }
    }

    fn append_data(&mut self, message: &mut [u8], byte: u8) -> Result<(), SseError> {
        let slot = message.get_mut(self.data_len).ok_or(SseError::TooBig)?;
        *slot = byte;
        self.data_len = self.data_len.saturating_add(1);
        Ok(())
    }

    fn value_byte(&mut self, message: &mut [u8], field: Field, byte: u8) -> Result<(), SseError> {
        match field {
            Field::Data => self.append_data(message, byte)?,
            Field::Event => {
                if self.event_type.push(&[byte]).is_err() {
                    self.event_type_overflow = true;
                }
            }
            Field::Id => {
                if byte.is_ascii_control() || self.id_pending.push(&[byte]).is_err() {
                    // Section 9.2.6: an id containing NUL is ignored; so is
                    // one with any other control byte or DEL, which could not
                    // be sent back as `Last-Event-ID` (RFC 9110 section 5.5,
                    // decision E.114 item 7, finding F-76), and one too long
                    // to keep.
                    self.id_invalid = true;
                }
            }
            Field::Ignored => {}
        }
        Ok(())
    }

    fn end_line(&mut self, message: &mut [u8]) -> Result<Option<SseEvent>, SseError> {
        let state = self.state;
        self.state = State::LineStart;
        match state {
            State::Bom(_) | State::LineStart => return Ok(self.dispatch()),
            State::Name => {
                // A line with no colon: the whole line is the field name and
                // the value is empty.
                let field = self.field();
                self.begin_value(field);
                self.finish_field(message, field)?;
            }
            State::ValueStart(field) | State::Value(field) => self.finish_field(message, field)?,
        }
        Ok(None)
    }

    fn finish_field(&mut self, message: &mut [u8], field: Field) -> Result<(), SseError> {
        match field {
            Field::Data => self.append_data(message, b'\n'),
            Field::Id => {
                // An id that is not UTF-8 is ignored too (finding F-76).
                if !self.id_invalid && core::str::from_utf8(self.id_pending.as_bytes()).is_ok() {
                    self.id_buffer = self.id_pending;
                }
                Ok(())
            }
            Field::Event | Field::Ignored => Ok(()),
        }
    }

    /// Dispatches at a blank line (section 9.2.6 "dispatch the event").
    fn dispatch(&mut self) -> Option<SseEvent> {
        self.last_event_id = self.id_buffer;
        let had_data = self.has_data;
        // Remove the final line feed.
        let len = self.data_len.saturating_sub(1);
        let kind = if self.event_type_overflow { b"".as_slice() } else { self.event_type.as_bytes() };
        let event = if kind == EVENT_PAGE {
            SseEvent::Page(len)
        } else if kind == EVENT_PACKETS {
            SseEvent::Packets(len)
        } else {
            SseEvent::Other
        };
        self.data_len = 0;
        self.has_data = false;
        self.event_type.clear();
        self.event_type_overflow = false;
        had_data.then_some(event)
    }
}

#[cfg(test)]
mod tests;
