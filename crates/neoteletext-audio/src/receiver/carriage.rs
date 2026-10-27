//! The broadcast carriage into the receiver (REQ-AUD-007): an audio
//! packet's fragment reassembled into a record; and the network carriage
//! (REQ-AUD-008), a WebSocket's binary message of the same packets.

use super::{Receiver, bump};
use crate::idl::fragment;
use crate::port::VoiceSink;
use crate::record::is_input_type;
use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::packet::Packet;

/// Why a WebSocket message was refused (REQ-AUD-008).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MessageError {
    /// Not a whole number of 42-byte packets, as REQ-WEB-005 requires of a
    /// binary message.
    PacketLength,
}

impl<const OBJECTS: usize, const SLOTS: usize> Receiver<'_, OBJECTS, SLOTS> {
    /// Reads one binary message of a channel's push WebSocket: whole T42
    /// packets in their internal form (REQ-WEB-005), never a bare record
    /// (decision E.92). The audio packets among them are read as broadcast
    /// ones; the rest are the teletext decoder's, which ignores the audio
    /// packets (REQ-PKT-008), so a port gives it the whole message too.
    /// Returns how many packets were the audio channel's.
    ///
    /// # Errors
    /// [`MessageError::PacketLength`] for a message that is not whole
    /// packets: it is refused whole, before any of it is read.
    pub fn push_message(&mut self, message: &[u8], sink: &mut dyn VoiceSink) -> Result<usize, MessageError> {
        let (packets, rest) = message.as_chunks::<PACKET_BYTES>();
        if !rest.is_empty() {
            bump(&mut self.statistics.messages_refused);
            return Err(MessageError::PacketLength);
        }
        let mut audio: usize = 0;
        for bytes in packets {
            if self.push_packet(&Packet { bytes: *bytes }, sink) {
                audio = audio.saturating_add(1);
            }
        }
        Ok(audio)
    }

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
