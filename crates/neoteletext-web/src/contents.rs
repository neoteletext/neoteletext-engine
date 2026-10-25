//! A channel's contents file, `contents.txt` (REQ-WEB-025, decision E.70
//! item 8): its sections, pages and linked channels, one entry per line.
//!
//! Fields are separated by one tab, written `→` here:
//!
//! ```text
//! # comments and empty lines are ignored
//! s→News
//! p→101→Headlines→2026-10-12
//! x→weather:100→Weather
//! ```
//!
//! The reader is strict and bounded and allocates nothing: [`read`]
//! checks the file's size and line count, then [`Entries`] yields each
//! line as an entry or a fault. A fault never stops the reading; the
//! caller counts and reports them.

use neoteletext_core::types::PageAddress;

use crate::channel;

/// The largest contents file read (REQ-WEB-025).
pub const MAX_BYTES: usize = 64 * 1024;
/// The most lines read.
pub const MAX_LINES: usize = 1000;
/// The most page entries a file can list: one for each page number a
/// viewer can type, 100 to 899, since a page listed twice is a fault.
pub const MAX_PAGES: usize = 800;
/// The longest title kept; longer titles are cut (decision E.120: a whole
/// headline, which a receiver fits to the room it has).
pub const MAX_TITLE: usize = 120;
/// Page numbers a viewer can type: 100 to 8FF, as bits from 100.
const PAGE_SLOTS: usize = 0x800;
/// The first page number, 100.
const FIRST_PAGE: u16 = 0x100;
/// Bits in one word of the set of pages seen.
const WORD_BITS: usize = 32;
/// The field separator.
const TAB: u8 = b'\t';
/// The first character of a comment line.
const COMMENT: u8 = b'#';
/// The separator of a linked channel's name and page.
const NAME_SEPARATOR: char = ':';
/// The characters of a date, `YYYY-MM-DD`.
const DATE_LENGTH: usize = 10;
/// The last month and the last day a date may give.
const LAST_MONTH: u8 = 12;
const LAST_DAY: u8 = 31;

/// Why a contents file was refused whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentsError {
    /// More than [`MAX_BYTES`].
    TooLarge,
    /// More than [`MAX_LINES`].
    TooManyLines,
}

/// Why one line was ignored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// A kind other than `s`, `p` and `x`, kept for later versions.
    UnknownKind,
    /// The wrong number of fields for its kind.
    Fields,
    /// A byte outside printable ASCII, other than the tab.
    Character,
    /// A page that is not three decimal digits, magazine 1 to 8.
    Page,
    /// A date that is not `YYYY-MM-DD`.
    Date,
    /// A target that is neither `name:mpp` nor a channel's address.
    Target,
    /// An empty title.
    Title,
    /// A page already listed.
    Duplicate,
}

/// A date, as `contents.txt` writes it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Date {
    /// The year.
    pub year: u16,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day, 1 to 31.
    pub day: u8,
}

/// One entry of the contents.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entry<'a> {
    /// A section heading.
    Section {
        /// The heading, at most [`MAX_TITLE`] characters.
        title: &'a str,
    },
    /// A page of this channel.
    Page {
        /// The page.
        page: PageAddress,
        /// Its title.
        title: &'a str,
        /// When it last changed.
        date: Option<Date>,
    },
    /// Another channel, or a page of one.
    Channel {
        /// `name:mpp`, or an address.
        target: &'a str,
        /// Its title.
        title: &'a str,
    },
}

/// One line that was ignored, by its number from 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineFault {
    /// The line.
    pub line: usize,
    /// Why.
    pub fault: Fault,
}

/// The numbered lines of a contents file.
type Lines<'a> = core::iter::Enumerate<core::slice::Split<'a, u8, fn(&u8) -> bool>>;

/// The entries of a contents file, in order.
#[derive(Clone, Debug)]
pub struct Entries<'a> {
    lines: Lines<'a>,
    seen: [u32; PAGE_SLOTS / WORD_BITS],
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "the predicate `slice::split` calls takes a reference")]
fn is_line_end(byte: &u8) -> bool {
    *byte == b'\n'
}

/// Checks a contents file's size and length, and returns its entries.
pub fn read(bytes: &[u8]) -> Result<Entries<'_>, ContentsError> {
    if bytes.len() > MAX_BYTES {
        return Err(ContentsError::TooLarge);
    }
    #[allow(clippy::naive_bytecount, reason = "at most 64 kilobytes, counted once; no crate is added for it")]
    let lines = bytes.iter().filter(|&&byte| byte == b'\n').count();
    let unterminated = usize::from(bytes.last().is_some_and(|&byte| byte != b'\n'));
    if lines.saturating_add(unterminated) > MAX_LINES {
        return Err(ContentsError::TooManyLines);
    }
    Ok(Entries { lines: bytes.split(is_line_end as fn(&u8) -> bool).enumerate(), seen: [0; PAGE_SLOTS / WORD_BITS] })
}

