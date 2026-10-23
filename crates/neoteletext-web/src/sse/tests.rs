#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The tests of the event-stream parser.

use super::*;

fn events(stream: &[u8], step: usize, limit: usize) -> Result<std::vec::Vec<(SseEvent, std::vec::Vec<u8>)>, SseError> {
    let mut parser = SseParser::new();
    let mut message = std::vec![0u8; limit];
    let mut out = std::vec::Vec::new();
    for piece in stream.chunks(step) {
        let mut rest = piece;
        while !rest.is_empty() {
            let (used, event) = parser.feed(&mut message, rest)?;
            rest = &rest[used..];
            if let Some(event) = event {
                let data = match event {
                    SseEvent::Page(n) | SseEvent::Packets(n) => message[..n].to_vec(),
                    SseEvent::Other => std::vec::Vec::new(),
                };
                out.push((event, data));
            }
        }
    }
    Ok(out)
}

#[test]
fn page_and_packets_events() {
    let stream = b"\xEF\xBB\xBF: comment\r\nevent: page\r\ndata: PN,10000\r\ndata:OL,1,HELLO\r\nid: 7\r\n\r\n\
                       event:packets\ndata: AAAA\n\nretry: 10\ndata: x\n\n";
    for step in [1, 2, 3, 7, stream.len()] {
        let got = events(stream, step, 256).unwrap();
        assert_eq!(got.len(), 3, "step {step}");
        assert_eq!(got[0], (SseEvent::Page(19), b"PN,10000\nOL,1,HELLO".to_vec()));
        assert_eq!(got[1], (SseEvent::Packets(4), b"AAAA".to_vec()));
        assert_eq!(got[2].0, SseEvent::Other, "the default type is ignored");
    }
}

#[test]
fn line_endings_and_ids() {
    let mut parser = SseParser::new();
    let mut message = [0u8; 64];
    let stream = b"id: a\rdata: 1\r\r";
    let (_, event) = parser.feed(&mut message, stream).unwrap();
    assert_eq!(event, Some(SseEvent::Other));
    assert_eq!(parser.last_event_id(), b"a");
    // An id with NUL is ignored; an empty id resets.
    let (_, _) = parser.feed(&mut message, b"id: b\0c\ndata: 2\n\n").unwrap();
    assert_eq!(parser.last_event_id(), b"a");
    let (_, _) = parser.feed(&mut message, b"id\ndata: 3\n\n").unwrap();
    assert_eq!(parser.last_event_id(), b"");
    // CR LF split across pieces counts as one line end.
    let mut parser = SseParser::new();
    assert_eq!(parser.feed(&mut message, b"event: page\r").unwrap().1, None);
    assert_eq!(parser.feed(&mut message, b"\ndata: z\r").unwrap().1, None);
    assert_eq!(parser.feed(&mut message, b"\n\r\n").unwrap().1, Some(SseEvent::Page(1)));
}

#[test]
fn resumption_keeps_last_event_id() {
    let mut parser = SseParser::new();
    let mut message = [0u8; 64];
    parser.feed(&mut message, b"id: 42\ndata: x\n\nevent: page\ndata: half").unwrap();
    parser.restart();
    assert_eq!(parser.last_event_id(), b"42");
    let (_, event) = parser.feed(&mut message, b"data: y\n\n").unwrap();
    assert_eq!(event, Some(SseEvent::Other), "the half event before the drop is discarded");
}

#[test]
fn empty_events_are_not_dispatched() {
    assert_eq!(events(b"event: page\n\n\n\n", 1, 16).unwrap(), []);
    assert_eq!(events(b"data\n\n", 1, 16).unwrap().len(), 1, "data with no colon is an empty line of data");
}

#[test]
fn oversize_event_is_refused() {
    assert_eq!(events(b"event: page\ndata: 0123456789\n\n", 4, 8).map(|e| e.len()), Err(SseError::TooBig));
}

#[test]
fn long_field_names_and_types_are_ignored() {
    let got =
        events(b"eventually: page\ndata: x\n\nevent: pagexxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\ndata: y\n\n", 3, 16)
            .unwrap();
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|(event, _)| *event == SseEvent::Other));
}
