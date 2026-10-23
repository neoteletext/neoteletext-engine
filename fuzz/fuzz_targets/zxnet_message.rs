//! The zxnet live feed's server messages (REQ-INP-022, REQ-TST-040): a feed
//! past its WebSocket handshake reads each line of the input as one text
//! message, so the message parser and what follows it, the cross-check and
//! the rebuilt packets, see arbitrary JSON.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_core::types::Instant;
use neoteletext_web::port::{ConnectionId, NetInput, RandomSource};
use neoteletext_web::url::IpAddress;
use neoteletext_web::ws::accept_for;
use neoteletext_web::zxnet::{Channel, LiveInput, LiveOutput, LiveStorage, ZxnetFeed};

/// The feed's server: a public address, so the local-network rule passes.
const SERVER: IpAddress = IpAddress::V4([46, 235, 226, 40]);
/// Microseconds between messages: a tenth of a second, inside the feed's
/// inactivity limit.
const STEP_US: u64 = 100_000;
/// The header that carries the handshake's key (RFC 6455 section 4.1).
const KEY_HEADER: &[u8] = b"Sec-WebSocket-Key: ";
/// RFC 6455 section 5.2: a final text frame, unmasked as a server sends it.
const FIN_TEXT: u8 = 0x81;
/// RFC 6455 section 5.2: the payload length codes for a 16-bit and a
/// 64-bit length.
const LENGTH_16: u8 = 126;
const LENGTH_64: u8 = 127;

/// Random bytes that are not random, so a crash reproduces.
struct Counter(u8);

impl RandomSource for Counter {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}

/// One input to the feed; the bytes it sent.
fn step(feed: &mut ZxnetFeed<'_, Counter>, now: Instant, input: LiveInput<'_>) -> Vec<u8> {
    let mut sent = Vec::new();
    feed.handle(now, input, &mut |output| match output {
        LiveOutput::Send(bytes) => sent.extend_from_slice(bytes),
        other => {
            core::hint::black_box(&other);
        }
    });
    sent
}

/// A server's text frame holding `payload`.
fn frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![FIN_TEXT];
    match (u8::try_from(payload.len()), u16::try_from(payload.len())) {
        (Ok(short), _) if short < LENGTH_16 => frame.push(short),
        (_, Ok(medium)) => {
            frame.push(LENGTH_16);
            frame.extend_from_slice(&medium.to_be_bytes());
        }
        _ => {
            frame.push(LENGTH_64);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    frame
}

fuzz_target!(|data: &[u8]| {
    let Ok(channel) = Channel::from_short_name("teefax") else { return };
    let mut storage = Box::new(LiveStorage::new());
    let mut feed = ZxnetFeed::new(channel, storage.buffers(), Counter(0));
    let mut now = Instant(STEP_US);
    step(&mut feed, now, LiveInput::Start);
    let request = step(&mut feed, now, LiveInput::Net(NetInput::Connected(ConnectionId::Live, SERVER)));
    let Some(key) = request
        .split(|&byte| byte == b'\n')
        .find_map(|line| line.strip_prefix(KEY_HEADER))
        .map(|key| key.strip_suffix(b"\r").unwrap_or(key))
    else {
        return;
    };
    let mut answer =
        b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: "
            .to_vec();
    answer.extend_from_slice(&accept_for(key));
    answer.extend_from_slice(b"\r\n\r\n");
    step(&mut feed, now, LiveInput::Net(NetInput::Received(ConnectionId::Live, &answer)));
    for message in data.split(|&byte| byte == b'\n') {
        now = Instant(now.0.saturating_add(STEP_US));
        step(&mut feed, now, LiveInput::Net(NetInput::Received(ConnectionId::Live, &frame(message))));
        step(&mut feed, now, LiveInput::Tick);
    }
});
