//! The LINKS record (`neoteletext-common/INPUT.md` section 7.8, DEC-1495,
//! REQ-1494): a sub-page's TTI Link Extension links (Appendix E), which
//! packets cannot carry, sent straight after the sub-page's page frame.
//!
//! ```text
//! bytes 0-1  page: 0x100 to 0x8FF
//! bytes 2-3  sub: the sub-code, 0x0000 to 0x3F7E
//! byte 4     part: 0 to parts - 1
//! byte 5     parts: 1 to 21
//! byte 6     keys: 0 to 5, then each: key letter, target, label
//! then       numbers: 0 to 8, then each: page u16, target
//! then       texts: 0 to 8, then each: row, column, length, target
//! ```
//!
//! A target is a `u16` length and 1 to 256 bytes of printable ASCII, read
//! by the TTI reader's own parser; a label a `u8` length and 0 to 9 bytes.
//! The reader gives the links as the TTI reader gives `LN`, `LK` and `LT`
//! lines ([`LinkLine`], [`NumberLink`], [`TextLink`]).

use neoteletext_audio::MAX_BODY;
use neoteletext_core::consts::COLUMNS;
use neoteletext_core::limits::LAST_SUBPAGE_CODE;
use neoteletext_core::packet::Packet;
use neoteletext_core::types::{PageAddress, SubCode};
use neoteletext_formats::page::{
    LINK_ADDRESS_BYTES, LINK_LINES, LinkDestination, LinkKey, LinkLine, NUMBER_LINKS, NumberLink, PageFile, TEXT_LINKS,
    TextLink,
};
use neoteletext_formats::text::Text;
use neoteletext_formats::tti::link_destination;

use super::{RecordFault, TYPE_LINKS, take_u16, write_packets};
use crate::bytes::{Overflow, Writer};

/// The longest label of a link in the same service (Appendix E.1).
pub const MAX_LABEL: usize = 9;
/// The longest label of a link to another service or an address.
pub const MAX_OTHER_LABEL: usize = 8;
/// The most coloured key links (Appendix E.2 rule 1).
const MAX_COLOURED: usize = 4;
/// The most records one sub-page's links take: one a link at the most.
pub const MAX_PARTS: usize = LINK_LINES + NUMBER_LINKS + TEXT_LINKS;
/// The last row a text link may be on: rows 1 to 23 are the page's text.
const TEXT_LINK_LAST_ROW: u8 = 23;
/// The fixed fields: page, sub-code, part and parts, and the three counts.
const FIXED: usize = 2 + 2 + 1 + 1 + 3;
/// A sub-code's hexadecimal digits in a target (`mpp:ssss`).
const SUBCODE_DIGITS: u32 = 4;
/// What separates a target's parts.
const TARGET_SEPARATOR: u8 = b':';

/// One LINKS record, checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinksRecord<'a> {
    /// The sub-page's page.
    pub page: PageAddress,
    /// Its sub-code.
    pub subcode: SubCode,
    /// This record's place in the sending: part 0 replaces the links held
    /// for the sub-page, and later parts add to them.
    pub part: u8,
    /// The records of the sending.
    pub parts: u8,
    /// The links' bytes, checked.
    links: &'a [u8],
}

/// A link of any of the three kinds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    /// An `LN` line's link.
    Key(LinkLine),
    /// An `LK` line's link.
    Number(NumberLink),
    /// An `LT` line's link.
    Text(TextLink),
}

impl LinksRecord<'_> {
    /// Every link of the record, in order: key links, then page-number
    /// links, then text links.
    #[must_use]
    pub fn links(&self) -> Links<'_> {
        Links { bytes: self.links, kind: 0, left: 0 }
    }

    /// The record's key links, as `LN` lines give them.
    pub fn key_links(&self) -> impl Iterator<Item = LinkLine> + '_ {
        self.links().filter_map(|link| if let Link::Key(link) = link { Some(link) } else { None })
    }

    /// The record's page-number links, as `LK` lines give them.
    pub fn number_links(&self) -> impl Iterator<Item = NumberLink> + '_ {
        self.links().filter_map(|link| if let Link::Number(link) = link { Some(link) } else { None })
    }

    /// The record's text links, as `LT` lines give them.
    pub fn text_links(&self) -> impl Iterator<Item = TextLink> + '_ {
        self.links().filter_map(|link| if let Link::Text(link) = link { Some(link) } else { None })
    }
}

/// The links of a record, in order.
#[derive(Clone, Debug)]
pub struct Links<'a> {
    bytes: &'a [u8],
    /// 0 before the key links' count is read, then 1, 2 and 3 for key,
    /// page-number and text links.
    kind: u8,
    left: u8,
}

