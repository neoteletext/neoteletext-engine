//! `neottx sound` (REQ-CLI-011, REQ-AUD-020): a WAVE file written as the
//! audio profile's packets plays back, through the receiver, as the same
//! sound on the page it was written for.
#![cfg(feature = "sound")]
#![allow(missing_docs)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use neoteletext_audio::port::{SoundFormat, SoundId, Start, VoiceSink};
use neoteletext_audio::receiver::Receiver;
use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::packet::Packet;

fn neottx(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_neottx")).args(arguments).output().expect("run neottx")
}

fn folder(name: &str) -> PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("make the test folder");
    folder
}

/// A mono 16-bit WAVE file of `samples` at `rate`.
fn wave(samples: &[i16], rate: u32) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
    let mut bytes = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&[2, 0, 16, 0]);
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&data);
    bytes
}

/// A sink that keeps the samples it is given and the starts.
#[derive(Default)]
struct Keeper {
    pcm: Vec<i16>,
    starts: Vec<(u8, Start)>,
}

impl VoiceSink for Keeper {
    fn define(&mut self, _: SoundId, format: SoundFormat) {
        self.pcm = vec![0; format.samples as usize];
    }
    fn samples(&mut self, _: SoundId, first: u32, pcm: &[i16]) {
        self.pcm[first as usize..first as usize + pcm.len()].copy_from_slice(pcm);
    }
    fn forget(&mut self, _: SoundId) {}
    fn start(&mut self, channel: u8, start: Start) {
        self.starts.push((channel, start));
    }
    fn stop(&mut self, _: u8, _: u16) {}
    fn gain(&mut self, _: u8, _: u8, _: u16) {}
}

/// Writes `samples` with `codec` for page 123, channel 2, and plays the
/// packets back.
fn round_trip(name: &str, samples: &[i16], codec: &str) -> Keeper {
    let folder = folder(name);
    let input = folder.join("sound.wav");
    let output = folder.join("sound.t42");
    std::fs::write(&input, wave(samples, 22_050)).unwrap();
    let run = neottx(&[
        "sound",
        "--page",
        "123",
        "--channel",
        "2",
        "--codec",
        codec,
        input.to_str().unwrap(),
        output.to_str().unwrap(),
    ]);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let stream = std::fs::read(&output).unwrap();
    assert_eq!(stream.len() % PACKET_BYTES, 0, "whole packets");
    let mut arena = vec![0; 1 << 20];
    let mut receiver = Receiver::<8, 2>::new(&mut arena);
    let mut sink = Keeper::default();
    receiver.open_page(Some(0x123), &mut sink);
    for bytes in stream.as_chunks::<PACKET_BYTES>().0 {
        assert!(receiver.push_packet(&Packet { bytes: *bytes }, &mut sink), "every packet is audio");
    }
    sink
}

fn tone(count: usize) -> Vec<i16> {
    (0..count).map(|n| i16::try_from(i32::try_from(n % 100).unwrap() * 300 - 15_000).unwrap()).collect()
}

#[test]
fn pcm_plays_back_exactly_on_its_page() {
    let samples = tone(5000);
    let sink = round_trip("pcm", &samples, "pcm");
    assert_eq!(sink.starts.len(), 1);
    assert_eq!(sink.starts[0].0, 2);
    assert_eq!(sink.starts[0].1.gain, 80);
    assert_eq!(sink.pcm, samples);
}

#[test]
fn ima_adpcm_plays_back_closely() {
    let samples: Vec<i16> = (0..20_000).map(|n| (f64::from(n) * 0.05).sin().mul_add(12_000.0, 0.0) as i16).collect();
    let sink = round_trip("ima", &samples, "ima");
    assert_eq!(sink.starts.len(), 1);
    assert_eq!(sink.pcm.len(), samples.len());
    let signal: f64 = samples.iter().map(|&s| f64::from(s).powi(2)).sum();
    let noise: f64 = samples.iter().zip(&sink.pcm).map(|(&a, &b)| (f64::from(a) - f64::from(b)).powi(2)).sum();
    let snr = 10.0 * (signal / noise).log10();
    assert!(snr > 25.0, "signal to noise {snr:.1} dB");
}

#[test]
fn a_rate_outside_the_profile_is_refused() {
    let folder = folder("rate");
    let input = folder.join("sound.wav");
    let mut bytes = wave(&tone(10), 22_050);
    bytes[24..28].copy_from_slice(&12_345_u32.to_le_bytes());
    std::fs::write(&input, bytes).unwrap();
    let run = neottx(&["sound", "--page", "100", input.to_str().unwrap(), folder.join("out.t42").to_str().unwrap()]);
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("Rate(12345)"), "{}", String::from_utf8_lossy(&run.stderr));
}
