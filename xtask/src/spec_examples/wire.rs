//! The byte examples of the upstream protocol and the downstream record set
//! (neoteletext-common's REQ-1732 item 2 and REQ-1734 detail 1): each is
//! written by the encoder a receiver or a server uses, and each that the wire
//! vectors also hold is tested to be that vector's bytes.

use neoteletext_audio::idl::fragment;
use neoteletext_audio::record::{framed_length, split};
use neoteletext_core::consts::{C11_SERIAL, DATA_BYTES, HEADER_DISPLAY_BYTES, PACKET_BYTES};
use neoteletext_core::ecc::with_odd_parity;
use neoteletext_core::packet::{Packet, encode};
use neoteletext_core::types::{ControlBits, Magazine, PageAddress, PageNumber, SubCode};
use neoteletext_formats::item::{Item as Item_, Reader};
use neoteletext_formats::log::LogLevel;
use neoteletext_formats::tti::TtiReader;
use neoteletext_input::TOKEN_BYTES;
use neoteletext_input::caps::{CURSOR_CAP, RESUME_CAP, SELECT_CAP};
use neoteletext_input::control::{Control, Resumption, packet_of, write_body};
use neoteletext_input::cursor::{BLOCK, Cursor, Place};
use neoteletext_input::event::{
    Action, Device, Event, Item, Key, KeyCode, Mouse, MouseAction, Point, Resume, Select, Stamp, Touch, Wheel, write,
};
use neoteletext_input::{MAX_MESSAGE, VERSION};
use neoteletext_web::cookie::SameSite;
use neoteletext_web::records::cookie::SetCookie;
use neoteletext_web::records::denied::{self, DeniedRecord, STATUS_NOT_FOUND, STATUS_UNAVAILABLE};
use neoteletext_web::records::index::{self, IndexMagazine, IndexPage};
use neoteletext_web::records::redirect::{self, RedirectRecord};
use neoteletext_web::records::subpages::{self, SubPageEntry};
use neoteletext_web::records::{links, log};

use super::{Example, Field, framed};

/// The page every stamped example is on, an example value.
const ON_PAGE: u16 = 0x100;
/// Its sub-code, an example value.
const ON_SUB: u16 = 0;
/// Where a stamped event's fields start: the event byte, the type, and the
/// three bytes of a stamp on page 100, sub-code 0000.
const AFTER_STAMP: usize = 5;
/// The stamp of page 100, sub-code 0000, as the examples' fields name it.
const STAMP_VALUE: &str = "page 100, sub-code 0000";
/// The capabilities the HELLO and WELCOME examples set: CURSOR and SELECT.
const EXAMPLE_CAPS: u32 = CURSOR_CAP | SELECT_CAP;
/// A cell, an example value.
const EXAMPLE_COLUMN: u32 = 10;
/// A cell, an example value.
const EXAMPLE_ROW: u32 = 5;
/// That cell's first dot across: 12 dots to a cell.
const EXAMPLE_DOT_X: u32 = EXAMPLE_COLUMN * 12;
/// That cell's first dot down: 20 dots to a cell.
const EXAMPLE_DOT_Y: u32 = EXAMPLE_ROW * 20;
/// One line of the wheel, in 1/120ths.
const WHEEL_LINE: i32 = 120;
/// The audio and input service the records use (DOC-1951).
const SERVICE: u8 = 0;
/// A token, an example value.
const EXAMPLE_TOKEN: [u8; TOKEN_BYTES] = *b"0123456789abcdef";
/// A resumed session's count, an example value.
const EXAMPLE_TAKEN: u16 = 300;
/// An acknowledgement, an example value.
const EXAMPLE_ACKED: u16 = 42;
/// The sub-page list's example version.
const LIST_VERSION: u16 = 7;
/// The index's example generation.
const INDEX_GENERATION: u16 = 1;
/// An example page for the records.
const RECORD_PAGE: u16 = 0x302;
/// The sub-code the example records name.
const RECORD_SUB: u16 = 2;
/// The example cookie's life in seconds.
const COOKIE_AGE: u32 = 3_600;
/// A sub-page of the LINKS example, as a page file writes it, with each kind
/// of link: `LN`, `LK` and `LT` (the engine's Appendix E).
const LINKS_PAGE: &str = "PN,30002\nSC,0002\nLN,R,200,More\nLK,303,news:100\nLT,5,2,8,https://b.example/n/\n";
/// The row the changed-row frame carries, an example value.
const CHANGED_ROW: u8 = 5;

