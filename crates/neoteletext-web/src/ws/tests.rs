#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The tests of the WebSocket handshake, frame writing and frame reader (RFC 6455).

use super::*;

struct Counter(u8);
impl RandomSource for Counter {
    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte = self.0;
            self.0 = self.0.wrapping_add(1);
        }
    }
}

#[test]
fn rfc6455_accept_example() {
    // RFC 6455 section 1.3.
    assert_eq!(&accept_for(b"dGhlIHNhbXBsZSBub25jZQ=="), b"s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
}

#[test]
fn key_is_base64_of_sixteen_random_bytes() {
    let key = new_key(&mut Counter(0));
    let mut decoded = [0u8; 18];
    assert_eq!(base64::decode(&key, &mut decoded).unwrap(), 16);
    assert_eq!(&decoded[..16], &core::array::from_fn::<u8, 16, _>(|i| i as u8));
}

#[test]
fn handshake_request_has_only_profile_headers() {
    let url = Url::parse("wss://zxnet.co.uk/ws/teefax").unwrap();
    let key = *b"dGhlIHNhbXBsZSBub25jZQ==";
    let mut buffer = [0u8; 512];
    let mut writer = Writer::new(&mut buffer);
    write_handshake_headers(&mut writer, &url, &key, b"NeoTeletext/0.1 (macOS)").unwrap();
    writer.put(crate::consts::CRLF).unwrap();
    let text = core::str::from_utf8(writer.written()).unwrap();
    assert_eq!(
        text,
        "GET /ws/teefax HTTP/1.1\r\nHost: zxnet.co.uk\r\nUser-Agent: NeoTeletext/0.1 (macOS)\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
}

#[test]
fn handshake_response_checks() {
    let key = *b"dGhlIHNhbXBsZSBub25jZQ==";
    let good = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
                     Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
    let head = http::parse_head(good, &mut |_| {}).unwrap();
    assert_eq!(check_handshake(&head, &key), Ok(()));
    let bad: [&[u8]; 5] = [
            b"HTTP/1.1 200 OK\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nContent-Length: 0\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: AAAALMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n",
            b"HTTP/1.1 101 OK\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nSec-WebSocket-Extensions: permessage-deflate\r\n\r\n",
        ];
    for text in bad {
        let head = http::parse_head(text, &mut |_| {}).unwrap();
        assert_eq!(check_handshake(&head, &key), Err(WsError::Handshake));
    }
}

#[test]
fn masking_rfc6455_example() {
    // RFC 6455 section 5.7: a masked "Hello".
    let mut buffer = [0u8; 16];
    let mut writer = Writer::new(&mut buffer);
    write_frame_with_mask(&mut writer, OPCODE_TEXT, b"Hello", [0x37, 0xfa, 0x21, 0x3d]).unwrap();
    assert_eq!(writer.written(), &[0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58]);
    let mut writer = Writer::new(&mut buffer);
    write_frame(&mut writer, OPCODE_PING, b"", &mut Counter(9)).unwrap();
    assert_eq!(writer.written(), &[0x89, 0x80, 9, 10, 11, 12]);
}

#[test]
fn long_lengths_are_encoded_minimally() {
    let payload = [0u8; 300];
    let mut buffer = [0u8; 400];
    let mut writer = Writer::new(&mut buffer);
    write_frame_with_mask(&mut writer, OPCODE_BINARY, &payload, [0; 4]).unwrap();
    assert_eq!(&writer.written()[..4], &[0x82, 0xFE, 0x01, 0x2C]);
    assert_eq!(writer.len(), 2 + 2 + 4 + 300);
}

fn read_all(input: &[u8], limit: usize, step: usize) -> Result<std::vec::Vec<(WsMessage, std::vec::Vec<u8>)>, WsError> {
    let mut reader = FrameReader::new();
    let mut message = std::vec![0u8; limit];
    let mut out = std::vec::Vec::new();
    for piece in input.chunks(step) {
        let mut rest = piece;
        loop {
            let (used, done) = reader.feed(&mut message, rest)?;
            rest = &rest[used..];
            if let Some(done) = done {
                let data = match done {
                    WsMessage::Text(n) | WsMessage::Binary(n) => message[..n].to_vec(),
                    _ => reader.control_payload().to_vec(),
                };
                out.push((done, data));
            }
            if rest.is_empty() {
                break;
            }
        }
    }
    Ok(out)
}

#[test]
fn rfc6455_unmasked_examples() {
    // Section 5.7: single-frame text, fragmented text, ping, 256-byte binary.
    let mut stream = std::vec::Vec::new();
    stream.extend_from_slice(&[0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]);
    stream.extend_from_slice(&[0x01, 0x03, 0x48, 0x65, 0x6c]);
    stream.extend_from_slice(&[0x89, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]); // interleaved ping
    stream.extend_from_slice(&[0x80, 0x02, 0x6c, 0x6f]);
    stream.extend_from_slice(&[0x82, 0x7E, 0x01, 0x00]);
    stream.extend_from_slice(&[7u8; 256]);
    stream.extend_from_slice(&[0x88, 0x02, 0x03, 0xE8]);
    for step in [1, 2, 3, 5, stream.len()] {
        let messages = read_all(&stream, 512, step).unwrap();
        assert_eq!(messages.len(), 5, "step {step}");
        assert_eq!(messages[0], (WsMessage::Text(5), b"Hello".to_vec()));
        assert_eq!(messages[1], (WsMessage::Ping, b"Hello".to_vec()));
        assert_eq!(messages[2], (WsMessage::Text(5), b"Hello".to_vec()));
        assert_eq!(messages[3], (WsMessage::Binary(256), std::vec![7u8; 256]));
        assert_eq!(messages[4].0, WsMessage::Close(Some(1000)));
    }
}

#[test]
fn framing_violations() {
    let cases: [(&[u8], WsError); 11] = [
        (&[0x81, 0x85, 0, 0, 0, 0, 1, 2, 3, 4, 5], WsError::Protocol), // masked server frame
        (&[0xC1, 0x00], WsError::Protocol),                            // RSV1 without extension
        (&[0x83, 0x00], WsError::Protocol),                            // reserved opcode
        (&[0x8B, 0x00], WsError::Protocol),                            // reserved control opcode
        (&[0x09, 0x00], WsError::Protocol),                            // fragmented ping
        (&[0x89, 0x7E, 0x00, 0x7E], WsError::Protocol),                // control frame over 125
        (&[0x80, 0x00], WsError::Protocol),                            // continuation without start
        (&[0x01, 0x01, b'a', 0x81, 0x00], WsError::Protocol),          // new message inside fragmented one
        (&[0x82, 0x7E, 0x00, 0x05, 1, 2, 3, 4, 5], WsError::Protocol), // non-minimal 16-bit length
        (&[0x81, 0x02, 0xC3, 0x28], WsError::InvalidText),             // invalid UTF-8
        (&[0x88, 0x01, 0x03], WsError::Protocol),                      // one-byte close payload
    ];
    for (input, error) in cases {
        assert_eq!(read_all(input, 64, input.len()).map(|m| m.len()), Err(error), "{input:02x?}");
    }
    let mut huge = std::vec![0x82, 0x7F];
    huge.extend_from_slice(&(1u64 << 63).to_be_bytes());
    assert_eq!(read_all(&huge, 64, 64).map(|m| m.len()), Err(WsError::Protocol));
}

#[test]
fn oversize_messages() {
    let mut frame = std::vec![0x82, 0x7E, 0x01, 0x00];
    frame.extend_from_slice(&[0u8; 256]);
    assert_eq!(read_all(&frame, 255, 64).map(|m| m.len()), Err(WsError::TooBig));
    // Fragments together exceeding the limit.
    let mut fragments = std::vec![0x02, 0x7E, 0x00, 0x80];
    fragments.extend_from_slice(&[0u8; 128]);
    fragments.extend_from_slice(&[0x80, 0x7E, 0x00, 0x80]);
    fragments.extend_from_slice(&[0u8; 128]);
    assert_eq!(read_all(&fragments, 200, 64).map(|m| m.len()), Err(WsError::TooBig));
    assert_eq!(read_all(&fragments, 256, 64).map(|m| m.len()), Ok(1));
}

/// An oversized data message: the strict reader fails with `TooBig`; the
/// skipping reader drops it, fragments included, and reads the next.
#[test]
fn oversized_messages_fail_or_are_skipped() {
    let big = [b'a'; 200];
    let mut stream = std::vec![OPCODE_TEXT, 126];
    stream.extend_from_slice(&200u16.to_be_bytes());
    stream.extend_from_slice(&big);
    // The same, split into a first frame and a final continuation.
    stream.extend_from_slice(&[OPCODE_TEXT, 100]);
    stream.extend_from_slice(&big[..100]);
    stream.extend_from_slice(&[WS_FIN | OPCODE_CONTINUATION, 100]);
    stream.extend_from_slice(&big[..100]);
    stream.extend_from_slice(&[WS_FIN | OPCODE_TEXT, 2, b'h', b'i']);
    stream[0] |= WS_FIN;
    let mut buffer = [0u8; 16];
    assert_eq!(FrameReader::new().feed(&mut buffer, &stream), Err(WsError::TooBig));
    let mut reader = FrameReader::skipping_oversized();
    let mut rest = stream.as_slice();
    let mut got = std::vec::Vec::new();
    while !rest.is_empty() {
        let (used, message) = reader.feed(&mut buffer, rest).unwrap();
        got.extend(message);
        rest = &rest[used..];
    }
    assert_eq!(got, [WsMessage::Skipped, WsMessage::Skipped, WsMessage::Text(2)]);
    assert_eq!(&buffer[..2], b"hi");
}
