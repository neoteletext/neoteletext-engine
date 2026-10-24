//! Tests of packet decoding and encoding. REQ-TST-010: headers, links and
//! enhancement packets built by the encoder, every field checked; real
//! captures are decoded by tests/corpus.rs and the service data packet by
//! service.rs.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see `packet.rs`); the attribute
// also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use super::header::HEADER_UNITS;
use crate::consts::{
    C4_ERASE, C8_UPDATE, C11_SERIAL, C12, C14, OBJECT_POINTER_DATA_BIT, OBJECT_POINTER_UNUSED, TRIPLET_BYTES,
};
use crate::ecc::{hamming2418_encode, with_odd_parity};
use crate::types::{ControlBits, DefaultColourFields, Designations, PageNumber, Rgb12, SidePanelFields, SubCode};

fn mag(number: u8) -> Magazine {
    Magazine::new(number).unwrap()
}

#[test]
fn addresses_for_every_magazine_and_packet() {
    for magazine in 1..=8 {
        for number in 0..32 {
            let packet = encode::address(mag(magazine), number);
            let address = decode_address(&packet).unwrap();
            assert_eq!(address.magazine.number(), magazine);
            assert_eq!(address.number, number);
        }
    }
}

#[test]
fn uncorrectable_address_is_rejected() {
    let mut packet = encode::address(mag(1), 1);
    packet.bytes[0] ^= 0b11;
    assert_eq!(decode(&packet), Err(PacketError::BadAddress));
}

#[test]
fn header_fields_round_trip() {
    let display = [with_odd_parity(b'A'); 32];
    let control = ControlBits(C4_ERASE | C8_UPDATE | C11_SERIAL | C12 | C14);
    let packet = encode::header(mag(3), PageNumber(0x45), SubCode::new(0x2A5B), control, &display);
    let Ok(Decoded::Header(header)) = decode(&packet) else { panic!("not a header") };
    assert_eq!(header.magazine.number(), 3);
    assert_eq!(header.page, Some(PageNumber(0x45)));
    assert_eq!(header.subcode, Some(SubCode::new(0x2A5B)));
    assert_eq!(header.control, Some(control));
    assert_eq!(header.display, display);
    assert!(header.is_complete());
    assert_eq!(header.control.unwrap().national_index(), 0b101);
}

#[test]
fn every_control_bit_round_trips() {
    for bit in 4..=14 {
        let control = ControlBits(1 << bit);
        let packet = encode::header(mag(1), PageNumber(0), SubCode::new(0), control, &[0x20; 32]);
        let Ok(Decoded::Header(header)) = decode(&packet) else { panic!() };
        assert_eq!(header.control, Some(control), "C{bit}");
    }
}

#[test]
fn header_with_bad_page_byte_is_incomplete() {
    let mut packet = encode::header(mag(1), PageNumber(0x23), SubCode::new(0), ControlBits(0), &[0x20; 32]);
    packet.bytes[HEADER_UNITS] ^= 0b11;
    let Ok(Decoded::Header(header)) = decode(&packet) else { panic!() };
    assert!(header.page.is_none());
    assert!(!header.is_complete());
}

#[test]
fn links_apply_relative_magazine_and_null() {
    let links = [
        Some((mag(2), PageNumber(0x34), SubCode::ANY)),
        Some((mag(8), PageNumber(0x00), SubCode::new(1))),
        None,
        Some((mag(1), PageNumber(0x99), SubCode::ANY)),
        None,
        Some((mag(5), PageNumber(0x50), SubCode::ANY)),
    ];
    let packet = encode::links(mag(1), &links, true, 0xBEEF);
    let Ok(Decoded::Links(decoded)) = decode(&packet) else { panic!() };
    assert!(decoded.display_row_24);
    assert_eq!(decoded.checksum_bytes, [0xBE, 0xEF]);
    for (expected, got) in links.iter().zip(decoded.links.iter()) {
        match (expected, got) {
            (None, None) => {}
            (Some((m, p, s)), Some(link)) => {
                assert_eq!((link.magazine, link.page, link.subcode), (*m, *p, *s));
            }
            other => panic!("mismatch {other:?}"),
        }
    }
}

