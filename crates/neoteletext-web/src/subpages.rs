//! A page's catalogue of sub-pages, `NNN.sub` (REQ-WEB-034, decision
//! E.132): for a page of a channel whose sub-pages are files of their own,
//! `NNN-SSSS.t42` (REQ-WEB-019), each sub-page's sub-code, validator and
//! title, one entry per line, in the page's order.
//!
//! Fields are separated by one tab, written `→` here:
//!
//! ```text
//! 0001→3fa94c0e12d7b655→Headlines
//! 0002→91c0aa2b4e6f0d13→Weather
//! 0003→0be5d2c4a9187f60
//! ```
//!
//! The reader is strict and bounded and allocates nothing: [`read`] checks
//! the file's size, then [`Entries`] yields each line as an entry or a
//! fault. A fault never stops the reading; the caller counts and reports
//! the faults. [`write_entry`] writes one line, and [`Validator::of`] gives
//! a sub-page file the validator this engine's writers put beside it.

use neoteletext_core::consts::COLUMNS;
use neoteletext_core::limits;
use neoteletext_core::types::SubCode;

use crate::bytes::{Overflow, TextBuf, Writer};
use crate::sha256;

/// Every value a sub-code can take, 8,192 (REQ-NUM-031: the number is
/// `limits`').
const SUBCODE_VALUES: usize = limits::SUBCODE_VALUES;
/// The most entries kept: every sub-page a display page may have, 0001 to
/// 3F7E, 8,190 (REQ-NUM-020). Any sub-code may be listed (REQ-NUM-002);
/// only the count is bounded.
pub const MAX_ENTRIES: usize = limits::MAX_SUBPAGES;
/// Digits of a sub-code, as in a sub-page's file name (REQ-WEB-019).
pub const SUBCODE_DIGITS: usize = 4;
/// The longest validator: 64 bits of a SHA-256 digest in hexadecimal.
pub const MAX_VALIDATOR: usize = 16;
/// The longest title kept; longer titles are cut. A sub-page's title is a
/// line of its text, and a teletext row is 40 characters.
pub const MAX_TITLE: usize = COLUMNS;
/// The longest entry: sub-code, validator and title, each but the first
/// after a tab, and CR LF.
pub const MAX_LINE: usize = SUBCODE_DIGITS + TAB_BYTES + MAX_VALIDATOR + TAB_BYTES + MAX_TITLE + LINE_END_BYTES;
/// One tab between fields.
const TAB_BYTES: usize = 1;
/// The largest catalogue read: the most entries, each as long as an entry
/// can be. A larger file is refused whole.
pub const MAX_BYTES: usize = MAX_ENTRIES * MAX_LINE;
/// The longest line end, CR LF.
const LINE_END_BYTES: usize = 2;
/// The digits of a sub-code, as [`Writer::hex`] counts them.
const SUBCODE_HEX_DIGITS: u32 = SUBCODE_DIGITS as u32;
/// The bits of S1 and S2, the low byte of a sub-code's sixteen-bit form
/// less the bit where control bit C4 is sent (clause 9.3.1.2).
const S1_S2_BITS: u32 = 7;
/// Where S3 starts in a sub-code's sixteen-bit form (clause 9.3.1.2).
const S3_SHIFT: u32 = 8;
/// S1 and S2 of a sub-code's sixteen-bit form.
const S1_S2_MASK: usize = (1 << S1_S2_BITS) - 1;
/// Bits in one word of the set of sub-codes seen.
const WORD_BITS: usize = 32;
/// The field separator.
const TAB: u8 = b'\t';
/// The first and last printable ASCII characters.
const FIRST_PRINTABLE: u8 = b' ';
const LAST_PRINTABLE: u8 = b'~';

/// Why a catalogue was refused whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubPagesError {
    /// More than [`MAX_BYTES`].
    TooLarge,
}

/// Why one line was ignored.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fault {
    /// Fewer than two fields, or more than three.
    Fields,
    /// A byte outside printable ASCII, other than the tab.
    Character,
    /// A sub-code that is not four capital hexadecimal digits within the
    /// thirteen bits a sub-code has.
    SubCode,
    /// A validator that is empty, longer than [`MAX_VALIDATOR`], or holds a
    /// character other than a letter, a digit, `-` or `_`.
    Validator,
    /// A title field that is present but empty.
    Title,
    /// A sub-code already listed.
    Duplicate,
    /// An entry after the [`MAX_ENTRIES`]th.
    TooMany,
}

/// A sub-page's validator: it changes when the sub-page's file changes,
/// so that a receiver holding the sub-page knows from the catalogue alone
/// whether it is current.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Validator(TextBuf<MAX_VALIDATOR>);

impl Validator {
    /// A validator as a catalogue writes it: 1 to [`MAX_VALIDATOR`]
    /// letters, digits, `-` and `_`.
    #[must_use]
    pub fn parse(text: &[u8]) -> Option<Self> {
        let allowed = |byte: &u8| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_');
        if text.is_empty() || !text.iter().all(allowed) {
            return None;
        }
        TextBuf::from_bytes(text).map(Self)
    }

