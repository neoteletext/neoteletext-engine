//! The input stream's reader, from arbitrary bytes (REQ-INT-101,
//! REQ-INT-102, REQ-TST-040): a message it accepts is read alike whole and
//! split anywhere, and what the writer writes of it reads back the same.
//! Every stamp it accepts is one a channel accepts (REQ-1555), and held
//! input fed from it ends with nothing held (REQ-207).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_input::MAX_MESSAGE;
use neoteletext_input::event::{Event, Item, MAX_STREAM_ITEM, Stamp, StreamDecoder, read_message, write};
use neoteletext_input::held::Held;

fuzz_target!(|data: &[u8]| {
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