#[test]
fn enhancement_triplets_round_trip() {
    let mut triplets = [Triplet::default(); 13];
    triplets[0] = Triplet { address: 41, mode: 4, data: 0 };
    triplets[1] = Triplet { address: 5, mode: 0x10, data: 0x41 };
    triplets[12] = Triplet { address: 63, mode: 0x1F, data: 0 };
    let packet = encode::triplet_packet(mag(1), 26, 7, &triplets);
    let Ok(Decoded::Enhancement(decoded)) = decode(&packet) else { panic!() };
    assert_eq!(decoded.designation, 7);
    for (expected, got) in triplets.iter().zip(decoded.triplets.iter()) {
        assert_eq!(Some(*expected), *got);
    }
}

#[test]
fn page_enhancement_designations() {
    let [first, second] = encode::designation_triplets(0x2B, 0x55);
    let mut triplets = [Triplet::default(); 13];
    triplets[0] = first;
    triplets[1] = second;
    let packet = encode::triplet_packet(mag(2), 28, 0, &triplets);
    let Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) = decode(&packet) else { panic!() };
    assert_eq!(data.format, Some(PageFormat { function: 0, coding: 0 }));
    assert_eq!(data.fields.designations, Some(Designations { default: 0x2B, second: 0x55 }));
    let packet = encode::triplet_packet(mag(2), 29, 0, &triplets);
    assert!(matches!(decode(&packet), Ok(Decoded::Magazine { packet: MagazinePacket::Data(_), .. })));
}

/// Breaks triplet `index` (0 to 12) of a packet with a double error,
/// which Hamming 24/18 detects but cannot correct.
fn break_triplet(packet: &mut Packet, index: usize) {
    let start = DATA_START + 1 + index * TRIPLET_BYTES;
    packet.bytes[start] ^= 0b0110_0000;
    assert_eq!(triplet_values(packet)[index], None, "triplet {index} is uncorrectable");
}

/// The 18-bit value and on-air bytes of a worked encoding.
fn worked(function: TripletFunction, value: u32, bytes: [u8; 3]) {
    let triplet = Triplet::from_value(value);
    assert_eq!(triplet.function(), function, "{value:#07x} decodes");
    assert_eq!(function.to_triplet().value(), value, "{function:?} encodes");
    assert_eq!(hamming2418_encode(value), bytes, "{value:#07x} on air");
}

/// REQ-PKT-010: decoding an encoded function gives the same function,
/// for every one of the 2^18 triplet values.
#[test]
fn every_triplet_function_survives_encoding() {
    for value in 0..1u32 << 18 {
        let function = Triplet::from_value(value).function();
        assert_eq!(function.to_triplet().function(), function, "{value:#07x}");
    }
}

/// REQ-PKT-010: the worked encodings of research/notes-level25-objects.md
/// section 13.2.
#[test]
fn worked_object_triplets() {
    let pop_c3 = ObjectNumber(0x32);
    assert_eq!((pop_c3.subpage(), pop_c3.pointer_packet_index()), (2, 0));
    assert_eq!((pop_c3.pointer_group(), pop_c3.pointer_odd()), (1, true));
    worked(
        TripletFunction::ObjectInvocation { source: ObjectSource::Public, kind: ObjectKind::Active, number: pop_c3 },
        0x19470,
        [0x81, 0x47, 0x32],
    );
    worked(
        TripletFunction::ObjectDefinition { usage: ObjectUsage(0b11), kind: ObjectKind::Active, number: pop_c3 },
        0x19578,
        [0x43, 0x57, 0x32],
    );
    let local = ObjectNumber(0x15);
    assert_eq!((local.n8(), local.designation(), local.triplet()), (false, 1, 5));
    worked(
        TripletFunction::ObjectInvocation { source: ObjectSource::Local, kind: ObjectKind::Passive, number: local },
        0x0ACE8,
        [0xC9, 0x4E, 0x15],
    );
    worked(TripletFunction::OriginModifier { row_offset: 2, column_offset: 56 }, 0x1C42A, [0xDB, 0x42, 0x38]);
    worked(TripletFunction::Termination { kind: 0b111 }, 0x03FFF, [0x74, 0x7F, 0x87]);
    worked(TripletFunction::Termination { kind: 0b010 }, 0x017FF, [0x76, 0x7F, 0x02]);
    let pointers = encode::pointer_triplet(OBJECT_POINTER_UNUSED, 30);
    assert_eq!((pointers.value(), hamming2418_encode(pointers.value())), (0x03DFF, [0xFE, 0x5F, 0x87]));
    let usage = ObjectUsage(0b01);
    assert!(usage.level_25() && !usage.level_35());
}

