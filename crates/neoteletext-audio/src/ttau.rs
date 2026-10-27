//! The file carriage (REQ-AUD-008, `design/audio.md` section 6.2): a
//! `.ttau` file is records back to back, with nothing between or around
//! them. A file is untrusted input: each record is framed by its own length,
//! which is checked against [`MAX_BODY`](crate::MAX_BODY) before anything is
//! read past it, and the reader allocates nothing.
//!
//! A record whose length breaks the bound ends the file, since nothing
//! after it can be framed safely, and a final record cut short is
//! discarded. A record that is framed but damaged (its CRC, version, type or
//! a field) is still handed out, for the receiver to refuse as on any
//! carriage; the file goes on after it.

use crate::record::{RecordError, framed_length};

/// Why a file's records ended.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    /// Every byte was a whole record.
    Complete,
    /// The final record was cut short, and was discarded.
    Truncated,
    /// A record's length broke the bound: what followed was not read.
    BadLength,
}

/// The records of a file, in order, each exactly as long as its header
/// says.
#[derive(Clone, Debug)]
pub struct Records<'a> {
    rest: &'a [u8],
    end: Option<End>,
}

/// The records of `file`.
#[must_use]
pub fn records(file: &[u8]) -> Records<'_> {
    Records { rest: file, end: None }
}

impl Records<'_> {
    /// Why the records ended, once they have; `None` while some may remain.
    #[must_use]
    pub fn end(&self) -> Option<End> {
        self.end
    }

    fn finish(&mut self, end: End) {
        self.end = Some(end);
        self.rest = &[];
    }
}

impl<'a> Iterator for Records<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        if self.end.is_some() {
            return None;
        }
        if self.rest.is_empty() {
            self.finish(End::Complete);
            return None;
        }
        match framed_length(self.rest) {
            Ok(length) => {
                if let Some((record, after)) = self.rest.split_at_checked(length) {
                    self.rest = after;
                    return Some(record);
                }
                self.finish(End::Truncated);
            }
            Err(RecordError::TooLong) => self.finish(End::BadLength),
            // Shorter than a record's header: the final record was cut short.
            Err(_) => self.finish(End::Truncated),
        }
        None
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]
mod tests {
    // Tests of REQ-AUD-008 and REQ-AUD-021.
    use super::*;
    use crate::MAX_BODY;
    use crate::record::{TYPE_CONTROL, write};
    use std::vec::Vec;

    /// A file of records with these bodies.
    fn file(bodies: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for body in bodies {
            let mut record = [0_u8; crate::MAX_RECORD];
            let length = write(TYPE_CONTROL, body, &mut record).unwrap();
            bytes.extend_from_slice(&record[..length]);
        }
        bytes
    }

    #[test]
    fn a_file_is_records_back_to_back() {
        let bytes = file(&[b"one", b"", b"three"]);
        let mut reader = records(&bytes);
        let lengths: Vec<usize> = reader.by_ref().map(<[u8]>::len).collect();
        assert_eq!(lengths, [11, 8, 13]);
        assert_eq!(reader.end(), Some(End::Complete));
        assert_eq!(reader.next(), None, "ended for good");
        let mut empty = records(&[]);
        assert_eq!(empty.next(), None);
        assert_eq!(empty.end(), Some(End::Complete));
    }

    #[test]
    fn a_truncated_final_record_is_discarded() {
        let bytes = file(&[b"one", b"two"]);
        for cut in 1..11 {
            let mut reader = records(&bytes[..bytes.len() - cut]);
            assert_eq!(reader.next().map(<[u8]>::len), Some(11), "cut {cut}");
            assert_eq!(reader.next(), None);
            assert_eq!(reader.end(), Some(End::Truncated), "cut {cut}");
        }
    }

    #[test]
    fn a_length_beyond_the_bound_ends_the_file() {
        let mut bytes = file(&[b"one"]);
        let too_long = u16::try_from(MAX_BODY + 1).unwrap().to_be_bytes();
        bytes.extend_from_slice(&[crate::PROFILE_VERSION, TYPE_CONTROL, too_long[0], too_long[1]]);
        bytes.extend_from_slice(&file(&[b"after"]));
        let mut reader = records(&bytes);
        assert_eq!(reader.next().map(<[u8]>::len), Some(11));
        assert_eq!(reader.next(), None, "nothing after the bad length is read");
        assert_eq!(reader.end(), Some(End::BadLength));
    }

    #[test]
    fn a_damaged_record_is_handed_out_and_the_file_goes_on() {
        let mut bytes = file(&[b"one", b"two"]);
        bytes[5] ^= 0xFF;
        let records: Vec<&[u8]> = records(&bytes).collect();
        assert_eq!(records.len(), 2);
        assert!(crate::record::split(records[0]).is_err());
        assert!(crate::record::split(records[1]).is_ok());
    }
}
