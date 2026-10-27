#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::arithmetic_side_effects, clippy::panic)]

// Tests of REQ-AUD-001 and decision E.131: a codec the build leaves out is
// unsupported, its sound skipped, never played and never a panic.
use super::*;
use crate::port::{SoundFormat, Start};
use crate::record::{TYPE_CONTROL, TYPE_DATA, TYPE_OBJECT, write};
use std::vec::Vec;

/// Counts what the sink is asked to do.
#[derive(Default)]
struct Counter {
    defined: usize,
    samples: usize,
    started: usize,
}

impl VoiceSink for Counter {
    fn define(&mut self, _: SoundId, _: SoundFormat) {
        self.defined += 1;
    }
    fn samples(&mut self, _: SoundId, _: u32, _: &[i16]) {
        self.samples += 1;
    }
    fn forget(&mut self, _: SoundId) {}
    fn stream(&mut self, _: SoundId, _: u32) {}
    fn stream_samples(&mut self, _: SoundId, _: &[i16]) {}
    fn start(&mut self, _: u8, _: Start) {
        self.started += 1;
    }
    fn stop(&mut self, _: u8, _: u16) {}
    fn gain(&mut self, _: u8, _: u8, _: u16) {}
}

fn record(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut bytes = [0_u8; crate::MAX_RECORD];
    let length = write(kind, body, &mut bytes).unwrap();
    bytes[..length].to_vec()
}

/// A one-block sound of handle 1 in codec `codec`, with `block` bytes of
/// `samples` samples, and a play of it for the whole service.
fn sound(codec: u8, block: &[u8], samples: u32) -> [Vec<u8>; 3] {
    const RATE_8_KHZ: u8 = 0;
    let mut object = std::vec![1, 0, 1, 0, 0, 0, 1, codec, RATE_8_KHZ];
    object.extend_from_slice(&u16::try_from(block.len()).unwrap().to_be_bytes());
    object.extend_from_slice(&samples.to_be_bytes());
    object.extend_from_slice(&1_u16.to_be_bytes());
    let mut data = std::vec![1, 0, 1, 0, 0];
    data.extend_from_slice(block);
    let play = [0, 0, 1, 1, 1, 0, 1, 80, 0, 0, 0, 0, 0xFF, 0xFF];
    [record(TYPE_OBJECT, &object), record(TYPE_DATA, &data), record(TYPE_CONTROL, &play)]
}

fn hear(records: &[Vec<u8>]) -> (Counter, Statistics) {
    let mut arena = std::vec![0_u8; 4096];
    let mut receiver = Receiver::<4, 2>::new(&mut arena);
    let mut sink = Counter::default();
    for record in records {
        receiver.push_record(record, &mut sink);
    }
    let statistics = receiver.statistics();
    (sink, statistics)
}

#[test]
fn a_pcm16_sound_plays_only_in_a_build_with_pcm16() {
    let (sink, statistics) = hear(&sound(0, &[0x10; 100], 50));
    assert_eq!(statistics.records_refused, 0);
    if cfg!(feature = "pcm16") {
        assert_eq!((sink.defined, sink.samples, sink.started, statistics.unsupported), (1, 1, 1, 0));
    } else {
        assert_eq!((sink.defined, sink.samples, sink.started, statistics.unsupported), (0, 0, 0, 1));
    }
}

#[test]
fn an_ima_adpcm_sound_plays_only_in_a_build_with_ima() {
    // Predictor 0, step index 0, then 252 bytes of codes: 505 samples.
    let mut block = std::vec![0_u8; usize::from(crate::IMA_BLOCK_SMALL)];
    block[4..].fill(0x17);
    let (sink, statistics) = hear(&sound(1, &block, 505));
    assert_eq!(statistics.records_refused, 0);
    if cfg!(feature = "ima") {
        assert_eq!((sink.defined, sink.samples, sink.started, statistics.unsupported), (1, 1, 1, 0));
    } else {
        assert_eq!((sink.defined, sink.samples, sink.started, statistics.unsupported), (0, 0, 0, 1));
    }
}
