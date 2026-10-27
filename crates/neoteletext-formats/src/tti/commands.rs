//! The line dispatcher: each line to its command, the page commands, the
//! file-level commands (`PN`, `DE`, `DS`, `LG`), `OL` rows, and the
//! diagnostics and issues every command reports.

use neoteletext_core::consts::{COLUMNS, PACKET_ENHANCEMENT, PACKET_MAGAZINE_ENHANCEMENT, STORED_ROWS};
use neoteletext_core::types::SubCode;

use crate::consts::TTI_COMMAND_BYTES;
use crate::item::{Capacity, Diagnostic, DiagnosticKind, Item};
use crate::page::{INPUT_MODE_BITS, IssueKind, ModesLine, PageFunction, Warning};

use super::fields::{Args, CODING_LAST, REGION_LAST, decimal, hex, page_address, set_value};
use super::row_text::decode_text;
use super::{PAGE_COMMANDS, TtiReader};

impl TtiReader {
    pub(super) fn diagnostic(&self, kind: DiagnosticKind, out: &mut dyn FnMut(Item<'_>)) {
        out(Item::Diagnostic(Diagnostic::at_line(self.lines.offset(), self.lines.number(), kind)));
    }

    pub(super) fn issue(&mut self, kind: IssueKind, out: &mut dyn FnMut(Item<'_>)) {
        self.issue_at(kind, self.lines.number(), out);
    }

    pub(super) fn issue_at(&mut self, kind: IssueKind, line: u32, out: &mut dyn FnMut(Item<'_>)) {
        self.diagnostic(DiagnosticKind::Page(kind), out);
        if !self.page.add_issue(kind, line) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Issues }, out);
        }
    }

    pub(super) fn end_line(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        // A copy, so that handling the line can borrow the reader.
        let lines = self.lines;
        let line = lines.line();
        if line.overflow {
            self.diagnostic(DiagnosticKind::LineTooLong, out);
        }
        let mut text = line.text;
        // Every carriage return before the line feed is line ending
        // (REQ-INP-013): a value kept with one could not be written back
        // unchanged, since the writer's line feed would make it CR LF.
        while let Some(stripped) = text.strip_suffix(b"\r") {
            text = stripped;
        }
        self.command(text, out);
        self.lines.clear();
    }

    fn command(&mut self, line: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        if line.trim_ascii().is_empty() {
            return;
        }
        let Some(name) = line.first_chunk::<TTI_COMMAND_BYTES>().map(|name| name.map(|b| b.to_ascii_uppercase()))
        else {
            self.diagnostic(DiagnosticKind::UnknownCommand { command: [line.first().copied().unwrap_or(0), 0] }, out);
            return;
        };
        let args = line.iter().position(|&byte| byte == b',').and_then(|index| line.get(index.saturating_add(1)..));
        let args = args.unwrap_or(&[]);
        match &name {
            b"PN" => self.page_number(args, out),
            b"DE" | b"DS" => self.description(name, args, out),
            b"CT" => {}
            b"LG" => self.log_line(args, out),
            _ if !PAGE_COMMANDS.contains(&name) => {
                self.diagnostic(DiagnosticKind::UnknownCommand { command: name }, out);
            }
            _ => {
                if !self.started && !self.has_content {
                    // Content before any PN: the page gets the default address.
                    self.diagnostic(DiagnosticKind::MissingPageNumber, out);
                    self.page.line = self.lines.number();
                }
                self.has_content = true;
                self.page_command(name, args, out);
            }
        }
    }

