//! `spec-examples OUT.json`: writes the byte examples of the generated
//! protocol document (neoteletext-common's REQ-1734), from the engine's own
//! encoders, so that a table in the document cannot drift from the code.
//! The format is the one agreed with rdm: `format`, `producer` and
//! `examples`; a field's `bits` count from 1 as the least significant bit
//! of the byte, as b1 to b8 of EN 300 706.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use neoteletext_audio::Codec;
use neoteletext_audio::encode::{Command as SoundCommand, Sound, packets};
use neoteletext_audio::idl::fragment;
use neoteletext_audio::record::{MAX_LABEL, Record, parse, split};
use neoteletext_core::consts::{
    C11_SERIAL, HEADER_DISPLAY_BYTES, MODE_SET_ACTIVE_POSITION, PACKET_BYTES, TRIPLET_ADDRESS_BITS, TRIPLET_MODE_BITS,
};
use neoteletext_core::ecc::{hamming84_encode, hamming2418, hamming2418_encode, with_odd_parity};
use neoteletext_core::packet::{Packet, decode_address, encode};
use neoteletext_core::types::{ControlBits, Magazine, PageNumber, SubCode};
use neoteletext_input::MAX_MESSAGE;
use neoteletext_input::event::{Event, Item, Select, write};

/// The version of the file's format.
const FORMAT: u32 = 1;
/// The repository named as the producer.
const REPOSITORY: &str = "neoteletext-engine";

/// The packet address example's magazine, an example value.
const EXAMPLE_MAGAZINE: u8 = 3;
/// The packet address example's packet number, an example value.
const EXAMPLE_PACKET: u8 = 26;
/// Where a packet's address sits among its bytes, after the clock run-in
/// and framing code a T42 packet does not hold.
const ADDRESS_OFFSETS: [usize; 2] = [0, 1];
/// The packet number's low bit within the first address value
/// (EN 300 706 clause 7.1.2).
const PACKET_LOW_BIT_SHIFT: u8 = 3;
/// The selection example's page, an example value.
const EXAMPLE_PAGE: u16 = 0x302;
/// The selection example's sub-code, an example value.
const EXAMPLE_SUB: u16 = 0x0002;
/// The selection example's page parameters, an example value.
const EXAMPLE_PARAMETERS: &str = "a=1";

/// The page header example's magazine, an example value.
const HEADER_MAGAZINE: u8 = 1;
/// The page header example's page number, 100, an example value.
const HEADER_PAGE: u8 = 0x00;
/// The page header example's sub-code, an example value.
const HEADER_SUBCODE: u16 = 0x0002;
/// The triplet example's row address: address 41 is row 1 (clause 12.3.2).
const TRIPLET_ROW_ADDRESS: u32 = 41;
/// The triplet example's column, carried as data by Set Active Position.
const TRIPLET_COLUMN: u32 = 5;
/// The audio examples' handle, an example value.
const SOUND_HANDLE: u8 = 7;
/// The audio examples' generation, an example value.
const SOUND_GENERATION: u16 = 1;
/// The audio examples' sample rate, an example value.
const SOUND_RATE: u32 = 8_000;
/// The audio examples' samples, an example value: two blocks of two samples.
const SOUND_SAMPLES: [i16; 4] = [0, 256, -256, 512];
/// The bytes of one of the examples' PCM blocks: two 16-bit samples.
const SOUND_BLOCK_BYTES: u16 = 4;
/// The audio examples' label, an example value.
const SOUND_LABEL: &[u8] = b"ding";
/// The audio example's page, 100 in EN 300 706 form.
const SOUND_PAGE: u16 = 0x100;
/// The example's gain, per cent.
const SOUND_GAIN: u8 = 80;
/// The example's fade, milliseconds.
const SOUND_FADE_MS: u16 = 20;
/// The example's duck code, 12 dB of EN 300 708 Table 19.
const SOUND_DUCK: u8 = 3;
/// The operation code of a play (REQ-AUD-006).
const OPERATION_PLAY: u8 = 1;
/// The audio service the carriage example uses.
const AUDIO_SERVICE: u8 = 0;
/// The record header: version, type and a two-byte length.
const RECORD_HEADER_FIELDS: [(usize, usize, &str); 3] = [(0, 1, "version"), (1, 1, "type"), (2, 2, "body length")];
/// Bytes of a record's CRC-32.
const RECORD_CRC_BYTES: usize = 4;

