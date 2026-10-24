#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]

// Tests of REQ-AUD-011, REQ-AUD-014, REQ-AUD-015 and REQ-AUD-019.
use super::*;
use std::vec;
use std::vec::Vec;

const RATE: u32 = 8_000;

fn start(key: SoundKey, gain: u8) -> Start {
    Start { key, gain, fade_ms: 5, looped: false, duck_percent: 100, after_current: false }
}

fn constant(mixer: &mut Mixer, key: SoundKey, value: i16, samples: usize) {
    mixer.define(key, RATE, samples);
    mixer.samples(key, 0, &vec![value; samples]);
}

fn render(mixer: &mut Mixer, samples: usize) -> Vec<i16> {
    let mut out = vec![0; samples];
    mixer.render(&mut out);
    out
}

#[test]
fn a_sound_ramps_in_over_its_fade_and_plays_at_its_gain() {
    let mut mixer = Mixer::new(RATE);
    constant(&mut mixer, (1, 1), 10_000, 4000);
    mixer.start(0, start((1, 1), 50));
    let out = render(&mut mixer, 400);
    // 5 ms at 8 kHz is 40 samples of ramp: no step at the start.
    assert!(out[0].abs() < 1000, "starts near silence: {}", out[0]);
    assert!(out[20] > out[5] && out[20] < 5000, "rising");
    // Settled: half of 10 000, give or take the filter's ripple.
    assert!((4_900..=5_100).contains(&out[300]), "{}", out[300]);
}

#[test]
fn a_stop_fades_out_and_ends_the_voice() {
    let mut mixer = Mixer::new(RATE);
    constant(&mut mixer, (1, 1), 10_000, 8000);
    mixer.start(0, start((1, 1), 100));
    render(&mut mixer, 200);
    mixer.stop(0, 10);
    let out = render(&mut mixer, 200);
    assert!(out[0] > 9_000, "still sounding as the fade begins");
    assert_eq!(out[100], 0, "silent once 10 ms have passed");
    assert!(!mixer.active());
}

#[test]
fn voices_sum_and_the_sum_is_clamped() {
    let mut mixer = Mixer::new(RATE);
    constant(&mut mixer, (1, 1), 20_000, 2000);
    constant(&mut mixer, (2, 1), 20_000, 2000);
    mixer.start(0, start((1, 1), 100));
    mixer.start(1, start((2, 1), 100));
    assert_eq!(render(&mut mixer, 500)[400], i16::MAX);
}

#[test]
fn ducking_lowers_the_other_voices_by_table_19s_step() {
    let mut mixer = Mixer::new(RATE);
    constant(&mut mixer, (1, 1), 10_000, 8000);
    constant(&mut mixer, (2, 1), 0, 8000);
    mixer.start(0, start((1, 1), 100));
    render(&mut mixer, 200);
    // Voice 1 plays silence but asks the others to duck by 12 dB (25 %).
    mixer.start(1, Start { duck_percent: 25, ..start((2, 1), 100) });
    let out = render(&mut mixer, 400);
    assert!((2_400..=2_600).contains(&out[399]), "{}", out[399]);
}

#[test]
fn a_queued_sound_follows_the_current_one() {
    let mut mixer = Mixer::new(RATE);
    constant(&mut mixer, (1, 1), 1_000, 100);
    constant(&mut mixer, (2, 1), 8_000, 2000);
    mixer.start(0, start((1, 1), 100));
    mixer.start(0, Start { after_current: true, ..start((2, 1), 100) });
    let out = render(&mut mixer, 600);
    assert!((900..=1_100).contains(&out[60]), "the first sound");
    assert!((7_800..=8_200).contains(&out[500]), "then the queued one: {}", out[500]);
}

#[test]
fn a_sound_is_resampled_to_the_output_rate() {
    // A 1 kHz tone at 8 kHz played at 48 kHz keeps its pitch: zero
    // crossings every 24 output samples.
    let mut mixer = Mixer::new(48_000);
    let tone: Vec<i16> = (0..8000).map(|n| [0, 7071, 10_000, 7071, 0, -7071, -10_000, -7071][n % 8]).collect();
    mixer.define((1, 1), 8_000, tone.len());
    mixer.samples((1, 1), 0, &tone);
    mixer.start(0, start((1, 1), 100));
    let out = render(&mut mixer, 4800);
    let crossings: Vec<usize> = (2000..4700).filter(|&n| out[n] < 0 && out[n + 1] >= 0).collect();
    let spacings: Vec<usize> = crossings.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(!spacings.is_empty() && spacings.iter().all(|&gap| gap == 48), "{spacings:?}");
    let peak = out[2000..4700].iter().map(|value| value.unsigned_abs()).max().unwrap();
    assert!((9_500..=10_500).contains(&peak), "amplitude kept: {peak}");
}

