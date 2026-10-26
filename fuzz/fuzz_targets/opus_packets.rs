//! The Opus decoder (REQ-AUD-022, REQ-AUD-023, REQ-TST-040): any bytes, read as a stream of
//! packets, losses and resets, through two decoders; no input may panic, overflow or read out of
//! bounds, and the two must agree exactly although one writes into a buffer that held zeros and
//! the other into one that held a pattern, so no frame may keep its buffer's old contents.
//!
//! Each record is a length byte and then that many bytes: a length of 0 is a lost packet,
//! concealed; 255 resets both decoders and carries no packet.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_opus::{DecodeError, Decoder, FRAME_SAMPLES};

/// The length byte that resets the decoders: longer than any profile packet (`MAX_PACKET`).
const RESET: u8 = 255;
/// What the second decoder's buffer holds before each frame.
const PATTERN: i16 = 0x5A5A;

fuzz_target!(|data: &[u8]| {
    let (mut clean, mut dirty) = (Decoder::new(), Decoder::new());
    let mut rest = data;
    while let Some((&length, after)) = rest.split_first() {
        if length == RESET {
            clean.reset();
            dirty.reset();
            rest = after;
            continue;
        }
        let (packet, after) = after.split_at(usize::from(length).min(after.len()));
        rest = after;
        let mut zeroed = [0i16; FRAME_SAMPLES];
        let mut patterned = [PATTERN; FRAME_SAMPLES];
        if packet.is_empty() {
            clean.conceal(&mut zeroed);
            dirty.conceal(&mut patterned);
        } else {
            let results = (clean.decode(packet, &mut zeroed), dirty.decode(packet, &mut patterned));
            assert!(results.0 == results.1, "the decoders' results differ");
            if matches!(results.0, Err(DecodeError::Packet(_))) {
                // A refused packet leaves the buffer as it was, by design.
                continue;
            }
        }
        assert!(zeroed == patterned, "a frame kept its buffer's old contents");
        assert_eq!(clean.final_range(), dirty.final_range());
    }
});
