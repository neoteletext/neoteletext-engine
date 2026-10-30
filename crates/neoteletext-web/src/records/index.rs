//! The index record (`neoteletext-common/INPUT.md` section 7.5): one
//! magazine's name and pages, a part of one sending of the channel's index
//! (neoteletext-app-server's DEC-1482, REQ-1477), what a static channel's
//! `contents.txt` gives (REQ-WEB-025).
//!
//! ```text
//! bytes 0-1  generation: the sending's version
//! bytes 2-3  part: 0 to parts - 1
//! bytes 4-5  parts: the records of the sending
//! byte 6     magazine: 1 to 8
//! then       name: text, 0 to 120 characters
//! then       pages, ascending, each:
//!              page u16, subpages u16 (1 to 8,190),
//!              title text (1 to 120 characters),
//!              section text (0 to 120 characters)
//! ```
//!
//! Text is a big-endian `u16` byte length, then UTF-8 with no control
//! character. [`read`] checks a record whole; [`encode`] writes a whole
//! index as records; [`Progress`] tells a receiver when a sending is
//! complete.

use neoteletext_audio::MAX_BODY;
use neoteletext_core::limits::{
    FIRST_MAGAZINE, FIRST_PAGE_ADDRESS, LAST_PAGE_ADDRESS, MAGAZINES, MAX_SUBPAGES, MPP_MAGAZINE_SHIFT,
    USABLE_PAGE_NUMBERS, is_reserved_page,
};
use neoteletext_core::packet::Packet;
use neoteletext_core::types::PageAddress;

use super::{RecordFault, TYPE_INDEX, take_u16, write_packets};
use crate::contents::{Entry, MAX_TITLE};

/// The longest name, title or section, in characters (`contents::MAX_TITLE`).
pub const MAX_TEXT_CHARACTERS: usize = MAX_TITLE;
/// The bytes of UTF-8 a character takes at most (RFC 3629).
const UTF8_MAX: usize = 4;
/// The longest name, title or section, in bytes.
pub const MAX_TEXT_BYTES: usize = MAX_TEXT_CHARACTERS * UTF8_MAX;
/// A text's length field.
const LENGTH_BYTES: usize = 2;
/// The fixed fields: generation, part, parts and magazine.
const FIXED: usize = 2 + 2 + 2 + 1;
/// A page's fixed fields: its number and its sub-page count.
const PAGE_FIXED: usize = 2 + 2;
/// The most records one sending may have: every usable page in a record of
/// its own, and a record for each magazine's name alone.
pub const MAX_PARTS: usize = USABLE_PAGE_NUMBERS + MAGAZINES;
/// The most sub-pages a page may have (`MAX_SUBPAGES`).
const MAX_PAGE_SUBPAGES: u16 = MAX_SUBPAGES as u16;
const _: () = assert!(MAX_SUBPAGES <= u16::MAX as usize, "a sub-page count fits its field");
const _: () = assert!(MAX_PARTS <= u16::MAX as usize, "a part fits its field");
// The largest page fits a record beside the fixed fields and an empty
// name, so every index can be written (section 7.5).
const _: () = assert!(FIXED + LENGTH_BYTES + PAGE_FIXED + 2 * (LENGTH_BYTES + MAX_TEXT_BYTES) <= MAX_BODY);

/// One page of the index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IndexPage<'a> {
    /// The page, `0x100` to `0x8FF`, of its magazine, never reserved.
    pub page: u16,
    /// Its sub-pages, 1 to 8,190.
    pub subpages: u16,
    /// Its title, 1 to [`MAX_TEXT_CHARACTERS`] characters.
    pub title: &'a str,
    /// The heading it is listed under; empty for none.
    pub section: &'a str,
}

/// One magazine of the index, as a program gives it (REQ-1477).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IndexMagazine<'a> {
    /// 1 to 8.
    pub magazine: u8,
    /// Its name; empty for none.
    pub name: &'a str,
    /// Its pages, in ascending order.
    pub pages: &'a [IndexPage<'a>],
}

/// One index record, checked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IndexRecord<'a> {
    /// The sending's version.
    pub generation: u16,
    /// This record's place in the sending.
    pub part: u16,
    /// The records of the sending.
    pub parts: u16,
    /// The magazine, 1 to 8.
    pub magazine: u8,
    /// The magazine's name; empty in a later record of the same magazine
    /// and sending, which keeps the name given.
    pub name: &'a str,
    /// The pages' bytes, checked.
    pages: &'a [u8],
}

