#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The SUBPAGES record at its bounds, through packets and a reader.

use super::{MAX_TITLE_CHARACTERS, SubPageEntry, encode, read};
use crate::records::{ChannelRecord, Read, RecordFault, RecordReader, TYPE_SUBPAGES, write_packets};
use neoteletext_core::limits::{LAST_SUBPAGE_CODE, MAX_SUBPAGES, SUBCODE_ANY, SUBCODE_MASK};
use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};
use std::string::{String, ToString};
use std::vec::Vec;

const SERVICE: u8 = 2;
const VERSION: u16 = 9;
const PAGE: u16 = 0x100;

fn page() -> PageAddress {
    PageAddress::from_mpp(PAGE).unwrap()
}

/// The first `count` sub-codes in ascending order.
fn codes(count: usize) -> Vec<u16> {
    (0..=LAST_SUBPAGE_CODE).filter(|code| code & !SUBCODE_MASK == 0).take(count).collect()
}

fn entries(codes: &[u16]) -> Vec<SubPageEntry<'static>> {
    codes.iter().map(|&code| SubPageEntry { subcode: SubCode::new(code), title: "" }).collect()
}

fn write(entries: &[SubPageEntry<'_>]) -> Result<Vec<Packet>, RecordFault> {
    let mut packets = Vec::new();
    encode(page(), VERSION, entries, SERVICE, 0, &mut |packet| packets.push(packet))?;
    Ok(packets)
}

/// A record as read: its part, its parts, its count and its entries.
type ReadRecord = (u16, u16, u16, Vec<(u16, String)>);

/// Each record a reader takes from `packets`.
fn read_back(packets: &[Packet]) -> Vec<ReadRecord> {
    let mut reader = RecordReader::new();
    let mut found = Vec::new();
    for packet in packets {
        match reader.push(packet) {
            Read::Record(ChannelRecord::SubPages(record)) => {
                assert_eq!((record.page, record.version), (page(), VERSION));
                let list = record.entries().map(|entry| (entry.subcode.value(), entry.title.to_string())).collect();
                found.push((record.part, record.parts, record.count, list));
            }
            Read::Invalid(count) => panic!("{count} packets refused"),
            _ => {}
        }
    }
    found
}

/// A SUBPAGES body with the fixed fields of one record of one sub-page list.
fn body(count: u16, codes: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for field in [PAGE, VERSION, 0, 1, count] {
        bytes.extend_from_slice(&field.to_be_bytes());
    }
    for code in codes {
        bytes.extend_from_slice(&code.to_be_bytes());
        bytes.push(0);
    }
    bytes
}

/// REQ-1503: a list of one sub-page and one of 8,190, split into records,
/// reads back whole, each part numbered and every sub-code in its order.
#[test]
fn a_list_of_one_and_of_8190_sub_pages_reads_back_whole() {
    for count in [1, MAX_SUBPAGES] {
        let codes = codes(count);
        let found = read_back(&write(&entries(&codes)).unwrap());
        let parts = found.len();
        assert_eq!(parts > 1, count > 1, "a long list is split, a short one is not");
        let mut listed = Vec::new();
        for (index, (part, of, total, list)) in found.into_iter().enumerate() {
            assert_eq!((usize::from(part), usize::from(of), usize::from(total)), (index, parts, count));
            listed.extend(list.into_iter().map(|(code, _)| code));
        }
        assert_eq!(listed, codes);
    }
}

/// REQ-1503: every sub-code is a peer, so a list may hold 0000 beside the
/// others, and it stays first.
#[test]
fn a_list_may_hold_0000_beside_other_sub_codes() {
    let found = read_back(&write(&entries(&[0x0000, 0x0001, 0x0002])).unwrap());
    let list: Vec<u16> = found[0].3.iter().map(|(code, _)| *code).collect();
    assert_eq!(list, [0x0000, 0x0001, 0x0002]);
}

/// REQ-1503: a title of 40 characters travels, and one of 41 does not.
#[test]
fn a_title_is_at_most_a_row() {
    let longest: String = core::iter::repeat_n('x', MAX_TITLE_CHARACTERS).collect();
    let entry = SubPageEntry { subcode: SubCode::new(1), title: &longest };
    assert_eq!(read_back(&write(&[entry]).unwrap())[0].3, [(1, longest.clone())]);
    let over = longest + "x";
    let entry = SubPageEntry { subcode: SubCode::new(1), title: &over };
    assert!(write(&[entry]).is_err());
}

/// REQ-1503: a list out of ascending order or with a sub-code repeated is
/// not written, and a reader refuses one.
#[test]
fn a_list_out_of_order_or_repeated_is_refused() {
    for bad in [[0x0002, 0x0001], [0x0001, 0x0001]] {
        assert_eq!(write(&entries(&bad)), Err(RecordFault::BadValue("sub-codes not ascending")), "{bad:04X?}");
        assert_eq!(read(&body(2, &bad)).err(), Some(RecordFault::BadValue("sub-codes not ascending")), "{bad:04X?}");
    }
    assert!(read(&body(2, &[0x0001, 0x0002])).is_ok());
}

/// REQ-1503: each bound of the record, and one past it.
#[test]
fn a_record_past_a_bound_is_refused() {
    let most = u16::try_from(MAX_SUBPAGES).unwrap();
    assert!(read(&body(most, &[0x0001])).is_ok());
    assert!(read(&body(most + 1, &[0x0001])).is_err(), "more sub-pages than a page may have");
    assert!(read(&body(0, &[0x0001])).is_err(), "no sub-pages");
    assert!(read(&body(1, &[])).is_err(), "no entries");
    assert!(read(&body(1, &[0x0001, 0x0002])).is_err(), "more entries than the count");
    assert!(read(&body(1, &[LAST_SUBPAGE_CODE])).is_ok());
    assert!(read(&body(1, &[SUBCODE_ANY])).is_err(), "3F7F names no sub-page");
    assert!(write(&[]).is_err(), "an empty list");
    assert!(write(&entries(&[0x0001; MAX_SUBPAGES + 1])).is_err(), "more sub-pages than a page may have");
}

/// REQ-1503, REQ-1462: a reader discards a record that breaks its layout and
/// counts its packets as invalid.
#[test]
fn a_malformed_record_is_discarded_and_counted() {
    let mut packets = Vec::new();
    write_packets(TYPE_SUBPAGES, &body(2, &[0x0002, 0x0001]), SERVICE, 0, &mut |packet| packets.push(packet)).unwrap();
    let mut reader = RecordReader::new();
    let invalid: usize = packets
        .iter()
        .map(|packet| match reader.push(packet) {
            Read::Invalid(count) => count,
            Read::Record(_) => panic!("a malformed record was taken"),
            Read::Nothing => 0,
        })
        .sum();
    assert_eq!(invalid, packets.len());
}