/// One named part of an example's bytes.
struct Field {
    offset: usize,
    length: usize,
    bits: Option<&'static str>,
    name: &'static str,
    value: String,
}

/// One example, as the document embeds it.
struct Example {
    name: &'static str,
    caption: &'static str,
    bytes: Vec<u8>,
    fields: Vec<Field>,
}

/// Packet X/26 of magazine 3's address, written with the engine's Hamming
/// 8/4 encoder and read back with its packet decoder (REQ-349).
fn packet_address() -> Result<Example, String> {
    let first = EXAMPLE_MAGAZINE | ((EXAMPLE_PACKET & 1) << PACKET_LOW_BIT_SHIFT);
    let second = EXAMPLE_PACKET >> 1;
    let bytes = vec![hamming84_encode(first), hamming84_encode(second)];
    let mut packet = Packet { bytes: [0; PACKET_BYTES] };
    for (offset, byte) in ADDRESS_OFFSETS.into_iter().zip(&bytes) {
        *packet.bytes.get_mut(offset).ok_or("no room for the address")? = *byte;
    }
    let address = decode_address(&packet).map_err(|error| format!("the example does not decode: {error:?}"))?;
    if address.number != EXAMPLE_PACKET {
        return Err(format!("the example decodes as packet {}", address.number));
    }
    Ok(Example {
        name: "packet-address-magazine-3-packet-26",
        caption: "The two Hamming 8/4 address bytes of packet 3/26",
        bytes,
        fields: vec![
            Field { offset: 0, length: 1, bits: Some("2,4,6"), name: "magazine", value: EXAMPLE_MAGAZINE.to_string() },
            Field {
                offset: 0,
                length: 1,
                bits: Some("8"),
                name: "packet number, low bit",
                value: (EXAMPLE_PACKET & 1).to_string(),
            },
            Field {
                offset: 1,
                length: 1,
                bits: Some("2,4,6,8"),
                name: "packet number, high bits",
                value: second.to_string(),
            },
        ],
    })
}

/// A SELECT of sub-page 0002 of page 302 with page parameters, written with
/// the input protocol's encoder (INPUT.md section 5.4).
fn select() -> Result<Example, String> {
    let selection = Select::new(EXAMPLE_PAGE, EXAMPLE_SUB).ok_or("the example is not a selection")?;
    let mut buffer = [0u8; MAX_MESSAGE];
    let item = Item::Event(Event::Select(selection, EXAMPLE_PARAMETERS));
    let length = write(&item, &mut buffer).map_err(|error| format!("the example does not encode: {error:?}"))?;
    let bytes = buffer.get(..length).ok_or("the encoder overran its buffer")?.to_vec();
    Ok(Example {
        name: "select-page-302-sub-0002",
        caption: "SELECT of sub-page 0002 of page 302, with the page parameters a=1",
        fields: vec![
            Field { offset: 0, length: 1, bits: None, name: "event marker", value: "0xFF".to_owned() },
            Field { offset: 1, length: 1, bits: None, name: "SELECT", value: "0x40".to_owned() },
            Field { offset: 2, length: 1, bits: None, name: "payload length", value: "12".to_owned() },
            Field { offset: 3, length: 8, bits: None, name: "page and sub-page", value: "302/0002".to_owned() },
            Field { offset: 11, length: 1, bits: None, name: "query mark", value: "?".to_owned() },
            Field { offset: 12, length: 3, bits: None, name: "page parameters", value: EXAMPLE_PARAMETERS.to_owned() },
        ],
        bytes,
    })
}

