//! Every input format reader, chosen by the first byte, fed the rest in
//! pieces whose size the second byte chooses (REQ-TST-040, REQ-INP-001).
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_formats::Item;
use neoteletext_formats::detect::{AutoConfig, AutoReader, Format};
use neoteletext_formats::item::Reader;
use neoteletext_formats::sliced::ByteOrder;
use neoteletext_formats::t42::T42Variant;
use neoteletext_formats::transport::TsPacketSize;

/// Detection, then each explicit format the readers support.
const FORMATS: [Option<Format>; 15] = [
    None,
    Some(Format::T42(T42Variant::Plain)),
    Some(Format::T42(T42Variant::Reversed)),
    Some(Format::T42(T42Variant::Framed)),
    Some(Format::T42(T42Variant::RunIn)),
    Some(Format::TransportStream(TsPacketSize::Standard)),
    Some(Format::TransportStream(TsPacketSize::M2ts)),
    Some(Format::TransportStream(TsPacketSize::Fec)),
    Some(Format::Pes),
    Some(Format::Sliced(ByteOrder::Little)),
    Some(Format::Sliced(ByteOrder::Big)),
    Some(Format::Tti),
    Some(Format::Ep1),
    Some(Format::EditTf),
    Some(Format::ScreenDump),
];

fuzz_target!(|data: &[u8]| {
    let [selector, piece, body @ ..] = data else { return };
    let format = FORMATS[usize::from(*selector) % FORMATS.len()];
    let mut reader = Box::new(AutoReader::new(AutoConfig { format, encode_pages: true, ..AutoConfig::default() }));
    let mut sink = |item: Item<'_>| {
        core::hint::black_box(&item);
    };
    for chunk in body.chunks(usize::from(*piece).max(1)) {
        reader.feed(chunk, &mut sink);
    }
    reader.finish(&mut sink);
});
