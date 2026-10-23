//! TTI page files (REQ-INP-013): the MRG Systems format with the vbit2
//! extensions, the TTI Link Extension (Appendix E) and the Form Extension
//! (Appendix C), read line by line.

use crate::consts::TTI_COMMAND_BYTES;
use crate::item::{Item, Reader, offset_of};
use crate::page::PageFile;
use crate::text::Text;

mod commands;
mod fields;
mod form;
mod links;
mod packed;
mod row_text;
mod targets;

pub use row_text::{DecodedText, decode_text};

/// The longest line read; longer lines are cut and reported. An `OL` line
/// in the escape form needs at most 6 + 80 bytes.
pub const LINE_BYTES: usize = 512;

/// The commands that belong to a page (REQ-INP-013, Appendices C and E,
/// `CK` of REQ-CLI-006, and `TF`, withdrawn by decision E.111). `OP` is `OPTION`,
/// recognised by its first two letters.
const PAGE_COMMANDS: [[u8; TTI_COMMAND_BYTES]; 19] = [
    *b"SC", *b"PS", *b"OL", *b"FL", *b"LN", *b"LK", *b"LT", *b"RE", *b"PF", *b"CK", *b"FD", *b"OP", *b"FS", *b"FP",
    *b"FT", *b"FH", *b"TF", *b"CU", *b"IM",
];

/// The streaming TTI reader. It reports each sub-page as an
/// [`Item::Page`] when the next `PN` line or the end of input completes it.
#[derive(Clone, Debug)]
pub struct TtiReader {
    line: [u8; LINE_BYTES],
    len: usize,
    overflow: bool,
    line_number: u32,
    line_offset: u64,
    position: u64,
    page: PageFile,
    started: bool,
    /// `LG` lines since the last `PN` (REQ-INP-026).
    log_lines: usize,
    has_content: bool,
    file_description: Text<{ crate::page::DESCRIPTION_BYTES }>,
    file_source: Text<{ crate::page::DESCRIPTION_BYTES }>,
    last_row: Option<u8>,
    last_choice_field: Option<u8>,
    /// The designation code of the packet X/28 that set the page function,
    /// if one did.
    function_designation: Option<u8>,
}

impl Default for TtiReader {
    fn default() -> Self {
        Self::new()
    }
}

impl TtiReader {
    /// A reader.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            line: [0; LINE_BYTES],
            len: 0,
            overflow: false,
            line_number: 0,
            line_offset: 0,
            position: 0,
            page: PageFile::new(),
            started: false,
            log_lines: 0,
            has_content: false,
            file_description: Text::new(),
            file_source: Text::new(),
            last_row: None,
            last_choice_field: None,
            function_designation: None,
        }
    }
}

impl Reader for TtiReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let base = self.position;
        self.position = offset_of(base, bytes.len());
        for (index, &byte) in bytes.iter().enumerate() {
            if self.len == 0 && !self.overflow {
                self.line_offset = offset_of(base, index);
            }
            if byte == b'\n' {
                self.end_line(out);
            } else if let Some(slot) = self.line.get_mut(self.len) {
                *slot = byte;
                self.len = self.len.saturating_add(1);
            } else {
                self.overflow = true;
            }
        }
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if self.len > 0 || self.overflow {
            self.end_line(out);
        }
        self.emit(out);
        self.started = false;
        self.has_content = false;
        self.log_lines = 0;
    }
}

#[cfg(test)]
pub(crate) mod tests;
