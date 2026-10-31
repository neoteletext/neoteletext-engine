//! The link lines: `FL` (X/27/0), `LN` (Appendix E), `LK` and `LT` (the
//! web-native concept section 5), and the `CU` cursor line (REQ-INT-006).

use neoteletext_core::consts::COLUMNS;
use neoteletext_core::types::{PageAddress, SubCode};

use crate::consts::{TTI_FL_LINKS, TTI_FL_NO_LINK, TTI_FL_NULL_LINK};
use crate::item::{Capacity, DiagnosticKind, Item};
use crate::page::{CursorLine, FastextLink, IssueKind, LinkDestination, LinkKey, LinkLine, NumberLink, TextLink};
use crate::text::Text;

use super::TtiReader;
use super::fields::{Args, decimal, hex, page_address, set_value};
use super::targets::link_destination;

/// The last row a text link may be on: rows 1 to 23 are the page's text.
const TEXT_LINK_LAST_ROW: u8 = 23;

impl TtiReader {
    pub(super) fn fastext_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let mut links = [None; TTI_FL_LINKS];
        for slot in &mut links {
            let Some(value) = hex(fields.next()).and_then(|value| u16::try_from(value).ok()) else {
                self.diagnostic(DiagnosticKind::BadArgument { command: *b"FL" }, out);
                return;
            };
            if value == TTI_FL_NO_LINK || value == TTI_FL_NULL_LINK {
                continue;
            }
            match PageAddress::from_mpp(value) {
                Some(address) => *slot = Some(FastextLink { address, subcode: SubCode::ANY }),
                None => self.diagnostic(DiagnosticKind::BadArgument { command: *b"FL" }, out),
            }
        }
        self.page.fastext = links;
        // A Fastext page shows its row 24 prompts.
        self.page.fastext_row_24 = true;
    }

    pub(super) fn link_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let key_text = fields.next().trim_ascii();
        let target = fields.next();
        let label = fields.rest();
        let key = match key_text {
            [letter] => LinkKey::from_letter(*letter),
            _ => None,
        };
        let (Some(key), Some(destination)) = (key, link_destination(target, &mut self.page.link_text)) else {
            self.issue(IssueKind::BadLinkTarget, out);
            return;
        };
        let LinkDestination { service, address, subcode, url } = destination;
        if self.page.links.as_slice().iter().any(|link| link.key == key) {
            self.page.links_valid = false;
            self.issue(IssueKind::RepeatedKey, out);
        } else if key.is_coloured()
            && self.page.links.as_slice().iter().filter(|link| link.key.is_coloured()).count() >= 4
        {
            self.page.links_valid = false;
            self.issue(IssueKind::TooManyLinks, out);
        }
        let mut kept = Text::new();
        let label_truncated = !set_value(&mut kept, label);
        let link =
            LinkLine { key, service, address, subcode, url, label: kept, label_truncated, line: self.lines.number() };
        if !self.page.links.push(link) {
            self.page.links_valid = false;
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Links }, out);
        }
    }

    /// `LK,mpp,target`: a page number on this page that leads elsewhere.
    pub(super) fn number_link_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let number = page_address(fields.next().trim_ascii());
        let destination = link_destination(fields.rest(), &mut self.page.link_text);
        // Typing a page number never leaves teletext (decision E.88).
        let (Some(number), Some(destination)) = (number, destination.filter(|destination| !destination.is_web()))
        else {
            self.issue(IssueKind::BadLinkTarget, out);
            return;
        };
        if self.page.number_links.as_slice().iter().any(|link| link.number == number) {
            self.issue(IssueKind::RepeatedKey, out);
            return;
        }
        if !self.page.number_links.push(NumberLink { number, destination, line: self.lines.number() }) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Links }, out);
        }
    }

    /// `CU,row,column,width,flags[,acked]` (REQ-INT-006): the page's cursor
    /// record, decimal. The last line is kept. The width, the echo width
    /// before decision E.134, keeps its place and is read as a number but
    /// not kept.
    pub(super) fn cursor_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let byte = |field: &[u8]| decimal(field).and_then(|value| u8::try_from(value).ok());
        let (row, column, width, flags) =
            (byte(fields.next()), byte(fields.next()), byte(fields.next()), byte(fields.next()));
        let rest = fields.rest().trim_ascii();
        let acked = if rest.is_empty() { Some(0) } else { decimal(rest).and_then(|value| u16::try_from(value).ok()) };
        match (row, column, width, flags, acked) {
            (Some(row), Some(column), Some(_width), Some(flags), Some(acked)) => {
                self.page.cursor = Some(CursorLine { row, column, flags, acked, line: self.lines.number() });
            }
            _ => self.issue(IssueKind::BadCursorLine, out),
        }
    }

    /// `LT,row,column,length,target`: a run of text that is a link.
    pub(super) fn text_link_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let number = |field: &[u8]| decimal(field).and_then(|value| u8::try_from(value).ok());
        let row = number(fields.next());
        let column = number(fields.next());
        let length = number(fields.next());
        let destination = link_destination(fields.rest(), &mut self.page.link_text);
        let columns = u8::try_from(COLUMNS).unwrap_or(u8::MAX);
        let fits = |row: u8, column: u8, length: u8| {
            (1..=TEXT_LINK_LAST_ROW).contains(&row)
                && length > 0
                && column.checked_add(length).is_some_and(|end| end <= columns)
        };
        let (Some(row), Some(column), Some(length), Some(destination)) = (row, column, length, destination) else {
            self.issue(IssueKind::BadLinkTarget, out);
            return;
        };
        if !fits(row, column, length) {
            self.issue(IssueKind::BadLinkTarget, out);
            return;
        }
        let link = TextLink { row, column, length, destination, line: self.lines.number() };
        if !self.page.text_links.push(link) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Links }, out);
        }
    }
}