impl Iterator for Links<'_> {
    type Item = Link;

    fn next(&mut self) -> Option<Link> {
        // The record was checked whole, so a link that does not read ends
        // nothing but this walk.
        while self.left == 0 {
            if self.kind >= 3 {
                return None;
            }
            let (&count, rest) = self.bytes.split_first()?;
            self.bytes = rest;
            self.left = count;
            self.kind = self.kind.saturating_add(1);
        }
        self.left = self.left.saturating_sub(1);
        let (link, rest) = read_link(self.kind, self.bytes).ok()?;
        self.bytes = rest;
        Some(link)
    }
}

fn take_target(bytes: &[u8]) -> Result<(LinkDestination, &[u8]), RecordFault> {
    let (length, rest) = take_u16(bytes)?;
    let (text, rest) = rest.split_at_checked(usize::from(length)).ok_or(RecordFault::Length)?;
    let printable = text.iter().all(|&byte| byte.is_ascii_graphic() && byte != b',');
    if text.is_empty() || text.len() > LINK_ADDRESS_BYTES || !printable {
        return Err(RecordFault::BadValue("link target"));
    }
    let destination = link_destination(text).ok_or(RecordFault::BadValue("link target"))?;
    Ok((destination, rest))
}

fn read_link(kind: u8, bytes: &[u8]) -> Result<(Link, &[u8]), RecordFault> {
    match kind {
        1 => {
            let (&letter, rest) = bytes.split_first().ok_or(RecordFault::Length)?;
            let key = if letter.is_ascii_uppercase() { LinkKey::from_letter(letter) } else { None }
                .ok_or(RecordFault::BadValue("link key"))?;
            let (destination, rest) = take_target(rest)?;
            let (&length, rest) = rest.split_first().ok_or(RecordFault::Length)?;
            let (label, rest) = rest.split_at_checked(usize::from(length)).ok_or(RecordFault::Length)?;
            let other = !destination.service.is_empty() || !destination.url.is_empty();
            let longest = match (key, other) {
                (LinkKey::Index, _) => 0,
                (_, true) => MAX_OTHER_LABEL,
                (_, false) => MAX_LABEL,
            };
            if label.len() > longest || !label.iter().all(|&byte| matches!(byte, b' '..=b'~')) {
                return Err(RecordFault::BadValue("link label"));
            }
            let LinkDestination { service, address, subcode, url } = destination;
            let (label, _) = Text::from_bytes(label);
            Ok((
                Link::Key(LinkLine { key, service, address, subcode, url, label, label_truncated: false, line: 0 }),
                rest,
            ))
        }
        2 => {
            let (number, rest) = take_u16(bytes)?;
            let number = PageAddress::from_mpp(number).ok_or(RecordFault::BadValue("link number"))?;
            let (destination, rest) = take_target(rest)?;
            // Typing a page number never leaves teletext (decision E.88).
            if destination.is_web() {
                return Err(RecordFault::BadValue("link number target"));
            }
            Ok((Link::Number(NumberLink { number, destination, line: 0 }), rest))
        }
        _ => {
            let (place, rest) = bytes.split_at_checked(3).ok_or(RecordFault::Length)?;
            let [row, column, length] = <[u8; 3]>::try_from(place).map_err(|_| RecordFault::Length)?;
            let columns = u8::try_from(COLUMNS).unwrap_or(u8::MAX);
            let fits = (1..=TEXT_LINK_LAST_ROW).contains(&row)
                && length > 0
                && column.checked_add(length).is_some_and(|end| end <= columns);
            if !fits {
                return Err(RecordFault::BadValue("text link place"));
            }
            let (destination, rest) = take_target(rest)?;
            Ok((Link::Text(TextLink { row, column, length, destination, line: 0 }), rest))
        }
    }
}