impl<'a> IndexRecord<'a> {
    /// The record's pages, in ascending order.
    #[must_use]
    pub fn pages(&self) -> Pages<'a> {
        Pages { bytes: self.pages }
    }

    /// The record's pages as `contents.txt`'s entries (REQ-WEB-025): a
    /// section wherever a page's section differs from the page before it,
    /// then the page with its title.
    #[must_use]
    pub fn entries(&self) -> Entries<'a> {
        Entries { pages: self.pages(), section: "", waiting: None }
    }
}

/// The pages of an index record.
#[derive(Clone, Debug)]
pub struct Pages<'a> {
    bytes: &'a [u8],
}

impl<'a> Iterator for Pages<'a> {
    type Item = IndexPage<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // The record was checked whole, so a page that does not read ends
        // nothing but this walk.
        let (page, rest) = read_page(self.bytes).ok()?;
        self.bytes = rest;
        Some(page)
    }
}

/// An index record's pages as `contents.txt`'s entries.
#[derive(Clone, Debug)]
pub struct Entries<'a> {
    pages: Pages<'a>,
    section: &'a str,
    waiting: Option<IndexPage<'a>>,
}

impl<'a> Iterator for Entries<'a> {
    type Item = Entry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let page = if let Some(page) = self.waiting.take() {
            page
        } else {
            let page = self.pages.next()?;
            if page.section != self.section {
                self.section = page.section;
                if !page.section.is_empty() {
                    self.waiting = Some(page);
                    return Some(Entry::Section { title: page.section });
                }
            }
            page
        };
        let address = PageAddress::from_mpp(page.page)?;
        Some(Entry::Page { page: address, title: page.title, date: None })
    }
}

/// Reads a text field: its length, then UTF-8 of `minimum` to
/// [`MAX_TEXT_CHARACTERS`] characters with no control character.
fn take_text(bytes: &[u8], minimum: usize) -> Result<(&str, &[u8]), RecordFault> {
    let (length, rest) = take_u16(bytes)?;
    let (text, rest) = rest.split_at_checked(usize::from(length)).ok_or(RecordFault::Length)?;
    let text = core::str::from_utf8(text).map_err(|_| RecordFault::BadValue("index text"))?;
    check_text(text, minimum)?;
    Ok((text, rest))
}

fn check_text(text: &str, minimum: usize) -> Result<(), RecordFault> {
    let characters = text.chars().count();
    if characters < minimum || characters > MAX_TEXT_CHARACTERS || text.chars().any(char::is_control) {
        return Err(RecordFault::BadValue("index text"));
    }
    Ok(())
}

/// The magazine of a page number, 1 to 8.
const fn magazine_of(page: u16) -> u8 {
    (page >> MPP_MAGAZINE_SHIFT) as u8
}

/// Whether `page` may be listed under `magazine` (section 7.5).
fn page_allowed(page: u16, magazine: u8) -> bool {
    (FIRST_PAGE_ADDRESS..=LAST_PAGE_ADDRESS).contains(&page)
        && magazine_of(page) == magazine
        && !is_reserved_page(magazine, page.to_be_bytes()[1])
}

fn read_page(bytes: &[u8]) -> Result<(IndexPage<'_>, &[u8]), RecordFault> {
    let (page, rest) = take_u16(bytes)?;
    let (subpages, rest) = take_u16(rest)?;
    let (title, rest) = take_text(rest, 1)?;
    let (section, rest) = take_text(rest, 0)?;
    Ok((IndexPage { page, subpages, title, section }, rest))
}

