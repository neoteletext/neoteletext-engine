//! A page's list of sub-pages, in parts (REQ-1888, DOC-2038; DEC-2116): a
//! page whose sub-pages are files of their own has its list in its folder,
//! `NNN/P.sub`, part `P` holding the [`ROWS_PER_PART`] rows from place
//! `64 × P`, so that a reader reaches any row with one request and no file
//! grows with the page.
//!
//! The first line is the part's header, the page's number of sub-pages and
//! its version; every line after it is one row, a sub-page's sub-code,
//! validator and title. Fields are separated by one tab, written `→` here:
//!
//! ```text
//! 3→4be81f7a09c2d635
//! 0001→3fa94c0e12d7b655→Headlines
//! 0002→91c0aa2b4e6f0d13→Weather
//! 0003→0be5d2c4a9187f60
//! ```
//!
//! The reader is strict and bounded and allocates nothing: [`read`] checks
//! the file's size and its header, then [`Entries`] yields each row as an
//! entry or a fault. A fault never stops the reading; the caller counts and
//! reports the faults. [`write_header`] and [`write_entry`] write one line
//! each, and [`Validator::of`] gives a file the validator this engine's
//! writers put beside it.

use neoteletext_core::consts::{
    COLUMNS, DESIGNATION_CODES, PACKET_BYTES, PACKET_ENHANCEMENT, PACKET_HEADER, PACKET_LINKS, PACKET_PAGE_ENHANCEMENT,
    PACKET_ROW_25,
};
use neoteletext_core::limits;
use neoteletext_core::types::SubCode;

use crate::bytes::{Overflow, TextBuf, Writer};
use crate::sha256::{self, Sha256};

/// The most sub-pages a list holds: every sub-page a display page may have,
/// 0001 to 3F7E, 8,190 (REQ-NUM-020).
pub const MAX_ENTRIES: usize = limits::MAX_SUBPAGES;
/// The rows of a part (DOC-2038, `ROWS_PER_PART`): a part other than a page's
/// last holds exactly this many.
pub const ROWS_PER_PART: usize = 64;
/// The most parts of a page's list.
pub const MAX_PARTS: usize = MAX_ENTRIES.div_ceil(ROWS_PER_PART);
/// Digits of a sub-code, as in a sub-page's file name (REQ-1887).
pub const SUBCODE_DIGITS: usize = 4;
/// The longest validator: 64 bits of a SHA-256 digest in hexadecimal.
pub const MAX_VALIDATOR: usize = 16;
/// The longest title kept; longer titles are cut. A sub-page's title is a
/// line of its text, and a teletext row is 40 characters.
pub const MAX_TITLE: usize = COLUMNS;
/// The longest row: sub-code, validator and title, each but the first after
/// a tab, and CR LF.
pub const MAX_LINE: usize = SUBCODE_DIGITS + TAB_BYTES + MAX_VALIDATOR + TAB_BYTES + MAX_TITLE + LINE_END_BYTES;
/// The longest header: the number of sub-pages, the version after a tab, and
/// CR LF.
pub const MAX_HEADER: usize = TOTAL_DIGITS + TAB_BYTES + MAX_VALIDATOR + LINE_END_BYTES;
/// The largest part read. A larger file is refused whole.
pub const MAX_BYTES: usize = MAX_HEADER + ROWS_PER_PART * MAX_LINE;
/// The packets of the largest sub-page (DOC-2038): the header and rows 1 to
/// 25, then packets X/26, X/27 and X/28 with each of their designation codes.
const MAX_SUBPAGE_PACKETS: usize = (PACKET_ROW_25 - PACKET_HEADER + 1) as usize
    + (PACKET_PAGE_ENHANCEMENT - PACKET_ENHANCEMENT + 1) as usize * DESIGNATION_CODES;
/// The largest sub-page's file as T42 (DOC-2034). A larger file is refused
/// whole (REQ-1904).
pub const MAX_SUBPAGE_FILE: usize = MAX_SUBPAGE_PACKETS * PACKET_BYTES;
/// Digits of the number of sub-pages in a header, 8,190.
const TOTAL_DIGITS: usize = 4;
/// One tab between fields.
const TAB_BYTES: usize = 1;
/// The longest line end, CR LF.
const LINE_END_BYTES: usize = 2;
/// The digits of a sub-code, as [`Writer::hex`] counts them.
const SUBCODE_HEX_DIGITS: u32 = SUBCODE_DIGITS as u32;
/// The field separator.
const TAB: u8 = b'\t';
/// The last byte of a line, or of a file's last line.
const LINE_FEED: u8 = b'\n';
/// What may stand before the line feed of a line.
const CARRIAGE_RETURN: u8 = b'\r';
/// The first and last printable ASCII characters.
const FIRST_PRINTABLE: u8 = b' ';
const LAST_PRINTABLE: u8 = b'~';

