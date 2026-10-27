//! Page encoder tests (REQ-INP-019).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

use super::*;
use crate::page::{EnhancementPacket, FastextLink, LinkKey, LinkLine, PageFile, PageFunction, RawBody, RawPacket};
use crate::text::Text;
use neoteletext_core::charset::{CharRef, CharSet, G2Set, designation, to_unicode};
use neoteletext_core::consts::{
    C4_ERASE, C7_SUPPRESS_HEADER, C13, COLOUR_WHITE, COLUMNS, DATA_BYTES, HEADER_DISPLAY_BYTES, MODE_G0_NO_DIACRITIC,
    MODE_G2, MODE_SET_ACTIVE_POSITION, ROW_ADDRESS_BASE, ROW_ADDRESS_ROW_24, SPACE, TRIPLETS_PER_PACKET,
};
use neoteletext_core::ecc::{check_parity, hamming84, hamming2418, with_odd_parity};
use neoteletext_core::packet::{Decoded, Packet, PageFormat, PagePacket, Triplet, decode};
use neoteletext_core::types::{ControlBits, Designations, PageAddress, SubCode};
use std::vec::Vec;

fn encode_all(page: &PageFile) -> Vec<Packet> {
    let mut packets = Vec::new();
    encode_page(page, &mut |packet| packets.push(packet));
    packets
}

#[test]
fn guillemet_code_is_the_g2_right_guillemet() {
    let character = to_unicode(CharRef::new(CharSet::G2(G2Set::Latin), G2_RIGHT_GUILLEMET));
    assert_eq!(character.base, '»');
}

#[test]
fn ep1_language_designations_name_their_languages() {
    use neoteletext_core::charset::NationalOption as N;
    let expected = [
        N::English,
        N::German,
        N::SwedishFinnishHungarian,
        N::Italian,
        N::French,
        N::PortugueseSpanish,
        N::CzechSlovak,
        N::Polish,
        N::Turkish,
        N::SerbianCroatianSlovenian,
        N::Rumanian,
    ];
    for ((_, value), national) in crate::consts::EP1_LANGUAGES.iter().zip(expected) {
        assert_eq!(designation(*value).unwrap().g0.national, Some(national), "{value:#x}");
    }
    let (_, greek) = crate::consts::EP1_LANGUAGES[11];
    assert_eq!(designation(greek).unwrap().g2, G2Set::Greek);
}

#[test]
fn header_carries_every_control_bit_and_the_display_bytes() {
    let mut page = PageFile::new();
    page.address = PageAddress::from_mpp(0x8FE).unwrap();
    page.subcode = SubCode::new(0x3F7E);
    page.set_control(ControlBits(C4_ERASE | C7_SUPPRESS_HEADER | C13));
    page.set_row(0, b"01234567Header text that is 32 chars");
    let packets = encode_all(&page);
    let Ok(Decoded::Header(header)) = decode(&packets[0]) else { panic!() };
    assert_eq!(header.magazine.number(), 8);
    assert_eq!(header.page.unwrap().0, 0xFE);
    assert_eq!(header.subcode.unwrap().value(), 0x3F7E);
    assert_eq!(header.control.unwrap(), page.control());
    let display: Vec<u8> = header.display.iter().map(|&byte| check_parity(byte).0).collect();
    assert_eq!(&display[..], &page.row(0).unwrap()[8..]);
    assert_eq!(packets.len(), 1);
}

/// REQ-NUM-007: a page at the null address XFF:3F7F is refused whole,
/// nothing written; page FF with another sub-code, and 3F7F on another
/// page, are written as given (REQ-NUM-002).
#[test]
fn a_page_at_the_null_address_is_not_written() {
    let mut page = PageFile::new();
    page.set_row(1, b"Hello");
    page.address = PageAddress::from_mpp(0x1FF).unwrap();
    page.subcode = SubCode::new(0x3F7F);
    assert_eq!(encode_all(&page).len(), 0);
    page.subcode = SubCode::new(0x3F7E);
    assert_eq!(encode_all(&page).len(), 2);
    page.address = PageAddress::from_mpp(0x1FE).unwrap();
    page.subcode = SubCode::new(0x3F7F);
    assert_eq!(encode_all(&page).len(), 2);
}

