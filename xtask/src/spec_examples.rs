//! `spec-examples OUT.json`: writes the byte examples of the generated
//! protocol document (neoteletext-common's REQ-1734), from the engine's own
//! encoders, so that a table in the document cannot drift from the code.
//! The format is the one agreed with rdm: `format`, `producer` and
//! `examples`; a field's `bits` count from 1 as the least significant bit
//! of the byte, as b1 to b8 of EN 300 706.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::ecc::hamming84_encode;
use neoteletext_core::packet::{Packet, decode_address};
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
            Field { offset: 2, length: 1, bits: None, name: "payload length", value: "6".to_owned() },
            Field { offset: 3, length: 2, bits: None, name: "page", value: "0x302".to_owned() },
            Field { offset: 5, length: 1, bits: None, name: "sub-code", value: "0x0002".to_owned() },
            Field { offset: 6, length: 3, bits: None, name: "page parameters", value: EXAMPLE_PARAMETERS.to_owned() },
        ],
        bytes,
    })
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
    let examples = [packet_address()?, select()?];
    let text = document(&head_commit(root)?, &examples)?;
    std::fs::write(path, text).map_err(|error| format!("{path}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_select_example_is_what_the_input_protocol_writes() {
        let example = select().expect("the example encodes");
        assert_eq!(example.bytes, [0xFF, 0x40, 0x06, 0x82, 0x06, 0x02, 0x61, 0x3D, 0x31]);
    }

    #[test]
    fn every_field_lies_within_its_example() {
        for example in [packet_address().expect("it encodes"), select().expect("it encodes")] {
            for field in &example.fields {
                assert!(field.offset + field.length <= example.bytes.len(), "{}: {}", example.name, field.name);
            }
        }
    }

    #[test]
    fn the_file_names_its_examples_and_producer() {
        let text = document("abc", &[select().expect("it encodes")]).expect("it writes");
        assert!(text.contains("\"commit\": \"abc\""), "{text}");
        assert!(text.contains("\"bytes\": \"FF 40 06 82 06 02 61 3D 31\""), "{text}");
    }
}
