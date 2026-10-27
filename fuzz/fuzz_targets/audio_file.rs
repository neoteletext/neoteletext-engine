//! The audio profile's file carriage, from arbitrary `.ttau` files
//! (REQ-AUD-008, REQ-AUD-021, REQ-TST-040): records framed by their own
//! lengths, then read by the receiver. Each record's frame is repaired half
//! the time, so most inputs reach the store and the commands; the file is
//! read whole, then again in two pieces, with a tick between.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_audio::port::{SoundFormat, SoundId, Start, VoiceSink};
use neoteletext_audio::receiver::Receiver;
use neoteletext_audio::ttau::{End, records};
use neoteletext_audio::{CHANNELS, MAX_BLOCK_SAMPLES, MAX_RECORD, MIN_FADE_MS, PROFILE_VERSION, RECORD_CRC, RECORD_HEADER};

/// A small store, so that sounds are evicted.
const ARENA: usize = 8 * 1024;

/// A sink that checks what it is given and keeps nothing.
struct Checker;

impl VoiceSink for Checker {
    fn define(&mut self, _: SoundId, format: SoundFormat) {
        assert!(format.samples > 0);
    }
    fn samples(&mut self, _: SoundId, _: u32, pcm: &[i16]) {
        assert!(pcm.len() <= MAX_BLOCK_SAMPLES);
    }
    fn forget(&mut self, _: SoundId) {}
    fn stream(&mut self, _: SoundId, rate_hz: u32) {
        assert!(neoteletext_audio::SAMPLE_RATES.contains(&rate_hz));
    }
    fn stream_samples(&mut self, _: SoundId, pcm: &[i16]) {
        assert!(pcm.len() <= MAX_BLOCK_SAMPLES);
    }
    fn start(&mut self, channel: u8, start: Start) {
        assert!(channel < CHANNELS && start.gain <= 100 && start.fade_ms >= MIN_FADE_MS);
    }
    fn stop(&mut self, channel: u8, _: u16) {
        assert!(channel < CHANNELS);
    }
    fn gain(&mut self, channel: u8, gain: u8, _: u16) {
        assert!(channel < CHANNELS && gain <= 100);
    }
    fn concealed(&mut self, _: SoundId, _: u16, samples: u32) {
        assert!(usize::try_from(samples).is_ok_and(|samples| samples <= MAX_BLOCK_SAMPLES));
    }
}

/// The input with every other record's version and CRC-32 made right, as the
/// file reader frames them, so the fuzzer gets past the frame check.
fn repaired(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut at = 0;
    let mut index = 0_usize;
    while let Some(rest) = out.get(at..) {
        let Some(record) = records(rest).next() else { break };
        let length = record.len();
        if index.is_multiple_of(2) && length >= RECORD_HEADER + RECORD_CRC {
            let body = length - RECORD_HEADER - RECORD_CRC;
            out[at] = PROFILE_VERSION;
            out[at + 1] = 1 + out[at + 1] % 3;
            let crc = neoteletext_audio::crc::crc32(&out[at..at + RECORD_HEADER + body]);
            out[at + RECORD_HEADER + body..at + length].copy_from_slice(&crc.to_be_bytes());
        }
        at += length;
        index += 1;
    }
    out
}

fuzz_target!(|data: &[u8]| {
    let file = repaired(data);
    let mut whole = records(&file);
    let mut framed = 0;
    for record in whole.by_ref() {
        assert!(record.len() <= MAX_RECORD);
        framed += record.len();
    }
    match whole.end() {
        Some(End::Complete) => assert_eq!(framed, file.len()),
        Some(End::Truncated | End::BadLength) => assert!(framed < file.len()),
        None => panic!("the records ended without saying why"),
    }
    let mut arena = vec![0u8; ARENA];
    let mut receiver = Receiver::<4, 2>::new(&mut arena);
    let mut sink = Checker;
    receiver.open_page(Some(0x100), &mut sink);
    receiver.push_file(&file, &mut sink);
    let (first, second) = file.split_at(file.len() / 2);
    receiver.tick(u64::from(first.first().copied().unwrap_or(0)) * 64, &mut sink);
    receiver.push_file(first, &mut sink);
    receiver.push_file(second, &mut sink);
    if let Some(next) = receiver.next_tick_ms() {
        receiver.tick(next, &mut sink);
    }
});
