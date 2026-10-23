//! The record frame every record type shares, the input profile's too
//! (REQ-AUD-003): version, type, length, body and CRC-32, and the reader
//! of a body's fields.

use crate::crc::crc32_parts;
use crate::{MAX_BODY, MAX_RECORD, PROFILE_VERSION, RECORD_CRC, RECORD_HEADER};

/// Why a record was refused. A refused record is discarded whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordError {
    /// Shorter than its header and CRC, or than its length says.
    Truncated,
    /// Longer than its length says.
    TrailingBytes,
    /// A body longer than [`MAX_BODY`].
    TooLong,
    /// A version this receiver does not read.
    Version(u8),
    /// A record type that does not exist.
    UnknownType(u8),
    /// The CRC-32 does not match.
    Crc,
    /// A field outside its range, and which.
    BadValue(&'static str),
}

/// The length of the record that starts `bytes`, once its header is there:
/// what a file reader needs to frame the next record.
///
/// # Errors
/// [`RecordError::Truncated`] before the header is complete, and
/// [`RecordError::TooLong`] for a length beyond the bound.
pub fn framed_length(bytes: &[u8]) -> Result<usize, RecordError> {
    let header = bytes.get(..RECORD_HEADER).ok_or(RecordError::Truncated)?;
    let body =
        usize::from(u16::from_be_bytes([header.get(2).copied().unwrap_or(0), header.get(3).copied().unwrap_or(0)]));
    if body > MAX_BODY {
        return Err(RecordError::TooLong);
    }
    Ok(record_length(body))
}

/// The bytes of a record with a body of `body` bytes.
#[must_use]
pub const fn record_length(body: usize) -> usize {
    RECORD_HEADER.saturating_add(body).saturating_add(RECORD_CRC)
}

/// Checks one whole record's frame, exactly `bytes` long, and returns its
/// type and body: the part of [`parse`](super::parse) that every record type shares, for
/// the profiles carried with this one.
///
/// # Errors
/// The frame's failures of REQ-AUD-003: the length, the CRC or the version.
pub fn split(bytes: &[u8]) -> Result<(u8, &[u8]), RecordError> {
    if bytes.len() > MAX_RECORD {
        return Err(RecordError::TooLong);
    }
    let length = framed_length(bytes)?;
    if bytes.len() < length {
        return Err(RecordError::Truncated);
    }
    if bytes.len() > length {
        return Err(RecordError::TrailingBytes);
    }
    let (header, rest) = bytes.split_at(RECORD_HEADER);
    let (body, crc) = rest.split_at(rest.len().saturating_sub(RECORD_CRC));
    let crc: [u8; RECORD_CRC] = crc.try_into().map_err(|_| RecordError::Truncated)?;
    if crc32_parts(header, body) != u32::from_be_bytes(crc) {
        return Err(RecordError::Crc);
    }
    let version = header.first().copied().unwrap_or(0);
    if version != PROFILE_VERSION {
        return Err(RecordError::Version(version));
    }
    Ok((header.get(1).copied().unwrap_or(0), body))
}

/// Writes a record of type `kind` around `body` into `out` and returns its
/// length, or `None` when the body is over [`MAX_BODY`] or `out` is short.
/// It allocates nothing.
pub fn write(kind: u8, body: &[u8], out: &mut [u8]) -> Option<usize> {
    if body.len() > MAX_BODY {
        return None;
    }
    let length = record_length(body.len());
    let record = out.get_mut(..length)?;
    let (header, rest) = record.split_at_mut(RECORD_HEADER);
    let (target, crc) = rest.split_at_mut(body.len());
    let [high, low] = u16::try_from(body.len()).ok()?.to_be_bytes();
    header.copy_from_slice(&[PROFILE_VERSION, kind, high, low]);
    target.copy_from_slice(body);
    crc.copy_from_slice(&crc32_parts(header, target).to_be_bytes());
    Some(length)
}

/// Reads fields from a body, failing on any short read.
pub(super) struct Fields<'a> {
    bytes: &'a [u8],
}

impl<'a> Fields<'a> {
    /// The fields of `bytes`, read from the start.
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Whether every byte has been read.
    pub(super) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], RecordError> {
        if self.bytes.len() < count {
            return Err(RecordError::Truncated);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RecordError> {
        self.take(N)?.try_into().map_err(|_| RecordError::Truncated)
    }

    pub(super) fn u8(&mut self) -> Result<u8, RecordError> {
        Ok(u8::from_be_bytes(self.array()?))
    }

    pub(super) fn u16(&mut self) -> Result<u16, RecordError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, RecordError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub(super) fn rest(&mut self) -> &'a [u8] {
        core::mem::take(&mut self.bytes)
    }

    pub(super) fn end(&self) -> Result<(), RecordError> {
        if self.bytes.is_empty() { Ok(()) } else { Err(RecordError::TrailingBytes) }
    }
}
