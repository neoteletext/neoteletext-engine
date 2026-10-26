//! The Opus decoder (REQ-AUD-022, REQ-TST-040): any bytes, read as a stream of packets and
//! losses, through one decoder; no input may panic, overflow or read out of bounds. Each packet
//! is a length byte and then its bytes; a length of 0 is a lost packet, concealed.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_opus::{Decoder, FRAME_SAMPLES};

fuzz_target!(|data: &[u8]| {
    let mut decoder = Decoder::new();
    let mut pcm = [0i16; FRAME_SAMPLES];
    let mut rest = data;
    while let Some((&length, after)) = rest.split_first() {
        let (packet, after) = after.split_at(usize::from(length).min(after.len()));
        rest = after;
        if packet.is_empty() {
            decoder.conceal(&mut pcm);
        } else {
            let _ = decoder.decode(packet, &mut pcm);
        }
    }
});