#[test]
fn checksum_covers_rows_1_to_25_with_absent_rows_as_spaces() {
    let mut page = PageFile::new();
    page.set_row(1, b"Hello");
    page.fastext[0] = Some(FastextLink { address: PageAddress::from_mpp(0x234).unwrap(), subcode: SubCode::ANY });
    page.fastext_row_24 = true;
    let packets = encode_all(&page);
    let Ok(Decoded::Links(links)) = decode(&packets[1]) else { panic!() };
    assert_eq!(links.links[0].unwrap().magazine.number(), 2);
    let mut rows = [None; 25];
    rows[0] = Some(page.row(1).unwrap().map(with_odd_parity));
    let display = [with_odd_parity(SPACE); HEADER_DISPLAY_BYTES];
    assert_eq!(links.checksum_bytes, checksum_word(page_checksum(&display, &rows)).to_be_bytes());
    // An explicit row of spaces gives the same checksum as an absent row,
    // and a changed byte a different one.
    let mut spaces = rows;
    spaces[10] = Some([with_odd_parity(SPACE); DATA_BYTES]);
    assert_eq!(page_checksum(&display, &spaces), page_checksum(&display, &rows));
    spaces[24] = Some([with_odd_parity(b'x'); DATA_BYTES]);
    assert_ne!(page_checksum(&display, &spaces), page_checksum(&display, &rows));
    // The clock bytes 38 to 45 are not covered.
    let mut clock = display;
    clock[HEADER_DISPLAY_BYTES - 1] = with_odd_parity(b'9');
    assert_eq!(page_checksum(&clock, &rows), page_checksum(&display, &rows));
}

fn page_data(packets: &[Packet]) -> Vec<neoteletext_core::packet::PageEnhancement> {
    packets
        .iter()
        .filter_map(|packet| match decode(packet) {
            Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) => Some(data),
            _ => None,
        })
        .collect()
}

/// REQ-INP-019: `RE` and `PF` of a basic page give X/28/0 format 1 with
/// the designation and, so that it redefines nothing else, the default
/// CLUTs 2 and 3 of Table 30 and the other fields at their defaults.
#[test]
fn page_enhancement_packet_from_region_and_function() {
    let mut page = PageFile::new();
    page.region = Some(3);
    page.function = Some(PageFunction { function: 0, coding: 1 });
    page.set_national_index(5);
    let packets = encode_all(&page);
    let data = page_data(&packets);
    assert_eq!(data.len(), 1);
    let data = data[0];
    assert_eq!(data.fields.designations, Some(Designations { default: (3 << 3) | 5, second: 0x7F }));
    assert_eq!(data.format, Some(PageFormat { function: 0, coding: 1 }));
    assert_eq!(data.fields.colour_words, Some(default_clut_2_and_3()));
    assert_eq!(default_clut_2_and_3()[0].0, 0xF05, "CLUT 2 entry 0 of Table 30");
    let colours = data.fields.colours.unwrap();
    assert_eq!(
        (colours.screen, colours.row, colours.black_background_substitution, colours.remapping),
        (0, 0, false, 0)
    );
    let panels = data.fields.side_panels.unwrap();
    assert!(!panels.left && !panels.right);
}