fn field(offset: usize, length: usize, name: &'static str, value: &str) -> Field {
    Field { offset, length, bits: None, name, value: value.to_owned() }
}

fn stamp() -> Result<Stamp, String> {
    Stamp::new(ON_PAGE, ON_SUB).ok_or_else(|| "the stamp is out of range".to_owned())
}

/// The bytes the input protocol's encoder writes for `item`.
fn written(item: &Item<'_>) -> Result<Vec<u8>, String> {
    let mut buffer = [0_u8; MAX_MESSAGE];
    let length = write(item, &mut buffer).map_err(|error| format!("it does not encode: {error:?}"))?;
    buffer.get(..length).map(<[u8]>::to_vec).ok_or_else(|| "the encoder overran its buffer".to_owned())
}

/// The fields every event begins with.
fn event_head(name: &'static str, type_byte: u8) -> Vec<Field> {
    vec![field(0, 1, "event marker", "0xFF"), field(1, 1, name, &format!("{type_byte:#04X}"))]
}

/// An event's head and its stamp.
fn stamped(name: &'static str, type_byte: u8) -> Vec<Field> {
    let mut fields = event_head(name, type_byte);
    fields.push(field(2, 3, "stamp: page, sub-code", STAMP_VALUE));
    fields
}

fn hello() -> Result<Example, String> {
    let item = Item::Event(Event::Hello { version: VERSION, caps: EXAMPLE_CAPS, agent: None });
    let mut fields = event_head("HELLO", 0x09);
    fields.push(field(2, 1, "version", &VERSION.to_string()));
    fields.push(field(3, 4, "capabilities", &format!("{EXAMPLE_CAPS}: CURSOR and SELECT")));
    Ok(Example {
        name: "input-hello",
        caption: "HELLO: version 7, with CURSOR and SELECT",
        bytes: written(&item)?,
        fields,
    })
}

fn key(on: Stamp, action: Action, code: KeyCode) -> Key<'static> {
    Key { on, action, key: code, mods: 0, text: None, base: None, lost: false }
}

fn key_press() -> Result<Example, String> {
    let press = Key { text: Some("a"), ..key(stamp()?, Action::Press, KeyCode::Character('a')) };
    let mut fields = stamped("KEY", 0x01);
    fields.push(field(AFTER_STAMP, 1, "flags", "0x10: press, T set"));
    fields.push(field(6, 1, "key", "0x61: the code point of a"));
    fields.push(field(7, 2, "text", "length 1, then a"));
    Ok(Example {
        name: "input-key-press",
        caption: "KEY: a pressed on page 100",
        bytes: written(&Item::Event(Event::Key(press)))?,
        fields,
    })
}

fn key_on_a_layout() -> Result<Example, String> {
    let press = Key { text: Some("я"), base: Some('z'), ..key(stamp()?, Action::Press, KeyCode::Character('я')) };
    let mut fields = stamped("KEY", 0x01);
    fields.push(field(AFTER_STAMP, 1, "flags", "0x30: press, T and B set"));
    fields.push(field(6, 2, "key", "U+044F, as an unsigned varint"));
    fields.push(field(8, 3, "text", "length 2, then the two bytes of я"));
    fields.push(field(11, 1, "base", "0x7A: z, the key's place on a United States layout"));
    Ok(Example {
        name: "input-key-base",
        caption: "KEY: я pressed on a Russian layout, with its base character",
        bytes: written(&Item::Event(Event::Key(press)))?,
        fields,
    })
}

fn key_control() -> Result<Example, String> {
    let press =
        Key { mods: neoteletext_input::event::CONTROL, ..key(stamp()?, Action::Press, KeyCode::Character('c')) };
    let mut fields = stamped("KEY", 0x01);
    fields.push(field(AFTER_STAMP, 1, "flags", "0x08: press, M set"));
    fields.push(field(6, 1, "key", "0x63: the code point of c"));
    fields.push(field(7, 1, "mods", "4: Control"));
    Ok(Example {
        name: "input-key-control",
        caption: "KEY: Control-C pressed",
        bytes: written(&Item::Event(Event::Key(press)))?,
        fields,
    })
}

fn key_functional() -> Result<Example, String> {
    let enter = neoteletext_input::keys::code("Enter").ok_or("no such key")?;
    let press = key(stamp()?, Action::Press, KeyCode::Functional(enter));
    let mut fields = stamped("KEY", 0x01);
    fields.push(field(AFTER_STAMP, 1, "flags", "0x04: press, F set"));
    fields.push(field(6, 1, "key", "2: the functional code of Enter"));
    Ok(Example {
        name: "input-key-functional",
        caption: "KEY: Enter pressed",
        bytes: written(&Item::Event(Event::Key(press)))?,
        fields,
    })
}

