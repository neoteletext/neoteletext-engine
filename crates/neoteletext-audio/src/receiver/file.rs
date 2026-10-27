//! The file carriage into the receiver (REQ-AUD-008): a `.ttau` file's
//! records, read in order.

use super::Receiver;
use crate::port::VoiceSink;
use crate::ttau::{End, records};

impl<const OBJECTS: usize, const SLOTS: usize> Receiver<'_, OBJECTS, SLOTS> {
    /// Reads a whole `.ttau` file, or as much of one as has been read, as
    /// records of this receiver's service: the host associates the file
    /// with it, since a record never names its service. Returns why the
    /// records ended.
    pub fn push_file(&mut self, file: &[u8], sink: &mut dyn VoiceSink) -> End {
        let mut reader = records(file);
        for record in reader.by_ref() {
            self.push_record(record, sink);
        }
        reader.end().unwrap_or(End::Complete)
    }
}