const _: () = assert!(PACKET_LINKS == PACKET_ENHANCEMENT + 1, "X/26, X/27 and X/28 are consecutive");
const _: () = assert!(MAX_SUBPAGE_FILE == 3_108, "DOC-2034's largest sub-page file");
const _: () = assert!(MAX_BYTES == 4_119, "DOC-2034's largest part");
const _: () = assert!(MAX_PARTS == 128, "DOC-2038: a page of 8,190 sub-pages has parts 0 to 127");
const _: () = assert!(MAX_PARTS <= u8::MAX as usize, "a part number fits its type");

/// A part's number: 0 up to the last part of a page of [`MAX_ENTRIES`]
/// sub-pages. Its file's name writes it in decimal, with no leading zeros.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct PartNumber(u8);

impl PartNumber {
    /// Part 0, whose answer says whether a page has a folder (DOC-2038).
    pub const FIRST: Self = Self(0);

    /// The part numbered `number`, when a page can have it.
    #[must_use]
    pub const fn new(number: usize) -> Option<Self> {
        if number < MAX_PARTS {
            // MAX_PARTS fits a byte, as asserted above.
            #[allow(clippy::cast_possible_truncation, reason = "bounded by MAX_PARTS, asserted to fit")]
            Some(Self(number as u8))
        } else {
            None
        }
    }

    /// The part that holds the row at `place`, counting from 0: a row is
    /// reached with one request.
    #[must_use]
    pub const fn of_place(place: usize) -> Option<Self> {
        Self::new(place / ROWS_PER_PART)
    }

    /// The part's number.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }

    /// The place of the part's first row, counting from 0.
    #[must_use]
    pub const fn first_place(self) -> usize {
        self.0 as usize * ROWS_PER_PART
    }

    /// Reads a part's number as a file's name writes it.
    #[must_use]
    pub fn parse(digits: &[u8]) -> Option<Self> {
        Self::new(usize::from(plain_decimal(digits)?))
    }
}

/// The parts that hold the `count` rows from `first`, counting from 0: what
/// a reader asks for to show them, and nothing else (REQ-1888). Rows past the
/// last a page can have name no part.
pub fn parts_of_rows(first: usize, count: usize) -> impl Iterator<Item = PartNumber> {
    let end = first.saturating_add(count).div_ceil(ROWS_PER_PART);
    let parts = if count == 0 { 0..0 } else { (first / ROWS_PER_PART)..end };
    parts.map_while(PartNumber::new)
}

/// A number in decimal, at most four digits, in its one spelling: no leading
/// zeros, so that one part or count has one name.
fn plain_decimal(digits: &[u8]) -> Option<u16> {
    if digits.len() > 1 && digits.first() == Some(&b'0') {
        return None;
    }
    crate::bytes::decimal_number(digits)
}

/// Why a part was refused whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubPagesError {
    /// More than [`MAX_BYTES`].
    TooLarge,
    /// No header, or an invalid one.
    Header,
}

/// Why one row was ignored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// Fewer than two fields, or more than three.
    Fields,
    /// A byte outside printable ASCII, other than the tab.
    Character,
    /// A sub-code that is not four capital hexadecimal digits naming a
    /// sub-page, 0000 to 3F7E within the thirteen bits a sub-code has.
    SubCode,
    /// A validator that is empty, longer than [`MAX_VALIDATOR`], or holds a
    /// character other than a letter, a digit, `-` or `_`.
    Validator,
    /// A title field that is present but empty.
    Title,
    /// A sub-code not above the row before it (REQ-1890).
    NotAscending,
    /// A row after the [`ROWS_PER_PART`]th.
    TooMany,
}

/// A validator: it changes when what it stands for changes, so that a
/// receiver holding a sub-page knows from its list alone whether it is
/// current, and a part from one page's version is told from another's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Validator(TextBuf<MAX_VALIDATOR>);

impl Validator {
    /// A validator as a list writes it: 1 to [`MAX_VALIDATOR`] letters,
    /// digits, `-` and `_`.
    #[must_use]
    pub fn parse(text: &[u8]) -> Option<Self> {
        let allowed = |byte: &u8| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_');
        if text.is_empty() || !text.iter().all(allowed) {
            return None;
        }
        TextBuf::from_bytes(text).map(Self)
    }