/// Reads an index record's body, checking every field.
///
/// # Errors
/// Any field outside its range: a part not below its parts, more parts
/// than [`MAX_PARTS`], a magazine outside 1 to 8, a page not of the
/// magazine or reserved or out of order, a sub-page count outside 1 to
/// 8,190, text that breaks its rule, or a field cut short.
pub fn read(body: &[u8]) -> Result<IndexRecord<'_>, RecordFault> {
    let (generation, rest) = take_u16(body)?;
    let (part, rest) = take_u16(rest)?;
    let (parts, rest) = take_u16(rest)?;
    if parts == 0 || part >= parts || usize::from(parts) > MAX_PARTS {
        return Err(RecordFault::BadValue("index part"));
    }
    let (&magazine, rest) = rest.split_first().ok_or(RecordFault::Length)?;
    if !(FIRST_MAGAZINE..=MAGAZINES as u8).contains(&magazine) {
        return Err(RecordFault::BadValue("index magazine"));
    }
    let (name, pages) = take_text(rest, 0)?;
    let mut walk = pages;
    let mut previous: Option<u16> = None;
    while !walk.is_empty() {
        let (page, rest) = read_page(walk)?;
        if !page_allowed(page.page, magazine) || previous.is_some_and(|previous| page.page <= previous) {
            return Err(RecordFault::BadValue("index page"));
        }
        if page.subpages == 0 || page.subpages > MAX_PAGE_SUBPAGES {
            return Err(RecordFault::BadValue("index sub-pages"));
        }
        previous = Some(page.page);
        walk = rest;
    }
    Ok(IndexRecord { generation, part, parts, magazine, name, pages })
}

/// Why an index could not be written: the first rule it breaks (REQ-1477).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexError {
    /// Magazines out of order, repeated, or outside 1 to 8.
    Magazine,
    /// A page out of order, repeated, of another magazine, or reserved.
    Page,
    /// A sub-page count outside 1 to 8,190.
    SubPages,
    /// A name, title or section that breaks its rule.
    Text,
    /// More records than [`MAX_PARTS`].
    TooLarge,
}

/// The bytes a page takes in a record.
fn page_bytes(page: &IndexPage<'_>) -> usize {
    PAGE_FIXED
        .saturating_add(LENGTH_BYTES.saturating_mul(2))
        .saturating_add(page.title.len())
        .saturating_add(page.section.len())
}