#[test]
fn mute_silences_and_a_forgotten_sound_waits_for_its_voice() {
    let mut mixer = Mixer::new(RATE);
    constant(&mut mixer, (1, 1), 10_000, 8000);
    mixer.start(0, start((1, 1), 100));
    mixer.forget((1, 1));
    assert!(render(&mut mixer, 100)[99] > 9_000, "still playing after being forgotten");
    mixer.set_master(100, true);
    assert_eq!(render(&mut mixer, 100)[99], 0);
}

/// Samples voice 0's live sound has buffered ahead of it.
fn buffered(mixer: &Mixer) -> i64 {
    let playing = mixer.voices[0].playing.unwrap();
    let sound = mixer.sounds.iter().find(|sound| sound.key() == playing.key).unwrap();
    sound.ahead(playing.whole)
}

#[test]
fn a_live_sound_waits_for_its_target_then_plays() {
    let mut mixer = Mixer::new(RATE);
    mixer.stream((1, 1), RATE);
    mixer.start(0, start((1, 1), 100));
    // 400 ms at 8 kHz is 3 200 samples: 3 100 are not enough.
    mixer.stream_samples((1, 1), &vec![10_000; 3_100]);
    assert!(render(&mut mixer, 200).iter().all(|&value| value == 0), "silent until the target is buffered");
    mixer.stream_samples((1, 1), &vec![10_000; 200]);
    let out = render(&mut mixer, 200);
    assert!((9_800..=10_200).contains(&out[150]), "{}", out[150]);
}

#[test]
fn a_live_sound_follows_a_sender_whose_clock_runs_one_per_cent_fast() {
    let mut mixer = Mixer::new(RATE);
    mixer.stream((1, 1), RATE);
    mixer.start(0, start((1, 1), 100));
    // 101 samples arrive for every 100 played; 200 000 samples (25 s)
    // would put 2 000 extra in the buffer were the rate not trimmed.
    for _ in 0..2_000 {
        mixer.stream_samples((1, 1), &[5_000; 101]);
        render(&mut mixer, 100);
    }
    let ahead = buffered(&mixer);
    // Steady state: 1 % of trim is a fifth of the target, 640 samples, over.
    assert!((3_200..=4_000).contains(&ahead), "buffer held near the target: {ahead}");
    // And one running slow drains less than the target's worth.
    for _ in 0..4_000 {
        mixer.stream_samples((1, 1), &[5_000; 99]);
        render(&mut mixer, 100);
    }
    let ahead = buffered(&mixer);
    assert!((2_400..=3_200).contains(&ahead), "buffer held near the target: {ahead}");
}

#[test]
fn a_forgotten_live_sound_plays_what_it_holds_then_ends() {
    let mut mixer = Mixer::new(RATE);
    mixer.stream((1, 1), RATE);
    mixer.start(0, start((1, 1), 100));
    mixer.stream_samples((1, 1), &vec![10_000; 3_600]);
    render(&mut mixer, 1_000);
    mixer.forget((1, 1));
    let out = render(&mut mixer, 3_000);
    assert!(out[2_000] > 9_000, "still playing what it holds");
    assert!(!mixer.active() && out[2_999] == 0, "then ended");
    // Rendering never frees: the next call that may allocate collects it.
    mixer.stream((2, 1), RATE);
    assert_eq!(mixer.sounds.iter().map(Sound::key).collect::<Vec<_>>(), [(2, 1)], "and gone");
}

#[test]
fn a_voice_that_falls_a_ring_behind_skips_to_the_target() {
    let mut mixer = Mixer::new(RATE);
    mixer.stream((1, 1), RATE);
    mixer.start(0, start((1, 1), 100));
    // Two seconds of samples at once: more than the ring keeps ahead.
    mixer.stream_samples((1, 1), &vec![10_000; 16_000]);
    assert_eq!(buffered(&mixer), 3_200);
}