/// REQ-PKT-010: the worked encodings of research/notes-level25-drcs.md
/// section 11. DRCS Mode reads as a reserved row triplet and DRCS
/// Character Invocation as a column triplet not acted on, each keeping
/// its fields as sent (decision E.95).
#[test]
fn worked_drcs_and_character_triplets() {
    let drcs_mode = |data| TripletFunction::ReservedRow { mode: 0b11000, address: 41, data };
    worked(drcs_mode(0x70), 0x38629, [0x4C, 0x62, 0x70]);
    worked(drcs_mode(0x13), 0x09E29, [0x4E, 0x62, 0x93]);
    let drcs_character = |column, data| TripletFunction::UnsupportedColumn { column, mode: 0b01101, data };
    worked(drcs_character(2, 0x40), 0x20342, [0x19, 0x34, 0x40]);
    worked(drcs_character(10, 47), 0x17B4A, [0x5B, 0x34, 0x2F]);
    assert_eq!(drcs_character(10, 47).active_column(), Some(10), "it moves the active position");
    assert_eq!(drcs_mode(0x70).active_column(), None);
    worked(TripletFunction::G3Character { column: 0, code: 0x20, level_15_form: false }, 0x102C0, [0x00, 0x2C, 0xA0]);
    worked(TripletFunction::G3Character { column: 5, code: 0x5B, level_15_form: true }, 0x2D885, [0x27, 0x08, 0xDB]);
    // Font Style, with every style and two more rows, is not acted on
    // (decision E.96).
    worked(TripletFunction::UnsupportedColumn { column: 4, mode: 0b01110, data: 0x27 }, 0x13B84, [0x20, 0xB8, 0x27]);
}

/// REQ-PKT-010: one triplet of every other mode of Tables 28 and 29,
/// with its fields.
#[test]
fn every_mode_has_its_fields() {
    use TripletFunction as F;
    let t = |address: u8, mode: u8, data: u8| Triplet { address, mode, data }.function();
    let colour = |index, high| ColourData { index, high };
    assert_eq!(t(45, 0b00000, 0x1A), F::FullScreenColour(colour(0x1A, 0)));
    assert!(!colour(3, 1).is_defined());
    assert_eq!(t(50, 0b00001, 0x7F), F::FullRowColour { address: 50, colour: colour(0x1F, 0b11) });
    assert_eq!(t(41, 0b00100, 45), F::SetActivePosition { address: 41, column: 45 });
    assert_eq!(t(63, 0b00111, 0x05), F::AddressDisplayRow0 { address: 63, colour: colour(5, 0) });
    for mode in 0b01000..=0b01101 {
        assert_eq!(t(44, mode, 9), F::ProgrammeDeliveryRow { mode, address: 44, data: 9 });
    }
    for mode in [0b00010, 0b00011, 0b00101, 0b00110, 0b01110, 0b01111, 0b10100, 0b11001, 0b11110] {
        assert_eq!(t(44, mode, 9), F::ReservedRow { mode, address: 44, data: 9 }, "row mode {mode:#07b}");
    }
    assert_eq!(t(62, 0b11111, 7), F::ReservedRow { mode: 0b11111, address: 62, data: 7 });
    let gpop = t(0b11_1011, 0b10010, 0x41);
    assert_eq!(
        gpop,
        F::ObjectInvocation { source: ObjectSource::Global, kind: ObjectKind::Adaptive, number: ObjectNumber(0x1C1) }
    );
    assert_eq!(ObjectNumber(0x1C1).pointer_packet_index(), 3);
    assert_eq!(t(1, 0b00000, 0x28), F::ForegroundColour { column: 1, colour: colour(8, 1) });
    assert_eq!(t(2, 0b00001, 0x7F), F::G1Character { column: 2, code: 0x7F });
    assert_eq!(t(3, 0b00011, 0x11), F::BackgroundColour { column: 3, colour: colour(0x11, 0) });
    for mode in [0b00100, 0b00101, 0b01010] {
        assert_eq!(t(4, mode, 1), F::ReservedColumn { column: 4, mode, data: 1 });
    }
    assert_eq!(t(5, 0b00110, 3), F::ProgrammeDeliveryColumn { column: 5, data: 3 });
    assert_eq!(t(6, 0b00111, 0b011_0011), F::AdditionalFlash { column: 6, mode: 0b11, rate: 0b100, reserved: 1 });
    assert_eq!(t(7, 0b01000, 0x37), F::ModifiedDesignation { column: 7, designation: 0x37 });
    assert_eq!(t(8, 0b01001, b'A'), F::G0Character { column: 8, code: b'A' });
    let attributes = DisplayAttributes {
        double_height: true,
        boxing: false,
        conceal: true,
        invert: false,
        underline: true,
        double_width: true,
    };
    // D3 is reserved and ignored.
    assert_eq!(t(9, 0b01100, 0b110_1101), F::DisplayAttributes { column: 9, attributes });
    assert_eq!(t(10, 0b01111, 0x3B), F::G2Character { column: 10, code: 0x3B });
    assert_eq!(t(11, 0b10000, 0x2A), F::G0Diacritic { column: 11, code: 0x2A, diacritic: 0 });
    assert_eq!(t(12, 0b11000, b'e'), F::G0Diacritic { column: 12, code: b'e', diacritic: 8 });
}