fn key_lost() -> Result<Example, String> {
    let release = Key { lost: true, ..key(stamp()?, Action::Release, KeyCode::Character('a')) };
    let mut fields = stamped("KEY", 0x01);
    fields.push(field(AFTER_STAMP, 1, "flags", "0x42: release, L set"));
    fields.push(field(6, 1, "key", "0x61: the code point of a"));
    Ok(Example {
        name: "input-key-lost",
        caption: "KEY: a released because the page lost the input",
        bytes: written(&Item::Event(Event::Key(release)))?,
        fields,
    })
}

fn text() -> Result<Example, String> {
    Ok(Example {
        name: "input-text",
        caption: "Raw text: é committed by an input method",
        bytes: written(&Item::Text("é"))?,
        fields: vec![field(0, 2, "UTF-8", "é, U+00E9")],
    })
}

/// The point of the pointer examples.
const fn point(pixels: Option<(u32, u32)>, mods: u8) -> Point {
    Point { column: EXAMPLE_COLUMN, row: EXAMPLE_ROW, pixels, mods }
}

fn mouse_press() -> Result<Example, String> {
    let press = Mouse {
        on: stamp()?,
        action: MouseAction::Press,
        button: 0,
        at: point(Some((EXAMPLE_DOT_X, EXAMPLE_DOT_Y)), neoteletext_input::event::SHIFT),
        device: Device::Mouse,
    };
    let mut fields = stamped("MOUSE", 0x02);
    fields.push(field(AFTER_STAMP, 1, "flags", "0x0C: press, dot and mods present"));
    fields.push(field(6, 1, "button", "0: left"));
    fields.push(field(7, 2, "col, row", "10, 5"));
    fields.push(field(9, 2, "px, py", "120, 100"));
    fields.push(field(11, 1, "mods", "1: Shift"));
    Ok(Example {
        name: "input-mouse-press",
        caption: "MOUSE: the left button pressed at column 10, row 5, with its dot, Shift held",
        bytes: written(&Item::Event(Event::Mouse(press)))?,
        fields,
    })
}

fn mouse_gone() -> Result<Example, String> {
    let gone = Mouse { on: stamp()?, action: MouseAction::Gone, button: 0, at: point(None, 0), device: Device::Mouse };
    let mut fields = stamped("MOUSE", 0x02);
    fields.push(field(AFTER_STAMP, 1, "flags", "0x03: gone"));
    fields.push(field(6, 1, "button", "0: the mask of buttons held, none"));
    fields.push(field(7, 2, "col, row", "10, 5: the last position sent"));
    Ok(Example {
        name: "input-mouse-gone",
        caption: "MOUSE: the pointer gone from column 10, row 5",
        bytes: written(&Item::Event(Event::Mouse(gone)))?,
        fields,
    })
}

fn wheel() -> Result<Example, String> {
    let turned = Wheel { on: stamp()?, dx: 0, dy: WHEEL_LINE, at: point(None, 0) };
    let mut fields = stamped("WHEEL", 0x03);
    fields.push(field(AFTER_STAMP, 1, "flags", "0"));
    fields.push(field(6, 1, "dx", "0"));
    fields.push(field(7, 2, "dy", "120, zigzag then unsigned varint: one line down"));
    fields.push(field(9, 2, "col, row", "10, 5"));
    Ok(Example {
        name: "input-wheel",
        caption: "WHEEL: one line down at column 10, row 5",
        bytes: written(&Item::Event(Event::Wheel(turned)))?,
        fields,
    })
}

fn paste() -> Result<Example, String> {
    let mut fields = stamped("PASTE", 0x04);
    fields.push(field(AFTER_STAMP, 3, "text", "length 2, then hi"));
    Ok(Example {
        name: "input-paste",
        caption: "PASTE: the text hi",
        bytes: written(&Item::Event(Event::Paste { on: stamp()?, text: "hi" }))?,
        fields,
    })
}

fn compose() -> Result<Example, String> {
    let mut fields = stamped("COMPOSE", 0x08);
    fields.push(field(AFTER_STAMP, 1, "cursor", "1: after the one character"));
    fields.push(field(6, 4, "text", "length 3, then か"));
    Ok(Example {
        name: "input-compose",
        caption: "COMPOSE: the preedit か, the cursor after it",
        bytes: written(&Item::Event(Event::Compose { on: stamp()?, cursor: 1, text: "か" }))?,
        fields,
    })
}

