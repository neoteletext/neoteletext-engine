//! Server-sent event streams (REQ-TST-040, REQ-WEB-014).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::sse::SseParser;

/// Room for one event; larger events must be refused, not overflow.
const EVENT_BYTES: usize = 2048;

fuzz_target!(|data: &[u8]| {
    let mut parser = SseParser::new();
    let mut event = [0u8; EVENT_BYTES];
    let mut rest = data;
    while let Ok((used, found)) = parser.feed(&mut event, rest) {
        rest = rest.get(used..).unwrap_or_default();
        if used == 0 && found.is_none() {
            break;
        }
    }
    core::hint::black_box(parser.last_event_id());
});
