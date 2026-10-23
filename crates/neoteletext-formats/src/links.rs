//! A page's links as a receiver follows them (Appendix E of the TTI Link
//! Extension, `docs/spec/requirements/tti-link-extension.md`): where each
//! coloured key, Index, page-number link and text link leads, with the
//! services the descriptor names resolved, and the row 24 the receiver
//! draws from the `LN` lines (REQ-NAV-011, decision E.108). One implementation for every
//! port: `neottx` through `neoteletext-app`, and NeoTeletext's decoder.
//!
//! Packet X/27/0, which the page encoder writes from the same lines
//! (Appendix E.2 rule 4), carries only the links to this service's pages;
//! the links to other services and to addresses are here.

use neoteletext_core::types::{PageAddress, SubCode};

use crate::encode::{LinkRow, Prompt, draw_link_row, key_colour};
use crate::page::{LinkDestination, LinkKey, PageFile, WEB_MARK};

/// The coloured keys and Index: red, green, yellow, cyan and Index, in
/// that order in [`ReceiverLinks::keys`].
pub const KEYS_WITH_INDEX: usize = 5;
/// Index's place in [`ReceiverLinks::keys`].
pub const INDEX_KEY: usize = 4;

/// Where a link leads, with its service resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Destination<'a> {
    /// A page of this service (`service` `None`), or of the service at that
    /// index among the descriptor's linked services.
    Page {
        /// The page.
        address: PageAddress,
        /// The sub-page, `None` for any.
        subcode: Option<SubCode>,
        /// The linked service's index, `None` for this service.
        service: Option<u8>,
    },
    /// Another site's page file, by its address as the page wrote it.
    Address(&'a [u8]),
    /// A web page, to open outside teletext (decision E.88), its `web:`
    /// mark kept.
    Web(&'a [u8]),
}

impl Destination<'_> {
    /// True when following it leaves this service: the receiver's "»"
    /// mark goes before its label (REQ-NAV-011).
    #[must_use]
    pub const fn leaves_service(&self) -> bool {
        !matches!(self, Self::Page { service: None, .. })
    }
}

/// Where `target` leads, or `None` when the link is null: it names a
/// service that `services`, the descriptor's linked services in order,
/// does not hold (Appendix E.2 rule 2), or it is a web page and `web` is
/// false, for a port that opens only teletext (decision E.88).
#[must_use]
pub fn resolve<'a, S: AsRef<str>>(target: &'a LinkDestination, services: &[S], web: bool) -> Option<Destination<'a>> {
    resolve_parts(target.service.as_bytes(), target.address, target.subcode, target.url.as_bytes(), services, web)
}

/// [`resolve`] of a destination's parts, as an `LN`, `LK` or `LT` line
/// holds them.
fn resolve_parts<'a, S: AsRef<str>>(
    service: &[u8],
    address: PageAddress,
    subcode: Option<SubCode>,
    url: &'a [u8],
    services: &[S],
    web: bool,
) -> Option<Destination<'a>> {
    if url.starts_with(WEB_MARK) {
        return web.then_some(Destination::Web(url));
    }
    if !url.is_empty() {
        return Some(Destination::Address(url));
    }
    let service = if service.is_empty() {
        None
    } else {
        let index = services.iter().position(|known| known.as_ref().as_bytes() == service)?;
        Some(u8::try_from(index).ok()?)
    };
    Some(Destination::Page { address, subcode, service })
}

/// The place of a key in [`ReceiverLinks::keys`].
const fn key_place(key: LinkKey) -> usize {
    match key {
        LinkKey::Red => 0,
        LinkKey::Green => 1,
        LinkKey::Yellow => 2,
        LinkKey::Cyan => 3,
        LinkKey::Index => INDEX_KEY,
    }
}

/// The links of a page's `LN` lines as a receiver follows them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReceiverLinks<'a> {
    /// Where each key leads: red, green, yellow, cyan, then Index; `None`
    /// for a key with no link or a null one.
    pub keys: [Option<Destination<'a>>; KEYS_WITH_INDEX],
    /// Row 24 as the receiver draws it (REQ-NAV-011): each coloured key's
    /// prompt in its key's colour, a null link's prompt blank, and the "»"
    /// mark before the label of a link that leaves the service, at the
    /// columns `marks` lists, written as `>` among the codes for a
    /// renderer that draws no mark of its own.
    pub row: LinkRow,
}

/// The receiver's links and row 24 of a page with valid `LN` lines, or
/// `None` when it has none or they are invalid (Appendix E.2 rule 1).
/// `services` and `web` are as [`resolve`] takes them.
#[must_use]
pub fn receiver_links<'a, S: AsRef<str>>(page: &'a PageFile, services: &[S], web: bool) -> Option<ReceiverLinks<'a>> {
    if !page.uses_link_lines() || !page.links_valid {
        return None;
    }
    let mut keys = [None; KEYS_WITH_INDEX];
    for line in page.links.as_slice() {
        if let Some(slot) = keys.get_mut(key_place(line.key)) {
            *slot =
                resolve_parts(line.service.as_bytes(), line.address, line.subcode, line.url.as_bytes(), services, web);
        }
    }
    let row = draw_link_row(page, |line| {
        let destination = keys.get(key_place(line.key)).copied().flatten()?;
        Some(Prompt { colour: key_colour(line.key), leaves: destination.leaves_service() })
    });
    Some(ReceiverLinks { keys, row })
}

#[cfg(all(test, feature = "tti"))]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "a test")]
mod tests {
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
}