fn focus(
    name: &'static str,
    caption: &'static str,
    label: &'static str,
    event: Event<'static>,
) -> Result<Example, String> {
    let bytes = written(&Item::Event(event))?;
    let type_byte = *bytes.get(1).ok_or("the event has no type")?;
    Ok(Example { name, caption, bytes, fields: event_head(label, type_byte) })
}

fn resize() -> Result<Example, String> {
    let item = Item::Event(Event::Resize { columns: 40, rows: 25, width: 0, height: 0 });
    let mut fields = event_head("RESIZE", 0x07);
    fields.push(field(2, 2, "cols, rows", "40, 25"));
    fields.push(field(4, 2, "pw, ph", "0, 0: unknown"));
    Ok(Example {
        name: "input-resize",
        caption: "RESIZE: a page of 40 by 25, drawn size unknown",
        bytes: written(&item)?,
        fields,
    })
}

fn hover() -> Result<Example, String> {
    let item = Item::Event(Event::Hover { on: stamp()?, leave: false, column: 2, row: 20 });
    let mut fields = event_head("HOVER", 0x43);
    fields.push(field(2, 1, "payload length", "6"));
    fields.push(field(3, 3, "stamp: page, sub-code", STAMP_VALUE));
    fields.push(field(6, 1, "flags", "0: the pointer entered"));
    fields.push(field(7, 2, "col, row", "2, 20: the control's first cell"));
    Ok(Example {
        name: "input-hover",
        caption: "HOVER: entering the control at column 2, row 20",
        bytes: written(&item)?,
        fields,
    })
}

fn touch() -> Result<Example, String> {
    let down = Touch {
        on: stamp()?,
        action: MouseAction::Press,
        id: 1,
        column: EXAMPLE_COLUMN + 1,
        row: EXAMPLE_ROW,
        pixels: None,
    };
    let mut fields = event_head("TOUCH", 0x44);
    fields.push(field(2, 1, "payload length", "7"));
    fields.push(field(3, 3, "stamp: page, sub-code", STAMP_VALUE));
    fields.push(field(6, 1, "flags", "0: down"));
    fields.push(field(7, 1, "id", "1"));
    fields.push(field(8, 2, "col, row", "11, 5"));
    Ok(Example {
        name: "input-touch",
        caption: "TOUCH: a second finger down at column 11, row 5",
        bytes: written(&Item::Event(Event::Touch(down)))?,
        fields,
    })
}

fn resume() -> Result<Example, String> {
    let item = Item::Event(Event::Resume(Resume { token: EXAMPLE_TOKEN, acked: EXAMPLE_ACKED }));
    let mut fields = event_head("RESUME", 0x45);
    fields.push(field(2, 1, "payload length", "18"));
    fields.push(field(3, TOKEN_BYTES, "token", "the 16 bytes of the last WELCOME"));
    fields.push(field(19, 2, "acked", &EXAMPLE_ACKED.to_string()));
    Ok(Example {
        name: "input-resume",
        caption: "RESUME: a session's token and the last acknowledgement",
        bytes: written(&item)?,
        fields,
    })
}

fn select_page() -> Result<Example, String> {
    let selection = Select::new(RECORD_PAGE, SubCode::ANY.value()).ok_or("the example is not a selection")?;
    let mut fields = event_head("SELECT", 0x40);
    fields.push(field(2, 1, "payload length", "3"));
    fields.push(field(3, 3, "place", "302"));
    Ok(Example {
        name: "input-select-page",
        caption: "SELECT of page 302",
        bytes: written(&Item::Event(Event::Select(selection, "")))?,
        fields,
    })
}

fn select_nothing() -> Result<Example, String> {
    let nothing = Select::new(0, SubCode::ANY.value()).ok_or("the example is not a selection")?;
    let mut fields = event_head("SELECT", 0x40);
    fields.push(field(2, 1, "payload length", "0: no page of the channel"));
    Ok(Example {
        name: "input-select-nothing",
        caption: "SELECT of nothing: the tab left the channel's pages",
        bytes: written(&Item::Event(Event::Select(nothing, "")))?,
        fields,
    })
}

// ---------------------------------------------------------------- records

/// The first bytes of `packets`' fragments joined: the record they carry, in
/// the length its frame says.
fn record_of(packets: &[Packet]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for packet in packets {
        let read = fragment(packet).ok_or("not a packet of data channel 12")?;
        bytes.extend_from_slice(&read.map_err(|error| format!("it reads as {error:?}"))?.data);
    }
    let length = framed_length(&bytes).map_err(|error| format!("the frame's header does not read: {error:?}"))?;
    bytes.truncate(length);
    split(&bytes).map_err(|error| format!("the frame does not read: {error:?}"))?;
    Ok(bytes)
}