/// A page header of page 100, sub-code 0002, in serial mode (clause 9.3.1),
/// written with the engine's header encoder and read back by its decoder
/// (REQ-350).
fn page_header() -> Result<Example, String> {
    let display = [with_odd_parity(b' '); HEADER_DISPLAY_BYTES];
    let magazine = Magazine::new(HEADER_MAGAZINE).ok_or("no such magazine")?;
    let control = ControlBits(C11_SERIAL);
    let packet = encode::header(magazine, PageNumber(HEADER_PAGE), SubCode::new(HEADER_SUBCODE), control, &display)
        .ok_or("the null address has no header")?;
    let bytes = packet.bytes.to_vec();
    let field = |offset, name, value: &str| Field { offset, length: 1, bits: None, name, value: value.to_owned() };
    Ok(Example {
        name: "page-header-100-sub-0002",
        caption: "The header of page 100, sub-code 0002, in serial mode, showing 32 spaces",
        bytes,
        fields: vec![
            Field { offset: 0, length: 2, bits: None, name: "packet address", value: "1/0".to_owned() },
            field(2, "page units", "0"),
            field(3, "page tens", "0"),
            field(4, "S1", "2"),
            field(5, "S2 and C4", "S2 0, C4 clear"),
            field(6, "S3", "0"),
            field(7, "S4, C5 and C6", "S4 0, C5 and C6 clear"),
            field(8, "C7 to C10", "clear"),
            field(9, "C11 to C14", "C11 set: serial mode"),
            Field { offset: 10, length: 32, bits: None, name: "display bytes", value: "spaces, odd parity".to_owned() },
        ],
    })
}

/// A Set Active Position triplet, to row 1 and column 5 (clause 12.3.3),
/// written with the engine's Hamming 24/18 encoder and read back.
fn triplet() -> Result<Example, String> {
    let mode = u32::from(MODE_SET_ACTIVE_POSITION);
    let value = TRIPLET_ROW_ADDRESS
        | (mode << TRIPLET_ADDRESS_BITS)
        | (TRIPLET_COLUMN << (TRIPLET_ADDRESS_BITS + TRIPLET_MODE_BITS));
    let bytes = hamming2418_encode(value);
    if hamming2418(bytes) != Some(value) {
        return Err("the triplet does not decode".to_owned());
    }
    Ok(Example {
        name: "triplet-set-active-position-row-1-column-5",
        caption: "A Hamming 24/18 triplet: Set Active Position, row 1, column 5",
        bytes: bytes.to_vec(),
        fields: vec![Field {
            offset: 0,
            length: 3,
            bits: None,
            name: "address, mode and data",
            value: format!("address {TRIPLET_ROW_ADDRESS}, mode 00100, data {TRIPLET_COLUMN}"),
        }],
    })
}

/// The record header's fields and the CRC-32 after a body of `body` bytes.
fn framed(body: usize, mut fields: Vec<Field>, kind: &str) -> Vec<Field> {
    let values = ["2".to_owned(), kind.to_owned(), body.to_string()];
    let mut all: Vec<Field> = RECORD_HEADER_FIELDS
        .iter()
        .zip(values)
        .map(|(&(offset, length, name), value)| Field { offset, length, bits: None, name, value })
        .collect();
    all.append(&mut fields);
    all.push(Field {
        offset: all.iter().map(|field| field.offset + field.length).max().unwrap_or(0),
        length: RECORD_CRC_BYTES,
        bits: None,
        name: "CRC-32",
        value: "over the header and body".to_owned(),
    });
    all
}

/// The example sound, with its label.
fn sound() -> Result<Sound, String> {
    let mut sound =
        Sound::new(SOUND_HANDLE, SOUND_GENERATION, Codec::Pcm16, SOUND_RATE, SOUND_BLOCK_BYTES, &SOUND_SAMPLES)
            .map_err(|error| format!("the sound does not encode: {error:?}"))?;
    if SOUND_LABEL.len() > MAX_LABEL {
        return Err("the label is too long".to_owned());
    }
    sound.label = Some(SOUND_LABEL.to_vec());
    Ok(sound)
}

/// An OBJECT record, written by the audio encoder (REQ-AUD-004).
fn audio_object() -> Result<Example, String> {
    let bytes = sound()?.object_record().map_err(|error| format!("the object does not encode: {error:?}"))?;
    let (_, body) = split(&bytes).map_err(|error| format!("the object does not frame: {error:?}"))?;
    let field = |offset, length, name, value: String| Field { offset, length, bits: None, name, value };
    let fields = vec![
        field(4, 1, "handle", SOUND_HANDLE.to_string()),
        field(5, 2, "generation", SOUND_GENERATION.to_string()),
        field(7, 4, "id", SOUND_HANDLE.to_string()),
        field(11, 1, "codec", "0, linear PCM".to_owned()),
        field(12, 1, "sample-rate code", format!("0, {SOUND_RATE} samples a second")),
        field(13, 2, "block bytes", SOUND_BLOCK_BYTES.to_string()),
        field(15, 4, "sample count", SOUND_SAMPLES.len().to_string()),
        field(19, 2, "block count", "2".to_owned()),
        field(21, 1, "extension tag", "1, a label".to_owned()),
        field(22, 1, "extension length", SOUND_LABEL.len().to_string()),
        field(23, SOUND_LABEL.len(), "label", String::from_utf8_lossy(SOUND_LABEL).into_owned()),
    ];
    let fields = framed(body.len(), fields, "1, OBJECT");
    Ok(Example {
        name: "audio-object-record",
        caption: "An OBJECT record: a labelled sound of 4 samples in 2 blocks of linear PCM",
        bytes,
        fields,
    })
}