/// REQ-PKT-011: the worked link of research/notes-level25-objects.md
/// section 13.2, an encoded packet, and an injected error.
#[test]
fn compositional_links_decode_and_mark_damage() {
    let link = CompositionalLink::from_values(mag(1), 0x39CCD, 0x0000C);
    let expected = CompositionalLink {
        function: 0b01,
        validity: 0b11,
        magazine: mag(2),
        page: PageNumber(0xE3),
        subpages: 0b11,
        marker_first: true,
        marker_second: false,
    };
    assert_eq!(link, expected);
    assert!(link.is_format_1() && !link.is_null());
    assert_eq!(link.to_values(mag(1)), (0x39CCD, 0x0000C));
    assert_eq!(hamming2418_encode(0x39CCD), [0x66, 0x4C, 0xF3]);
    assert_eq!(hamming2418_encode(0x0000C), [0xEA, 0x80, 0x80]);
    let null = CompositionalLink { page: PageNumber(0xFF), subpages: 0xFFFF, ..expected };
    let mut links = [expected, null, expected, expected, expected, expected];
    links[2].magazine = mag(8);
    links[3].subpages = 0;
    let mut packet = encode::compositional_links(mag(1), 5, &links);
    let Ok(Decoded::CompositionalLinks { designation: 5, links: decoded, .. }) = decode(&packet) else { panic!() };
    assert_eq!(decoded, links.map(Some));
    assert!(decoded[1].unwrap().is_null() && decoded[3].unwrap().is_null());
    break_triplet(&mut packet, 3);
    let Ok(Decoded::CompositionalLinks { links: decoded, .. }) = decode(&packet) else { panic!() };
    assert_eq!((decoded[0], decoded[1], decoded[2]), (Some(links[0]), None, Some(links[2])));
    assert!(matches!(decode(&encode::compositional_links(mag(1), 6, &links)), Ok(Decoded::Ignored(_))));
}

fn sample_fields() -> PresentationFields {
    let mut words = [Rgb12::default(); 16];
    for (index, word) in words.iter_mut().enumerate() {
        let value = index as u8;
        *word = Rgb12::new(value, 15 - value, value ^ 0b1010);
    }
    PresentationFields {
        designations: Some(Designations { default: 0x55, second: 0x2A }),
        side_panels: Some(SidePanelFields { left: true, right: false, status_flag: true, left_columns: 11 }),
        colour_words: Some(words),
        colours: Some(DefaultColourFields {
            screen: 0x13,
            row: 0x1D,
            black_background_substitution: true,
            remapping: 0b101,
        }),
    }
}