/// The packets `write` gives.
fn collected<E: core::fmt::Debug, T>(
    write: impl FnOnce(&mut dyn FnMut(Packet)) -> Result<T, E>,
) -> Result<Vec<Packet>, String> {
    let mut packets = Vec::new();
    write(&mut |packet| packets.push(packet)).map_err(|error| format!("it does not encode: {error:?}"))?;
    Ok(packets)
}

fn record_page() -> Result<PageAddress, String> {
    PageAddress::from_mpp(RECORD_PAGE).ok_or_else(|| "the page is out of range".to_owned())
}

/// A WELCOME as one whole packet, every byte of it (vector `welcome`).
fn welcome_packet() -> Result<Example, String> {
    let control = Control::Welcome { version: VERSION, caps: EXAMPLE_CAPS, resume: None };
    let packet = packet_of(&control, SERVICE, 0).map_err(|error| format!("it does not encode: {error:?}"))?;
    Ok(Example {
        name: "record-welcome-packet",
        caption: "WELCOME as the 42-byte packet that carries it: version 7, with CURSOR and SELECT",
        bytes: packet.bytes.to_vec(),
        fields: vec![
            field(0, 2, "data channel and designation code", "12 and 15"),
            field(2, 1, "Service Byte", "user-defined, audio service 0"),
            field(3, 1, "Control Byte", "first and last fragment"),
            field(4, 1, "record id", "0"),
            field(5, 1, "fragment index", "0"),
            field(6, 4, "frame header", "profile version 2, type 5, body length 7"),
            field(10, 7, "body", "0xFF, WELCOME 0x01, version 7, capabilities"),
            field(17, 4, "CRC-32", "over the header and body"),
            field(21, 19, "padding", "zeros"),
            field(40, 2, "CRC-16", "over the 36 bytes before it"),
        ],
    })
}

/// A WELCOME of a resumed session, the longest there is in use.
fn welcome_resumed() -> Result<Example, String> {
    let resume = Resumption { token: EXAMPLE_TOKEN, taken: Some(EXAMPLE_TAKEN) };
    let control = Control::Welcome { version: VERSION, caps: EXAMPLE_CAPS | RESUME_CAP, resume: Some(resume) };
    let packet = packet_of(&control, SERVICE, 0).map_err(|error| format!("it does not encode: {error:?}"))?;
    let mut body = [0_u8; neoteletext_input::control::MAX_BODY];
    let length = write_body(&control, &mut body).map_err(|error| format!("it does not encode: {error:?}"))?;
    let fields = vec![
        field(4, 1, "event marker", "0xFF"),
        field(5, 1, "WELCOME", "0x01"),
        field(6, 1, "version", "7"),
        field(7, 4, "capabilities", "CURSOR, SELECT and RESUME"),
        field(11, 1, "session", "1: resumed"),
        field(12, TOKEN_BYTES, "token", "for the next reconnection"),
        field(28, 2, "taken", &EXAMPLE_TAKEN.to_string()),
    ];
    Ok(Example {
        name: "record-welcome-resumed",
        caption: "WELCOME of a resumed session",
        bytes: record_of(&[packet])?,
        fields: framed(length, fields, "5, the control record"),
    })
}

fn cursor() -> Result<Example, String> {
    let record =
        Cursor { page: RECORD_PAGE, acked: EXAMPLE_ACKED, place: Some(Place { row: 5, column: 10 }), flags: BLOCK };
    let bytes = record.record().ok_or("the cursor is out of range")?.to_vec();
    let fields = vec![
        field(4, 2, "page", "0x0302"),
        field(6, 2, "acknowledged", &EXAMPLE_ACKED.to_string()),
        field(8, 1, "row", "5"),
        field(9, 1, "column", "10"),
        field(10, 1, "reserved", "0"),
        field(11, 1, "flags", "4: BLOCK"),
    ];
    Ok(Example {
        name: "record-cursor",
        caption: "A cursor record: a block at row 5, column 10 of page 302",
        bytes,
        fields: framed(8, fields, "4, cursor"),
    })
}

/// The one-magazine index the examples use.
fn index_packets() -> Result<Vec<Packet>, String> {
    let pages = [IndexPage { page: 0x100, subpages: 1, title: "Headlines", section: "Today" }];
    let magazines = [IndexMagazine { magazine: 1, name: "News", pages: &pages }];
    collected(|out| index::encode(INDEX_GENERATION, &magazines, SERVICE, 0, out))
}

