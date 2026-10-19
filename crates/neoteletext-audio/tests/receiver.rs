//! The receiver end to end: sounds written by the publishing encoder, cut
//! into broadcast packets, read back and played through a recording sink
//! (REQ-AUD-001, REQ-AUD-007, REQ-AUD-009, REQ-AUD-010, REQ-AUD-011,
//! REQ-AUD-012, REQ-AUD-013, REQ-AUD-016, REQ-AUD-018 and REQ-AUD-020).
#![cfg(feature = "encode")]

use neoteletext_audio::encode::{Command, Sound, packets};
use neoteletext_audio::ima::decode_block;
use neoteletext_audio::port::{SoundFormat, SoundId, Start, VoiceSink};
use neoteletext_audio::receiver::Receiver;
use neoteletext_audio::record::{FLAG_PROGRESSIVE, MAX_LABEL};
use neoteletext_audio::{Codec, IMA_BLOCK_SMALL, MIN_FADE_MS};
use neoteletext_core::packet::Packet;

/// What the sink was asked to do, in order.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Call {
    Define(SoundId, SoundFormat),
    Samples(SoundId, u32, usize),
    Forget(SoundId),
    Start(u8, Start),
    Stop(u8, u16),
    Gain(u8, u8, u16),
}

#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
    /// The samples received, per sound, assembled.
    pcm: Vec<(SoundId, Vec<i16>)>,
    channels: u8,
}

impl Recorder {
    fn new() -> Self {
        Self { channels: 4, ..Self::default() }
    }

    fn starts(&self) -> Vec<(u8, SoundId)> {
        self.calls
            .iter()
            .filter_map(
                |call| if let Call::Start(channel, start) = call { Some((*channel, start.sound)) } else { None },
            )
            .collect()
    }

    fn take(&mut self) -> Vec<Call> {
        std::mem::take(&mut self.calls)
    }
}

impl VoiceSink for Recorder {
    fn channels(&self) -> u8 {
        self.channels
    }
    fn define(&mut self, sound: SoundId, format: SoundFormat) {
        self.calls.push(Call::Define(sound, format));
        self.pcm.push((sound, vec![0; format.samples as usize]));
    }
    fn samples(&mut self, sound: SoundId, first: u32, pcm: &[i16]) {
        self.calls.push(Call::Samples(sound, first, pcm.len()));
        if let Some((_, buffer)) = self.pcm.iter_mut().rev().find(|(id, _)| *id == sound) {
            buffer[first as usize..first as usize + pcm.len()].copy_from_slice(pcm);
        }
    }
    fn forget(&mut self, sound: SoundId) {
        self.calls.push(Call::Forget(sound));
    }
    fn start(&mut self, channel: u8, start: Start) {
        self.calls.push(Call::Start(channel, start));
    }
    fn stop(&mut self, channel: u8, fade_ms: u16) {
        self.calls.push(Call::Stop(channel, fade_ms));
    }
    fn gain(&mut self, channel: u8, gain: u8, fade_ms: u16) {
        self.calls.push(Call::Gain(channel, gain, fade_ms));
    }
}

/// A tone of `samples` samples.
fn tone(samples: usize) -> Vec<i16> {
    (0..samples).map(|n| i16::try_from(i32::try_from(n % 50).unwrap() * 400 - 10_000).unwrap()).collect()
}

fn play(channel: u8, sequence: u16, sound: &Sound, page: Option<u16>) -> Command {
    Command {
        channel,
        sequence,
        operation: 1,
        handle: sound.handle,
        generation: sound.generation,
        gain: 80,
        fade_ms: 0,
        flags: 0,
        duck: 0,
        page,
    }
}

fn stop(channel: u8, sequence: u16) -> Command {
    Command {
        channel,
        sequence,
        operation: 2,
        handle: 0,
        generation: 0,
        gain: 0,
        fade_ms: 50,
        flags: 0,
        duck: 0,
        page: None,
    }
}

/// The broadcast packets of a sound and some commands.
fn broadcast(sound: &Sound, commands: &[Command]) -> Vec<Packet> {
    let mut records = vec![sound.object_record().unwrap()];
    records.extend(sound.data_records());
    records.extend(commands.iter().map(Command::record));
    packets(0, 0, &records).0
}