    /// The validator this engine's writers give a sub-page's file: the
    /// first 64 bits of the SHA-256 digest of its bytes, in lower-case
    /// hexadecimal. It depends only on the file, so a sub-page written again
    /// unchanged keeps it, and a receiver can check a file it fetched.
    #[must_use]
    pub fn of(file: &[u8]) -> Self {
        let hex = sha256::to_hex(&sha256::digest(file));
        Self(TextBuf::from_bytes(hex.get(..MAX_VALIDATOR).unwrap_or(&[])).unwrap_or_default())
    }

    /// The validator's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// One entry of a catalogue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry<'a> {
    /// The sub-page's sub-code: its file is `NNN-SSSS.t42`.
    pub subcode: SubCode,
    /// Its validator.
    pub validator: Validator,
    /// Its title, at most [`MAX_TITLE`] characters, when it has one.
    pub title: Option<&'a str>,
}

/// One line that was ignored, by its number from 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineFault {
    /// The line.
    pub line: usize,
    /// Why.
    pub fault: Fault,
}

/// The numbered lines of a catalogue.
type Lines<'a> = core::iter::Enumerate<core::slice::Split<'a, u8, fn(&u8) -> bool>>;

/// The entries of a catalogue, in order.
#[derive(Clone, Debug)]
pub struct Entries<'a> {
    lines: Lines<'a>,
    seen: [u32; SUBCODE_VALUES / WORD_BITS],
    kept: usize,
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "the predicate `slice::split` calls takes a reference")]
fn is_line_end(byte: &u8) -> bool {
    *byte == b'\n'
}

/// Checks a catalogue's size, and returns its entries.
pub fn read(bytes: &[u8]) -> Result<Entries<'_>, SubPagesError> {
    if bytes.len() > MAX_BYTES {
        return Err(SubPagesError::TooLarge);
    }
    Ok(Entries {
        lines: bytes.split(is_line_end as fn(&u8) -> bool).enumerate(),
        seen: [0; SUBCODE_VALUES / WORD_BITS],
        kept: 0,
    })
}

/// The thirteen bits of a sub-code packed into a number below
/// [`SUBCODE_VALUES`]: S4 S3 S2 S1 with no gap.
fn packed(subcode: SubCode) -> usize {
    let value = usize::from(subcode.value());
    ((value >> S3_SHIFT) << S1_S2_BITS) | (value & S1_S2_MASK)
}

/// A sub-code as a catalogue and a sub-page's file name write it: four
/// capital hexadecimal digits, S4 first, with no bit outside the thirteen
/// a sub-code has (clause 9.3.1.2). Every one of them is accepted
/// (REQ-NUM-002).
#[must_use]
pub fn parse_subcode(field: &[u8]) -> Option<SubCode> {
    if field.len() != SUBCODE_DIGITS || !field.iter().all(|&byte| crate::channel::is_capital_hex(byte)) {
        return None;
    }
    let value = crate::bytes::hex_number(field)?;
    limits::is_sub_code(value).then_some(SubCode::new(value))
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

impl Entries<'_> {
    /// Marks a sub-code as listed; false when it already was.
    fn first_time(&mut self, subcode: SubCode) -> bool {
        let index = packed(subcode);
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
            if raw.is_empty() {
                continue;
            }
            let fail = |fault| Some(Err(LineFault { line, fault }));
            if !raw.iter().all(|&byte| byte == TAB || (FIRST_PRINTABLE..=LAST_PRINTABLE).contains(&byte)) {
                return fail(Fault::Character);
            }
            // Printable ASCII is UTF-8.
            let Ok(text) = core::str::from_utf8(raw) else { return fail(Fault::Character) };
            let mut fields = text.split(char::from(TAB));
            let parts = [fields.next(), fields.next(), fields.next(), fields.next()];
            let entry = match parts {
                [Some(code), Some(validator), name, None] => {
                    parse_subcode(code.as_bytes()).ok_or(Fault::SubCode).and_then(|subcode| {
                        let validator = Validator::parse(validator.as_bytes()).ok_or(Fault::Validator)?;
                        Ok(Entry { subcode, validator, title: name.map(title).transpose()? })
                    })
                }
                _ => Err(Fault::Fields),
            };
            let entry = match entry {
                Ok(entry) if !self.first_time(entry.subcode) => Err(Fault::Duplicate),
                Ok(_) if self.kept >= MAX_ENTRIES => Err(Fault::TooMany),
                other => other,
            };
            if entry.is_ok() {
                self.kept = self.kept.saturating_add(1);
            }
            return Some(entry.map_err(|fault| LineFault { line, fault }));
        }
    }
}

/// Writes one entry, ended by a line feed. The title is made a valid one:
/// each byte outside printable ASCII becomes a space, runs of spaces
/// become one, the ends are trimmed and the rest cut to [`MAX_TITLE`]; a
/// title left empty is not written. Nothing is written when the entry
/// does not fit.
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
    writer.byte(b'\n')
}

#[cfg(feature = "std")]
mod layout;
#[cfg(feature = "std")]
pub use layout::{ChannelFiles, LayoutError, SubPageFile, page_files};

#[cfg(test)]
mod tests;