fn index_record() -> Result<Example, String> {
    let bytes = record_of(&index_packets()?)?;
    let body = bytes.len() - neoteletext_audio::record::record_length(0);
    let fields = vec![
        field(4, 2, "generation", "1"),
        field(6, 2, "part", "0"),
        field(8, 2, "parts", "1"),
        field(10, 1, "magazine", "1"),
        field(11, 6, "name", "length 4, then News"),
        field(17, 2, "page", "0x0100"),
        field(19, 2, "subpages", "1"),
        field(21, 11, "title", "length 9, then Headlines"),
        field(32, 7, "section", "length 5, then Today"),
    ];
    Ok(Example {
        name: "record-index",
        caption: "An index record: magazine 1, News, with page 100",
        bytes,
        fields: framed(body, fields, "6, index"),
    })
}

fn index_fragments() -> Result<Example, String> {
    let packets = index_packets()?;
    let [first, second] = packets.as_slice() else { return Err("the index is not two packets".to_owned()) };
    let bytes = [first.bytes.as_slice(), second.bytes.as_slice()].concat();
    Ok(Example {
        name: "record-index-fragments",
        caption: "That index record in its two packets of 42 bytes each",
        bytes,
        fields: vec![
            field(0, 4, "address, Service Byte, Control Byte", "first fragment, not last"),
            field(4, 2, "record id, fragment index", "0, 0"),
            field(6, 34, "the record's first 34 bytes", "frame header, then body"),
            field(40, 2, "CRC-16", "of the first packet"),
            field(PACKET_BYTES, 4, "address, Service Byte, Control Byte", "last fragment"),
            field(PACKET_BYTES + 4, 2, "record id, fragment index", "0, 1"),
            field(PACKET_BYTES + 6, 34, "the rest of the record", "padded with zeros"),
            field(PACKET_BYTES + 40, 2, "CRC-16", "of the second packet"),
        ],
    })
}

fn log_record() -> Result<Example, String> {
    let page = record_page()?;
    let packets = collected(|out| log::encode(LogLevel::Warning, Some(page), "no score", SERVICE, 0, out))?;
    let bytes = record_of(&packets)?;
    let fields =
        vec![field(4, 1, "level", "W: a warning"), field(5, 2, "page", "0x0302"), field(7, 8, "text", "no score")];
    Ok(Example {
        name: "record-log",
        caption: "A LOG record: a warning about page 302",
        bytes,
        fields: framed(11, fields, "7, LOG"),
    })
}

fn set_cookie() -> Result<Example, String> {
    let cookie = SetCookie {
        name: "score",
        value: "12",
        path: None,
        max_age: Some(COOKIE_AGE),
        same_site: SameSite::Lax,
        secure: true,
        http_only: true,
    };
    let packets = collected(|out| cookie.encode(SERVICE, 0, out))?;
    let bytes = record_of(&packets)?;
    let body = bytes.len() - neoteletext_audio::record::record_length(0);
    let fields = vec![field(4, body, "the line", "one Set-Cookie header value")];
    Ok(Example {
        name: "record-set-cookie",
        caption: "A SET_COOKIE record",
        bytes,
        fields: framed(body, fields, "8, SET_COOKIE"),
    })
}

fn links_record() -> Result<Example, String> {
    let mut reader = TtiReader::new();
    let mut packets = None;
    let mut take = |item: Item_<'_>| {
        if let Item_::Page(page) = item {
            packets = Some(collected(|out| links::encode_page(page, SERVICE, 0, out)));
        }
    };
    reader.feed(LINKS_PAGE.as_bytes(), &mut take);
    reader.finish(&mut take);
    let packets = packets.ok_or("the page does not read")??;
    let bytes = record_of(&packets)?;
    let body = bytes.len() - neoteletext_audio::record::record_length(0);
    let fields = vec![
        field(4, 2, "page", "0x0302"),
        field(6, 2, "sub", "0x0002"),
        field(8, 1, "part", "0"),
        field(9, 1, "parts", "1"),
        field(10, 1, "keys", "1"),
        field(11, 1, "key", "R"),
        field(12, 5, "target", "length 3, then 200"),
        field(17, 5, "label", "length 4, then More"),
        field(22, 1, "numbers", "1"),
        field(23, 2, "number", "0x0303"),
        field(25, 10, "target", "length 8, then news:100"),
        field(35, 1, "texts", "1"),
        field(36, 3, "row, column, length", "5, 2, 8"),
        field(39, 22, "target", "length 20, then https://b.example/n/"),
    ];
    Ok(Example {
        name: "record-links",
        caption: "A LINKS record: a key link, a page-number link and a text link",
        bytes,
        fields: framed(body, fields, "9, LINKS"),
    })
}

