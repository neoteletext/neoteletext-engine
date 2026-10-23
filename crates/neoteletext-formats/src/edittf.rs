//! Page addresses of the edit.tf and zxnet editors (REQ-INP-016): a
//! metadata digit, a colon and base64url data carrying 25 rows of 40 7-bit
//! codes, with optional `key=value` fields (`research/notes-formats.md`
//! section 7). Each line of input is one address; text before a `#` (the
//! rest of a web address) is ignored.

use neoteletext_core::consts::MODE_COLUMN_RESERVED_1;
use neoteletext_core::packet::Triplet;

use crate::consts::{BASE64_BITS, EDITTF_TRIPLET_CHARACTERS};
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader};
use crate::lines::Lines;
use crate::page::PageFile;

mod base64;
mod parse;
mod write;

pub use parse::parse;
pub use write::write;

/// The longest address line read: the data, every known field at its
/// largest and room for a web address in front.
pub const LINE_BYTES: usize = 4096;
/// Mask of one base64 character's bits.
const BASE64_MASK: u32 = (1 << BASE64_BITS) - 1;
/// Mask of the edit.tf metadata character-set bits.
const METADATA_SET_MASK: u8 = 0x07;
/// The X270 flag character meaning "display row 24".
const ROW_24_FLAG: u8 = b'F';
/// The X270 flag character meaning "do not display row 24".
const NO_ROW_24_FLAG: u8 = b'0';
/// Shifts of the three base64 characters of an `X26` triplet.
const TRIPLET_SHIFTS: [u32; EDITTF_TRIPLET_CHARACTERS] = [2 * BASE64_BITS as u32, BASE64_BITS as u32, 0];
/// A triplet that does nothing, written for designation codes a page lacks
/// so that the `X26` sequence keeps its packet numbering: a reserved column
/// mode (clause 12.3.4) at column 0.
const FILLER: Triplet = Triplet { address: 0, mode: MODE_COLUMN_RESERVED_1, data: 0 };

/// A reader of edit.tf addresses, one per line.
#[derive(Clone, Debug)]
pub struct EditTfReader {
    lines: Lines<LINE_BYTES>,
    page: PageFile,
}

impl Default for EditTfReader {
    fn default() -> Self {
        Self::new()
    }
}

impl EditTfReader {
    /// A reader.
    #[must_use]
    pub const fn new() -> Self {
        Self { lines: Lines::new(), page: PageFile::new() }
    }

    fn end_line(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        // A copy, so that parsing the line can borrow the reader's page.
        let lines = self.lines;
        let line = lines.line();
        let text = line.text;
        let diagnostic = |kind| Item::Diagnostic(Diagnostic::at_line(lines.offset(), lines.number(), kind));
        if line.overflow {
            out(diagnostic(DiagnosticKind::LineTooLong));
        } else if !text.trim_ascii().is_empty() {
            match parse(text, &mut self.page) {
                Ok(()) => out(Item::Page(&self.page)),
                Err(kind @ (DiagnosticKind::BadArgument { .. } | DiagnosticKind::UnmappedLanguage { .. })) => {
                    out(diagnostic(kind));
                    out(Item::Page(&self.page));
                }
                Err(kind) => out(diagnostic(kind)),
            }
        }
        self.lines.clear();
    }
}

impl Reader for EditTfReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut rest = bytes;
        while let Some(after) = self.lines.take(rest) {
            self.end_line(out);
            rest = after;
        }
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if self.lines.finish() {
            self.end_line(out);
        }
    }
}

#[cfg(test)]
mod tests;
