#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The LINKS record at the bound of REQ-1661 and DEC-1668.

use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};
use neoteletext_formats::page::{LinkDestination, LinkKey, LinkLine, LinkText, NumberLink, TextLink};
use neoteletext_formats::text::Text;
use neoteletext_formats::tti::link_destination;

use super::{MAX_PARTS, SubPageLinks, encode};
use crate::records::{ChannelRecord, Read, RecordFault, RecordReader};
use std::string::String;
use std::vec::Vec;

const SERVICE: u8 = 2;

fn page() -> PageAddress {
    PageAddress::from_mpp(0x100).unwrap()
}

/// An address of `length` octets, told apart from the others by `mark`.
fn address(length: usize, mark: char) -> String {
    let mut text = String::from("https://a.example/");
    text.push(mark);
    text.extend(core::iter::repeat_n('a', length - text.len()));
    text
}

fn key(key: LinkKey, destination: LinkDestination) -> LinkLine {
    let LinkDestination { service, address, subcode, url } = destination;
    LinkLine { key, service, address, subcode, url, label: Text::new(), label_truncated: false, line: 0 }
}

fn write(links: SubPageLinks<'_>) -> Result<Vec<Packet>, RecordFault> {
    let mut packets = Vec::new();
    encode(page(), SubCode::new(0), links, SERVICE, 0, &mut |packet| packets.push(packet))?;
    Ok(packets)
}

/// Every link a reader gives from `packets`, with its address.
fn read_back(packets: &[Packet]) -> Vec<(Option<LinkKey>, Vec<u8>)> {
    let mut reader = RecordReader::new();
    let mut found = Vec::new();
    for packet in packets {
        match reader.push(packet) {
            Read::Record(ChannelRecord::Links(record)) => {
                for link in record.key_links() {
                    found.push((Some(link.key), record.address(link.url).to_vec()));
                }
                for link in record.number_links() {
                    found.push((None, record.address(link.destination.url).to_vec()));
                }
                for link in record.text_links() {
                    found.push((None, record.address(link.destination.url).to_vec()));
                }
            }
            Read::Invalid(count) => panic!("{count} packets refused"),
            _ => {}
        }
    }
    found
}

/// INPUT.md section 7.8: a target of 8,000 octets goes through a record.
#[test]
fn a_target_of_8000_octets_travels() {
    let mut addresses = LinkText::new();
    let long = address(8000, 'x');
    let keys = [key(LinkKey::Red, link_destination(long.as_bytes(), &mut addresses).unwrap())];
    let packets = write(SubPageLinks { keys: &keys, numbers: &[], texts: &[], addresses: &addresses }).unwrap();
    assert_eq!(read_back(&packets), [(Some(LinkKey::Red), long.into_bytes())]);
}

/// REQ-1661: 8,001 octets is no target, in a TTI link or a record's.
#[test]
fn a_target_past_the_bound_is_no_link() {
    let mut addresses = LinkText::new();
    let web = std::format!("web:{}", address(8000, 'x'));
    assert!(link_destination(web.as_bytes(), &mut addresses).is_some());
    let over = std::format!("web:{}", address(8001, 'x'));
    assert!(link_destination(over.as_bytes(), &mut addresses).is_none());
}

/// REQ-1661: a sub-page with all 21 links at the bound passes through the
/// records, one record a link, and a reader gets each whole and in order.
#[test]
fn every_link_at_the_bound_travels() {
    let mut addresses = LinkText::new();
    let targets: Vec<String> = ('a'..='u').map(|mark| address(8000, mark)).collect();
    let mut destinations = targets.iter().map(|target| link_destination(target.as_bytes(), &mut addresses).unwrap());
    let colours = [LinkKey::Red, LinkKey::Green, LinkKey::Yellow, LinkKey::Cyan, LinkKey::Index];
    let keys = colours.map(|colour| key(colour, destinations.next().unwrap()));
    let numbers: Vec<NumberLink> = (0..8)
        .map(|index| NumberLink {
            number: PageAddress::from_mpp(0x150 + index).unwrap(),
            destination: destinations.next().unwrap(),
            line: 0,
        })
        .collect();
    let texts: Vec<TextLink> = (0..8)
        .map(|index| TextLink {
            row: index + 2,
            column: 0,
            length: 4,
            destination: destinations.next().unwrap(),
            line: 0,
        })
        .collect();
    let packets = write(SubPageLinks { keys: &keys, numbers: &numbers, texts: &texts, addresses: &addresses }).unwrap();
    let found = read_back(&packets);
    assert_eq!(found.len(), MAX_PARTS);
    for (link, target) in found.iter().zip(&targets) {
        assert_eq!(link.1, target.as_bytes());
    }
    // The bandwidth DEC-1668 asks for: 34 bytes of a record in each packet
    // of 42, every time the sub-page is sent.
    assert_eq!(packets.len() * PACKET_BYTES, 208_152);
}

/// A short link costs what it did: one record, not the bound's.
#[test]
fn short_links_stay_in_one_record() {
    let mut addresses = LinkText::new();
    let keys = [key(LinkKey::Red, link_destination(b"https://a.example/", &mut addresses).unwrap())];
    let packets = write(SubPageLinks { keys: &keys, numbers: &[], texts: &[], addresses: &addresses }).unwrap();
    assert_eq!(packets.len(), 2);
}