/// REQ-INP-019: `PF,0,0` alone needs no X/28/0, because a page without one
/// is a basic Level 1 page with odd parity; an explicit `OL,28` line with
/// designation code 0 always replaces the one `RE` and `PF` would give.
#[test]
fn basic_function_alone_sends_no_page_enhancement_packet() {
    let mut page = PageFile::new();
    page.function = Some(PageFunction { function: 0, coding: 0 });
    assert_eq!(page_data(&encode_all(&page)), Vec::new());
    page.region = Some(1);
    let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
    triplets[12] = Triplet::from_value(3 << 4);
    let _ = page.raw_packets.push(RawPacket { number: 28, designation: 0, body: RawBody::Triplets(triplets) });
    let data = page_data(&encode_all(&page));
    assert_eq!(data.len(), 1);
    assert_eq!(data[0].fields.colours.unwrap().screen, 3);
}

/// REQ-INP-024 and Table 6: a page whose function is not basic gets
/// X/28/0 with its function and coding, bits 8 to 18 set to 11111111100
/// read from bit 8, and triplets 2 to 13 reserved as 0.
#[test]
fn page_enhancement_packet_for_other_page_functions() {
    let mut page = PageFile::new();
    page.region = Some(3);
    page.function = Some(PageFunction { function: 2, coding: 2 });
    let packets = encode_all(&page);
    let Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) = decode(&packets[1]) else { panic!() };
    assert_eq!(data.format, Some(PageFormat { function: 2, coding: 2 }));
    let bytes = packets[1].data();
    let first = hamming2418([bytes[1], bytes[2], bytes[3]]).unwrap();
    assert_eq!(first, 0x22 | 0b1111_1111_1000_0000, "bits 8 to 16 set");
    for triplet in bytes[4..].chunks(3) {
        assert_eq!(hamming2418([triplet[0], triplet[1], triplet[2]]), Some(0));
    }
}

#[test]
fn page_triplets_in_row_24_give_way_to_the_link_row() {
    let mut page = PageFile::new();
    let mut triplets = [TERMINATION; TRIPLETS_PER_PACKET];
    triplets[0] = Triplet { address: ROW_ADDRESS_BASE + 1, mode: MODE_SET_ACTIVE_POSITION, data: 0 };
    triplets[1] = Triplet { address: 3, mode: MODE_G0_NO_DIACRITIC, data: b'A' };
    triplets[2] = Triplet { address: ROW_ADDRESS_ROW_24, mode: MODE_SET_ACTIVE_POSITION, data: 0 };
    triplets[3] = Triplet { address: 5, mode: MODE_G0_NO_DIACRITIC, data: b'B' };
    let _ = page.enhancements.push(EnhancementPacket { designation: 0, triplets });
    let (label, _) = Text::from_bytes(b"Away");
    let (service, _) = Text::from_bytes(b"other");
    let link = LinkLine {
        key: LinkKey::Green,
        service,
        address: PageAddress::from_mpp(0x300).unwrap(),
        subcode: None,
        url: Text::new(),
        label,
        label_truncated: false,
        line: 1,
    };
    let _ = page.links.push(link);
    let packets = encode_all(&page);
    let written: Vec<Triplet> = packets
        .iter()
        .filter_map(
            |packet| if let Ok(Decoded::Enhancement(enhancement)) = decode(packet) { Some(enhancement) } else { None },
        )
        .flat_map(|enhancement| enhancement.triplets.map(Option::unwrap))
        .collect();
    assert_eq!(written[..2], triplets[..2]);
    assert_eq!(written[2], Triplet { address: ROW_ADDRESS_ROW_24, mode: MODE_SET_ACTIVE_POSITION, data: 0 });
    assert_eq!(written[3], Triplet { address: 11, mode: MODE_G2, data: G2_RIGHT_GUILLEMET });
    assert_eq!(written[4], TERMINATION);
    // The other-service link is null in X/27/0, and its prompt is white.
    let Some(Ok(Decoded::Links(links))) = packets.iter().map(decode).find(|item| matches!(item, Ok(Decoded::Links(_))))
    else {
        panic!()
    };
    assert!(links.links.iter().all(Option::is_none));
    let row = link_row(&page).unwrap();
    assert_eq!(row.codes[10], COLOUR_WHITE);
    assert_eq!(&row.codes[11..16], b">Away");
}