    /// The validator this engine's writers give a file (DOC-2035): the first
    /// 64 bits of the SHA-256 digest of its bytes, in lower-case hexadecimal.
    /// It depends only on the file, so a sub-page written again unchanged
    /// keeps it, and a receiver can check a file it fetched.
    #[must_use]
    pub fn of(file: &[u8]) -> Self {
        Self::of_digest(&sha256::digest(file))
    }

    /// A page's version (DOC-2035): the validator of its sub-pages' packets
    /// end to end, in the page's order, what `NNN.t42` would hold if it held
    /// them all. It changes when any sub-page changes, is added or is removed.
    #[must_use]
    pub fn of_page<'a>(sub_pages: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let mut digest = Sha256::new();
        for sub_page in sub_pages {
            digest.update(sub_page);
        }
        Self::of_digest(&digest.finish())
    }

    fn of_digest(digest: &sha256::Digest) -> Self {
        let hex = sha256::to_hex(digest);
        Self(TextBuf::from_bytes(hex.get(..MAX_VALIDATOR).unwrap_or(&[])).unwrap_or_default())
    }

    /// The validator's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// One row of a part.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry<'a> {
    /// The sub-page's sub-code: its file is `NNN/SSSS.t42`.
    pub subcode: SubCode,
    /// Its validator.
    pub validator: Validator,
    /// Its title, at most [`MAX_TITLE`] characters, when it has one.
    pub title: Option<&'a str>,
}

/// One row that was ignored, by its line in the file, from 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineFault {
    /// The line.
    pub line: usize,
    /// Why.
    pub fault: Fault,
}

/// The numbered lines of a part.
type Lines<'a> = core::iter::Enumerate<core::slice::Split<'a, u8, fn(&u8) -> bool>>;

/// A part of a page's list.
#[derive(Clone, Debug)]
pub struct ListPart<'a> {
    /// The page's number of sub-pages, 1 to [`MAX_ENTRIES`]: every part of one
    /// version says the same, so whichever arrives first sizes a list.
    pub total: u16,
    /// The page's version.
    pub version: Validator,
    /// Its rows.
    pub entries: Entries<'a>,
}

/// The rows of a part, in order.
#[derive(Clone, Debug)]
pub struct Entries<'a> {
    lines: Lines<'a>,
    last: Option<SubCode>,
    kept: usize,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "the predicate `slice::split` calls takes a reference")]
fn is_line_end(byte: &u8) -> bool {
    *byte == LINE_FEED
}

/// A line without the carriage return that may end it.
fn without_return(line: &[u8]) -> &[u8] {
    line.strip_suffix(&[CARRIAGE_RETURN]).unwrap_or(line)
}

/// The header: the number of sub-pages and the page's version.
fn header(line: &[u8]) -> Option<(u16, Validator)> {
    let at = line.iter().position(|&byte| byte == TAB)?;
    let (total, rest) = line.split_at_checked(at)?;
    let total = plain_decimal(total).filter(|&total| (1..=MAX_ENTRIES).contains(&usize::from(total)))?;
    Some((total, Validator::parse(rest.get(TAB_BYTES..)?)?))
}

/// Checks a part's size and its header, and returns its rows.
pub fn read(bytes: &[u8]) -> Result<ListPart<'_>, SubPagesError> {
    if bytes.len() > MAX_BYTES {
        return Err(SubPagesError::TooLarge);
    }
    // A line end ends a line: no line follows the last.
    let text = bytes.strip_suffix(&[LINE_FEED]).unwrap_or(bytes);
    let mut lines = text.split(is_line_end as fn(&u8) -> bool).enumerate();
    let (_, first) = lines.next().ok_or(SubPagesError::Header)?;
    let (total, version) = header(without_return(first)).ok_or(SubPagesError::Header)?;
    Ok(ListPart { total, version, entries: Entries { lines, last: None, kept: 0 } })
}

/// A sub-code as a list and a sub-page's file name write it: four capital
/// hexadecimal digits, S4 first, with no bit outside the thirteen a sub-code
/// has (clause 9.3.1.2), naming a sub-page. 3F7F names no particular sub-page
/// (clause 9.6.1), so it has no file and no row (DOC-2038).
#[must_use]
pub fn parse_subcode(field: &[u8]) -> Option<SubCode> {
    if field.len() != SUBCODE_DIGITS || !field.iter().all(|&byte| crate::channel::is_capital_hex(byte)) {
        return None;
    }
    let value = crate::bytes::hex_number(field)?;
    (limits::is_sub_code(value) && value != limits::SUBCODE_ANY).then_some(SubCode::new(value))
}