/// Reads a LINKS record's body, checking every field and Appendix E's
/// rules within it.
///
/// # Errors
/// Any field outside its range: a page or sub-code out of range, a part
/// not below its parts, too many links of a kind, a key given twice or a
/// fifth coloured key, a page number given twice, a target or label that
/// breaks its rule, or bytes after the last link.
pub fn read(body: &[u8]) -> Result<LinksRecord<'_>, RecordFault> {
    let (page, rest) = take_u16(body)?;
    let page = PageAddress::from_mpp(page).ok_or(RecordFault::BadValue("links page"))?;
    let (subcode, rest) = take_u16(rest)?;
    if subcode > LAST_SUBPAGE_CODE || subcode & !neoteletext_core::limits::SUBCODE_MASK != 0 {
        return Err(RecordFault::BadValue("links sub-code"));
    }
    let (&part, rest) = rest.split_first().ok_or(RecordFault::Length)?;
    let (&parts, links) = rest.split_first().ok_or(RecordFault::Length)?;
    if parts == 0 || part >= parts || usize::from(parts) > MAX_PARTS {
        return Err(RecordFault::BadValue("links part"));
    }
    let mut walk = links;
    let mut keys: [Option<LinkKey>; LINK_LINES] = [None; LINK_LINES];
    let mut numbers: [Option<PageAddress>; NUMBER_LINKS] = [None; NUMBER_LINKS];
    for (kind, most) in [(1_u8, LINK_LINES), (2, NUMBER_LINKS), (3, TEXT_LINKS)] {
        let (&count, rest) = walk.split_first().ok_or(RecordFault::Length)?;
        walk = rest;
        if usize::from(count) > most {
            return Err(RecordFault::BadValue("links count"));
        }
        for index in 0..usize::from(count) {
            let (link, rest) = read_link(kind, walk)?;
            walk = rest;
            match link {
                Link::Key(link) => {
                    if keys.contains(&Some(link.key)) {
                        return Err(RecordFault::BadValue("link key repeated"));
                    }
                    if let Some(slot) = keys.get_mut(index) {
                        *slot = Some(link.key);
                    }
                }
                Link::Number(link) => {
                    if numbers.contains(&Some(link.number)) {
                        return Err(RecordFault::BadValue("link number repeated"));
                    }
                    if let Some(slot) = numbers.get_mut(index) {
                        *slot = Some(link.number);
                    }
                }
                Link::Text(_) => {}
            }
        }
    }
    if keys.iter().flatten().filter(|key| key.is_coloured()).count() > MAX_COLOURED {
        return Err(RecordFault::BadValue("links coloured keys"));
    }
    if !walk.is_empty() {
        return Err(RecordFault::Length);
    }
    Ok(LinksRecord { page, subcode: SubCode::new(subcode), part, parts, links })
}

/// Writes a link's target in the form Appendix E.1 gives it.
fn write_target(destination: &LinkDestination, writer: &mut Writer<'_>) -> Result<(), Overflow> {
    let mut text = [0_u8; LINK_ADDRESS_BYTES];
    let mut inner = Writer::new(&mut text);
    if destination.url.is_empty() {
        if !destination.service.is_empty() {
            inner.put(destination.service.as_bytes())?;
            inner.byte(TARGET_SEPARATOR)?;
        }
        inner.put(&destination.address.ascii())?;
        if let Some(subcode) = destination.subcode {
            inner.byte(TARGET_SEPARATOR)?;
            inner.hex(u32::from(subcode.value()), SUBCODE_DIGITS)?;
        }
    } else {
        inner.put(destination.url.as_bytes())?;
    }
    let length = inner.len();
    writer.put(&u16::try_from(length).map_err(|_| Overflow)?.to_be_bytes())?;
    writer.put(text.get(..length).ok_or(Overflow)?)
}

fn write_link(link: &Link, writer: &mut Writer<'_>) -> Result<(), Overflow> {
    match link {
        Link::Key(link) => {
            writer.byte(link.key.letter())?;
            let LinkLine { service, address, subcode, url, .. } = *link;
            write_target(&LinkDestination { service, address, subcode, url }, writer)?;
            let label = if link.key == LinkKey::Index { &[][..] } else { link.label.as_bytes() };
            writer.byte(u8::try_from(label.len()).map_err(|_| Overflow)?)?;
            writer.put(label)
        }
        Link::Number(link) => {
            writer.put(&link.number.mpp().to_be_bytes())?;
            write_target(&link.destination, writer)
        }
        Link::Text(link) => {
            writer.put(&[link.row, link.column, link.length])?;
            write_target(&link.destination, writer)
        }
    }
}

/// The bytes a link takes in a record.
fn link_bytes(link: &Link) -> Option<usize> {
    let mut scratch = [0_u8; MAX_BODY];
    let mut writer = Writer::new(&mut scratch);
    write_link(link, &mut writer).ok()?;
    Some(writer.len())
}

/// A sub-page's links, as a page file gives them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SubPageLinks<'a> {
    /// Its `LN` links: at most five, each key once, at most four coloured.
    pub keys: &'a [LinkLine],
    /// Its `LK` links: at most eight, each number once.
    pub numbers: &'a [NumberLink],
    /// Its `LT` links: at most eight.
    pub texts: &'a [TextLink],
}

