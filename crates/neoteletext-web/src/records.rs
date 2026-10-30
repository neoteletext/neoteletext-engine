//! The channel's records on its push connection beside its pages
//! (`neoteletext-common/INPUT.md` section 7): the index (type 6), LOG
//! (type 7) and `SET_COOKIE` (type 8), in the audio profile's record frame
//! (REQ-AUD-003) on data channel 12, packet 4/31, cut into as many
//! fragments as a record needs (section 7.8).
//!
//! Each record's encoder writes its packets through a callback, so it
//! allocates nothing, and checks what it writes with its reader, so a
//! server can never send what a receiver refuses. [`RecordReader`]
//! reassembles a push stream's fragments and reads the records it carries.

use neoteletext_audio::MAX_RECORD;
use neoteletext_audio::idl::{FRAGMENT_DATA, Reassembler, fragment, packet};
use neoteletext_audio::record::{RecordError, record_length, split, write};
use neoteletext_core::packet::Packet;

pub mod cookie;
pub mod index;
pub mod log;

pub use neoteletext_audio::record::{TYPE_INDEX, TYPE_LOG, TYPE_SET_COOKIE};

/// Why a record was refused: it is discarded whole, and its packets are
/// counted as invalid (REQ-1462, `INPUT.md` section 7.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordFault {
    /// The frame failed: its length, CRC-32 or version.
    Frame(RecordError),
    /// A field outside its range, and which.
    BadValue(&'static str),
    /// Bytes after the last field, or a field cut short.
    Length,
}

impl From<RecordError> for RecordFault {
    fn from(error: RecordError) -> Self {
        Self::Frame(error)
    }
}

/// The fragments a record of `body` bytes takes (section 7.8).
#[must_use]
pub const fn fragments(body: usize) -> usize {
    record_length(body).div_ceil(FRAGMENT_DATA)
}

/// Writes a record of type `kind` around `body` as packets 4/31 of audio
/// service `service`, record id `record_id`, each given to `out` in order,
/// fragment 0 first. Returns the packets written, or `None` when the body
/// is over the frame's largest.
pub fn write_packets(kind: u8, body: &[u8], service: u8, record_id: u8, out: &mut dyn FnMut(Packet)) -> Option<usize> {
    let mut record = [0_u8; MAX_RECORD];
    let length = write(kind, body, &mut record)?;
    let bytes = record.get(..length)?;
    let count = bytes.len().div_ceil(FRAGMENT_DATA);
    for (index, chunk) in bytes.chunks(FRAGMENT_DATA).enumerate() {
        let index = u8::try_from(index).ok()?;
        let last = usize::from(index).saturating_add(1) == count;
        out(packet(service, record_id, index, last, chunk));
    }
    Some(count)
}

/// Reads a big-endian `u16` at the front of `bytes`, and what follows it.
pub(crate) fn take_u16(bytes: &[u8]) -> Result<(u16, &[u8]), RecordFault> {
    let (head, rest) = bytes.split_at_checked(2).ok_or(RecordFault::Length)?;
    let head: [u8; 2] = head.try_into().map_err(|_| RecordFault::Length)?;
    Ok((u16::from_be_bytes(head), rest))
}

/// One record the reader accepted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChannelRecord<'a> {
    /// A part of the channel's index (section 7.5).
    Index(index::IndexRecord<'a>),
    /// A log line for the channel's author (section 7.6).
    Log(log::LogRecord<'a>),
    /// A cookie's `Set-Cookie` value (section 7.7).
    SetCookie(&'a [u8]),
}

/// What one packet gave the reader.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Read<'a> {
    /// Nothing yet: a fragment of a record still incomplete, or a packet
    /// that is not one of these records.
    Nothing,
    /// A whole record, checked.
    Record(ChannelRecord<'a>),
    /// A damaged fragment, or a whole record refused: this many packets
    /// are invalid.
    Invalid(usize),
}

/// Reassembles the index, LOG and `SET_COOKIE` records from a push stream's
/// packets (section 7.8). A record is followed from its fragment 0, which
/// names its type, so the audio profile's records and the one-fragment
/// cursor and control records, which other readers take, are never copied
/// here.
pub struct RecordReader {
    reassembler: Reassembler<1>,
    /// The service and record id of the record being reassembled.
    following: Option<(u8, u8)>,
}

impl Default for RecordReader {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordReader {
    /// A reader with nothing in flight.
    #[must_use]
    pub const fn new() -> Self {
        Self { reassembler: Reassembler::new(), following: None }
    }

    /// Forgets the record in flight, as a new connection does.
    pub fn clear(&mut self) {
        self.reassembler.clear();
        self.following = None;
    }

    /// Reads one packet of the stream.
    pub fn push(&mut self, packet: &Packet) -> Read<'_> {
        let Some(result) = fragment(packet) else { return Read::Nothing };
        let Ok(fragment) = result else { return Read::Invalid(1) };
        if fragment.index == 0 {
            let kind = fragment.data.get(1).copied().unwrap_or(0);
            if !matches!(kind, TYPE_INDEX | TYPE_LOG | TYPE_SET_COOKIE) {
                return Read::Nothing;
            }
            self.reassembler.clear();
            self.following = Some((fragment.service, fragment.record));
        } else if self.following != Some((fragment.service, fragment.record)) {
            return Read::Nothing;
        }
        let Some((_, bytes)) = self.reassembler.push(&fragment) else { return Read::Nothing };
        self.following = None;
        let packets = bytes.len().div_ceil(FRAGMENT_DATA);
        match read_record(bytes) {
            Ok(record) => Read::Record(record),
            Err(_) => Read::Invalid(packets),
        }
    }
}

/// Reads one whole record of the three types, frame and CRC included.
///
/// # Errors
/// The frame's failures, another record type, or any field of the
/// record's layout.
pub fn read_record(bytes: &[u8]) -> Result<ChannelRecord<'_>, RecordFault> {
    let (kind, body) = split(bytes)?;
    match kind {
        TYPE_INDEX => index::read(body).map(ChannelRecord::Index),
        TYPE_LOG => log::read(body).map(ChannelRecord::Log),
        TYPE_SET_COOKIE => cookie::read(body).map(ChannelRecord::SetCookie),
        _ => Err(RecordFault::BadValue("record type")),
    }
}
