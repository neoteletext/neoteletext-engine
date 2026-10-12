//! TTI page files (REQ-INP-013): the MRG Systems format with the vbit2
//! extensions, the TTI Link Extension (Appendix E) and the Form Extension
//! (Appendix C), read line by line.

use neoteletext_core::consts::{
    CODE_MASK_7BIT, COLUMNS, DOUBLE_HEIGHT, ESC, NIBBLE_MASK, PACKET_ENHANCEMENT, PACKET_LINKS,
    PACKET_MAGAZINE_ENHANCEMENT, PACKET_PAGE_ENHANCEMENT, ROWS, STORED_ROWS, TTI_DOUBLE_HEIGHT_ALIAS,
    TTI_ESCAPE_OFFSET,
};
use neoteletext_core::packet::{Decoded, PageEnhancement, PagePacket, decode};
use neoteletext_core::types::{PageAddress, SubCode};

use crate::consts::{TTI_COMMAND_BYTES, TTI_FL_LINKS, TTI_FL_NO_LINK, TTI_FL_NULL_LINK, TTI_PACKED_CHARACTERS};
use crate::item::{Capacity, Diagnostic, DiagnosticKind, Item, Reader, offset_of};
use crate::page::{
    DEFAULT_ADDRESS, EnhancementPacket, FONT_ADDRESS_BYTES, FONT_CHECKSUM_DIGITS, FastextLink, FieldKind, FormField,
    FormMethod, FormOption, IssueKind, LINK_ADDRESS_BYTES, LinkDestination, LinkKey, LinkLine, NumberLink, PageFile,
    PageFont, PageFunction, RawBody, RawPacket, SERVICE_NAME_BYTES, TextLink, Warning,
};
use crate::text::Text;

/// The longest line read; longer lines are cut and reported. An `OL` line
/// in the escape form needs at most 6 + 80 bytes.
pub const LINE_BYTES: usize = 512;
/// Characters of a packed enhancement line: the designation code and 39.
const PACKED_LINE: usize = 1 + TTI_PACKED_CHARACTERS;
/// Highest region and page function values.
const REGION_LAST: u32 = 0xF;
/// Page coding is 3 bits (Table 3).
const CODING_LAST: u32 = 0x7;

/// The commands that belong to a page (REQ-INP-013, Appendices C and E,
/// `CK` of REQ-CLI-006 and `TF` of REQ-FNT-007). `OP` is `OPTION`,
/// recognised by its first two letters.
const PAGE_COMMANDS: [[u8; TTI_COMMAND_BYTES]; 17] = [
    *b"SC", *b"PS", *b"OL", *b"FL", *b"LN", *b"LK", *b"LT", *b"RE", *b"PF", *b"CK", *b"FD", *b"OP", *b"FS", *b"FP",
    *b"FT", *b"FH", *b"TF",
];
/// What starts an address as a link target, rather than a page or a named
/// service (`docs/web-native-concept.md` section 3: "a target containing
/// `://` is an address").
const ADDRESS_MARK: &[u8] = b"://";
/// The schemes an address in a page may use.
const ADDRESS_SCHEMES: [&[u8]; 2] = [b"https://", b"http://"];
/// The last row a text link may be on: rows 1 to 23 are the page's text.
const TEXT_LINK_LAST_ROW: u8 = 23;

/// Codes decoded from the text of an `OL` line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DecodedText {
    /// The codes, 7-bit, padded with spaces for rows.
    pub codes: [u8; PACKED_LINE],
    /// Codes found.
    pub count: usize,
    /// More codes than fit.
    pub overflow: bool,
    /// An escape byte ended the line.
    pub bad_escape: bool,
}

