//! The broadcast carriage into the receiver (REQ-AUD-007): an audio
//! packet's fragment reassembled into a record.

use super::{Receiver, bump};
use crate::idl::fragment;
use crate::port::VoiceSink;
use crate::record::is_input_type;
use neoteletext_core::packet::Packet;

impl<const OBJECTS: usize, const SLOTS: usize> Receiver<'_, OBJECTS, SLOTS> {
    /// Reads one packet. Returns whether it was an audio packet, so the
    /// port can leave the rest to the teletext decoder.
    pub fn push_packet(&mut self, packet: &Packet, sink: &mut dyn VoiceSink) -> bool {
        let Some(result) = fragment(packet) else { return false };
        match result {
            // The input profile's records, the cursor and the control
            // record, are one fragment each; they never enter reassembly,
            // so their record ids cannot meet a sound's.
            Ok(fragment)
                if fragment.first && fragment.last && fragment.data.get(1).is_some_and(|&kind| is_input_type(kind)) => {
            }
            Ok(fragment) if fragment.service == self.service => {
                if let Some((_, record)) = self.reassembler.push(&fragment) {
                    let mut copy = [0_u8; crate::MAX_RECORD];
                    let length = record.len().min(copy.len());
                    if let (Some(target), Some(source)) = (copy.get_mut(..length), record.get(..length)) {
                        target.copy_from_slice(source);
                    }
                    self.push_record(copy.get(..length).unwrap_or(&[]), sink);
                }
            }
            Ok(_) => {}
            Err(_) => bump(&mut self.statistics.packets_dropped),
        }
        true
    }
}