/// Writes a sub-code as four capital hexadecimal digits.
pub fn write_subcode(writer: &mut Writer<'_>, subcode: SubCode) -> Result<(), Overflow> {
    writer.hex(u32::from(subcode.value()), SUBCODE_HEX_DIGITS)
}

/// A title: never empty when present, at most [`MAX_TITLE`] characters.
fn title(field: &str) -> Result<&str, Fault> {
    if field.is_empty() {
        return Err(Fault::Title);
    }
    // The line is printable ASCII, so any byte is a character boundary.
    Ok(field.get(..field.len().min(MAX_TITLE)).unwrap_or(field))
}

/// Reads a row's fields.
fn row(raw: &[u8]) -> Result<Entry<'_>, Fault> {
    if !raw.iter().all(|&byte| byte == TAB || (FIRST_PRINTABLE..=LAST_PRINTABLE).contains(&byte)) {
        return Err(Fault::Character);
    }
    // Printable ASCII is UTF-8.
    let text = core::str::from_utf8(raw).map_err(|_| Fault::Character)?;
    let mut fields = text.split(char::from(TAB));
    match [fields.next(), fields.next(), fields.next(), fields.next()] {
        [Some(code), Some(validator), name, None] => {
            let subcode = parse_subcode(code.as_bytes()).ok_or(Fault::SubCode)?;
            let validator = Validator::parse(validator.as_bytes()).ok_or(Fault::Validator)?;
            Ok(Entry { subcode, validator, title: name.map(title).transpose()? })
        }
        _ => Err(Fault::Fields),
    }
}

impl<'a> Iterator for Entries<'a> {
    type Item = Result<Entry<'a>, LineFault>;

    fn next(&mut self) -> Option<Self::Item> {
        let (index, raw) = self.lines.next()?;
        let line = index.saturating_add(1);
        let entry = row(without_return(raw)).and_then(|entry| {
            if self.kept >= ROWS_PER_PART {
                Err(Fault::TooMany)
            } else if self.last.is_some_and(|last| entry.subcode <= last) {
                Err(Fault::NotAscending)
            } else {
                Ok(entry)
            }
        });
        if let Ok(entry) = &entry {
            self.kept = self.kept.saturating_add(1);
            self.last = Some(entry.subcode);
        }
        Some(entry.map_err(|fault| LineFault { line, fault }))
    }
}

/// Writes a part's header, ended by a line feed.
pub fn write_header(writer: &mut Writer<'_>, total: u16, version: &Validator) -> Result<(), Overflow> {
    writer.decimal(u64::from(total))?;
    writer.byte(TAB)?;
    writer.put(version.as_str().as_bytes())?;
    writer.byte(LINE_FEED)
}

/// Writes one row, ended by a line feed. The title is made a valid one:
/// each byte outside printable ASCII becomes a space, runs of spaces
/// become one, the ends are trimmed and the rest cut to [`MAX_TITLE`]; a
/// title left empty is not written. Nothing is written when the row does
/// not fit.
pub fn write_entry(
    writer: &mut Writer<'_>,
    subcode: SubCode,
    validator: &Validator,
    title: &str,
) -> Result<(), Overflow> {
    let start = writer.len();
    let written = put_entry(writer, subcode, validator, title);
    if written.is_err() {
        writer.truncate(start);
    }
    written
}

fn put_entry(writer: &mut Writer<'_>, subcode: SubCode, validator: &Validator, title: &str) -> Result<(), Overflow> {
    write_subcode(writer, subcode)?;
    writer.byte(TAB)?;
    writer.put(validator.as_str().as_bytes())?;
    let mut kept = 0usize;
    let words = title.as_bytes().split(|&byte| !(byte > FIRST_PRINTABLE && byte <= LAST_PRINTABLE));
    for word in words.filter(|word| !word.is_empty()) {
        // A separator, then at least one character of the word.
        let separator = if kept == 0 { TAB } else { FIRST_PRINTABLE };
        let after = if kept == 0 { 1 } else { kept.saturating_add(2) };
        if after > MAX_TITLE {
            break;
        }
        writer.byte(separator)?;
        if kept > 0 {
            kept = kept.saturating_add(1);
        }
        for &byte in word.iter().take(MAX_TITLE.saturating_sub(kept)) {
            writer.byte(byte)?;
            kept = kept.saturating_add(1);
        }
    }
    writer.byte(LINE_FEED)
}

#[cfg(feature = "std")]
mod layout;
#[cfg(feature = "std")]
pub use layout::{ChannelFiles, LayoutError, SubPageFile, page_files};

#[cfg(test)]
mod tests;