/// Decodes the text of an `OL` line (REQ-INP-013). Control codes may be
/// written as the byte plus 0x80, or as ESC followed by the code plus 0x40.
/// With `row_text`, a raw 0x10 is Double Height, as the MRG format
/// specifies; packed enhancement lines take no such alias.
#[must_use]
pub fn decode_text(text: &[u8], row_text: bool, capacity: usize) -> DecodedText {
    let mut result = DecodedText { codes: [0; PACKED_LINE], count: 0, overflow: false, bad_escape: false };
    let capacity = capacity.min(PACKED_LINE);
    let mut bytes = text.iter();
    while let Some(&byte) = bytes.next() {
        let code = if byte == ESC {
            if let Some(&next) = bytes.next() {
                next.wrapping_sub(TTI_ESCAPE_OFFSET) & CODE_MASK_7BIT
            } else {
                result.bad_escape = true;
                break;
            }
        } else {
            let code = byte & CODE_MASK_7BIT;
            if row_text && code == TTI_DOUBLE_HEIGHT_ALIAS { DOUBLE_HEIGHT } else { code }
        };
        if result.count >= capacity {
            result.overflow = true;
            break;
        }
        if let Some(slot) = result.codes.get_mut(result.count) {
            *slot = code;
        }
        result.count = result.count.saturating_add(1);
    }
    result
}

/// Comma-separated arguments.
#[derive(Clone, Copy, Debug)]
struct Args<'a> {
    rest: Option<&'a [u8]>,
}

impl<'a> Args<'a> {
    const fn new(text: &'a [u8]) -> Self {
        Self { rest: Some(text) }
    }

    /// The next argument; empty once the arguments run out.
    fn next(&mut self) -> &'a [u8] {
        let Some(rest) = self.rest else { return &[] };
        if let Some(index) = rest.iter().position(|&byte| byte == b',') {
            self.rest = rest.get(index.saturating_add(1)..);
            rest.get(..index).unwrap_or(&[])
        } else {
            self.rest = None;
            rest
        }
    }

    /// Everything not yet taken, commas included.
    fn rest(&mut self) -> &'a [u8] {
        self.rest.take().unwrap_or(&[])
    }
}

/// Parses an unsigned number in `radix` (10 or 16), ignoring surrounding
/// spaces. `None` when empty, malformed or above `u32::MAX`.
fn parse_number(text: &[u8], radix: u32) -> Option<u32> {
    let text = text.trim_ascii();
    if text.is_empty() || text.len() > 8 {
        return None;
    }
    let mut value = 0u32;
    for &byte in text {
        let digit = char::from(byte).to_digit(radix)?;
        value = value.checked_mul(radix)?.checked_add(digit)?;
    }
    Some(value)
}

/// The font of a `TF` line's arguments, `address,sha256` (REQ-FNT-007):
/// the address is everything before the last comma, printable ASCII with
/// no spaces, and the checksum is 64 hexadecimal digits.
fn font_line(args: &[u8]) -> Option<PageFont> {
    let comma = args.iter().rposition(|&byte| byte == b',')?;
    let (address, sha256) = (args.get(..comma)?, args.get(comma.saturating_add(1)..)?);
    if address.is_empty() || address.len() > FONT_ADDRESS_BYTES || !address.iter().all(u8::is_ascii_graphic) {
        return None;
    }
    let sha256: [u8; FONT_CHECKSUM_DIGITS] = sha256.try_into().ok()?;
    if !sha256.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let (address, _) = Text::from_bytes(address);
    Some(PageFont::Named { address, sha256 })
}

fn hex(text: &[u8]) -> Option<u32> {
    parse_number(text, 16)
}

fn decimal(text: &[u8]) -> Option<u32> {
    parse_number(text, 10)
}

/// A `Y`/`N` flag; `Some(false)` for empty, `None` for anything else.
fn flag(text: &[u8]) -> Option<bool> {
    match text.trim_ascii() {
        b"Y" | b"y" => Some(true),
        b"" | b"N" | b"n" => Some(false),
        other if other.eq_ignore_ascii_case(b"selected") => Some(true),
        _ => None,
    }
}

/// A three-digit `mpp` page address.
fn page_address(text: &[u8]) -> Option<PageAddress> {
    let text = text.trim_ascii();
    if text.len() != 3 {
        return None;
    }
    PageAddress::from_mpp(u16::try_from(hex(text)?).ok()?)
}