const ARENA: usize = 64 * 1024;
const NO_STARTS: [(u8, SoundId); 0] = [];
const NO_CALLS: [Call; 0] = [];

#[test]
fn a_page_sound_plays_when_its_page_is_opened() {
    let pcm = tone(4000);
    let sound = Sound::new(1, 1, Codec::ImaAdpcm, 16_000, IMA_BLOCK_SMALL, &pcm).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    for packet in broadcast(&sound, &[play(0, 1, &sound, Some(0x100))]) {
        assert!(receiver.push_packet(&packet, &mut sink));
    }
    assert_eq!(sink.take(), NO_CALLS, "nothing plays before page 100 is open");
    receiver.open_page(Some(0x100), &mut sink);
    let calls = sink.take();
    let id = SoundId { handle: 1, generation: 1 };
    assert_eq!(calls.first(), Some(&Call::Define(id, SoundFormat { rate_hz: 16_000, samples: 4000 })));
    assert!(
        matches!(calls.last(), Some(Call::Start(0, Start { sound, gain: 80, fade_ms: MIN_FADE_MS, .. })) if *sound == id)
    );
    // The samples are exactly the encoded blocks decoded.
    let mut expected = Vec::new();
    for block in sound.data.chunks(usize::from(IMA_BLOCK_SMALL)) {
        let mut out = [0_i16; 1017];
        let count = decode_block(block, &mut out).unwrap();
        expected.extend_from_slice(&out[..count]);
    }
    expected.truncate(4000);
    assert_eq!(sink.pcm[0].1, expected);
    // Leaving the page fades the sound out; coming back plays it again.
    receiver.open_page(Some(0x101), &mut sink);
    assert_eq!(sink.take(), [Call::Stop(0, MIN_FADE_MS)]);
    receiver.open_page(Some(0x100), &mut sink);
    assert_eq!(sink.starts(), [(0, id)], "already at the sink, so only started");
}

#[test]
fn pcm_round_trips_exactly() {
    let pcm = tone(777);
    let sound = Sound::new(2, 1, Codec::Pcm16, 8_000, 1000, &pcm).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    for packet in broadcast(&sound, &[play(1, 1, &sound, None)]) {
        receiver.push_packet(&packet, &mut sink);
    }
    assert_eq!(sink.starts(), [(1, SoundId { handle: 2, generation: 1 })]);
    assert_eq!(sink.pcm[0].1, pcm);
}