/// REQ-PKT-012 and REQ-PKT-015: every field of X/28/0 format 1 and
/// X/28/4, the bit order of the colour words, and an error injected
/// into each field group in turn (decision E.44).
#[test]
fn page_enhancement_fields_and_field_groups() {
    let fields = sample_fields();
    let format = PageFormat { function: 0, coding: 0 };
    for designation in [0, 4] {
        let packet = encode::page_enhancement(mag(3), designation, format, &fields);
        let decoded = match decode(&packet) {
            Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) if designation == 0 => data,
            Ok(Decoded::PageData { packet: PagePacket::Level35(data), .. }) if designation == 4 => data,
            other => panic!("{other:?}"),
        };
        assert_eq!(decoded, PageEnhancement { format: Some(format), fields });
    }
    // Colour word 0's red is the first stream bit, triplet 2 bit 11;
    // word 15's blue ends at triplet 13 bit 4 (Table 4).
    let mut words = [Rgb12::default(); 16];
    words[0] = Rgb12::new(1, 0, 0);
    words[15] = Rgb12::new(0, 0, 8);
    let only_words = PresentationFields { colour_words: Some(words), ..PresentationFields::default() };
    let packet = encode::page_enhancement(mag(1), 0, format, &only_words);
    let values = triplet_values(&packet);
    assert_eq!((values[1], values[12]), (Some(1 << 10), Some(1 << 3)));
    assert!(values[2..12].iter().all(|value| *value == Some(0)));
    // Triplet 13: screen bits 5 to 9, row 10 to 14, substitution 15,
    // re-mapping 16 to 18.
    let colours = PresentationFields { colours: fields.colours, ..PresentationFields::default() };
    let values = triplet_values(&encode::page_enhancement(mag(1), 0, format, &colours));
    assert_eq!(values[12], Some(0x13 << 4 | 0x1D << 9 | 1 << 14 | 0b101 << 15));
    // Side panels: triplet 2 bits 4 to 10.
    let panels = PresentationFields { side_panels: fields.side_panels, ..PresentationFields::default() };
    let values = triplet_values(&encode::page_enhancement(mag(1), 0, format, &panels));
    assert_eq!(values[1], Some(1 << 3 | 1 << 5 | 11 << 6));
    let damaged = |index: usize| {
        let mut packet = encode::page_enhancement(mag(3), 0, format, &fields);
        break_triplet(&mut packet, index);
        let Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) = decode(&packet) else { panic!() };
        data
    };
    let data = damaged(0);
    assert_eq!((data.format, data.fields.designations), (None, None));
    assert_eq!((data.fields.side_panels, data.fields.colours), (fields.side_panels, fields.colours));
    assert_eq!(data.fields.colour_words, fields.colour_words);
    let data = damaged(1);
    assert_eq!(data.format, Some(format));
    assert_eq!((data.fields.designations, data.fields.side_panels, data.fields.colour_words), (None, None, None));
    assert_eq!(data.fields.colours, fields.colours);
    let data = damaged(6);
    assert_eq!(data.fields, PresentationFields { colour_words: None, ..fields });
    let data = damaged(12);
    assert_eq!(data.fields, PresentationFields { colour_words: None, colours: None, ..fields });
}

/// REQ-PKT-016: M/29/0, M/29/1 and M/29/4, and the packet functions
/// that are ignored.
#[test]
fn magazine_packets() {
    let fields = sample_fields();
    let packet = encode::magazine_enhancement(mag(4), 0, &fields);
    assert_eq!(decode(&packet), Ok(Decoded::Magazine { magazine: mag(4), packet: MagazinePacket::Data(fields) }));
    let packet = encode::magazine_enhancement(mag(4), 4, &fields);
    assert_eq!(decode(&packet), Ok(Decoded::Magazine { magazine: mag(4), packet: MagazinePacket::Level35(fields) }));
    // M/29/1, the DRCS colour tables, is not decoded (decision E.95).
    let tables = encode::triplet_packet(mag(4), 29, 1, &[Triplet::default(); 13]);
    assert!(matches!(decode(&tables), Ok(Decoded::Ignored(_))));
    // A reserved packet function, and an uncorrectable triplet 1.
    let mut values = [0u32; 13];
    values[0] = 1;
    let reserved = encode::triplet_packet(mag(4), 29, 0, &values.map(Triplet::from_value));
    assert!(matches!(decode(&reserved), Ok(Decoded::Ignored(_))));
    let mut broken = encode::magazine_enhancement(mag(4), 4, &fields);
    break_triplet(&mut broken, 0);
    assert!(matches!(decode(&broken), Ok(Decoded::Ignored(_))));
    let other = encode::magazine_enhancement(mag(4), 2, &fields);
    assert!(matches!(decode(&other), Ok(Decoded::Ignored(_))));
}

