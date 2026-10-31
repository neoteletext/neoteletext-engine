//! TTI page files (REQ-INP-013): the MRG Systems format with the vbit2
//! extensions and the TTI Link Extension (Appendix E), read line by line.

use crate::consts::TTI_COMMAND_BYTES;
use crate::item::{Item, Reader};
use crate::lines::Lines;
use crate::page::{LINK_TARGET_BYTES, PageFile};
use crate::text::Text;

mod commands;
mod fields;
mod links;
mod packed;
mod row_text;
mod targets;

pub use row_text::{DecodedText, decode_text};
/// A link target of Appendix E, read as an `LN`, `LK` or `LT` line's is:
/// also how a LINKS record's targets are read (`INPUT.md` section 7.8).
pub use targets::{link_destination, parse_target};

/// The longest line read; longer lines are cut and reported. An `OL` line
/// in the escape form needs at most 6 + 80 bytes, and a link line room for
/// its target (REQ-1661) beside its other fields, as a line always had.
pub const LINE_BYTES: usize = LINK_TARGET_BYTES + 512;

/// The commands that belong to a page (REQ-INP-013, Appendix E, `CK` of
/// REQ-CLI-006, and `TF`, withdrawn by decision E.111). The lines of the
/// withdrawn Form Extension (decision E.126) are not among them, so they
/// are unknown commands, reported and ignored as every other is.
const PAGE_COMMANDS: [[u8; TTI_COMMAND_BYTES]; 13] =
    [*b"SC", *b"PS", *b"OL", *b"FL", *b"LN", *b"LK", *b"LT", *b"RE", *b"PF", *b"CK", *b"TF", *b"CU", *b"IM"];

/// The streaming TTI reader. It reports each sub-page as an
/// [`Item::Page`] when the next `PN` line or the end of input completes it.
#[derive(Clone, Debug)]
pub struct TtiReader {
    lines: Lines<LINE_BYTES>,
    page: PageFile,
    started: bool,
    /// `LG` lines since the last `PN` (REQ-INP-026).
    log_lines: usize,
    has_content: bool,
    file_description: Text<{ crate::page::DESCRIPTION_BYTES }>,
    file_source: Text<{ crate::page::DESCRIPTION_BYTES }>,
    last_row: Option<u8>,
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
            lines: Lines::new(),
            page: PageFile::new(),
            started: false,
            log_lines: 0,
            has_content: false,
            file_description: Text::new(),
            file_source: Text::new(),
            last_row: None,
            function_designation: None,
        }
    }
}

impl Reader for TtiReader {
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
        self.emit(out);
        self.started = false;
        self.has_content = false;
        self.log_lines = 0;
    }
}

#[cfg(test)]
pub(crate) mod tests;
