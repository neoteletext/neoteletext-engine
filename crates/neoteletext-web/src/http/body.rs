//! The response body as its framing delimits it (RFC 9112 sections 6.3 and
//! 7.1, REQ-SEC-011): `Content-Length`, chunked transfer coding with its
//! trailers ignored, or the end of the connection, under the caller's size
//! limit.

use super::{Framing, HttpError};
use crate::bytes::{copy_prefix, hex_value};
use crate::consts::{CRLF, MAX_CHUNK_LINE, MAX_CHUNK_SIZE_DIGITS};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ChunkState {
    /// Reading a chunk-size line.
    Size,
    /// Reading chunk data.
    Data,
    /// Expecting the CR LF after chunk data.
    DataEnd,
    /// Reading trailer lines after the last chunk.
    Trailer,
}

/// One step of body decoding.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BodyStep<'a> {
    /// Bytes of the input used.
    pub consumed: usize,
    /// Body data found in them, if any.
    pub data: &'a [u8],
    /// The body is complete.
    pub done: bool,
}

/// Decodes a response body according to its framing, enforcing a limit.
#[derive(Clone, Copy, Debug)]
pub struct BodyDecoder {
    framing: Framing,
    limit: usize,
    received: usize,
    remaining: u64,
    chunk: ChunkState,
    line: [u8; MAX_CHUNK_LINE],
    line_len: usize,
    done: bool,
}

impl BodyDecoder {
    /// A decoder for `framing` that accepts at most `limit` body bytes.
    pub fn new(framing: Framing, limit: usize) -> Result<Self, HttpError> {
        let (remaining, done) = match framing {
            Framing::None => (0, true),
            Framing::Length(length) => {
                if usize::try_from(length).map_or(true, |length| length > limit) {
                    return Err(HttpError::BodyTooLarge);
                }
                (length, length == 0)
            }
            Framing::Chunked | Framing::UntilClose => (0, false),
        };
        Ok(Self {
            framing,
            limit,
            received: 0,
            remaining,
            chunk: ChunkState::Size,
            line: [0; MAX_CHUNK_LINE],
            line_len: 0,
            done,
        })
    }