/// A DATA record, written by the audio encoder (REQ-AUD-005).
fn audio_data() -> Result<Example, String> {
    let bytes = sound()?.data_records().into_iter().next().ok_or("the sound has no data")?;
    let (_, body) = split(&bytes).map_err(|error| format!("the data does not frame: {error:?}"))?;
    let blocks = body.len() - neoteletext_audio::record::DATA_FIXED;
    let field = |offset, length, name, value: String| Field { offset, length, bits: None, name, value };
    let fields = vec![
        field(4, 1, "handle", SOUND_HANDLE.to_string()),
        field(5, 2, "generation", SOUND_GENERATION.to_string()),
        field(7, 2, "first block", "0".to_owned()),
        field(9, blocks, "blocks", format!("{} samples, little-endian", SOUND_SAMPLES.len())),
    ];
    let fields = framed(body.len(), fields, "2, DATA");
    Ok(Example { name: "audio-data-record", caption: "A DATA record: both blocks of that sound", bytes, fields })
}

/// The example command: play the sound on page 100.
fn play() -> SoundCommand {
    SoundCommand {
        channel: 0,
        sequence: 1,
        operation: OPERATION_PLAY,
        handle: SOUND_HANDLE,
        generation: SOUND_GENERATION,
        gain: SOUND_GAIN,
        fade_ms: SOUND_FADE_MS,
        flags: 0,
        duck: SOUND_DUCK,
        page: Some(SOUND_PAGE),
    }
}

/// A CONTROL record, written by the audio encoder and read back (REQ-AUD-006).
fn audio_control() -> Result<Example, String> {
    let bytes = play().record();
    let Ok(Record::Control(read)) = parse(&bytes) else { return Err("the command does not read back".to_owned()) };
    if read.page != Some(SOUND_PAGE) {
        return Err("the command names another page".to_owned());
    }
    let (_, body) = split(&bytes).map_err(|error| format!("the command does not frame: {error:?}"))?;
    let field = |offset, length, name, value: String| Field { offset, length, bits: None, name, value };
    let fields = vec![
        field(4, 1, "channel", "0".to_owned()),
        field(5, 2, "sequence", "1".to_owned()),
        field(7, 1, "operation", "1, play".to_owned()),
        field(8, 1, "handle", SOUND_HANDLE.to_string()),
        field(9, 2, "generation", SOUND_GENERATION.to_string()),
        field(11, 1, "gain", format!("{SOUND_GAIN} per cent")),
        field(12, 2, "fade", format!("{SOUND_FADE_MS} milliseconds")),
        field(14, 1, "flags", "0".to_owned()),
        field(15, 1, "duck", format!("{SOUND_DUCK}, 12 dB")),
        field(16, 2, "page", "0x0100, page 100".to_owned()),
    ];
    let fields = framed(body.len(), fields, "3, CONTROL");
    Ok(Example {
        name: "audio-control-record",
        caption: "A CONTROL record: play that sound on page 100, at 80 per cent, ducking the other channels",
        bytes,
        fields,
    })
}

/// The CONTROL record cut into its one packet 4/31 (REQ-AUD-007).
fn audio_packet() -> Result<Example, String> {
    let (written, _) = packets(AUDIO_SERVICE, 0, &[play().record()]);
    let [packet] = written.as_slice() else { return Err("the record is not one packet".to_owned()) };
    let read = fragment(packet).ok_or("not an audio packet")?.map_err(|error| format!("it reads as {error:?}"))?;
    if !(read.first && read.last) {
        return Err("the fragment is not first and last".to_owned());
    }
    let field = |offset, length, name, value: &str| Field { offset, length, bits: None, name, value: value.to_owned() };
    Ok(Example {
        name: "audio-carriage-packet",
        caption: "A CONTROL record in one packet of data channel 12, with bytes counted from the packet's start",
        bytes: packet.bytes.to_vec(),
        fields: vec![
            field(0, 2, "data channel and designation code", "12 and 15"),
            field(2, 1, "Service Byte", "user-defined, audio service 0"),
            field(3, 1, "Control Byte", "first and last fragment"),
            field(4, 1, "record id", "0"),
            field(5, 1, "fragment index", "0"),
            field(6, 34, "the record, padded with zeros", "22 bytes, then 12 zeros"),
            field(40, 2, "CRC-16", "over the 36 bytes before it"),
        ],
    })
}