fn subpages_record() -> Result<Example, String> {
    let entries = [
        SubPageEntry { subcode: SubCode::new(1), title: "Monday" },
        SubPageEntry { subcode: SubCode::new(RECORD_SUB), title: "" },
    ];
    let page = record_page()?;
    let packets = collected(|out| subpages::encode(page, LIST_VERSION, &entries, SERVICE, 0, out))?;
    let bytes = record_of(&packets)?;
    let body = bytes.len() - neoteletext_audio::record::record_length(0);
    let fields = vec![
        field(4, 2, "page", "0x0302"),
        field(6, 2, "version", &LIST_VERSION.to_string()),
        field(8, 2, "part", "0"),
        field(10, 2, "parts", "1"),
        field(12, 2, "count", "2"),
        field(14, 2, "sub", "0x0001"),
        field(16, 7, "title", "length 6, then Monday"),
        field(23, 2, "sub", "0x0002"),
        field(25, 1, "title", "length 0: none"),
    ];
    Ok(Example {
        name: "record-subpages",
        caption: "A SUBPAGES record: page 302 has two sub-pages",
        bytes,
        fields: framed(body, fields, "10, SUBPAGES"),
    })
}

fn redirect_place() -> Result<Example, String> {
    let moved =
        RedirectRecord::Page { page: record_page()?, subcode: SubCode::new(RECORD_SUB), parameters: Some("a=1") };
    let packets = collected(|out| redirect::encode(moved, SERVICE, 0, out))?;
    let bytes = record_of(&packets)?;
    let fields = vec![field(4, 12, "place", "302/0002?a=1")];
    Ok(Example {
        name: "record-redirect-place",
        caption: "A REDIRECT record to a place of this channel, keeping a query",
        bytes,
        fields: framed(12, fields, "11, REDIRECT"),
    })
}

fn redirect_address() -> Result<Example, String> {
    let address = "https://b.example/news/";
    let packets = collected(|out| redirect::encode(RedirectRecord::Address(address), SERVICE, 0, out))?;
    let bytes = record_of(&packets)?;
    let fields = vec![field(4, address.len(), "address", address)];
    Ok(Example {
        name: "record-redirect-address",
        caption: "A REDIRECT record to another channel",
        bytes,
        fields: framed(address.len(), fields, "11, REDIRECT"),
    })
}

/// A DENIED record's packet, the vector `denied_not_found`.
fn denied_packets(status: u16, place: &str) -> Result<Vec<Packet>, String> {
    collected(|out| denied::encode(DeniedRecord { status, place }, SERVICE, 0, out))
}

fn denied_page() -> Result<Example, String> {
    let bytes = record_of(&denied_packets(STATUS_NOT_FOUND, "302/0002")?)?;
    let fields = vec![field(4, 2, "status", "404"), field(6, 8, "place", "302/0002")];
    Ok(Example {
        name: "record-denied",
        caption: "A DENIED record: no such sub-page",
        bytes,
        fields: framed(10, fields, "12, DENIED"),
    })
}

fn denied_channel() -> Result<Example, String> {
    let bytes = record_of(&denied_packets(STATUS_UNAVAILABLE, "")?)?;
    let fields = vec![field(4, 2, "status", "503")];
    Ok(Example {
        name: "record-denied-channel",
        caption: "A DENIED record: the whole channel is unavailable",
        bytes,
        fields: framed(2, fields, "12, DENIED"),
    })
}

/// A later version of a sub-page: its header without the erase bit, and the
/// one row that changed (DOC-1949).
fn changed_row() -> Result<Example, String> {
    let display = [with_odd_parity(b' '); HEADER_DISPLAY_BYTES];
    let magazine = Magazine::new(1).ok_or("no such magazine")?;
    let control = ControlBits(C11_SERIAL);
    let header = encode::header(magazine, PageNumber(0), SubCode::new(0), control, &display)
        .ok_or("the null address has no header")?;
    let mut row = [with_odd_parity(b' '); DATA_BYTES];
    for (cell, letter) in row.iter_mut().zip(b"SCORE 12") {
        *cell = with_odd_parity(*letter);
    }
    let changed = encode::row(magazine, CHANGED_ROW, &row);
    Ok(Example {
        name: "page-frame-changed-row",
        caption: "A frame of changed rows for page 100: its header, no erase bit, and row 5",
        bytes: [header.bytes.as_slice(), changed.bytes.as_slice()].concat(),
        fields: vec![
            field(0, 10, "header's address, page and control bytes", "1/0, page 100, C4 clear, C11 set"),
            field(10, 32, "header's display bytes", "spaces, odd parity"),
            field(PACKET_BYTES, 2, "row's address", "1/5"),
            field(PACKET_BYTES + 2, DATA_BYTES, "row's characters", "SCORE 12, then spaces, odd parity"),
        ],
    })
}