fn check_index(magazines: &[IndexMagazine<'_>]) -> Result<(), IndexError> {
    let mut last_magazine = 0;
    for magazine in magazines {
        if magazine.magazine <= last_magazine || magazine.magazine > MAGAZINES as u8 {
            return Err(IndexError::Magazine);
        }
        last_magazine = magazine.magazine;
        check_text(magazine.name, 0).map_err(|_| IndexError::Text)?;
        let mut last_page = 0;
        for page in magazine.pages {
            if page.page <= last_page || !page_allowed(page.page, magazine.magazine) {
                return Err(IndexError::Page);
            }
            last_page = page.page;
            if page.subpages == 0 || page.subpages > MAX_PAGE_SUBPAGES {
                return Err(IndexError::SubPages);
            }
            check_text(page.title, 1).map_err(|_| IndexError::Text)?;
            check_text(page.section, 0).map_err(|_| IndexError::Text)?;
        }
    }
    Ok(())
}

/// Walks the records an index takes, giving each record's magazine,
/// whether it carries the name, and its pages, to `each`. Returns the
/// number of records.
fn plan(magazines: &[IndexMagazine<'_>], each: &mut dyn FnMut(&IndexMagazine<'_>, bool, &[IndexPage<'_>])) -> usize {
    let mut records: usize = 0;
    for magazine in magazines {
        let mut named = true;
        let mut rest = magazine.pages;
        loop {
            let name = if named { magazine.name.len() } else { 0 };
            let mut used = FIXED.saturating_add(LENGTH_BYTES).saturating_add(name);
            let mut count: usize = 0;
            for page in rest {
                let next = used.saturating_add(page_bytes(page));
                if next > MAX_BODY {
                    break;
                }
                used = next;
                count = count.saturating_add(1);
            }
            let (taken, after) = rest.split_at(count.min(rest.len()));
            each(magazine, named, taken);
            records = records.saturating_add(1);
            named = false;
            rest = after;
            if rest.is_empty() {
                break;
            }
        }
    }
    records
}

fn put(body: &mut [u8], at: &mut usize, bytes: &[u8]) {
    let end = at.saturating_add(bytes.len());
    if let Some(target) = body.get_mut(*at..end) {
        target.copy_from_slice(bytes);
    }
    *at = end;
}

fn put_text(body: &mut [u8], at: &mut usize, text: &str) {
    let length = u16::try_from(text.len()).unwrap_or(u16::MAX);
    put(body, at, &length.to_be_bytes());
    put(body, at, text.as_bytes());
}

/// The index of a channel that lists nothing: one record of magazine 1,
/// unnamed, with no pages.
const EMPTY: [IndexMagazine<'static>; 1] = [IndexMagazine { magazine: FIRST_MAGAZINE, name: "", pages: &[] }];

/// What [`encode`] wrote.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sent {
    /// The records of the sending.
    pub records: u16,
    /// The packets written.
    pub packets: usize,
    /// The record id after the last record's, for the service's next
    /// record (section 7.10).
    pub next_record_id: u8,
}

/// Writes a whole index as one sending of `generation`: its records, as
/// packets 4/31 of audio service `service`, given to `out` in order, the
/// first record with id `first_record_id` and each after it the next
/// (section 7.10). An empty index is one record of magazine 1, unnamed,
/// with no pages.
///
/// # Errors
/// The first rule of REQ-1477 the index breaks, before anything is
/// written.
pub fn encode(
    generation: u16,
    magazines: &[IndexMagazine<'_>],
    service: u8,
    first_record_id: u8,
    out: &mut dyn FnMut(Packet),
) -> Result<Sent, IndexError> {
    check_index(magazines)?;
    let magazines = if magazines.is_empty() { &EMPTY[..] } else { magazines };
    let parts = plan(magazines, &mut |_, _, _| {});
    let parts =
        u16::try_from(parts).ok().filter(|&parts| usize::from(parts) <= MAX_PARTS).ok_or(IndexError::TooLarge)?;
    let mut part: u16 = 0;
    let mut record_id = first_record_id;
    let mut packets: usize = 0;
    plan(magazines, &mut |magazine, named, pages| {
        let mut body = [0_u8; MAX_BODY];
        let mut at = 0;
        put(&mut body, &mut at, &generation.to_be_bytes());
        put(&mut body, &mut at, &part.to_be_bytes());
        put(&mut body, &mut at, &parts.to_be_bytes());
        put(&mut body, &mut at, &[magazine.magazine]);
        put_text(&mut body, &mut at, if named { magazine.name } else { "" });
        for page in pages {
            put(&mut body, &mut at, &page.page.to_be_bytes());
            put(&mut body, &mut at, &page.subpages.to_be_bytes());
            put_text(&mut body, &mut at, page.title);
            put_text(&mut body, &mut at, page.section);
        }
        let body = body.get(..at).unwrap_or(&[]);
        // The writer checks with the reader: what it writes always reads.
        if read(body).is_ok() {
            packets = packets.saturating_add(write_packets(TYPE_INDEX, body, service, record_id, out).unwrap_or(0));
        }
        part = part.saturating_add(1);
        record_id = record_id.wrapping_add(1);
    });
    Ok(Sent { records: parts, packets, next_record_id: record_id })
}

/// Words of the set of parts received.
const WORD_BITS: usize = 32;
const PART_WORDS: usize = MAX_PARTS.div_ceil(WORD_BITS);

/// What one index record meant for the sending (section 7.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Its pages are shown; the sending is not yet complete.
    Partial,
    /// The sending is complete: it is the whole index, and every page it
    /// did not list is removed.
    Complete,
    /// It disagrees with its sending's first record about the parts: it is
    /// discarded.
    Refused,
}

/// A receiver's sending in progress (section 7.5): which parts of which
/// generation have arrived.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Progress {
    generation: Option<u16>,
    parts: u16,
    received: [u32; PART_WORDS],
    count: u16,
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

impl Progress {
    /// Nothing received.
    #[must_use]
    pub const fn new() -> Self {
        Self { generation: None, parts: 0, received: [0; PART_WORDS], count: 0 }
    }

    /// Notes a record; a record of another generation begins a new
    /// sending.
    pub fn note(&mut self, record: &IndexRecord<'_>) -> Step {
        if self.generation != Some(record.generation) {
            *self = Self { generation: Some(record.generation), parts: record.parts, ..Self::new() };
        } else if self.parts != record.parts {
            return Step::Refused;
        }
        let index = usize::from(record.part);
        let (word, bit) = (index / WORD_BITS, index % WORD_BITS);
        if let Some(slot) = self.received.get_mut(word) {
            let mask = 1_u32 << bit;
            if *slot & mask == 0 {
                *slot |= mask;
                self.count = self.count.saturating_add(1);
            }
        }
        if self.count == self.parts { Step::Complete } else { Step::Partial }
    }
}