/// An `FP` page: `mpp`, or `mppss` as in the Appendix C.7 example, whose
/// sub-page digits are not used.
fn result_page(text: &[u8]) -> Option<PageAddress> {
    let text = text.trim_ascii();
    match text.len() {
        3 => page_address(text),
        5 => text.get(..3).and_then(page_address).filter(|_| decimal(text.get(3..).unwrap_or(&[])).is_some()),
        _ => None,
    }
}

/// A four-digit sub-code.
fn subcode(text: &[u8]) -> Option<SubCode> {
    let text = text.trim_ascii();
    if text.len() != 4 {
        return None;
    }
    Some(SubCode::new(u16::try_from(hex(text)?).ok()?))
}

/// A service name: letters, digits and hyphens (Appendix E.1).
fn service_name(text: &[u8]) -> Option<Text<SERVICE_NAME_BYTES>> {
    if text.is_empty() || !text.iter().all(|&byte| byte.is_ascii_alphanumeric() || byte == b'-') {
        return None;
    }
    let (name, truncated) = Text::from_bytes(text);
    if truncated { None } else { Some(name) }
}

/// A link target: `mpp`, `mpp:ssss`, `name:mpp` or `name:mpp:ssss`
/// (Appendix E.1).
/// An address as a link target: `http` or `https`, printable ASCII
/// without spaces or commas, and short enough to keep.
fn link_address(text: &[u8]) -> Option<Text<LINK_ADDRESS_BYTES>> {
    let lower_scheme = |scheme: &[u8]| text.get(..scheme.len()).is_some_and(|start| start.eq_ignore_ascii_case(scheme));
    let valid = ADDRESS_SCHEMES.iter().any(|scheme| lower_scheme(scheme))
        && text.iter().all(|&byte| byte.is_ascii_graphic() && byte != b',');
    let (url, truncated) = Text::from_bytes(text);
    (valid && !truncated).then_some(url)
}

/// A link target of Appendix E, or an address (concept section 3).
fn link_destination(text: &[u8]) -> Option<LinkDestination> {
    let text = text.trim_ascii();
    if text.windows(ADDRESS_MARK.len()).any(|window| window == ADDRESS_MARK) {
        let url = link_address(text)?;
        return Some(LinkDestination { service: Text::new(), address: DEFAULT_ADDRESS, subcode: None, url });
    }
    let (service, address, subcode) = link_target(text)?;
    Some(LinkDestination { service, address, subcode, url: Text::new() })
}

