#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]
//! The LINKS record at the bound of REQ-1661 and DEC-1668.

use neoteletext_core::consts::PACKET_BYTES;
use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};
use neoteletext_formats::page::{
    LINK_LINES, LinkDestination, LinkKey, LinkLine, LinkText, NUMBER_LINKS, NumberLink, TEXT_LINKS, TextLink,
};
use neoteletext_formats::text::Text;
use neoteletext_formats::tti::link_destination;

use super::{MAX_LABEL, MAX_OTHER_LABEL, MAX_PARTS, SubPageLinks, encode};
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
/// REQ-1494: a key link's target goes through a record, whatever its length.
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
/// REQ-1494: every kind of link at its most, read back in order.
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

/// A target in a page of this channel.
fn same_channel(addresses: &mut LinkText) -> LinkDestination {
    link_destination(b"150", addresses).unwrap()
}

/// `count` number links, each with a page number of its own.
fn number_links(count: u16, addresses: &mut LinkText) -> Vec<NumberLink> {
    (0..count)
        .map(|index| NumberLink {
            number: PageAddress::from_mpp(0x150 + index).unwrap(),
            destination: same_channel(addresses),
            line: 0,
        })
        .collect()
}

fn text_link(row: u8, column: u8, length: u8, addresses: &mut LinkText) -> TextLink {
    TextLink { row, column, length, destination: same_channel(addresses), line: 0 }
}

/// A key link with a label of `length` characters.
fn labelled(colour: LinkKey, destination: LinkDestination, length: usize) -> LinkLine {
    let (label, _) = Text::from_bytes(&std::vec![b'a'; length]);
    LinkLine { label, ..key(colour, destination) }
}

/// REQ-1494: at most five key links, with four coloured keys and the Index
/// key, each key once.
#[test]
fn a_record_holds_five_key_links_each_key_once() {
    let mut addresses = LinkText::new();
    let colours = [LinkKey::Red, LinkKey::Green, LinkKey::Yellow, LinkKey::Cyan, LinkKey::Index];
    assert_eq!(colours.len(), LINK_LINES);
    let keys = colours.map(|colour| key(colour, same_channel(&mut addresses)));
    let links = |keys: &[LinkLine], addresses: &LinkText| {
        write(SubPageLinks { keys, numbers: &[], texts: &[], addresses }).map(|packets| read_back(&packets).len())
    };
    assert_eq!(links(&keys, &addresses), Ok(LINK_LINES));
    let sixth = [keys.as_slice(), &keys[..1]].concat();
    assert!(links(&sixth, &addresses).is_err(), "a sixth key link");
    let twice = [keys[0], keys[0]];
    assert!(links(&twice, &addresses).is_err(), "a key given twice");
}

/// REQ-1494: at most eight page-number links, and at most eight text links.
#[test]
fn a_record_holds_eight_number_links_and_eight_text_links() {
    let mut addresses = LinkText::new();
    let most = u16::try_from(NUMBER_LINKS).unwrap();
    let numbers = number_links(most, &mut addresses);
    let one_more = number_links(most + 1, &mut addresses);
    let write_numbers = |numbers: &[NumberLink], addresses: &LinkText| {
        write(SubPageLinks { keys: &[], numbers, texts: &[], addresses }).map(|packets| read_back(&packets).len())
    };
    assert_eq!(write_numbers(&numbers, &addresses), Ok(NUMBER_LINKS));
    assert!(write_numbers(&one_more, &addresses).is_err());
    let texts: Vec<TextLink> = (0..=TEXT_LINKS).map(|row| text_link(row as u8 + 1, 0, 1, &mut addresses)).collect();
    let write_texts = |texts: &[TextLink], addresses: &LinkText| {
        write(SubPageLinks { keys: &[], numbers: &[], texts, addresses }).map(|packets| read_back(&packets).len())
    };
    assert_eq!(write_texts(&texts[..TEXT_LINKS], &addresses), Ok(TEXT_LINKS));
    assert!(write_texts(&texts, &addresses).is_err());
}

/// REQ-1494: a label is at most 9 characters for a link in the same service
/// and 8 for one elsewhere, and the Index key has none.
#[test]
fn a_label_keeps_its_bound() {
    let mut addresses = LinkText::new();
    let here = same_channel(&mut addresses);
    let elsewhere = link_destination(b"https://a.example/", &mut addresses).unwrap();
    let write_key = |line: LinkLine, addresses: &LinkText| {
        write(SubPageLinks { keys: &[line], numbers: &[], texts: &[], addresses }).map(|_| ())
    };
    assert_eq!(write_key(labelled(LinkKey::Red, here, MAX_LABEL), &addresses), Ok(()));
    assert!(write_key(labelled(LinkKey::Red, here, MAX_LABEL + 1), &addresses).is_err());
    assert_eq!(write_key(labelled(LinkKey::Red, elsewhere, MAX_OTHER_LABEL), &addresses), Ok(()));
    assert!(write_key(labelled(LinkKey::Red, elsewhere, MAX_OTHER_LABEL + 1), &addresses).is_err());
    let write_index = |length: usize, addresses: &LinkText| {
        let keys = [labelled(LinkKey::Index, here, length)];
        write(SubPageLinks { keys: &keys, numbers: &[], texts: &[], addresses }).unwrap()
    };
    assert_eq!(write_index(1, &addresses), write_index(0, &addresses), "the Index key's label is not written");
}

/// REQ-1494: a text link stands on rows 1 to 23, and its run is inside the
/// 40 columns.
#[test]
fn a_text_link_stays_on_the_page() {
    let mut addresses = LinkText::new();
    let place = |row: u8, column: u8, length: u8, addresses: &mut LinkText| {
        let texts = [text_link(row, column, length, addresses)];
        write(SubPageLinks { keys: &[], numbers: &[], texts: &texts, addresses }).is_ok()
    };
    for (row, column, length, fits) in [
        (1, 0, 1, true),
        (23, 39, 1, true),
        (1, 0, 40, true),
        (0, 0, 1, false),
        (24, 0, 1, false),
        (1, 0, 0, false),
        (1, 39, 2, false),
    ] {
        assert_eq!(place(row, column, length, &mut addresses), fits, "row {row} column {column} length {length}");
    }
}

/// A short link costs what it did: one record, not the bound's.
#[test]
fn short_links_stay_in_one_record() {
    let mut addresses = LinkText::new();
    let keys = [key(LinkKey::Red, link_destination(b"https://a.example/", &mut addresses).unwrap())];
    let packets = write(SubPageLinks { keys: &keys, numbers: &[], texts: &[], addresses: &addresses }).unwrap();
    assert_eq!(packets.len(), 2);
}