/// REQ-PKT-014, decision E.95: of X/28/3 only the page function and
/// coding are read, so a pattern download page is still known not to be
/// text; and X/28/1 is not decoded.
#[test]
fn x28_3_gives_only_the_page_function() {
    let format = PageFormat { function: 0b0101, coding: 0 };
    let mut packet = encode::page_function(mag(1), format);
    assert_eq!(
        decode(&packet),
        Ok(Decoded::PageData { magazine: mag(1), packet: PagePacket::PageFunction(Some(format)) })
    );
    break_triplet(&mut packet, 0);
    assert_eq!(decode(&packet), Ok(Decoded::PageData { magazine: mag(1), packet: PagePacket::PageFunction(None) }));
    let tables = encode::triplet_packet(mag(1), 28, 1, &[Triplet::default(); 13]);
    assert!(matches!(decode(&tables), Ok(Decoded::Ignored(_))));
}

/// REQ-PKT-017: object page packets, pointer tables, an uncorrectable
/// pointer triplet, and the extreme triplet numbers.
#[test]
fn object_pages_and_pointers() {
    let mut triplets = [Triplet::default(); 13];
    triplets[4] = encode::pointer_triplet(OBJECT_POINTER_UNUSED, 30);
    triplets[5] = encode::pointer_triplet(507, 0);
    let mut packet = encode::triplet_packet(mag(1), 1, OBJECT_POINTER_DATA_BIT, &triplets);
    let object = ObjectPacket::new(&packet.data());
    assert!(object.carries_pointers());
    assert_eq!((object.pointer(4, false), object.pointer(4, true)), (OBJECT_POINTER_UNUSED, 30));
    assert_eq!((object.pointer(5, false), object.pointer(5, true)), (507, 0));
    assert_eq!(object.pointer(0, false), OBJECT_POINTER_UNUSED, "triplet 0 is reserved");
    assert_eq!(triplet_location(30), Some(TripletLocation::Row { packet: 5, triplet: 4 }));
    break_triplet(&mut packet, 5);
    let object = ObjectPacket::new(&packet.data());
    assert_eq!((object.pointer(5, false), object.pointer(5, true)), (OBJECT_POINTER_UNUSED, OBJECT_POINTER_UNUSED));
    let definitions = encode::triplet_packet(mag(1), 2, 0, &triplets);
    assert!(!ObjectPacket::new(&definitions.data()).carries_pointers());
    assert_eq!(triplet_location(0), Some(TripletLocation::Row { packet: 3, triplet: 0 }));
    assert_eq!(triplet_location(298), Some(TripletLocation::Row { packet: 25, triplet: 12 }));
    assert_eq!(triplet_location(299), Some(TripletLocation::Enhancement { designation: 0, triplet: 0 }));
    assert_eq!(triplet_location(506), Some(TripletLocation::Enhancement { designation: 15, triplet: 12 }));
    assert_eq!(triplet_location(507), None);
}

/// REQ-PKT-018: byte-pair positions of Figure 20, as
/// research/notes-level25-objects.md sections 7.2 and 13.3 list them.
#[test]
fn mot_positions() {
    for (page, packet, byte) in [
        (0x00, 1, 6),
        (0x19, 1, 44),
        (0x99, 5, 44),
        (0xA0, 6, 6),
        (0xF9, 8, 44),
        (0x0A, 9, 6),
        (0x2F, 9, 40),
        (0xFE, 14, 14),
        (0xFF, 14, 16),
        (0x23, 2, 12),
        (0x4F, 10, 28),
        (0xB7, 6, 40),
    ] {
        assert_eq!(mot_position(PageNumber(page)), MotPosition { packet, index: byte - 6 }, "page {page:02X}");
    }
}

