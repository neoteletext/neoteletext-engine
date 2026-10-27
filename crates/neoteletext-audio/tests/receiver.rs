//! The receiver end to end: sounds written by the publishing encoder, cut
//! into broadcast packets, read back and played through a recording sink
//! (REQ-AUD-001, REQ-AUD-007, REQ-AUD-009, REQ-AUD-010, REQ-AUD-011,
//! REQ-AUD-012, REQ-AUD-013, REQ-AUD-014, REQ-AUD-016, REQ-AUD-018 and
//! REQ-AUD-020).
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
    Stream(SoundId, u32),
    StreamSamples(SoundId, usize),
    Start(u8, Start),
    Stop(u8, u16),
    Gain(u8, u8, u16),
    Concealed(SoundId, u16, u32),
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
    fn stream(&mut self, sound: SoundId, rate_hz: u32) {
        self.calls.push(Call::Stream(sound, rate_hz));
        self.pcm.push((sound, Vec::new()));
    }
    fn stream_samples(&mut self, sound: SoundId, pcm: &[i16]) {
        self.calls.push(Call::StreamSamples(sound, pcm.len()));
        if let Some((_, buffer)) = self.pcm.iter_mut().rev().find(|(id, _)| *id == sound) {
            buffer.extend_from_slice(pcm);
        }
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
    fn concealed(&mut self, sound: SoundId, block: u16, samples: u32) {
        self.calls.push(Call::Concealed(sound, block, samples));
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

#[cfg(feature = "pcm16")]
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

#[cfg(feature = "pcm16")]
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

#[cfg(feature = "pcm16")]
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

#[cfg(feature = "pcm16")]
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

#[cfg(feature = "pcm16")]
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

#[cfg(feature = "pcm16")]
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

#[cfg(feature = "pcm16")]
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

#[cfg(feature = "pcm16")]
/// A live PCM sound at 8 kHz, 100-byte blocks of 50 samples.
fn live_sound() -> Sound {
    Sound::live(4, 1, Codec::Pcm16, 8_000, 100).unwrap()
}

/// The packets of live blocks `first..first + count`, block `n` holding the
/// value `n` in every sample, one block a record.
#[cfg(feature = "pcm16")]
fn live_blocks(sound: &Sound, first: u16, count: u16) -> Vec<Packet> {
    let records: Vec<Vec<u8>> = (0..count)
        .flat_map(|offset| {
            let index = first.wrapping_add(offset);
            sound.live_records(index, &[i16::try_from(index % 1000).unwrap(); 50])
        })
        .collect();
    packets(0, 0, &records).0
}

#[cfg(any(feature = "pcm16", feature = "opus"))]
fn push(receiver: &mut Receiver<'_, 8, 2>, packets: &[Packet], sink: &mut Recorder) {
    for packet in packets {
        assert!(receiver.push_packet(packet, sink));
    }
}

#[cfg(feature = "pcm16")]
/// The value of each block of 50 samples the sink received, in order: its
/// loudest sample, since a block next to a silence is faded at that edge.
fn live_values(sink: &Recorder, sound: &Sound) -> Vec<i16> {
    live_samples(sink, sound).chunks(50).map(|block| block.iter().copied().max().unwrap()).collect()
}

/// Every sample of a live sound the sink received, in order.
#[cfg(feature = "pcm16")]
fn live_samples(sink: &Recorder, sound: &Sound) -> Vec<i16> {
    let id = SoundId { handle: sound.handle, generation: sound.generation };
    sink.pcm.iter().rev().find(|(sound, _)| *sound == id).unwrap().1.clone()
}

#[cfg(feature = "pcm16")]
#[test]
fn a_live_sound_joins_at_its_newest_block_and_follows_in_order() {
    let sound = live_sound();
    let id = SoundId { handle: 4, generation: 1 };
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let mut announce = vec![sound.object_record().unwrap()];
    announce.push(play(0, 1, &sound, None).record());
    receiver.tick(1_000, &mut sink);
    push(&mut receiver, &packets(0, 0, &announce).0, &mut sink);
    assert_eq!(sink.take(), NO_CALLS, "no block yet: the play waits");
    push(&mut receiver, &live_blocks(&sound, 100, 3), &mut sink);
    let calls = sink.take();
    assert_eq!(calls[0], Call::Stream(id, 8_000));
    assert!(matches!(calls[2], Call::Start(0, Start { sound, looped: false, .. }) if sound == id), "{calls:?}");
    // Joined at block 100, the newest when the play could start, then 101 and 102.
    push(&mut receiver, &live_blocks(&sound, 103, 2), &mut sink);
    assert_eq!(live_values(&sink, &sound), [100, 101, 102, 103, 104]);
    assert_eq!(receiver.statistics().concealed, 0);
}

#[cfg(feature = "pcm16")]
#[test]
fn a_lost_live_block_is_heard_as_silence_once_three_newer_have_come() {
    let sound = live_sound();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let announce = [sound.object_record().unwrap(), play(0, 1, &sound, None).record()];
    push(&mut receiver, &packets(0, 0, &announce).0, &mut sink);
    push(&mut receiver, &live_blocks(&sound, 10, 1), &mut sink);
    // Block 11 is lost; 12 and 13 wait for it.
    push(&mut receiver, &live_blocks(&sound, 12, 2), &mut sink);
    assert_eq!(live_values(&sink, &sound), [10]);
    // A late 11 is played in its place.
    push(&mut receiver, &live_blocks(&sound, 11, 1), &mut sink);
    assert_eq!(live_values(&sink, &sound), [10, 11, 12, 13]);
    // 15 is lost for good: once 18 is here, three ahead, it is silence.
    push(&mut receiver, &live_blocks(&sound, 14, 1), &mut sink);
    push(&mut receiver, &live_blocks(&sound, 16, 2), &mut sink);
    assert_eq!(live_values(&sink, &sound), [10, 11, 12, 13, 14]);
    push(&mut receiver, &live_blocks(&sound, 18, 1), &mut sink);
    assert_eq!(live_values(&sink, &sound), [10, 11, 12, 13, 14, 0, 16, 17, 18]);
    assert_eq!(receiver.statistics().concealed, 1);
}

#[cfg(feature = "pcm16")]
#[test]
fn a_stalled_live_sound_fades_out_and_resumes_when_its_blocks_return() {
    let sound = live_sound();
    let id = SoundId { handle: 4, generation: 1 };
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let announce = [sound.object_record().unwrap(), play(0, 1, &sound, None).record()];
    receiver.tick(5_000, &mut sink);
    assert_eq!(receiver.next_tick_ms(), None, "nothing live: no tick needed");
    push(&mut receiver, &packets(0, 0, &announce).0, &mut sink);
    push(&mut receiver, &live_blocks(&sound, 0, 4), &mut sink);
    sink.take();
    assert_eq!(receiver.next_tick_ms(), Some(7_000), "the stall, two seconds after the last block");
    // Just short of two seconds without a block: still playing.
    receiver.tick(6_999, &mut sink);
    assert_eq!(sink.take(), NO_CALLS);
    // Two seconds (EN 300 708 clause 6.7.1.1): it fades out and is forgotten.
    receiver.tick(7_000, &mut sink);
    assert_eq!(sink.take(), [Call::Stop(0, neoteletext_audio::LIVE_STALL_FADE_MS), Call::Forget(id)]);
    assert_eq!(receiver.next_tick_ms(), None, "faded: no tick needed");
    receiver.tick(9_000, &mut sink);
    assert_eq!(sink.take(), NO_CALLS, "once only");
    // The sender comes back: the play starts again at the newest block.
    push(&mut receiver, &live_blocks(&sound, 500, 1), &mut sink);
    let calls = sink.take();
    assert_eq!(calls[0], Call::Stream(id, 8_000), "{calls:?}");
    assert!(calls.iter().any(|call| matches!(call, Call::Start(0, _))));
    assert_eq!(live_values(&sink, &sound), [500]);
}

#[cfg(feature = "pcm16")]
#[test]
fn a_stopped_live_sound_is_not_restarted_by_its_blocks() {
    let sound = live_sound();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let announce = [sound.object_record().unwrap(), play(0, 1, &sound, None).record(), stop(0, 2).record()];
    push(&mut receiver, &live_blocks(&sound, 0, 1), &mut sink);
    push(&mut receiver, &packets(0, 0, &announce).0, &mut sink);
    sink.take();
    push(&mut receiver, &live_blocks(&sound, 1, 3), &mut sink);
    assert_eq!(sink.starts(), NO_STARTS);
}

#[cfg(feature = "pcm16")]
#[test]
fn a_cursor_record_on_the_audio_channel_is_left_to_the_input_profile() {
    // The input profile's cursor record shares data channel 12 (decision
    // E.91); with the record id of a sound's fragment in flight it must not
    // disturb the sound, nor count as a refused record.
    let sound = Sound::new(1, 1, Codec::Pcm16, 8_000, 1000, &tone(777)).unwrap();
    let packets = broadcast(&sound, &[play(1, 1, &sound, None)]);
    let mut cursor = [0_u8; 16];
    let length = neoteletext_audio::record::write(
        neoteletext_audio::record::TYPE_CURSOR,
        &[0x01, 0x00, 0, 0, 255, 0, 0, 0],
        &mut cursor,
    )
    .unwrap();
    let cursor = neoteletext_audio::idl::packet(0, 0, 0, true, &cursor[..length]);
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let (first, rest) = packets.split_first().unwrap();
    receiver.push_packet(first, &mut sink);
    assert!(receiver.push_packet(&cursor, &mut sink), "it is a packet of the audio channel");
    for packet in rest {
        receiver.push_packet(packet, &mut sink);
    }
    assert_eq!(receiver.statistics().records_refused, 0);
    assert_eq!(sink.starts(), [(1, SoundId { handle: 1, generation: 1 })]);
}

/// The samples of a 5 ms loss fade at 8 kHz (REQ-AUD-013).
#[cfg(feature = "pcm16")]
const FADE_8_KHZ: usize = 40;

/// Whether `got` ramps `level` down to silence (or up from it, `rising`)
/// over its length, as (L − n) / (L + 1) of the level, within the rounding
/// of 16-bit fixed point.
#[cfg(feature = "pcm16")]
fn ramps(got: &[i16], level: i16, rising: bool) -> bool {
    let length = i32::try_from(got.len()).unwrap();
    got.iter().zip(0..).all(|(&sample, n)| {
        let steps = if rising { n + 1 } else { length - n };
        let want = i32::from(level) * steps / (length + 1);
        (i32::from(sample) - want).abs() <= 4
    })
}

/// The packets of live blocks `first..first + count`, every sample `level`.
#[cfg(feature = "pcm16")]
fn level_blocks(sound: &Sound, first: u16, count: u16, level: i16) -> Vec<Packet> {
    let records: Vec<Vec<u8>> =
        (0..count).flat_map(|offset| sound.live_records(first.wrapping_add(offset), &[level; 50])).collect();
    packets(0, 0, &records).0
}

/// REQ-AUD-013: a live block given up is silence, faded into from the block
/// before and out of into the block after over 5 ms, and reported.
#[cfg(feature = "pcm16")]
#[test]
fn a_lost_live_block_is_faded_into_and_out_of_and_reported_concealed() {
    const LEVEL: i16 = 8000;
    let sound = live_sound();
    let id = SoundId { handle: 4, generation: 1 };
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let announce = [sound.object_record().unwrap(), play(0, 1, &sound, None).record()];
    push(&mut receiver, &packets(0, 0, &announce).0, &mut sink);
    push(&mut receiver, &level_blocks(&sound, 10, 1, LEVEL), &mut sink);
    // Block 11 is lost: once 14 is here, three ahead, it is silence.
    push(&mut receiver, &level_blocks(&sound, 12, 3, LEVEL), &mut sink);
    let heard = live_samples(&sink, &sound);
    let (before, rest) = heard.split_at(50);
    let (gap, after) = rest.split_at(50);
    assert!(before[..50 - FADE_8_KHZ].iter().all(|&sample| sample == LEVEL));
    assert!(ramps(&before[50 - FADE_8_KHZ..], LEVEL, false), "faded into the gap: {before:?}");
    assert!(gap.iter().all(|&sample| sample == 0), "{gap:?}");
    assert!(ramps(&after[..FADE_8_KHZ], LEVEL, true), "faded out of the gap: {after:?}");
    // Then whole blocks, the newest's last 5 ms held until its successor.
    assert!(after[FADE_8_KHZ..].iter().all(|&sample| sample == LEVEL));
    assert_eq!(after.len(), 3 * 50 - FADE_8_KHZ);
    let reports: Vec<&Call> = sink.calls.iter().filter(|call| matches!(call, Call::Concealed(..))).collect();
    assert_eq!(reports, [&Call::Concealed(id, 11, 50)]);
    assert_eq!(receiver.statistics().concealed, 1);
    // The held tail goes, whole, once the next block comes.
    push(&mut receiver, &level_blocks(&sound, 15, 1, LEVEL), &mut sink);
    let heard = live_samples(&sink, &sound);
    assert!(heard[100 + FADE_8_KHZ..].iter().all(|&sample| sample == LEVEL));
    assert_eq!(heard.len(), 100 + 4 * 50 - FADE_8_KHZ);
}

/// A stored PCM sound of four 500-sample blocks at one level, and its
/// records, one block each.
#[cfg(feature = "pcm16")]
fn level_sound(level: i16) -> (Sound, Vec<Vec<u8>>) {
    let sound = Sound::new(3, 1, Codec::Pcm16, 8_000, 1000, &[level; 2000]).unwrap();
    let data = sound.data_records();
    assert_eq!(data.len(), 4);
    (sound, data)
}

/// REQ-AUD-013: a missing stored block is silence at the sink, with the
/// blocks either side faded over 5 ms, and whole once it arrives.
#[cfg(feature = "pcm16")]
#[test]
fn a_missing_stored_block_is_faded_either_side_and_mended_when_it_comes() {
    const LEVEL: i16 = -9000;
    let (sound, data) = level_sound(LEVEL);
    let mut command = play(0, 1, &sound, None);
    command.flags = FLAG_PROGRESSIVE;
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let records = [sound.object_record().unwrap(), command.record(), data[0].clone(), data[1].clone(), data[3].clone()];
    push(&mut receiver, &packets(0, 0, &records).0, &mut sink);
    assert_eq!(sink.starts().len(), 1);
    let held = &sink.pcm[0].1;
    assert!(held[..1000 - FADE_8_KHZ].iter().all(|&sample| sample == LEVEL));
    assert!(ramps(&held[1000 - FADE_8_KHZ..1000], LEVEL, false), "block 1 fades into the gap");
    assert!(held[1000..1500].iter().all(|&sample| sample == 0), "block 2 is silence");
    assert!(ramps(&held[1500..1500 + FADE_8_KHZ], LEVEL, true), "block 3 fades out of it");
    assert!(held[1500 + FADE_8_KHZ..].iter().all(|&sample| sample == LEVEL));
    push(&mut receiver, &packets(0, 9, &data[2..3]).0, &mut sink);
    assert_eq!(sink.pcm[0].1, [LEVEL; 2000], "whole once block 2 is here");
}

/// REQ-AUD-013: the receiver follows a progressive play by its clock and
/// reports each block not there when the play reaches it, once; it asks
/// for a tick at that moment.
#[cfg(feature = "pcm16")]
#[test]
fn a_progressive_play_reports_each_block_not_there_when_it_is_due() {
    let (sound, data) = level_sound(1000);
    let id = SoundId { handle: 3, generation: 1 };
    let mut command = play(0, 1, &sound, None);
    command.flags = FLAG_PROGRESSIVE;
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    receiver.tick(1_000, &mut sink);
    let records = [sound.object_record().unwrap(), command.record(), data[0].clone(), data[1].clone()];
    push(&mut receiver, &packets(0, 0, &records).0, &mut sink);
    assert_eq!(sink.starts().len(), 1);
    // Block 2 starts 1 000 samples in: 125 ms at 8 kHz.
    assert_eq!(receiver.next_tick_ms(), Some(1_125));
    receiver.tick(1_124, &mut sink);
    let concealed = |sink: &Recorder| -> Vec<Call> {
        sink.calls.iter().filter(|call| matches!(call, Call::Concealed(..))).cloned().collect()
    };
    assert_eq!(concealed(&sink), NO_CALLS);
    receiver.tick(1_125, &mut sink);
    assert_eq!(concealed(&sink), [Call::Concealed(id, 2, 500)]);
    // Block 3 comes in time, block 2 late: nothing more is reported.
    push(&mut receiver, &packets(0, 7, &data[3..4]).0, &mut sink);
    assert_eq!(receiver.next_tick_ms(), None, "nothing more can be missed");
    push(&mut receiver, &packets(0, 9, &data[2..3]).0, &mut sink);
    receiver.tick(2_000, &mut sink);
    assert_eq!(concealed(&sink), [Call::Concealed(id, 2, 500)]);
    assert_eq!(receiver.statistics().concealed, 1);
}

/// An IMA ADPCM sound, its records and a play of it, as a carousel sends them.
fn ima_records() -> (Sound, Vec<Vec<u8>>) {
    let sound = Sound::new(1, 1, Codec::ImaAdpcm, 8_000, IMA_BLOCK_SMALL, &tone(3000)).unwrap();
    let mut records = vec![sound.object_record().unwrap()];
    records.extend(sound.data_records());
    records.push(play(0, 1, &sound, None).record());
    (sound, records)
}

/// REQ-AUD-008: a `.ttau` file is records back to back; a truncated final
/// record is discarded and a length beyond the bound ends the file.
#[test]
fn a_ttau_file_is_read_as_records_back_to_back() {
    let (sound, records) = ima_records();
    let file: Vec<u8> = records.concat();
    let id = SoundId { handle: sound.handle, generation: sound.generation };
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    assert_eq!(receiver.push_file(&file, &mut sink), neoteletext_audio::ttau::End::Complete);
    assert_eq!(sink.starts(), [(0, id)]);
    assert_eq!(receiver.statistics().records, u32::try_from(records.len()).unwrap());
    // Cut short, the last record (the play) is discarded.
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    assert_eq!(receiver.push_file(&file[..file.len() - 1], &mut sink), neoteletext_audio::ttau::End::Truncated);
    assert_eq!(sink.starts(), NO_STARTS);
    assert_eq!(receiver.statistics().records_refused, 0);
    // A length beyond the bound: nothing after it is read.
    let mut bad = records[0].clone();
    bad.extend_from_slice(&[neoteletext_audio::PROFILE_VERSION, 2, 0xFF, 0xFF]);
    bad.extend(records[1..].concat());
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    assert_eq!(receiver.push_file(&bad, &mut sink), neoteletext_audio::ttau::End::BadLength);
    assert_eq!(receiver.statistics().records, 1);
}

/// REQ-AUD-008 and REQ-WEB-005: a WebSocket's binary message is whole
/// packets, the audio ones read as broadcast; any other length is refused
/// whole.
#[test]
fn a_websocket_message_is_packets_and_a_ragged_one_is_refused_whole() {
    let (sound, records) = ima_records();
    let packets = packets(0, 0, &records).0;
    let mut message: Vec<u8> = packets.iter().flat_map(|packet| packet.bytes).collect();
    // A teletext packet among them is left to the decoder.
    message.extend_from_slice(&[0x15; 42]);
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    let mut ragged = message.clone();
    ragged.push(0);
    assert_eq!(receiver.push_message(&ragged, &mut sink), Err(neoteletext_audio::receiver::MessageError::PacketLength));
    assert_eq!((receiver.statistics().messages_refused, receiver.statistics().records), (1, 0));
    assert_eq!(receiver.push_message(&message, &mut sink), Ok(packets.len()));
    assert_eq!(sink.starts(), [(0, SoundId { handle: sound.handle, generation: sound.generation })]);
}

/// Decision E.131: built without `pcm16`, a PCM16 sound is unsupported and
/// skipped; the commands that name it play nothing.
#[cfg(not(feature = "pcm16"))]
#[test]
fn without_pcm16_a_pcm_sound_is_unsupported_and_skipped() {
    let sound = Sound::new(2, 1, Codec::Pcm16, 8_000, 1000, &tone(777)).unwrap();
    let mut arena = vec![0; ARENA];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Recorder::new();
    for packet in broadcast(&sound, &[play(1, 1, &sound, None)]) {
        receiver.push_packet(&packet, &mut sink);
    }
    assert_eq!(sink.take(), NO_CALLS);
    assert_eq!(receiver.statistics().unsupported, 1);
    assert_eq!(receiver.statistics().records_refused, 0);
}

/// Decision E.124: Opus sounds reach the sink exactly as one decoder fed
/// their packets in order writes them, however the blocks arrive.
#[cfg(feature = "opus")]
mod opus {
    use super::*;
    use neoteletext_audio::record::FLAG_PROGRESSIVE as PROGRESSIVE;
    use neoteletext_opus::{Decoder, FRAME_SAMPLES};

    /// A constant-rate packet's length: 16 kbit/s at 20 ms.
    const PACKET: u16 = 40;
    /// The table of contents of a CELT-only full-band 20 ms mono frame, one a
    /// packet (RFC 6716 section 3.1: configuration 31, code 0).
    const CELT_FULL_BAND_20_MS: u8 = 31 << 3;

    /// `count` packets of `PACKET` bytes made from a seed: the engine holds no
    /// sample stream, and a CELT frame decodes whatever its bytes.
    fn packets(count: usize, seed: u32) -> Vec<u8> {
        let mut state = seed | 1;
        let mut out = Vec::new();
        for _ in 0..count {
            let mut packet = vec![CELT_FULL_BAND_20_MS];
            packet.extend((1..PACKET).map(|_| {
                // xorshift32 (Marsaglia 2003).
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()[0]
            }));
            out.extend(packet);
        }
        out
    }

    /// What one decoder writes for `packets`, `None` a lost one concealed.
    fn decoded(packets: &[Option<&[u8]>]) -> Vec<i16> {
        let mut decoder = Decoder::new();
        let mut out = Vec::new();
        for packet in packets {
            let mut pcm = [0_i16; FRAME_SAMPLES];
            match packet {
                Some(packet) => assert!(decoder.decode(packet, &mut pcm).is_ok(), "a made packet decodes"),
                None => decoder.conceal(&mut pcm),
            }
            out.extend_from_slice(&pcm);
        }
        out
    }

    fn assembled(sink: &Recorder, sound: &Sound) -> Vec<i16> {
        let id = SoundId { handle: sound.handle, generation: sound.generation };
        sink.pcm.iter().rev().find(|(held, _)| *held == id).unwrap().1.clone()
    }

    #[test]
    fn a_stored_opus_sound_arriving_out_of_order_is_decoded_in_order() {
        let data = packets(30, 7);
        let samples = 29 * 960 + 500;
        let sound = Sound::opus(5, 1, PACKET, &data, samples).unwrap();
        assert_eq!(sound.blocks(), 30);
        let mut records = vec![sound.object_record().unwrap()];
        let mut blocks = sound.data_records();
        blocks.reverse();
        records.extend(blocks);
        records.push(play(0, 1, &sound, None).record());
        let mut arena = vec![0; ARENA];
        let mut receiver = Receiver::<8, 2>::new(&mut arena);
        let mut sink = Recorder::new();
        push(&mut receiver, &neoteletext_audio::encode::packets(0, 0, &records).0, &mut sink);
        let expected: Vec<Option<&[u8]>> = data.chunks(usize::from(PACKET)).map(Some).collect();
        let mut want = decoded(&expected);
        want.truncate(samples as usize);
        assert_eq!(assembled(&sink, &sound), want);
        assert_eq!(receiver.statistics().decode_failures, 0);
    }

    #[test]
    fn a_progressive_opus_sound_is_decoded_as_its_blocks_arrive() {
        let data = packets(60, 11);
        let samples = 60 * 960;
        let sound = Sound::opus(5, 1, PACKET, &data, samples).unwrap();
        let mut start = play(0, 1, &sound, None);
        start.flags = PROGRESSIVE;
        let mut arena = vec![0; ARENA];
        let mut receiver = Receiver::<8, 2>::new(&mut arena);
        let mut sink = Recorder::new();
        let announce = [sound.object_record().unwrap(), start.record()];
        push(&mut receiver, &neoteletext_audio::encode::packets(0, 0, &announce).0, &mut sink);
        for record in sound.data_records() {
            push(&mut receiver, &neoteletext_audio::encode::packets(0, 0, &[record]).0, &mut sink);
        }
        let expected: Vec<Option<&[u8]>> = data.chunks(usize::from(PACKET)).map(Some).collect();
        assert_eq!(assembled(&sink, &sound), decoded(&expected));
    }

    /// REQ-AUD-013: a stored Opus sound's samples end where its blocks stop
    /// arriving, faded out over 5 ms, and are whole once the next comes.
    #[test]
    fn a_stored_opus_sound_fades_where_its_blocks_stop_and_mends_when_they_come() {
        const FADE_48_KHZ: usize = 240;
        let data = packets(8, 5);
        let samples = 8 * 960;
        let sound = Sound::opus(5, 1, PACKET, &data, samples).unwrap();
        let packet_bytes = usize::from(PACKET);
        let mut start = play(0, 1, &sound, None);
        start.flags = PROGRESSIVE;
        let mut arena = vec![0; ARENA];
        let mut receiver = Receiver::<8, 2>::new(&mut arena);
        let mut sink = Recorder::new();
        let mut records = vec![sound.object_record().unwrap(), start.record()];
        // Blocks 0 to 2, then 4 onwards: block 3 is late.
        records.extend(sound.packet_records(0, &data[..3 * packet_bytes]).unwrap());
        records.extend(sound.packet_records(4, &data[4 * packet_bytes..]).unwrap());
        push(&mut receiver, &neoteletext_audio::encode::packets(0, 0, &records).0, &mut sink);
        let expected: Vec<Option<&[u8]>> = data.chunks(packet_bytes).map(Some).collect();
        let want = decoded(&expected);
        let held = assembled(&sink, &sound);
        let edge = 3 * 960;
        assert_eq!(held[..edge - FADE_48_KHZ], want[..edge - FADE_48_KHZ]);
        let length = i32::try_from(FADE_48_KHZ).unwrap();
        for (n, (&got, &whole)) in (0..).zip(held[edge - FADE_48_KHZ..edge].iter().zip(&want[edge - FADE_48_KHZ..edge]))
        {
            let faded = i32::from(whole) * (length - n) / (length + 1);
            assert!(
                (i32::from(got) - faded).abs() <= i32::from(whole).abs() / 200 + 2,
                "sample {n}: {got} for {whole}"
            );
        }
        assert!(held[edge..].iter().all(|&sample| sample == 0), "nothing decoded after the gap");
        assert!(held[edge - 1].abs() <= want[edge - 1].abs() / 100 + 2, "faded to silence");
        push(
            &mut receiver,
            &neoteletext_audio::encode::packets(
                0,
                9,
                &sound.packet_records(3, &data[3 * packet_bytes..4 * packet_bytes]).unwrap(),
            )
            .0,
            &mut sink,
        );
        assert_eq!(assembled(&sink, &sound), want, "whole once block 3 is here");
    }

    #[test]
    fn a_lost_live_opus_packet_is_concealed_by_the_decoder() {
        let data = packets(10, 3);
        let packet = |index: usize| &data[index * usize::from(PACKET)..(index + 1) * usize::from(PACKET)];
        let sound = Sound::live(6, 1, Codec::Opus, 48_000, PACKET).unwrap();
        let mut arena = vec![0; ARENA];
        let mut receiver = Receiver::<8, 2>::new(&mut arena);
        let mut sink = Recorder::new();
        let announce = [sound.object_record().unwrap(), play(0, 1, &sound, None).record()];
        push(&mut receiver, &neoteletext_audio::encode::packets(0, 0, &announce).0, &mut sink);
        // Packet 4 is lost for good.
        for index in (0..10).filter(|&index| index != 4) {
            let records = sound.packet_records(u16::try_from(index).unwrap(), packet(index)).unwrap();
            push(&mut receiver, &neoteletext_audio::encode::packets(0, 0, &records).0, &mut sink);
        }
        let expected: Vec<Option<&[u8]>> = (0..10).map(|index| (index != 4).then(|| packet(index))).collect();
        assert_eq!(assembled(&sink, &sound), decoded(&expected));
        assert_eq!(receiver.statistics().concealed, 1);
    }

    #[test]
    fn opus_is_carried_as_packets_and_not_encoded_here() {
        assert_eq!(
            Sound::new(1, 1, Codec::Opus, 48_000, PACKET, &tone(960)),
            Err(neoteletext_audio::encode::EncodeError::Encoder)
        );
        let uneven = vec![0_u8; usize::from(PACKET) * 3 + 1];
        assert_eq!(Sound::opus(1, 1, PACKET, &uneven, 960), Err(neoteletext_audio::encode::EncodeError::PacketLength));
        let data = packets(3, 1);
        assert_eq!(
            Sound::opus(1, 1, PACKET, &data, 4 * 960),
            Err(neoteletext_audio::encode::EncodeError::Length),
            "more samples than the packets hold"
        );
        assert_eq!(
            Sound::opus(1, 1, PACKET, &data, 960),
            Err(neoteletext_audio::encode::EncodeError::Length),
            "a packet beyond the sound's end"
        );
    }
}