fn link_target(text: &[u8]) -> Option<(Text<SERVICE_NAME_BYTES>, PageAddress, Option<SubCode>)> {
    let mut parts = text.trim_ascii().split(|&byte| byte == b':');
    let first = parts.next()?;
    let second = parts.next();
    let third = parts.next();
    if parts.next().is_some() {
        return None;
    }
    match (second, third) {
        (None, _) => Some((Text::new(), page_address(first)?, None)),
        (Some(second), None) => {
            if let (Some(address), Some(sub)) = (page_address(first), subcode(second)) {
                Some((Text::new(), address, Some(sub)))
            } else {
                Some((service_name(first)?, page_address(second)?, None))
            }
        }
        (Some(second), Some(third)) => Some((service_name(first)?, page_address(second)?, Some(subcode(third)?))),
    }
}

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
            has_content: false,
            file_description: Text::new(),
            file_source: Text::new(),
            last_row: None,
            last_choice_field: None,
            function_designation: None,
        }
    }

    fn diagnostic(&self, kind: DiagnosticKind, out: &mut dyn FnMut(Item<'_>)) {
        out(Item::Diagnostic(Diagnostic::at_line(self.line_offset, self.line_number, kind)));
    }

    fn issue(&mut self, kind: IssueKind, out: &mut dyn FnMut(Item<'_>)) {
        self.issue_at(kind, self.line_number, out);
    }

    fn issue_at(&mut self, kind: IssueKind, line: u32, out: &mut dyn FnMut(Item<'_>)) {
        self.diagnostic(DiagnosticKind::Page(kind), out);
        if !self.page.add_issue(kind, line) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Issues }, out);
        }
    }

    fn end_line(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.line_number = self.line_number.saturating_add(1);
        if self.overflow {
            self.diagnostic(DiagnosticKind::LineTooLong, out);
        }
        let line = self.line;
        let mut text = line.get(..self.len).unwrap_or(&[]);
        if let Some(stripped) = text.strip_suffix(b"\r") {
            text = stripped;
        }
        self.command(text, out);
        self.len = 0;
        self.overflow = false;
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
            _ if !PAGE_COMMANDS.contains(&name) => {
                self.diagnostic(DiagnosticKind::UnknownCommand { command: name }, out);
            }
            _ => {
                if !self.started && !self.has_content {
                    // Content before any PN: the page gets the default address.
                    self.diagnostic(DiagnosticKind::MissingPageNumber, out);
                    self.page.line = self.line_number;
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
            b"FD" => self.field_definition(args, out),
            b"OP" => self.option_line(args, out),
            b"FS" => self.form_submission(args, out),
            b"FP" => {
                let mut fields = Args::new(args);
                match (result_page(fields.next()), result_page(fields.next())) {
                    (Some(success), Some(error)) => self.page.form.result_pages = Some((success, error)),
                    _ => self.diagnostic(bad, out),
                }
            }
            b"FT" => {
                if !self.page.form.token.set(args) {
                    self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Text }, out);
                }
            }
            b"FH" => self.issue(IssueKind::WithdrawnCommand, out),
            b"TF" => self.font_line(args, out),
            _ => self.diagnostic(DiagnosticKind::UnknownCommand { command: name }, out),
        }
    }

    /// `TF,address,sha256` (REQ-FNT-007): the page's font. A second line or
    /// a malformed one makes the page's font null, and the page is still
    /// read.
    fn font_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        if let (PageFont::None, Some(font)) = (self.page.font, font_line(args)) {
            self.page.font = font;
        } else {
            self.page.font = PageFont::Null;
            self.issue(IssueKind::BadFontLine, out);
        }
    }

    fn page_number(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        self.emit(out);
        self.page.clear();
        self.function_designation = None;
        self.page.description = self.file_description;
        self.page.source = self.file_source;
        self.page.line = self.line_number;
        self.started = true;
        self.has_content = false;
        self.last_row = None;
        self.last_choice_field = None;
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
        if !target.set(args) {
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

    fn packed_line(&mut self, number: u8, text: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let decoded = decode_text(text, false, PACKED_LINE);
        // REQ-INP-023: exactly the designation code and 39 characters.
        if decoded.count != PACKED_LINE || decoded.overflow || decoded.bad_escape {
            self.diagnostic(DiagnosticKind::BadArgument { command: *b"OL" }, out);
        }
        let designation = decoded.codes.first().copied().unwrap_or(0) & NIBBLE_MASK;
        let values = decoded.codes.get(1..).unwrap_or(&[]);
        let magazine = self.page.address.magazine;
        if number == PACKET_ENHANCEMENT {
            let packet = EnhancementPacket { designation, triplets: crate::encode::unpack_triplets(values) };
            let packets = self.page.enhancements.as_mut_slice();
            if let Some(existing) = packets.iter_mut().find(|packet| packet.designation == designation) {
                *existing = packet;
            } else if !self.page.enhancements.push(packet) {
                self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Enhancements }, out);
            }
            return;
        }
        // Designation codes 0 to 3 of X/27 carry Hamming 8/4 nibbles, one per
        // character (vbit2).
        let nibbles = number == PACKET_LINKS && designation <= 3;
        let body = if nibbles {
            let mut values_4 = [0; TTI_PACKED_CHARACTERS];
            for (slot, &value) in values_4.iter_mut().zip(values) {
                *slot = value & NIBBLE_MASK;
            }
            RawBody::Nibbles(values_4)
        } else {
            RawBody::Triplets(crate::encode::unpack_triplets(values))
        };
        let raw = RawPacket { number, designation, body };
        let packet = crate::encode::raw_packet(magazine, &raw);
        if number == PACKET_LINKS && designation == 0 {
            if let Ok(Decoded::Links(links)) = decode(&packet) {
                for (slot, link) in self.page.fastext.iter_mut().zip(links.links) {
                    *slot = link.map(|link| FastextLink {
                        address: PageAddress { magazine: link.magazine, page: link.page },
                        subcode: link.subcode,
                    });
                }
                self.page.fastext_row_24 = links.display_row_24;
            }
            return;
        }
        // REQ-INP-023: X/28/0, X/28/3 and X/28/4 also set the page function
        // and coding, since they are what a decoder receives. They win over a
        // `PF` line, and the lowest designation code wins among them, before
        // or after in the file alike, so that writing a page read from a
        // file and reading it back gives the same page.
        if number == PACKET_PAGE_ENHANCEMENT
            && self.function_designation.is_none_or(|earlier| designation <= earlier)
            && let Ok(Decoded::PageData { packet, .. }) = decode(&packet)
        {
            let format = match packet {
                PagePacket::Format1(PageEnhancement { format, .. })
                | PagePacket::Level35(PageEnhancement { format, .. }) => format,
                PagePacket::DrcsModes(modes) => modes.format,
                PagePacket::DrcsColours(_) => None,
            };
            if let Some(format) = format {
                self.page.function = Some(PageFunction { function: format.function, coding: format.coding });
                self.function_designation = Some(designation);
            }
        }
        let packets = self.page.raw_packets.as_mut_slice();
        if let Some(existing) =
            packets.iter_mut().find(|packet| packet.number == number && packet.designation == designation)
        {
            *existing = raw;
        } else if !self.page.raw_packets.push(raw) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::RawPackets }, out);
        }
    }

    fn fastext_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
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

    fn link_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let key_text = fields.next().trim_ascii();
        let target = fields.next();
        let label = fields.rest();
        let key = match key_text {
            [letter] => LinkKey::from_letter(*letter),
            _ => None,
        };
        let (Some(key), Some(destination)) = (key, link_destination(target)) else {
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
        let (label, label_truncated) = Text::from_bytes(label);
        let link = LinkLine { key, service, address, subcode, url, label, label_truncated, line: self.line_number };
        if !self.page.links.push(link) {
            self.page.links_valid = false;
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Links }, out);
        }
    }

    /// `LK,mpp,target`: a page number on this page that leads elsewhere.
    fn number_link_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let number = page_address(fields.next().trim_ascii());
        let destination = link_destination(fields.rest());
        let (Some(number), Some(destination)) = (number, destination) else {
            self.issue(IssueKind::BadLinkTarget, out);
            return;
        };
        if self.page.number_links.as_slice().iter().any(|link| link.number == number) {
            self.issue(IssueKind::RepeatedKey, out);
            return;
        }
        if !self.page.number_links.push(NumberLink { number, destination, line: self.line_number }) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Links }, out);
        }
    }

    /// `LT,row,column,length,target`: a run of text that is a link.
    fn text_link_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let number = |field: &[u8]| decimal(field).and_then(|value| u8::try_from(value).ok());
        let row = number(fields.next());
        let column = number(fields.next());
        let length = number(fields.next());
        let destination = link_destination(fields.rest());
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
        let link = TextLink { row, column, length, destination, line: self.line_number };
        if !self.page.text_links.push(link) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Links }, out);
        }
    }

    fn set_text<const N: usize>(&self, target: &mut Text<N>, value: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        if !target.set(value) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Text }, out);
        }
    }

    fn field_definition(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let Some(kind) = FieldKind::from_keyword(fields.next().trim_ascii()) else {
            self.issue(IssueKind::BadField, out);
            return;
        };
        let name = fields.next();
        let column = decimal(fields.next()).and_then(|value| u8::try_from(value).ok());
        let width = decimal(fields.next()).and_then(|value| u8::try_from(value).ok());
        let columns = u8::try_from(COLUMNS).unwrap_or(u8::MAX);
        let (Some(column), Some(width)) = (column, width) else {
            // Version 2.0 put type parameters where version 3 puts the
            // column and width (Appendix C.6).
            self.issue(IssueKind::FormNeedsConversion, out);
            return;
        };
        if column >= columns || width == 0 || width > columns {
            self.issue(IssueKind::FormNeedsConversion, out);
            return;
        }
        let Some(row) = self.last_row else {
            self.issue(IssueKind::FieldWithoutRow, out);
            return;
        };
        let mut field = FormField::new(kind);
        self.set_text(&mut field.name, name, out);
        field.row = row;
        field.column = column;
        field.width = width;
        field.line = self.line_number;
        let readable = self.field_parameters(&mut field, &mut fields, out);
        if !readable {
            self.issue(IssueKind::BadField, out);
        }
        let rows = u8::try_from(ROWS).unwrap_or(u8::MAX);
        if row == 0 || column.saturating_add(width) > columns || row.saturating_add(field.rows) > rows {
            self.issue(IssueKind::FieldOutsideRow, out);
        }
        let index = u8::try_from(self.page.form.fields.len()).ok();
        if !self.page.form.fields.push(field) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Fields }, out);
            return;
        }
        if kind.has_options() {
            self.last_choice_field = index;
        }
    }

    /// Reads the parameters after `column,width` (Appendix C.3). Returns
    /// false when one could not be read.
    fn field_parameters(&self, field: &mut FormField, fields: &mut Args<'_>, out: &mut dyn FnMut(Item<'_>)) -> bool {
        let mut ok = true;
        let max_length = |text: &[u8]| -> (Option<u16>, bool) {
            if text.trim_ascii().is_empty() {
                return (None, true);
            }
            let value = decimal(text).and_then(|value| u16::try_from(value).ok());
            (value, value.is_some())
        };
        match field.kind {
            FieldKind::Text | FieldKind::Telephone | FieldKind::Email => {
                let (length, length_ok) = max_length(fields.next());
                field.max_length = length;
                ok &= length_ok;
                let required = flag(fields.next());
                if field.kind != FieldKind::Email {
                    self.set_text(&mut field.pattern, fields.next(), out);
                }
                self.set_text(&mut field.default, fields.rest(), out);
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
            }
            FieldKind::Number | FieldKind::Date | FieldKind::Time => {
                self.set_text(&mut field.minimum, fields.next(), out);
                self.set_text(&mut field.maximum, fields.next(), out);
                if field.kind == FieldKind::Number {
                    self.set_text(&mut field.step, fields.next(), out);
                }
                let required = flag(fields.next());
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
                self.set_text(&mut field.default, fields.rest(), out);
            }
            FieldKind::Select | FieldKind::Radio => {
                let required = flag(fields.next());
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
            }
            FieldKind::Checkbox => {
                self.set_text(&mut field.value, fields.next(), out);
                let required = flag(fields.next());
                let checked = flag(fields.next());
                field.required = required.unwrap_or(false);
                field.checked = checked.unwrap_or(false);
                ok &= required.is_some() && checked.is_some();
            }
            FieldKind::TextArea => {
                let rows = decimal(fields.next()).and_then(|value| u8::try_from(value).ok()).filter(|&rows| rows > 0);
                ok &= rows.is_some();
                field.rows = rows.unwrap_or(1);
                let (length, length_ok) = max_length(fields.next());
                field.max_length = length;
                ok &= length_ok;
                let required = flag(fields.next());
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
                self.set_text(&mut field.default, fields.rest(), out);
            }
            FieldKind::Submit => {
                self.set_text(&mut field.label, fields.next(), out);
                self.set_text(&mut field.action, fields.next(), out);
            }
            FieldKind::Reset => self.set_text(&mut field.label, fields.next(), out),
        }
        ok
    }

    fn option_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let Some(field) = self.last_choice_field else {
            self.issue(IssueKind::OptionWithoutField, out);
            return;
        };
        let mut fields = Args::new(args);
        let mut option = FormOption {
            field,
            value: Text::new(),
            display: Text::new(),
            selected: false,
            column: None,
            line: self.line_number,
        };
        self.set_text(&mut option.value, fields.next(), out);
        self.set_text(&mut option.display, fields.next(), out);
        let selected = flag(fields.next());
        let column_text = fields.next();
        let column = decimal(column_text).and_then(|value| u8::try_from(value).ok());
        let columns = u8::try_from(COLUMNS).unwrap_or(u8::MAX);
        let column_ok = column_text.trim_ascii().is_empty() || column.is_some_and(|column| column < columns);
        option.selected = selected.unwrap_or(false);
        option.column = column.filter(|&column| column < columns);
        if selected.is_none() || !column_ok {
            self.issue(IssueKind::BadField, out);
        }
        if !self.page.form.options.push(option) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Options }, out);
        }
    }

    fn form_submission(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let method = match fields.next().trim_ascii() {
            method if method.eq_ignore_ascii_case(b"POST") => Some(FormMethod::Post),
            method if method.eq_ignore_ascii_case(b"GET") => Some(FormMethod::Get),
            _ => None,
        };
        let Some(method) = method else {
            self.diagnostic(DiagnosticKind::BadArgument { command: *b"FS" }, out);
            return;
        };
        let mut address = Text::new();
        self.set_text(&mut address, fields.rest().trim_ascii(), out);
        self.page.form.submission = Some((method, address));
    }

    /// Checks the form for overlapping fields (Appendix C.2) and decides
    /// whether it may be interactive.
    fn finish_form(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        let mut cells = [0u64; ROWS];
        let mut overlap: Option<u32> = None;
        let form = &self.page.form;
        for (index, field) in form.fields.as_slice().iter().enumerate() {
            let mut overlaps = mark_cells(&mut cells, field.row, field.rows, field.column, field.width);
            if field.kind == FieldKind::Radio {
                // The first choice's marker is the field's column; the others
                // give their own (Appendix C.3).
                let options = form.options.as_slice().iter().filter(|option| usize::from(option.field) == index);
                for column in options.skip(1).filter_map(|option| option.column) {
                    overlaps |= mark_cells(&mut cells, field.row, field.rows, column, field.width);
                }
            }
            if overlaps {
                overlap = overlap.or(Some(field.line));
            }
        }
        if let Some(line) = overlap {
            self.issue_at(IssueKind::FieldOverlap, line, out);
        }
        let has_fields = !self.page.form.fields.is_empty();
        let blocked = self.page.issues.as_slice().iter().any(|issue| issue.kind.affects_form());
        self.page.form.interactive = has_fields && !blocked;
    }

    fn emit(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if self.started || self.has_content {
            self.finish_form(out);
            out(Item::Page(&self.page));
        }
    }
}

/// Marks a field's cells, clipped to the page. Returns true when any cell
/// was already marked.
fn mark_cells(cells: &mut [u64; ROWS], row: u8, rows: u8, column: u8, width: u8) -> bool {
    let mut overlapped = false;
    let end_column = usize::from(column).saturating_add(usize::from(width)).min(COLUMNS);
    let mut mask = 0u64;
    for bit in usize::from(column)..end_column {
        mask |= 1u64.checked_shl(u32::try_from(bit).unwrap_or(u32::MAX)).unwrap_or(0);
    }
    let end_row = usize::from(row).saturating_add(usize::from(rows));
    for slot in cells.iter_mut().take(end_row).skip(usize::from(row)) {
        overlapped |= *slot & mask != 0;
        *slot |= mask;
    }
    overlapped
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
    }
}

#[cfg(test)]
pub(crate) mod tests;