#[test]
fn labels_cannot_carry_attributes() {
    let mut page = PageFile::new();
    let (label, _) = Text::from_bytes(&[b'a', 0x0D, b'b', 0x88, b'c']);
    let link = LinkLine {
        key: LinkKey::Red,
        service: Text::new(),
        address: PageAddress::from_mpp(0x101).unwrap(),
        subcode: None,
        url: Text::new(),
        label,
        label_truncated: false,
        line: 1,
    };
    let _ = page.links.push(link);
    let row = link_row(&page).unwrap();
    assert_eq!(&row.codes[1..6], b"a b c");
}

/// REQ-INP-023: each row coding turns the codes of a page file into the
/// transmitted bytes and back.
#[test]
fn row_codings_round_trip() {
    use neoteletext_core::packet::ObjectPacket;
    let mut triplets = [TERMINATION; TRIPLETS_PER_PACKET];
    triplets[0] = Triplet { address: 41, mode: MODE_SET_ACTIVE_POSITION, data: 7 };
    let codes = packed_row(1, &triplets);
    assert!(codes.iter().all(|&code| (0x40..0x80).contains(&code)), "printable");
    let bytes = row_bytes(RowCoding::Triplets, &codes);
    let object = ObjectPacket::new(&bytes);
    assert_eq!(object.byte_6, Some(1));
    assert_eq!(object.triplets.map(Option::unwrap), triplets);
    assert_eq!(row_codes(RowCoding::Triplets, &bytes), codes);
    assert_eq!(unpack_triplets(&pack_triplets(&triplets)), triplets);

    let values: [u8; DATA_BYTES] = core::array::from_fn(|index| (index * 7 % 16) as u8);
    let codes = nibble_row(&values);
    let bytes = row_bytes(RowCoding::Nibbles, &codes);
    assert_eq!(bytes.map(|byte| hamming84(byte).unwrap()), values);
    assert_eq!(row_codes(RowCoding::Nibbles, &bytes), codes);

    let mut text = [SPACE; COLUMNS];
    text[..5].copy_from_slice(b"Hello");
    let bytes = row_bytes(RowCoding::Parity, &text);
    assert!(bytes.iter().all(|&byte| check_parity(byte).1));
    assert_eq!(row_codes(RowCoding::Parity, &bytes), text);
    // Bytes that fail their check become spaces or 0, never a panic.
    let mut damaged = bytes;
    damaged[0] ^= 0x80;
    assert_eq!(row_codes(RowCoding::Parity, &damaged)[0], SPACE);
    let uncorrectable = (0..=u8::MAX).find(|&byte| hamming84(byte).is_none()).unwrap();
    assert_eq!(row_codes(RowCoding::Nibbles, &[uncorrectable; DATA_BYTES]), [0x40; COLUMNS]);
}

/// REQ-INP-023: the row coding comes from the page coding, and page mFE
/// without one is coded Hamming 8/4 (clause 10.6.1).
#[test]
fn row_coding_follows_the_page_coding() {
    let mut page = PageFile::new();
    assert_eq!(row_coding(&page), RowCoding::Parity);
    assert!(!is_enhancement_page(&page));
    page.address = PageAddress::from_mpp(0x2FE).unwrap();
    assert_eq!(row_coding(&page), RowCoding::Nibbles);
    assert!(is_enhancement_page(&page));
    page.function = Some(PageFunction { function: 5, coding: 0 });
    assert_eq!(row_coding(&page), RowCoding::Parity);
    for (coding, expected) in
        [(1, RowCoding::Parity), (2, RowCoding::Triplets), (3, RowCoding::Nibbles), (4, RowCoding::Parity)]
    {
        assert_eq!(RowCoding::from_coding(coding), expected);
    }
}