/// REQ-PKT-018: links, byte pairs, totals and fallback bits, the default
/// object of research/notes-level25-objects.md section 13.4, and
/// uncorrectable bytes.
#[test]
fn mot_links_and_entries() {
    let mut row = [0u8; DATA_BYTES];
    // POP link 1 (bytes 16 to 25) as nibbles: magazine 0 (8) with the
    // X/27 flag, page 1E3, 3 sub-pages, fallback right panel with
    // substitution, defaults 0x9 then objects 0x32 and 0x00.
    let link = [0x8, 0xE, 0x3, 0x2, 0b1100, 0x9, 0x2, 0x3, 0x0, 0x0];
    row[10..20].copy_from_slice(&link);
    row[0..2].copy_from_slice(&encode::mot_page_entry(MotPageEntry { global_objects: true, pop_link: 5 }));
    // The second byte names a DRCS page, which is not read (decision
    // E.95); the encoder writes it as 0.
    assert_eq!(row[1], 0);
    row[1] = 0x7;
    let packet = encode::nibble_packet(mag(2), 19, &row);
    let data = packet.data();
    let decoded = mot_object_link(&data, 1).unwrap();
    assert_eq!(decoded.magazine, mag(8));
    assert!(decoded.x27_flag);
    assert_eq!((decoded.page, decoded.subpage_count), (PageNumber(0xE3), 2));
    assert_eq!(
        decoded.fallback.side_panels,
        SidePanelFields { left: false, right: true, status_flag: true, left_columns: 0 }
    );
    assert!(decoded.fallback.black_background_substitution);
    assert_eq!(decoded.default_objects[0], Some(DefaultObject { kind: ObjectKind::Active, number: 0x32 }));
    assert_eq!(decoded.default_objects[1], Some(DefaultObject { kind: ObjectKind::Adaptive, number: 0 }));
    assert_eq!(encode::mot_object_link(&decoded), link);
    assert_eq!(mot_page_entry(&data, 0), Some(MotPageEntry { global_objects: true, pop_link: 5 }));
    assert_eq!(mot_fallback(0b1111).side_panels, SidePanelFields { status_flag: true, ..SidePanelFields::default() });
    assert_eq!(mot_fallback(0b0110).side_panels.left_columns, 8);
    assert!(mot_object_link(&data, 4).is_none());
    let mut broken = packet;
    broken.bytes[DATA_START + 12] ^= 0b11;
    assert!(mot_object_link(&broken.data(), 1).is_none());
    assert!(mot_object_link(&broken.data(), 0).is_some());
    // Only the first byte of a pair is read: a damaged second byte
    // leaves the entry, a damaged first byte does not.
    let mut broken = packet;
    broken.bytes[DATA_START + 1] ^= 0b11;
    assert_eq!(mot_page_entry(&broken.data(), 0), Some(MotPageEntry { global_objects: true, pop_link: 5 }));
    broken.bytes[DATA_START] ^= 0b11;
    assert_eq!(mot_page_entry(&broken.data(), 0), None);
    let totals = MotTotals { magazines: 0b1000_0011, object_pages: 0x2A };
    let mut row = [0u8; DATA_BYTES];
    row[34..40].copy_from_slice(&encode::mot_totals(&totals));
    assert_eq!(row[38..40], [0, 0], "no DRCS pages counted");
    // A DRCS total from a service is not read.
    row[38] = 0x1;
    let data = encode::nibble_packet(mag(2), 21, &row).data();
    assert_eq!(mot_totals(&data), Some(totals));
}

#[test]
fn packets_30_outside_magazine_8_and_31_are_ignored() {
    assert!(matches!(decode(&encode::address(mag(1), 30)), Ok(Decoded::Ignored(_))));
    assert!(matches!(decode(&encode::address(mag(4), 31)), Ok(Decoded::Ignored(_))));
}

/// Clause 9.3.1, decision E.7: an uncorrectable sub-code byte leaves the
/// header without a sub-code but with its control bits, an uncorrectable
/// control byte the reverse; either makes the header incomplete.
#[test]
fn header_sub_code_and_control_bits_fail_separately() {
    let control = ControlBits(C8_UPDATE | C11_SERIAL);
    let good = encode::header(mag(2), PageNumber(0x34), SubCode::new(0x0005), control, &[0x20; 32]);
    let decoded = |packet: &Packet| {
        let Ok(Decoded::Header(header)) = decode(packet) else { panic!("not a header") };
        header
    };
    let mut no_subcode = good;
    // S1, Hamming 8/4 in header byte 8: two bit errors.
    no_subcode.bytes[4] ^= 0b11;
    let header = decoded(&no_subcode);
    assert_eq!((header.page, header.subcode, header.control), (Some(PageNumber(0x34)), None, Some(control)));
    assert!(!header.is_complete());
    let mut no_control = good;
    // C7 to C10, header byte 12.
    no_control.bytes[8] ^= 0b11;
    let header = decoded(&no_control);
    assert_eq!((header.subcode, header.control), (Some(SubCode::new(0x0005)), None));
    assert!(!header.is_complete());
    assert_eq!(header.serial(), None, "C11 is unknown too");
}

