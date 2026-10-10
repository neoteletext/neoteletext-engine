//! Page encoder tests (REQ-INP-019).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

use super::*;
use crate::page::{EnhancementPacket, FastextLink, LinkKey, LinkLine, PageFunction};
use crate::text::Text;
use std::vec::Vec;
use tellytxt_core::charset::{CharRef, CharSet, G2Set, designation, to_unicode};
use tellytxt_core::consts::{C4_ERASE, C7_SUPPRESS_HEADER, C13, MODE_G0_NO_DIACRITIC, ROW_ADDRESS_BASE};
use tellytxt_core::ecc::check_parity;
use tellytxt_core::packet::{Decoded, PageFormat, PagePacket, decode};
use tellytxt_core::types::{ControlBits, Designations, PageAddress};

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
    use tellytxt_core::charset::NationalOption as N;
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

#[test]
fn page_enhancement_packet_from_region_and_function() {
    let mut page = PageFile::new();
    page.region = Some(3);
    page.function = Some(PageFunction { function: 2, coding: 1 });
    page.set_national_index(5);
    let packets = encode_all(&page);
    let Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) = decode(&packets[1]) else {
        panic!("{:?}", decode(&packets[1]))
    };
    assert_eq!(data.fields.designations, Some(Designations { default: (3 << 3) | 5, second: 0x7F }));
    assert_eq!(data.format, Some(PageFormat { function: 2, coding: 1 }));
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
        label,
        label_truncated: false,
        line: 1,
    };
    let _ = page.links.push(link);
    let row = link_row(&page).unwrap();
    assert_eq!(&row.codes[1..6], b"a b c");
}