/// A title: at most [`MAX_TITLE`] characters, never empty.
fn title(field: &str) -> Result<&str, Fault> {
    if field.is_empty() {
        return Err(Fault::Title);
    }
    // The line is printable ASCII, so any byte is a character boundary.
    Ok(field.get(..field.len().min(MAX_TITLE)).unwrap_or(field))
}

/// A page a viewer can type: three decimal digits, magazine 1 to 8.
fn page(field: &str) -> Result<PageAddress, Fault> {
    if field.len() != 3 || !field.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Fault::Page);
    }
    let mpp = crate::bytes::hex_number(field.as_bytes()).ok_or(Fault::Page)?;
    PageAddress::from_mpp(mpp).ok_or(Fault::Page)
}

/// `YYYY-MM-DD` (ISO 8601).
fn date(field: &str) -> Result<Date, Fault> {
    let bytes = field.as_bytes();
    let digits_at = |range: core::ops::Range<usize>| {
        let part = bytes.get(range).ok_or(Fault::Date)?;
        if !part.iter().all(u8::is_ascii_digit) {
            return Err(Fault::Date);
        }
        crate::bytes::decimal_number(part).ok_or(Fault::Date)
    };
    if bytes.len() != DATE_LENGTH || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return Err(Fault::Date);
    }
    let year = digits_at(0..4)?;
    let month = u8::try_from(digits_at(5..7)?).map_err(|_| Fault::Date)?;
    let day = u8::try_from(digits_at(8..10)?).map_err(|_| Fault::Date)?;
    if !(1..=LAST_MONTH).contains(&month) || !(1..=LAST_DAY).contains(&day) {
        return Err(Fault::Date);
    }
    Ok(Date { year, month, day })
}

/// A linked channel's target: `name:mpp` with a name of letters, digits
/// and hyphens, or the address of a channel's folder or page file.
fn target(field: &str) -> Result<&str, Fault> {
    if field.contains("://") {
        let address = channel::parse_address(field).map_err(|_| Fault::Target)?;
        return match address.file {
            None | Some(channel::ChannelFile::Page { .. }) => Ok(field),
            Some(_) => Err(Fault::Target),
        };
    }
    let (name, number) = field.split_once(NAME_SEPARATOR).ok_or(Fault::Target)?;
    let named = !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    let hex = number.len() == 3 && number.bytes().all(|byte| byte.is_ascii_hexdigit());
    let valid_page = hex && crate::bytes::hex_number(number.as_bytes()).and_then(PageAddress::from_mpp).is_some();
    if named && valid_page { Ok(field) } else { Err(Fault::Target) }
}

impl Entries<'_> {
    /// Marks a page as listed; false when it already was.
    fn first_time(&mut self, page: PageAddress) -> bool {
        let index = usize::from(page.mpp().saturating_sub(FIRST_PAGE));
        let (word, bit) = (index / WORD_BITS, index % WORD_BITS);
        let Some(slot) = self.seen.get_mut(word) else { return false };
        let mask = 1u32 << bit;
        let first = *slot & mask == 0;
        *slot |= mask;
        first
    }
}

impl<'a> Iterator for Entries<'a> {
    type Item = Result<Entry<'a>, LineFault>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (index, raw) = self.lines.next()?;
            let line = index.saturating_add(1);
            let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
            if raw.is_empty() || raw.first() == Some(&COMMENT) {
                continue;
            }
            let fail = |fault| Some(Err(LineFault { line, fault }));
            if !raw.iter().all(|&byte| byte == TAB || (0x20..0x7F).contains(&byte)) {
                return fail(Fault::Character);
            }
            // Printable ASCII is UTF-8.
            let Ok(text) = core::str::from_utf8(raw) else { return fail(Fault::Character) };
            let mut fields = text.split(char::from(TAB));
            let kind = fields.next().unwrap_or("");
            let rest: [Option<&str>; 4] = [fields.next(), fields.next(), fields.next(), fields.next()];
            let entry = match (kind, rest) {
                ("s", [Some(heading), None, None, None]) => title(heading).map(|title| Entry::Section { title }),
                ("p", [Some(number), Some(name), when, None]) => page(number).and_then(|page| {
                    let date = when.map(date).transpose()?;
                    Ok(Entry::Page { page, title: title(name)?, date })
                }),
                ("x", [Some(to), Some(name), None, None]) => {
                    target(to).and_then(|target| Ok(Entry::Channel { target, title: title(name)? }))
                }
                ("s" | "p" | "x", _) => Err(Fault::Fields),
                _ => Err(Fault::UnknownKind),
            };
            let entry = match entry {
                Ok(Entry::Page { page, .. }) if !self.first_time(page) => Err(Fault::Duplicate),
                other => other,
            };
            return Some(entry.map_err(|fault| LineFault { line, fault }));
        }
    }
}

#[cfg(test)]
mod tests;
