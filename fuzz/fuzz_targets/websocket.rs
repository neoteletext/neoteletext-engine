//! WebSocket frames from the server (REQ-TST-040, RFC 6455 clause 5).
#![no_main]

use libfuzzer_sys::fuzz_target;
use tellytxt_web::ws::FrameReader;

/// Room for one message; larger messages must be refused, not overflow.
const MESSAGE_BYTES: usize = 2048;

fuzz_target!(|data: &[u8]| {
    let mut reader = FrameReader::new();
    let mut message = [0u8; MESSAGE_BYTES];
    let mut rest = data;
    while let Ok((used, found)) = reader.feed(&mut message, rest) {
        rest = rest.get(used..).unwrap_or_default();
        if used == 0 && found.is_none() {
            break;
        }
    }
});