/// What [`encode`] wrote.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sent {
    /// The records written; 0 for a sub-page with no links, which is sent
    /// without a LINKS record.
    pub records: u8,
    /// The packets written.
    pub packets: usize,
    /// The record id after the last record's, for the service's next
    /// record (section 7.10).
    pub next_record_id: u8,
}

/// Writes a sub-page's links as LINKS records, packets 4/31 of audio
/// service `service`, the first with id `first_record_id` and each after
/// it the next, given to `out` in order. Nothing is written for a
/// sub-page with no links.
///
/// # Errors
/// Links the reader would refuse: too many of a kind, a key or number
/// given twice, or a target or label that breaks its rule. Nothing is
/// written then.
pub fn encode(
    page: PageAddress,
    subcode: SubCode,
    links: SubPageLinks<'_>,
    service: u8,
    first_record_id: u8,
    out: &mut dyn FnMut(Packet),
) -> Result<Sent, RecordFault> {
    let SubPageLinks { keys, numbers, texts } = links;
    if keys.len() > LINK_LINES || numbers.len() > NUMBER_LINKS || texts.len() > TEXT_LINKS {
        return Err(RecordFault::BadValue("links count"));
    }
    let all = || {
        keys.iter()
            .copied()
            .map(Link::Key)
            .chain(numbers.iter().copied().map(Link::Number))
            .chain(texts.iter().copied().map(Link::Text))
    };
    if all().next().is_none() {
        return Ok(Sent { records: 0, packets: 0, next_record_id: first_record_id });
    }
    // Plan the records: as many links in each as fit, in order.
    let mut ends = [0_usize; MAX_PARTS];
    let mut parts: usize = 0;
    let mut used = FIXED;
    for (index, link) in all().enumerate() {
        let bytes = link_bytes(&link).ok_or(RecordFault::Length)?;
        if used.saturating_add(bytes) > MAX_BODY && used > FIXED {
            parts = parts.saturating_add(1);
            used = FIXED;
        }
        used = used.saturating_add(bytes);
        *ends.get_mut(parts).ok_or(RecordFault::Length)? = index.saturating_add(1);
    }
    let parts = u8::try_from(parts.saturating_add(1)).map_err(|_| RecordFault::Length)?;
    let number_end = keys.len().saturating_add(numbers.len());
    let mut record_id = first_record_id;
    let mut packets: usize = 0;
    // The first pass checks every record with the reader, so nothing is
    // sent unless all of it would be read; the second sends them.
    for send in [false, true] {
        let mut start = 0;
        for part in 0..parts {
            let end = ends.get(usize::from(part)).copied().unwrap_or(0);
            let mut body = [0_u8; MAX_BODY];
            let mut writer = Writer::new(&mut body);
            // The links of kind [from, to) in this record.
            let count = |from: usize, to: usize| u8::try_from(end.min(to).saturating_sub(start.max(from))).unwrap_or(0);
            let written = (|| {
                writer.put(&page.mpp().to_be_bytes())?;
                writer.put(&subcode.value().to_be_bytes())?;
                writer.put(&[part, parts])?;
                for (from, to) in [(0, keys.len()), (keys.len(), number_end), (number_end, usize::MAX)] {
                    writer.byte(count(from, to))?;
                    for link in all().enumerate().take(end.min(to)).skip(start.max(from)).map(|(_, link)| link) {
                        write_link(&link, &mut writer)?;
                    }
                }
                Ok::<(), Overflow>(())
            })();
            written.map_err(|_| RecordFault::Length)?;
            let length = writer.len();
            let body = body.get(..length).ok_or(RecordFault::Length)?;
            if send {
                packets = packets.saturating_add(write_packets(TYPE_LINKS, body, service, record_id, out).unwrap_or(0));
                record_id = record_id.wrapping_add(1);
            } else {
                read(body)?;
            }
            start = end;
        }
    }
    Ok(Sent { records: parts, packets, next_record_id: record_id })
}

/// Writes a page's links as LINKS records: its `LN` links when they are
/// valid (Appendix E.2 rule 1), its `LK` and `LT` links, for the
/// sub-page the page is (REQ-1494). See [`encode`].
///
/// # Errors
/// As [`encode`].
pub fn encode_page(
    page: &PageFile,
    service: u8,
    first_record_id: u8,
    out: &mut dyn FnMut(Packet),
) -> Result<Sent, RecordFault> {
    let keys = if page.links_valid { page.links.as_slice() } else { &[] };
    let links = SubPageLinks { keys, numbers: page.number_links.as_slice(), texts: page.text_links.as_slice() };
    encode(page.address, page.subcode, links, service, first_record_id, out)
}