/// Every example of this module.
pub(super) fn examples() -> Result<Vec<Example>, String> {
    Ok(vec![
        hello()?,
        key_press()?,
        key_on_a_layout()?,
        key_control()?,
        key_functional()?,
        key_lost()?,
        text()?,
        mouse_press()?,
        mouse_gone()?,
        wheel()?,
        paste()?,
        compose()?,
        focus("input-focus-in", "FOCUS_IN", "FOCUS_IN", Event::FocusIn)?,
        focus("input-focus-out", "FOCUS_OUT", "FOCUS_OUT", Event::FocusOut)?,
        resize()?,
        hover()?,
        touch()?,
        resume()?,
        select_page()?,
        select_nothing()?,
        welcome_packet()?,
        welcome_resumed()?,
        cursor()?,
        index_record()?,
        index_fragments()?,
        log_record()?,
        set_cookie()?,
        links_record()?,
        subpages_record()?,
        redirect_place()?,
        redirect_address()?,
        denied_page()?,
        denied_channel()?,
        changed_row()?,
    ])
}

#[cfg(test)]
mod tests {
    use neoteletext_web::wire_vectors::{bytes_of, vectors};

    use super::*;

    /// The rows of `wire-vectors.tsv` an example also writes.
    const SAME_AS: [(&str, &str); 8] = [
        ("input-hello", "hello"),
        ("input-key-press", "key_press"),
        ("input-focus-in", "focus_in"),
        ("input-focus-out", "focus_out"),
        ("input-resize", "resize"),
        ("input-select-page", "select_page"),
        ("input-select-nothing", "select_nothing"),
        ("record-welcome-packet", "welcome"),
    ];

    #[test]
    fn an_example_the_wire_vectors_hold_is_that_vector() {
        let all = examples().expect("they encode");
        for (example, row) in SAME_AS {
            let found = all.iter().find(|candidate| candidate.name == example).expect("the example exists");
            let vector = vectors().find(|vector| vector.name == row).expect("the row exists");
            let (bytes, length) = bytes_of(vector.value).expect("the row is hexadecimal");
            assert_eq!(found.bytes, bytes.get(..length).expect("it has the length"), "{example}");
        }
    }

    #[test]
    fn the_denied_packets_are_the_vectors() {
        for (status, place, row) in
            [(STATUS_NOT_FOUND, "302/0002", "denied_not_found"), (STATUS_UNAVAILABLE, "", "denied_channel")]
        {
            let packets = denied_packets(status, place).expect("it encodes");
            let [packet] = packets.as_slice() else { panic!("one packet") };
            let vector = vectors().find(|vector| vector.name == row).expect("the row exists");
            let (bytes, length) = bytes_of(vector.value).expect("the row is hexadecimal");
            assert_eq!(packet.bytes.as_slice(), bytes.get(..length).expect("it has the length"), "{row}");
        }
    }

    #[test]
    fn every_record_example_reads_as_its_frame_and_its_fields_cover_it() {
        for example in examples().expect("they encode") {
            if example.name == "record-welcome-packet"
                || !example.name.starts_with("record-")
                || example.name == "record-index-fragments"
            {
                continue;
            }
            assert!(split(&example.bytes).is_ok(), "{}", example.name);
            let end = example.fields.iter().map(|field| field.offset + field.length).max();
            assert_eq!(end, Some(example.bytes.len()), "{}", example.name);
        }
    }

    #[test]
    fn every_field_of_a_record_is_contiguous() {
        for example in examples().expect("they encode") {
            if !example.name.starts_with("record-") || example.name == "record-index-fragments" {
                continue;
            }
            let mut fields: Vec<_> = example.fields.iter().collect();
            fields.sort_by_key(|field| field.offset);
            for pair in fields.windows(2) {
                let [one, two] = pair else { continue };
                assert_eq!(one.offset + one.length, two.offset, "{}: {} then {}", example.name, one.name, two.name);
            }
        }
    }
}
