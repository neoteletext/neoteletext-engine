//! The input stream's reader, from arbitrary bytes (REQ-INT-101,
//! REQ-INT-102, REQ-TST-040): a message it accepts is read alike whole and
//! split anywhere, and what the writer writes of it reads back the same.
//! Every stamp and SELECT it accepts is one a channel accepts (REQ-1555,
//! REQ-INT-140), and held input fed from it ends with nothing held
//! (REQ-207). The `0xFF` escape never loses its place (DEC-1759 lesson 17).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_input::MAX_MESSAGE;
use neoteletext_input::event::{
    EVENT, Event, FOCUS_IN, Item, MAX_STREAM_ITEM, Select, Stamp, StreamDecoder, is_page_parameters, read_message,
    write,
};
use neoteletext_input::held::Held;

/// An extension type no protocol version has used.
const UNKNOWN_EXTENSION: u8 = 0x7F;
/// The longest payload wrapped: its length is one varint byte.
const UNKNOWN_PAYLOAD: usize = 127;

/// What a decoder may hold: an event begun, or the start of one character.
/// Anything else is commands waiting as text (DEC-1759 lesson 17).
fn holds_one_item(held: &[u8]) -> bool {
    held.is_empty() || held.first() == Some(&EVENT) || (held.len() < 4 && !held.contains(&EVENT))
}

fuzz_target!(|data: &[u8]| {
    // DEC-1759 lesson 17: fed anything in any pieces, a decoder stops at a
    // fault or holds at most one item's start, and a stream's position is
    // never lost: whatever the input's bytes, they are an unknown
    // extension's payload and the event after them is read as an event.
    let mut decoder = StreamDecoder::<MAX_STREAM_ITEM>::default();
    for piece in data.chunks(data.first().map_or(1, |&byte| usize::from(byte % 7) + 1)) {
        if decoder.push(piece, &mut |_| {}).is_err() {
            break;
        }
        assert!(holds_one_item(decoder.holding()), "held {:02X?}", decoder.holding());
    }
    if let Some(payload) = data.get(..data.len().min(UNKNOWN_PAYLOAD)) {
        let mut wrapped = vec![EVENT, UNKNOWN_EXTENSION, payload.len() as u8];
        wrapped.extend_from_slice(payload);
        wrapped.extend_from_slice(&[EVENT, FOCUS_IN]);
        let mut read = Vec::new();
        read_message(&wrapped, &mut |item| read.push(format!("{item:?}"))).expect("a whole extension and an event");
        let skipped = format!("{:?}", Item::Event(Event::Extension { kind: UNKNOWN_EXTENSION, payload }));
        assert_eq!(read, [skipped, format!("{:?}", Item::Event(Event::FocusIn))]);
    }
    // SELECT built from the input reads back as it was, at any value.
    if let [a, b, c, d, ..] = *data
        && let Some(select) = Select::new(u16::from_le_bytes([a, b]), u16::from_le_bytes([c, d]))
    {
        let parameters = core::str::from_utf8(&data[4..]).unwrap_or_default();
        let parameters = if is_page_parameters(select, parameters) { parameters } else { "" };
        let item = Item::Event(Event::Select(select, parameters));
        let mut out = [0_u8; MAX_MESSAGE];
        let length = write(&item, &mut out).expect("a SELECT in bounds is written");
        read_message(&out[..length], &mut |back| assert_eq!(back, item)).expect("what was written is read");
    }
    let mut whole = Vec::new();
    let whole_result = read_message(data, &mut |item| whole.push(format!("{item:?}")));
    if whole_result.is_err() {
        return;
    }
    let mut held = Held::new();
    read_message(data, &mut |item| {
        if let Item::Event(event) = &item {
            let on = match event {
                Event::Key(key) => Some(key.on),
                Event::Mouse(mouse) => Some(mouse.on),
                Event::Wheel(wheel) => Some(wheel.on),
                Event::Touch(touch) => Some(touch.on),
                Event::Paste { on, .. } | Event::Compose { on, .. } | Event::Hover { on, .. } => Some(*on),
                _ => None,
            };
            if let Some(on) = on {
                assert_eq!(Stamp::new(on.page, on.sub), Some(on), "a stamp a channel would refuse");
            }
            if let Event::Select(select, parameters) = event {
                assert_eq!(Select::new(select.page, select.sub), Some(*select), "a SELECT a channel would refuse");
                assert!(is_page_parameters(*select, parameters), "parameters a channel would refuse");
            }
        }
        held.take(&item, &mut |_, _| {});
    })
    .expect("read once already");
    held.release_all(&mut |_, _| {});
    assert!(!held.any(), "nothing held after everything is released");
    // Split at a place the input chooses, the stream decodes the same.
    let split = data.first().map_or(0, |&byte| usize::from(byte)).min(data.len());
    let mut decoder = StreamDecoder::<MAX_STREAM_ITEM>::default();
    let mut streamed = Vec::new();
    decoder.push(&data[..split], &mut |item| streamed.push(format!("{item:?}"))).expect("a prefix of an accepted message");
    decoder.push(&data[split..], &mut |item| streamed.push(format!("{item:?}"))).expect("the rest of it");
    // Text split between pieces arrives as more runs: compare it joined.
    assert_eq!(joined(&streamed), joined(&whole));
    // Written again, each item reads back as it was.
    let mut again = Vec::new();
    read_message(data, &mut |item| {
        let mut out = [0_u8; MAX_MESSAGE];
        let length = write(&item, &mut out).expect("an accepted item is written");
        read_message(&out[..length], &mut |back| assert_eq!(back, item)).expect("what was written is read");
        again.push(());
    })
    .expect("read again");
    assert_eq!(again.len(), whole.len());
});

/// The items, with consecutive text runs joined.
fn joined(items: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if let (Some(last), Some(text)) = (out.last_mut(), item.strip_prefix("Text(\""))
            && let Some(previous) = last.strip_prefix("Text(\"").and_then(|rest| rest.strip_suffix("\")"))
        {
            *last = format!("Text(\"{previous}{text}");
            continue;
        }
        out.push(item.clone());
    }
    out
}