/// Every example the document embeds.
fn all_examples() -> Result<Vec<Example>, String> {
    Ok(vec![
        packet_address()?,
        page_header()?,
        triplet()?,
        select()?,
        audio_object()?,
        audio_data()?,
        audio_control()?,
        audio_packet()?,
    ])
}

/// A JSON string. The examples hold no character that needs more than these.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn field_json(field: &Field) -> String {
    let bits = field.bits.map_or_else(String::new, |bits| format!("\"bits\": {}, ", quoted(bits)));
    format!(
        "{{\"offset\": {}, \"length\": {}, {bits}\"name\": {}, \"value\": {}}}",
        field.offset,
        field.length,
        quoted(field.name),
        quoted(&field.value)
    )
}

fn example_json(example: &Example) -> String {
    let bytes = example.bytes.iter().map(|byte| format!("{byte:02X}")).collect::<Vec<_>>().join(" ");
    let fields = example.fields.iter().map(field_json).collect::<Vec<_>>().join(",\n        ");
    format!(
        "    {{\"name\": {}, \"caption\": {}, \"bytes\": {},\n      \"fields\": [\n        {fields}\n      ]}}",
        quoted(example.name),
        quoted(example.caption),
        quoted(&bytes)
    )
}

fn document(commit: &str, examples: &[Example]) -> Result<String, String> {
    let mut out = String::new();
    let body = examples.iter().map(example_json).collect::<Vec<_>>().join(",\n");
    writeln!(out, "{{\n  \"format\": {FORMAT},").map_err(|error| error.to_string())?;
    writeln!(out, "  \"producer\": {{\"repository\": {}, \"commit\": {}}},", quoted(REPOSITORY), quoted(commit))
        .map_err(|error| error.to_string())?;
    writeln!(out, "  \"examples\": [\n{body}\n  ]\n}}").map_err(|error| error.to_string())?;
    Ok(out)
}

fn head_commit(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if !output.status.success() {
        return Err("git could not name the commit".to_owned());
    }
    String::from_utf8(output.stdout).map(|text| text.trim().to_owned()).map_err(|error| error.to_string())
}

/// Writes every example to the path given.
pub fn write_examples(root: &Path, arguments: &[String]) -> Result<(), String> {
    let [path] = arguments else { return Err("usage: cargo xtask spec-examples OUT.json".to_owned()) };
    let text = document(&head_commit(root)?, &all_examples()?)?;
    std::fs::write(path, text).map_err(|error| format!("{path}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_select_example_is_what_the_input_protocol_writes() {
        let example = select().expect("the example encodes");
        assert_eq!(example.bytes, [&[0xFF, 0x40, 12][..], b"302/0002?a=1"].concat());
    }

    #[test]
    fn every_field_lies_within_its_example() {
        for example in all_examples().expect("they encode") {
            for field in &example.fields {
                assert!(field.offset + field.length <= example.bytes.len(), "{}: {}", example.name, field.name);
            }
        }
    }

    #[test]
    fn every_audio_example_reads_back_as_the_record_it_shows() {
        for example in [audio_object(), audio_data(), audio_control()] {
            let example = example.expect("it encodes");
            let end = example.fields.iter().map(|field| field.offset + field.length).max();
            assert_eq!(end, Some(example.bytes.len()), "{}", example.name);
            assert!(parse(&example.bytes).is_ok(), "{}", example.name);
        }
    }

    #[test]
    fn the_file_names_its_examples_and_producer() {
        let text = document("abc", &[select().expect("it encodes")]).expect("it writes");
        assert!(text.contains("\"commit\": \"abc\""), "{text}");
        assert!(text.contains("\"bytes\": \"FF 40 0C 33 30 32 2F 30 30 30 32 3F 61 3D 31\""), "{text}");
    }
}