    /// True when the body is complete.
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.done
    }

    /// Body bytes delivered so far.
    #[must_use]
    pub const fn received(&self) -> usize {
        self.received
    }

    fn count(&mut self, bytes: usize) -> Result<(), HttpError> {
        self.received = self.received.checked_add(bytes).ok_or(HttpError::BodyTooLarge)?;
        if self.received > self.limit { Err(HttpError::BodyTooLarge) } else { Ok(()) }
    }

    /// The connection closed: completes a close-delimited body, and is an
    /// error for any other unfinished body.
    pub fn finish_at_close(&mut self) -> Result<(), HttpError> {
        if self.done {
            return Ok(());
        }
        if self.framing == Framing::UntilClose {
            self.done = true;
            Ok(())
        } else {
            Err(HttpError::Truncated)
        }
    }

    /// Decodes from `input`, returning at most one run of body data.
    pub fn feed<'a>(&mut self, input: &'a [u8]) -> Result<BodyStep<'a>, HttpError> {
        if self.done {
            return Ok(BodyStep { consumed: 0, data: &[], done: true });
        }
        match self.framing {
            Framing::None => {
                self.done = true;
                Ok(BodyStep { consumed: 0, data: &[], done: true })
            }
            Framing::UntilClose => {
                self.count(input.len())?;
                Ok(BodyStep { consumed: input.len(), data: input, done: false })
            }
            Framing::Length(_) => {
                let take = usize::try_from(self.remaining).unwrap_or(usize::MAX).min(input.len());
                self.count(take)?;
                self.remaining = self.remaining.saturating_sub(take as u64);
                self.done = self.remaining == 0;
                Ok(BodyStep { consumed: take, data: input.get(..take).unwrap_or(&[]), done: self.done })
            }
            Framing::Chunked => self.feed_chunked(input),
        }
    }

    /// Reads up to a line feed into the line buffer. Returns the bytes used
    /// and whether the line is complete.
    fn read_line(&mut self, input: &[u8]) -> Result<(usize, bool), HttpError> {
        let end = input.iter().position(|&b| b == b'\n');
        let take = end.map_or(input.len(), |i| i.saturating_add(1));
        let new_len = self.line_len.checked_add(take).ok_or(HttpError::BadChunk)?;
        copy_prefix(self.line.get_mut(self.line_len..new_len).ok_or(HttpError::BadChunk)?, input);
        self.line_len = new_len;
        Ok((take, end.is_some()))
    }

    /// The completed line without its CR LF, which must both be present
    /// (RFC 9112 section 7.1).
    fn take_line(&mut self) -> Result<([u8; MAX_CHUNK_LINE], usize), HttpError> {
        let len = self.line_len;
        self.line_len = 0;
        let line = self.line.get(..len).unwrap_or(&[]);
        let content = line.strip_suffix(CRLF).ok_or(HttpError::BadChunk)?;
        if content.contains(&b'\r') {
            return Err(HttpError::BadChunk);
        }
        Ok((self.line, content.len()))
    }

    fn feed_chunked<'a>(&mut self, input: &'a [u8]) -> Result<BodyStep<'a>, HttpError> {
        let mut used = 0usize;
        loop {
            let rest = input.get(used..).unwrap_or(&[]);
            if rest.is_empty() {
                return Ok(BodyStep { consumed: used, data: &[], done: false });
            }
            match self.chunk {
                ChunkState::Size => {
                    let (take, complete) = self.read_line(rest)?;
                    used = used.saturating_add(take);
                    if complete {
                        let (line, len) = self.take_line()?;
                        let size = parse_chunk_size(line.get(..len).unwrap_or(&[]))?;
                        if size == 0 {
                            self.chunk = ChunkState::Trailer;
                        } else {
                            if usize::try_from(size)
                                .map_or(true, |size| size > self.limit.saturating_sub(self.received))
                            {
                                return Err(HttpError::BodyTooLarge);
                            }
                            self.remaining = size;
                            self.chunk = ChunkState::Data;
                        }
                    }
                }
                ChunkState::Data => {
                    let take = usize::try_from(self.remaining).unwrap_or(usize::MAX).min(rest.len());
                    self.count(take)?;
                    self.remaining = self.remaining.saturating_sub(take as u64);
                    if self.remaining == 0 {
                        self.chunk = ChunkState::DataEnd;
                    }
                    let data = rest.get(..take).unwrap_or(&[]);
                    return Ok(BodyStep { consumed: used.saturating_add(take), data, done: false });
                }
                ChunkState::DataEnd => {
                    let (take, complete) = self.read_line(rest)?;
                    used = used.saturating_add(take);
                    if complete {
                        let (_, len) = self.take_line()?;
                        if len != 0 {
                            return Err(HttpError::BadChunk);
                        }
                        self.chunk = ChunkState::Size;
                    } else if self.line_len > CRLF.len() {
                        return Err(HttpError::BadChunk);
                    }
                }
                ChunkState::Trailer => {
                    let (take, complete) = self.read_line(rest)?;
                    used = used.saturating_add(take);
                    if complete {
                        let (_, len) = self.take_line()?;
                        // Trailer fields are read and ignored (RFC 9112
                        // section 7.1.2); the empty line ends the body.
                        if len == 0 {
                            self.done = true;
                            return Ok(BodyStep { consumed: used, data: &[], done: true });
                        }
                    }
                }
            }
        }
    }
}

/// Parses a chunk-size line (RFC 9112 section 7.1): hexadecimal digits,
/// then optionally `;` and extensions, which are ignored. No white space.
fn parse_chunk_size(line: &[u8]) -> Result<u64, HttpError> {
    let digits_end = line.iter().position(|&b| b == b';').unwrap_or(line.len());
    let digits = line.get(..digits_end).unwrap_or(&[]);
    if digits.is_empty() || digits.len() > MAX_CHUNK_SIZE_DIGITS {
        return Err(HttpError::BadChunk);
    }
    let extension = line.get(digits_end..).unwrap_or(&[]);
    if extension.iter().any(|&b| b < 0x20 || b == 0x7F) {
        return Err(HttpError::BadChunk);
    }
    digits.iter().try_fold(0u64, |value, &d| Some((value << 4) | u64::from(hex_value(d)?))).ok_or(HttpError::BadChunk)
}
