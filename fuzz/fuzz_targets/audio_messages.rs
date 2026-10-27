//! The audio profile's network carriage, from arbitrary WebSocket binary
//! messages (REQ-AUD-008, REQ-AUD-021, REQ-TST-040): each input is split
//! into messages by a two-byte length, and each message read as whole
//! 42-byte packets. Each packet's address bytes are forced to data channel
//! 12, with a correct CRC-16, half the time, so most reach the reassembler.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_audio::port::{SoundFormat, SoundId, Start, VoiceSink};
use neoteletext_audio::receiver::{MessageError, Receiver};
use neoteletext_core::consts::{ADDRESS_BYTE_0, ADDRESS_BYTE_1, IDL_AUDIO_DATA_START, IDL_CHANNEL_INDEPENDENT_AUDIO, IDL_DESIGNATION, PACKET_BYTES};
use neoteletext_core::ecc::hamming84_encode;

const ARENA: usize = 8 * 1024;

struct Quiet;

impl VoiceSink for Quiet {
    fn define(&mut self, _: SoundId, _: SoundFormat) {}
    fn samples(&mut self, _: SoundId, _: u32, _: &[i16]) {}
    fn forget(&mut self, _: SoundId) {}
    fn stream(&mut self, _: SoundId, _: u32) {}
    fn stream_samples(&mut self, _: SoundId, _: &[i16]) {}
    fn start(&mut self, _: u8, _: Start) {}
    fn stop(&mut self, _: u8, _: u16) {}
    fn gain(&mut self, _: u8, _: u8, _: u16) {}
}

fuzz_target!(|data: &[u8]| {
    let mut arena = vec![0u8; ARENA];
    let mut receiver = Receiver::<4, 2>::new(&mut arena);
    let mut sink = Quiet;
    receiver.open_page(Some(0x100), &mut sink);
    let mut rest = data;
    let mut now_ms: u64 = 0;
    while let [high, low, tail @ ..] = rest {
        let length = usize::from(u16::from_be_bytes([*high, *low])) % (16 * PACKET_BYTES + 2);
        let (message, after) = tail.split_at(length.min(tail.len()));
        let mut message = message.to_vec();
        for packet in message.as_chunks_mut::<PACKET_BYTES>().0 {
            if packet[ADDRESS_BYTE_0] & 1 == 0 {
                packet[ADDRESS_BYTE_0] = hamming84_encode(IDL_CHANNEL_INDEPENDENT_AUDIO);
                packet[ADDRESS_BYTE_1] = hamming84_encode(IDL_DESIGNATION);
                let end = PACKET_BYTES - 2;
                let crc = neoteletext_audio::crc::crc16_idl(&packet[IDL_AUDIO_DATA_START..end]);
                packet[end..].copy_from_slice(&crc.to_be_bytes());
            }
        }
        let packets = message.len() / PACKET_BYTES;
        match receiver.push_message(&message, &mut sink) {
            Ok(audio) => assert!(message.len().is_multiple_of(PACKET_BYTES) && audio <= packets),
            Err(MessageError::PacketLength) => assert!(!message.len().is_multiple_of(PACKET_BYTES)),
        }
        now_ms += u64::from(*high) * 16;
        receiver.tick(now_ms, &mut sink);
        rest = after;
    }
});