/// A tone of eight samples a period.
fn tone(samples: usize) -> Vec<i16> {
    (0..samples).map(|n| [0, 7071, 10_000, 7071, 0, -7071, -10_000, -7071][n % 8]).collect()
}

#[test]
fn a_sound_defined_with_its_buffer_mixes_as_one_defined_by_length() {
    // Not the output rate, so the voice resamples it.
    const SOURCE_RATE: u32 = 11_025;
    let pcm = tone(4_000);
    let mut by_length = Mixer::new(RATE);
    by_length.define((1, 1), SOURCE_RATE, pcm.len());
    by_length.samples((1, 1), 0, &pcm);
    // A silent buffer filled afterwards, and one given filled.
    let mut silent = Mixer::new(RATE);
    silent.define_with((1, 1), SOURCE_RATE, vec![0; pcm.len()]);
    silent.samples((1, 1), 0, &pcm);
    let mut filled = Mixer::new(RATE);
    filled.define_with((1, 1), SOURCE_RATE, pcm.clone());
    by_length.start(0, start((1, 1), 80));
    let expected = render(&mut by_length, 3_000);
    assert!(expected.iter().any(|&value| value != 0), "the tone sounds");
    for mut mixer in [silent, filled] {
        mixer.start(0, start((1, 1), 80));
        assert_eq!(render(&mut mixer, 3_000), expected);
    }
}

#[test]
fn a_defined_buffer_is_held_as_given() {
    let buffer = vec![0; 1_000];
    let place = buffer.as_ptr();
    let mut mixer = Mixer::new(RATE);
    mixer.define_with((1, 1), RATE, buffer);
    assert!(mixer.sounds.iter().any(|sound| sound.holds(place)), "not copied");
}

/// Two seconds of a tone given a live sound in pieces of 160 samples, each
/// followed by 20 ms of rendering.
fn stream_tone(mixer: &mut Mixer) -> Vec<i16> {
    let pcm = tone(16_000);
    mixer.start(0, start((1, 1), 90));
    pcm.chunks(160)
        .flat_map(|piece| {
            mixer.stream_samples((1, 1), piece);
            render(mixer, 160)
        })
        .collect()
}

#[test]
fn a_live_sound_given_its_ring_mixes_as_one_made_by_rate() {
    let mut by_rate = Mixer::new(RATE);
    by_rate.stream((1, 1), RATE);
    let expected = stream_tone(&mut by_rate);
    assert!(expected.iter().any(|&value| value != 0), "the tone sounds");
    let mut given = Mixer::new(RATE);
    assert_eq!(given.stream_with((1, 1), RATE, vec![0; Mixer::stream_ring(RATE)]), Ok(()));
    assert_eq!(stream_tone(&mut given), expected);
}

#[test]
fn a_ring_of_the_wrong_length_is_handed_back_and_changes_nothing() {
    let mut mixer = Mixer::new(RATE);
    mixer.stream((1, 1), RATE);
    for length in [0, Mixer::stream_ring(RATE) - 1, Mixer::stream_ring(RATE) + 1] {
        let ring: Vec<i16> = Vec::with_capacity(length + 7);
        let (place, capacity) = (ring.as_ptr(), ring.capacity());
        let mut ring = ring;
        ring.resize(length, 0);
        let refused = mixer.stream_with((1, 1), RATE, ring).unwrap_err();
        assert_eq!((refused.as_ptr(), refused.len(), refused.capacity()), (place, length, capacity), "the same buffer");
        assert!(!mixer.sounds.iter().any(|sound| sound.holds(place)), "not kept");
    }
    // The live sound it would have replaced is still there and plays.
    let out = stream_tone(&mut mixer);
    assert!(out.iter().any(|&value| value != 0));
    // A refused ring for a new key makes no sound.
    assert!(mixer.stream_with((2, 1), RATE, vec![0; 3]).is_err());
    assert_eq!(mixer.sounds.iter().map(Sound::key).collect::<Vec<_>>(), [(1, 1)]);
}

#[test]
fn a_ring_is_four_times_the_target() {
    assert_eq!(Mixer::stream_ring(RATE), 12_800);
    assert_eq!(Mixer::stream_ring(48_000), 76_800);
    // A rate of zero is taken as one, as `stream` takes it.
    assert_eq!(Mixer::stream_ring(0), 1);
}
