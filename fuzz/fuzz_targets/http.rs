//! HTTP response heads and bodies (REQ-TST-040, REQ-WEB-015).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_web::http::{BodyDecoder, Framing, HeadCollector, parse_head};

/// The body limit the agent uses is far larger; a small one exercises the
/// limit path as well.
const BODY_LIMIT: usize = 4096;
/// Room for a response head.
const HEAD_BYTES: usize = 8192;

fuzz_target!(|data: &[u8]| {
    let [selector, body @ ..] = data else { return };
    let _ = parse_head(body, &mut |cookie| {
        core::hint::black_box(cookie);
    });
    let mut collector = HeadCollector::default();
    let mut buffer = [0u8; HEAD_BYTES];
    let mut rest = body;
    while let Ok((used, done)) = collector.feed(&mut buffer, rest) {
        rest = rest.get(used..).unwrap_or_default();
        if done || used == 0 {
            break;
        }
    }
    let framing = match selector % 4 {
        0 => Framing::Chunked,
        1 => Framing::UntilClose,
        2 => Framing::None,
        _ => Framing::Length(u64::from(*selector) * 37),
    };
    let Ok(mut decoder) = BodyDecoder::new(framing, BODY_LIMIT) else { return };
    let mut rest = body;
    while let Ok(step) = decoder.feed(rest) {
        rest = rest.get(step.consumed..).unwrap_or_default();
        if step.done || step.consumed == 0 {
            break;
        }
    }
    let _ = decoder.finish_at_close();
});
