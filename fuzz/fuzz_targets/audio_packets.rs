//! The audio profile's broadcast carriage, from arbitrary 42-byte packets
//! (REQ-AUD-021, REQ-TST-040): fragments, reassembly and every record after.
//! Each packet's address bytes are forced to data channel 12 half the time,
//! so most inputs reach the reassembler.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_audio::port::{SoundFormat, SoundId, Start, VoiceSink};
use neoteletext_audio::receiver::Receiver;
use neoteletext_core::consts::{ADDRESS_BYTE_0, ADDRESS_BYTE_1, IDL_AUDIO_DATA_START, IDL_CHANNEL_INDEPENDENT_AUDIO, IDL_DESIGNATION, PACKET_BYTES};
use neoteletext_core::ecc::hamming84_encode;
use neoteletext_core::packet::Packet;

const ARENA: usize = 8 * 1024;

struct Quiet;

impl VoiceSink for Quiet {
    fn define(&mut self, _: SoundId, _: SoundFormat) {}
    fn samples(&mut self, _: SoundId, _: u32, _: &[i16]) {}
    fn forget(&mut self, _: SoundId) {}
    fn start(&mut self, _: u8, _: Start) {}
    fn stop(&mut self, _: u8, _: u16) {}
    fn gain(&mut self, _: u8, _: u8, _: u16) {}
}

fuzz_target!(|data: &[u8]| {
    let mut arena = vec![0u8; ARENA];
    let mut receiver = Receiver::<4, 2>::new(&mut arena);
    let mut sink = Quiet;
    receiver.open_page(Some(0x100), &mut sink);
    for bytes in data.as_chunks::<PACKET_BYTES>().0 {
        let mut packet = Packet { bytes: *bytes };
        if bytes[ADDRESS_BYTE_0] & 1 == 0 {
            packet.bytes[ADDRESS_BYTE_0] = hamming84_encode(IDL_CHANNEL_INDEPENDENT_AUDIO);
            packet.bytes[ADDRESS_BYTE_1] = hamming84_encode(IDL_DESIGNATION);
            // A correct CRC-16, so fragments reach the reassembler.
            let end = PACKET_BYTES - 2;
            let crc = neoteletext_audio::crc::crc16_idl(&packet.bytes[IDL_AUDIO_DATA_START..end]);
            packet.bytes[end..].copy_from_slice(&crc.to_be_bytes());
        }
        receiver.push_packet(&packet, &mut sink);
    }
});