    fn page_command(&mut self, name: [u8; 2], args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let bad = DiagnosticKind::BadArgument { command: name };
        match &name {
            b"SC" => match hex(args).and_then(|value| u16::try_from(value).ok()) {
                Some(value) => self.page.subcode = SubCode::new(value),
                None => self.diagnostic(bad, out),
            },
            b"PS" => match hex(args).and_then(|value| u16::try_from(value).ok()) {
                Some(value) => self.page.status = value,
                None => self.diagnostic(bad, out),
            },
            b"OL" => self.output_line(args, out),
            b"FL" => self.fastext_line(args, out),
            b"LN" => self.link_line(args, out),
            b"LK" => self.number_link_line(args, out),
            b"LT" => self.text_link_line(args, out),
            b"CU" => self.cursor_line(args, out),
            b"IM" => match decimal(args.trim_ascii()).filter(|&modes| modes & !INPUT_MODE_BITS == 0) {
                Some(modes) => self.page.input_modes = Some(ModesLine { modes, line: self.lines.number() }),
                None => self.issue(IssueKind::BadModesLine, out),
            },
            b"RE" => match hex(args).filter(|&value| value <= REGION_LAST) {
                Some(value) => self.page.region = u8::try_from(value).ok(),
                None => self.diagnostic(bad, out),
            },
            b"PF" => {
                let mut fields = Args::new(args);
                let function = hex(fields.next()).filter(|&value| value <= REGION_LAST);
                let coding = hex(fields.next()).filter(|&value| value <= CODING_LAST);
                match (function, coding) {
                    // A function that a packet X/28 carries wins (below).
                    (Some(function), Some(coding)) => {
                        if self.function_designation.is_none() {
                            self.page.function = Some(PageFunction { function: function as u8, coding: coding as u8 });
                        }
                    }
                    _ => self.diagnostic(bad, out),
                }
            }
            b"CK" => {
                let mut fields = Args::new(args);
                let marker = fields.next();
                match Warning::from_name(fields.next().trim_ascii()) {
                    Some(warning) if marker.trim_ascii().eq_ignore_ascii_case(b"considered") => {
                        self.page.considered |= warning.bit();
                    }
                    _ => self.diagnostic(bad, out),
                }
            }
            // `TF` named a page's font, withdrawn on 23 October 2026
            // (decision E.111): reported, and nothing of it kept.
            b"TF" => self.issue(IssueKind::WithdrawnCommand, out),
            _ => self.diagnostic(DiagnosticKind::UnknownCommand { command: name }, out),
        }
    }

    /// `LG,level,text`: a log line for the channel's author (REQ-INP-026),
    /// passed on at once, at most `MAX_LOG_LINES_PER_PAGE` from a `PN`.
    fn log_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        if self.log_lines >= crate::log::MAX_LOG_LINES_PER_PAGE {
            self.issue(IssueKind::TooManyLogLines, out);
            return;
        }
        self.log_lines = self.log_lines.saturating_add(1);
        match crate::log::read(args) {
            Ok(line) => {
                let page = self.started.then_some(self.page.address);
                out(Item::Log { line, page, line_number: self.lines.number() });
            }
            Err(why) => self.issue(IssueKind::BadLogLine(why), out),
        }
    }

    fn page_number(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        self.emit(out);
        self.page.clear();
        self.log_lines = 0;
        self.function_designation = None;
        self.page.description = self.file_description;
        self.page.source = self.file_source;
        self.page.line = self.lines.number();
        self.started = true;
        self.has_content = false;
        self.last_row = None;
        let digits = args.trim_ascii();
        let address = digits.get(..3).and_then(page_address);
        // `ss` is a decimal sub-page number (vbit2).
        let subpage = match digits.get(3..) {
            None | Some([]) => Some(0),
            Some(rest) => decimal(rest).and_then(|value| u8::try_from(value).ok()),
        };
        match (address, subpage) {
            (Some(address), Some(subpage)) => {
                self.page.address = address;
                self.page.subpage = subpage;
            }
            _ => self.diagnostic(DiagnosticKind::BadArgument { command: *b"PN" }, out),
        }
    }

    fn description(&mut self, name: [u8; 2], args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let target = match (&name, self.started) {
            (b"DE", false) => &mut self.file_description,
            (b"DE", true) => &mut self.page.description,
            (_, false) => &mut self.file_source,
            (_, true) => &mut self.page.source,
        };
        if !set_value(target, args) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Text }, out);
        }
    }

    fn output_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let row = decimal(fields.next()).and_then(|row| u8::try_from(row).ok());
        let text = fields.rest();
        match row {
            Some(row) if usize::from(row) < STORED_ROWS => {
                let decoded = decode_text(text, true, COLUMNS);
                if decoded.overflow {
                    self.diagnostic(DiagnosticKind::RowTooLong { row }, out);
                }
                if decoded.bad_escape {
                    self.diagnostic(DiagnosticKind::BadEscape, out);
                }
                let count = decoded.count.min(COLUMNS);
                self.page.set_row(usize::from(row), decoded.codes.get(..count).unwrap_or(&[]));
                self.last_row = Some(row);
            }
            Some(number @ PACKET_ENHANCEMENT..=PACKET_MAGAZINE_ENHANCEMENT) => {
                self.packed_line(number, text, out);
            }
            _ => self.diagnostic(DiagnosticKind::BadArgument { command: *b"OL" }, out),
        }
    }

    pub(super) fn emit(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if self.started || self.has_content {
            out(Item::Page(&self.page));
        }
    }
}
