//! The audio profile's record parser and receiver, from arbitrary records
//! (REQ-AUD-021, REQ-TST-040): each input is split into records by a
//! two-byte length, read in order, with page changes between them.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_audio::port::{SoundFormat, SoundId, Start, VoiceSink};
use neoteletext_audio::receiver::Receiver;

/// A small store, so that sounds are evicted.
const ARENA: usize = 8 * 1024;

/// A sink that checks what it is given and keeps nothing.
struct Checker;

impl VoiceSink for Checker {
    fn define(&mut self, _: SoundId, format: SoundFormat) {
        assert!(format.samples > 0);
    }
    fn samples(&mut self, _: SoundId, _: u32, pcm: &[i16]) {
        assert!(pcm.len() <= neoteletext_audio::MAX_BLOCK_SAMPLES);
    }
    fn forget(&mut self, _: SoundId) {}
    fn start(&mut self, channel: u8, start: Start) {
        assert!(channel < neoteletext_audio::CHANNELS && start.gain <= 100 && start.fade_ms >= neoteletext_audio::MIN_FADE_MS);
    }
    fn stop(&mut self, channel: u8, _: u16) {
        assert!(channel < neoteletext_audio::CHANNELS);
    }
    fn gain(&mut self, channel: u8, gain: u8, _: u16) {
        assert!(channel < neoteletext_audio::CHANNELS && gain <= 100);
    }
}

fuzz_target!(|data: &[u8]| {
    let mut arena = vec![0u8; ARENA];
    let mut receiver = Receiver::<4, 2>::new(&mut arena);
    let mut sink = Checker;
    let mut rest = data;
    while let [high, low, tail @ ..] = rest {
        let length = usize::from(u16::from_be_bytes([*high, *low])) % 1100;
        let (record, after) = tail.split_at(length.min(tail.len()));
        // Half the records get a correct header and CRC-32, so the fuzzer
        // reaches the store and the commands, not only the frame check.
        let mut repaired = record.to_vec();
        if length & 1 == 0 && repaired.len() >= 8 {
            let body = repaired.len() - 8;
            repaired[0] = neoteletext_audio::PROFILE_VERSION;
            repaired[1] = 1 + repaired[1] % 3;
            repaired[2..4].copy_from_slice(&u16::try_from(body).unwrap_or(0).to_be_bytes());
            let crc = neoteletext_audio::crc::crc32(&repaired[..4 + body]);
            repaired[4 + body..].copy_from_slice(&crc.to_be_bytes());
        }
        receiver.push_record(&repaired, &mut sink);
        if let Some(&page) = record.first() {
            receiver.open_page(Some(0x100 + u16::from(page % 4)), &mut sink);
        }
        rest = after;
    }
});
