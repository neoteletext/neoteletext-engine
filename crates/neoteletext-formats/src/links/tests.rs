//! Receiver link resolution tests, read from TTI pages.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]

use super::*;
use crate::{Item, Reader, tti::TtiReader};
use neoteletext_core::consts::{ALPHA_BLACK, ALPHA_RED, ALPHA_YELLOW, COLOUR_GREEN_BIT, SPACE};

fn read(tti: &[u8], check: impl FnOnce(&PageFile)) {
    let mut reader = TtiReader::new();
    let mut check = Some(check);
    let mut take = |item: Item<'_>| {
        if let (Item::Page(page), Some(check)) = (item, check.take()) {
            check(page);
        }
    };
    reader.feed(tti, &mut take);
    reader.finish(&mut take);
    assert!(check.is_none(), "no page was read");
}

const SERVICES: [&str; 2] = ["news", "weather"];

fn page(address: PageAddress) -> Destination<'static> {
    Destination::Page { address, subcode: None, service: None }
}

/// REQ-NAV-011 and Appendix E: each key's destination, and the row a
/// receiver draws, in the key's colour, with the mark for leaving.
#[test]
fn a_receiver_follows_every_kind_of_link() {
    let tti = b"PN,10000\r\nOL,1,TEXT\r\nLN,R,101:0002,Headlines\r\nLN,G,weather:400,Weather today\r\n\
LN,Y,https://other.example/200.tti,Elsewhere\r\nLN,C,web:https://example.org/,Web site\r\nLN,I,100,\r\n";
    read(tti, |file| {
        let links = receiver_links(file, &SERVICES, false).unwrap();
        let address = |mpp| PageAddress::from_mpp(mpp).unwrap();
        assert_eq!(
            links.keys,
            [
                Some(Destination::Page { address: address(0x101), subcode: Some(SubCode::new(2)), service: None }),
                Some(Destination::Page { address: address(0x400), subcode: None, service: Some(1) }),
                Some(Destination::Address(b"https://other.example/200.tti")),
                None,
                Some(page(address(0x100))),
            ]
        );
        let codes = links.row.codes;
        assert_eq!(&codes[..10], &[[ALPHA_RED].as_slice(), b"Headlines"].concat()[..]);
        assert_eq!(
            &codes[10..20],
            &[[ALPHA_BLACK | COLOUR_GREEN_BIT].as_slice(), b">Weather "].concat()[..],
            "8 characters"
        );
        assert_eq!(&codes[20..30], &[[ALPHA_YELLOW].as_slice(), b">Elsewher"].concat()[..]);
        assert_eq!(codes[30..], [SPACE; 10], "a null link's prompt is blank");
        assert_eq!(links.row.marks.as_slice(), [11, 21]);

        let with_web = receiver_links(file, &SERVICES, true).unwrap();
        assert_eq!(with_web.keys[3], Some(Destination::Web(b"web:https://example.org/")));
        assert_eq!(with_web.row.marks.as_slice(), [11, 21, 31]);
    });
}

/// Appendix E.2 rule 2: a service the descriptor does not name makes
/// the link null, and its prompt blank.
#[test]
fn an_unknown_service_is_a_null_link() {
    read(b"PN,10000\r\nLN,R,sport:300,Sport\r\nLN,G,news:200,News\r\n", |file| {
        let links = receiver_links(file, &SERVICES, false).unwrap();
        assert_eq!(links.keys[0], None);
        assert_eq!(links.row.codes[..10], [SPACE; 10]);
        assert!(matches!(links.keys[1], Some(Destination::Page { service: Some(0), .. })));
        assert!(receiver_links(file, &[] as &[&str], false).unwrap().keys.iter().all(Option::is_none));
    });
}

/// REQ-NAV-011: no attribute from the page in the receiver's row.
#[test]
fn a_labels_control_codes_are_spaces() {
    read(b"PN,10000\r\nLN,R,101,Fl\x08ash\r\n", |file| {
        assert_eq!(&receiver_links(file, &SERVICES, false).unwrap().row.codes[1..7], b"Fl ash");
    });
}

/// Appendix E.2 rule 1: invalid links give no receiver links; a page
/// without `LN` lines has none.
#[test]
fn invalid_or_absent_links_give_none() {
    read(b"PN,10000\r\nLN,R,101,A\r\nLN,R,102,B\r\n", |file| {
        assert_eq!(receiver_links(file, &SERVICES, false), None);
    });
    read(b"PN,10000\r\nOL,1,TEXT\r\n", |file| assert_eq!(receiver_links(file, &SERVICES, false), None));
}

/// Decision E.108 item 4: a port that reads packets takes this
/// service's page links from X/27/0, which the encoder writes from the
/// `LN` lines, links leaving the service null; the rest come from
/// [`receiver_links`].
#[test]
fn link_lines_reach_a_decoder_as_x27() {
    use neoteletext_core::acq::{Decoder, StoredPage};
    use neoteletext_core::config::Config;
    use neoteletext_core::consts::HEADER_DISPLAY_BYTES;
    use neoteletext_core::nav::PageLinks;
    use neoteletext_core::packet::{PacketMeta, encode};
    use neoteletext_core::types::{ControlBits, Instant, PageNumber};
    let tti = b"PN,10000\r\nOL,1,TEXT\r\nLN,R,101,One\r\nLN,G,weather:400,Weather\r\nLN,Y,150:0003,Three\r\n\
LN,I,100,\r\n";
    read(tti, |file| {
        let mut packets = std::vec::Vec::new();
        crate::encode::encode_page(file, &mut |packet| packets.push(packet));
        // Another page of the magazine ends this one.
        packets.push(encode::header(
            file.address.magazine,
            PageNumber(neoteletext_core::consts::PAGE_TIME_FILLING),
            SubCode::new(0),
            ControlBits(0),
            &[neoteletext_core::ecc::with_odd_parity(SPACE); HEADER_DISPLAY_BYTES],
        ));
        let mut slots = std::vec![StoredPage::EMPTY; 8];
        let mut decoder = Decoder::new(&mut slots, Config::default());
        for (index, packet) in packets.iter().enumerate() {
            decoder.push(packet, &PacketMeta::default(), Instant(u64::try_from(index).unwrap()), &mut |_| {});
        }
        let stored = decoder.store().get(file.address, SubCode::ANY).unwrap();
        let links = PageLinks::from_page(stored);
        let mpp = |link: Option<neoteletext_core::nav::LinkTarget>| link.map(|link| link.address.mpp());
        assert_eq!(links.keys.map(mpp), [Some(0x101), None, Some(0x150), None], "the other service's link is null");
        assert_eq!(links.keys[2].unwrap().subcode, SubCode::new(3));
        assert_eq!(mpp(links.index), Some(0x100));
    });
}

/// The same rules for `LK` and `LT` destinations.
#[test]
fn destinations_resolve_alike() {
    read(b"PN,10000\r\nLK,150,weather:400\r\nLT,2,0,4,web:https://example.org/\r\n", |file| {
        let number = &file.number_links.as_slice()[0].destination;
        assert!(matches!(resolve(number, &SERVICES, false), Some(Destination::Page { service: Some(1), .. })));
        let text = &file.text_links.as_slice()[0].destination;
        assert_eq!(resolve(text, &SERVICES, false), None, "a port that opens only teletext");
        assert!(resolve(text, &SERVICES, true).is_some_and(|destination| destination.leaves_service()));
    });
}