#[test]
fn a_stop_that_arrives_before_the_older_play_wins() {
    let sound = Sound::new(1, 1, Codec::Pcm16, 8_000, 1000, &tone(100)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    // The stop (sequence 2) is sent before the play (sequence 1).
    for packet in broadcast(&sound, &[stop(0, 2), play(0, 1, &sound, None)]) {
        receiver.push_packet(&packet, &mut sink);
    }
    assert_eq!(sink.starts(), NO_STARTS);
    assert_eq!(receiver.statistics().stale_commands, 1);
    // Repeats never replay.
    let mut sink = Recorder::new();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let packets = broadcast(&sound, &[play(0, 1, &sound, None)]);
    for _ in 0..3 {
        for packet in &packets {
            receiver.push_packet(packet, &mut sink);
        }
    }
    assert_eq!(sink.starts().len(), 1);
}

#[test]
fn a_play_waits_for_its_sound_to_arrive() {
    let sound = Sound::new(1, 1, Codec::ImaAdpcm, 8_000, IMA_BLOCK_SMALL, &tone(3000)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let command = packets(0, 200, &[play(0, 1, &sound, None).record()]).0;
    for packet in &command {
        receiver.push_packet(packet, &mut sink);
    }
    assert_eq!(sink.starts(), NO_STARTS, "no sound yet");
    let mut records = vec![sound.object_record().unwrap()];
    records.extend(sound.data_records());
    let (packets, _) = packets(0, 0, &records);
    // Every packet but the last: the sound is incomplete.
    for packet in &packets[..packets.len() - 1] {
        receiver.push_packet(packet, &mut sink);
    }
    assert_eq!(sink.starts(), NO_STARTS, "incomplete");
    receiver.push_packet(packets.last().unwrap(), &mut sink);
    assert_eq!(sink.starts().len(), 1);
}

#[test]
fn a_progressive_play_starts_after_forty_milliseconds() {
    // 8 kHz IMA ADPCM: 505 samples, 63 ms, a block; the first DATA record
    // holds three blocks.
    let sound = Sound::new(1, 1, Codec::ImaAdpcm, 8_000, IMA_BLOCK_SMALL, &tone(8000)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let mut command = play(0, 1, &sound, None);
    command.flags = FLAG_PROGRESSIVE;
    let mut records = vec![command.record(), sound.object_record().unwrap()];
    let data = sound.data_records();
    records.push(data[0].clone());
    for packet in packets(0, 0, &records).0 {
        receiver.push_packet(&packet, &mut sink);
    }
    assert_eq!(sink.starts().len(), 1, "started on the first record's blocks");
    let before = sink.calls.len();
    // Later blocks go to the sink as they arrive.
    for packet in packets(0, 10, &data[1..2]).0 {
        receiver.push_packet(&packet, &mut sink);
    }
    assert!(sink.calls[before..].iter().all(|call| matches!(call, Call::Samples(..))) && sink.calls.len() > before);
}

#[test]
fn damaged_packets_are_dropped_and_the_carousel_repeat_recovers() {
    let sound = Sound::new(1, 1, Codec::Pcm16, 8_000, 1000, &tone(2000)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let packets = broadcast(&sound, &[play(0, 1, &sound, None)]);
    let mut damaged = packets.clone();
    damaged[3].bytes[20] ^= 0x40;
    for packet in &damaged {
        receiver.push_packet(packet, &mut sink);
    }
    assert_eq!(sink.starts(), NO_STARTS);
    assert_eq!(receiver.statistics().packets_dropped, 1);
    for packet in &packets {
        receiver.push_packet(packet, &mut sink);
    }
    assert_eq!(sink.starts().len(), 1);
}

#[test]
fn page_sounds_off_keeps_a_page_quiet() {
    let sound = Sound::new(1, 1, Codec::Pcm16, 8_000, 1000, &tone(100)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    receiver.set_page_sounds(false, &mut sink);
    receiver.open_page(Some(0x100), &mut sink);
    for packet in broadcast(&sound, &[play(0, 1, &sound, Some(0x100))]) {
        receiver.push_packet(&packet, &mut sink);
    }
    assert_eq!(sink.starts(), NO_STARTS);
    receiver.set_page_sounds(true, &mut sink);
    assert_eq!(sink.starts().len(), 1);
}

#[test]
fn channels_beyond_the_sink_are_ignored_and_a_lost_sink_is_refilled() {
    let sound = Sound::new(1, 1, Codec::Pcm16, 8_000, 1000, &tone(100)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder { channels: 2, ..Recorder::new() };
    for packet in broadcast(&sound, &[play(3, 1, &sound, None), play(1, 1, &sound, None)]) {
        receiver.push_packet(&packet, &mut sink);
    }
    assert_eq!(sink.starts(), [(1, SoundId { handle: 1, generation: 1 })]);
    // The sink forgot everything: the next play defines the sound again.
    receiver.sink_lost();
    sink.take();
    for packet in packets(0, 50, &[play(1, 2, &sound, None).record()]).0 {
        receiver.push_packet(&packet, &mut sink);
    }
    assert!(matches!(sink.calls.first(), Some(Call::Define(..))));
}

#[test]
fn a_newer_generation_replaces_the_sound_at_the_sink() {
    let first = Sound::new(1, 1, Codec::Pcm16, 8_000, 1000, &tone(100)).unwrap();
    let second = Sound::new(1, 2, Codec::Pcm16, 8_000, 1000, &tone(200)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    for packet in broadcast(&first, &[play(0, 1, &first, None)]) {
        receiver.push_packet(&packet, &mut sink);
    }
    sink.take();
    let mut records = vec![second.object_record().unwrap()];
    records.extend(second.data_records());
    for packet in packets(0, 100, &records).0 {
        receiver.push_packet(&packet, &mut sink);
    }
    assert_eq!(sink.take(), [Call::Forget(SoundId { handle: 1, generation: 1 })]);
}

#[test]
fn a_label_longer_than_the_bound_is_not_written() {
    let mut sound = Sound::new(1, 1, Codec::Pcm16, 8_000, 1000, &tone(10)).unwrap();
    sound.label = Some(vec![b'x'; MAX_LABEL + 1]);
    assert!(sound.object_record().is_err());
}