/// Table 21: the side panels of a fallback, byte N+4 bits 2 and 3, for
/// each of the four selections: none, left 16, right 16, and the 8 + 8
/// split of both; each encodes to the byte it was decoded from.
#[test]
fn mot_fallback_side_panels_round_trip() {
    use crate::consts::{MOT_FALLBACK_PANELS_SHIFT, SIDE_PANEL_HALF_COLUMNS};
    let panels = |left, right, left_columns| SidePanelFields { left, right, status_flag: true, left_columns };
    for (bits, expected) in [
        (0b00, panels(false, false, 0)),
        (0b01, panels(true, false, 0)),
        (0b10, panels(false, true, 0)),
        (0b11, panels(true, true, SIDE_PANEL_HALF_COLUMNS)),
    ] {
        let fallback = mot_fallback(bits << MOT_FALLBACK_PANELS_SHIFT);
        assert_eq!(fallback.side_panels, expected, "{bits:#04b}");
        let link = MotObjectLink {
            magazine: mag(1),
            x27_flag: false,
            page: PageNumber(0x23),
            subpage_count: 1,
            fallback,
            default_objects: [None; 2],
        };
        assert_eq!(encode::mot_object_link(&link)[4], bits << MOT_FALLBACK_PANELS_SHIFT, "{bits:#04b}");
    }
}

/// Table 28, REQ-PKT-010: source bits S1 S0 = 00 are illegal. An Object
/// Invocation written with them has address bit 5 clear, so it falls in
/// the column address group (0 to 39) and does not read back as an
/// invocation.
#[test]
fn an_illegal_object_source_leaves_the_row_address_group() {
    assert_eq!(ObjectSource::from_bits(0b00), ObjectSource::Illegal);
    let invocation = TripletFunction::ObjectInvocation {
        source: ObjectSource::Illegal,
        kind: ObjectKind::Active,
        number: ObjectNumber(0),
    };
    let triplet = invocation.to_triplet();
    assert!(!triplet.is_row_address(), "address {}", triplet.address);
    assert!(!matches!(triplet.function(), TripletFunction::ObjectInvocation { .. }));
}

/// REQ-FSM-010, REQ-PKT-010: the trace names each function as Tables 28
/// and 29 do.
#[test]
fn triplet_functions_are_named_as_tables_28_and_29() {
    let t = |address: u8, mode: u8, data: u8| Triplet { address, mode, data }.function().name();
    let named = [
        (t(40, 0b00000, 0), "Full Screen Colour"),
        (t(40, 0b00001, 0), "Full Row Colour"),
        (t(40, 0b00100, 0), "Set Active Position"),
        (t(63, 0b00111, 0), "Address Display Row 0"),
        (t(40, 0b01000, 0), "Programme Delivery Control row triplet"),
        (t(40, 0b10000, 0), "Origin Modifier"),
        (t(0b11_1000, 0b10001, 0), "Object Invocation"),
        (t(0b11_1000, 0b10101, 0), "Object Definition"),
        (t(63, 0b11111, 0), "Termination Marker"),
        (t(40, 0b00010, 0), "reserved row triplet"),
        (t(0, 0b00000, 0), "Foreground Colour"),
        (t(0, 0b00001, 0x21), "G1 character"),
        (t(0, 0b00010, 0x21), "G3 character"),
        (t(0, 0b01011, 0x21), "G3 character"),
        (t(0, 0b00011, 0), "Background Colour"),
        (t(0, 0b00110, 0), "Programme Delivery Control column triplet"),
        (t(0, 0b00111, 0), "Additional Flash Functions"),
        (t(0, 0b01000, 0), "Modified G0 and G2 Character Set Designation"),
        (t(0, 0b01001, 0x41), "G0 character"),
        (t(0, 0b01100, 0), "Display Attributes"),
        (t(0, 0b01101, 0x41), "column triplet not acted on"),
        (t(0, 0b01110, 0), "column triplet not acted on"),
        (t(0, 0b01111, 0x41), "G2 character"),
        (t(0, 0b10001, 0x41), "G0 character with diacritical mark"),
        (t(0, 0b00100, 0), "reserved column triplet"),
    ];
    for (name, expected) in named {
        assert_eq!(name, expected);
    }
}
