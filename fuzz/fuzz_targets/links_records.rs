//! The LINKS record (`neoteletext-common/INPUT.md` section 7.8, REQ-1494,
//! REQ-1661, REQ-TST-040): every address the reader gives lies in the record,
//! within the bound and printable, and links it accepts write back to the
//! same addresses.
#![no_main]

use libfuzzer_sys::fuzz_target;
use neoteletext_formats::page::{LINK_TARGET_BYTES, LinkDestination, LinkLine, LinkText, NumberLink, TextLink};
use neoteletext_web::records::links::{SubPageLinks, encode, read};
use neoteletext_web::records::{ChannelRecord, Read, RecordReader};

/// A service any record may be written on.
const SERVICE: u8 = 3;

fuzz_target!(|data: &[u8]| {
    let Ok(record) = read(data) else { return };
    let mut addresses = LinkText::new();
    let mut moved = |destination: LinkDestination, url| {
        let address = record.address(url);
        assert!(address.len() <= LINK_TARGET_BYTES);
        assert!(address.iter().all(u8::is_ascii_graphic));
        assert_eq!(address.is_empty(), url.is_empty());
        LinkDestination { url: if url.is_empty() { url } else { addresses.add(address).unwrap() }, ..destination }
    };
    let keys: Vec<LinkLine> = record
        .key_links()
        .map(|link| {
            let LinkLine { service, address, subcode, url, .. } = link;
            let to = moved(LinkDestination { service, address, subcode, url }, url);
            LinkLine { url: to.url, ..link }
        })
        .collect();
    let numbers: Vec<NumberLink> = record
        .number_links()
        .map(|link| NumberLink { destination: moved(link.destination, link.destination.url), ..link })
        .collect();
    let texts: Vec<TextLink> = record
        .text_links()
        .map(|link| TextLink { destination: moved(link.destination, link.destination.url), ..link })
        .collect();
    let links = SubPageLinks { keys: &keys, numbers: &numbers, texts: &texts, addresses: &addresses };
    let mut packets = Vec::new();
    encode(record.page, record.subcode, links, SERVICE, 1, &mut |packet| packets.push(packet)).unwrap();
    let mut reader = RecordReader::new();
    for packet in &packets {
        if let Read::Record(ChannelRecord::Links(back)) = reader.push(packet) {
            for link in back.key_links() {
                let found = keys.iter().find(|key| key.key == link.key).unwrap();
                assert_eq!(back.address(link.url), addresses.address(found.url));
            }
        }
    }
});
