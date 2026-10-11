//! Page addresses of the edit.tf and zxnet editors (REQ-INP-016): a
//! metadata digit, a colon and base64url data carrying 25 rows of 40 7-bit
//! codes, with optional `key=value` fields (`research/notes-formats.md`
//! section 7). Each line of input is one address; text before a `#` (the
//! rest of a web address) is ignored.

use neoteletext_core::consts::{
    CODE_MASK_7BIT, COLUMNS, LINK_NULL_PAGE, LINK_NULL_SUBCODE, MAGAZINE_ZERO_MEANS, MODE_COLUMN_RESERVED_1,
    NIBBLE_MASK, PACKET_ROW_25, ROWS, TRIPLET_DATA_MASK, TRIPLETS_PER_PACKET,
};
use neoteletext_core::packet::Triplet;
use neoteletext_core::types::{PageAddress, SubCode, hex_digit};

use crate::consts::{
    BASE64_BITS, BASE64URL_ALPHABET, EDITTF_CHARACTERS_24, EDITTF_CHARACTERS_25, EDITTF_CODE_BITS,
    EDITTF_LAST_NATIONAL, EDITTF_LINK_CHARACTERS, EDITTF_ROW_CHARACTERS, EDITTF_TRIPLET_CHARACTERS,
};
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader, offset_of};
use crate::page::{EnhancementPacket, FASTEXT_LINKS, FastextLink, PageFile};

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

/// The value of a base64url character.
fn base64_value(character: u8) -> Option<u32> {
    BASE64URL_ALPHABET.iter().position(|&known| known == character).and_then(|value| u32::try_from(value).ok())
}

/// Reads 7-bit codes, most significant bit first, from base64url text into
/// `codes`. Returns false on a character outside the alphabet.
fn unpack_codes(text: &[u8], codes: &mut [u8]) -> bool {
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut output = codes.iter_mut();
    for &character in text {
        let Some(value) = base64_value(character) else { return false };
        bits = (bits << BASE64_BITS) | value;
        held = held.saturating_add(u32::try_from(BASE64_BITS).unwrap_or(0));
        let code_bits = u32::try_from(EDITTF_CODE_BITS).unwrap_or(0);
        while held >= code_bits {
            held = held.saturating_sub(code_bits);
            if let Some(slot) = output.next() {
                *slot = ((bits >> held) as u8) & CODE_MASK_7BIT;
            }
        }
        bits &= 1u32.checked_shl(held).map_or(0, |bit| bit.wrapping_sub(1));
    }
    true
}

/// Writes 7-bit codes as base64url text, most significant bit first,
/// padding the last character with zeros.
fn pack_codes(codes: &[u8], out: &mut dyn FnMut(&[u8])) {
    let mut bits = 0u32;
    let mut held = 0u32;
    let base64_bits = u32::try_from(BASE64_BITS).unwrap_or(0);
    let emit = |value: u32, out: &mut dyn FnMut(&[u8])| {
        let index = usize::try_from(value & BASE64_MASK).unwrap_or(0);
        if let Some(character) = BASE64URL_ALPHABET.get(index) {
            out(core::slice::from_ref(character));
        }
    };
    for &code in codes {
        bits = (bits << EDITTF_CODE_BITS) | u32::from(code & CODE_MASK_7BIT);
        held = held.saturating_add(u32::try_from(EDITTF_CODE_BITS).unwrap_or(0));
        while held >= base64_bits {
            held = held.saturating_sub(base64_bits);
            emit(bits >> held, out);
        }
        bits &= 1u32.checked_shl(held).map_or(0, |bit| bit.wrapping_sub(1));
    }
    if held > 0 {
        emit(bits << base64_bits.saturating_sub(held), out);
    }
}

fn hex_value(text: &[u8]) -> Option<u32> {
    if text.is_empty() || text.len() > 8 {
        return None;
    }
    text.iter().try_fold(0u32, |value, &byte| Some((value << 4) | char::from(byte).to_digit(16)?))
}

/// Reads one address into `page`.
pub fn parse(text: &[u8], page: &mut PageFile) -> Result<(), DiagnosticKind> {
    page.clear();
    let text = text.trim_ascii();
    let text = text
        .iter()
        .rposition(|&byte| byte == b'#')
        .map_or(text, |index| text.get(index.saturating_add(1)..).unwrap_or(&[]));
    let mut parts = text.split(|&byte| byte == b':');
    let metadata = parts.next().unwrap_or(&[]);
    // Unreadable metadata counts as 0, as in edit.tf.
    let metadata = hex_value(metadata).and_then(|value| u8::try_from(value).ok()).unwrap_or(0);
    let data = parts.next().ok_or(DiagnosticKind::BadSignature)?;
    let rows = match data.len() {
        EDITTF_CHARACTERS_25 => ROWS,
        EDITTF_CHARACTERS_24 => ROWS.saturating_sub(1),
        other => return Err(DiagnosticKind::BadSize { size: u32::try_from(other).unwrap_or(u32::MAX) }),
    };
    let mut codes = [0u8; ROWS * COLUMNS];
    if !unpack_codes(data, &mut codes) {
        return Err(DiagnosticKind::BadBase64);
    }
    for (row, row_codes) in codes.chunks(COLUMNS).take(rows).enumerate() {
        page.set_row(row, row_codes);
    }
    let national = metadata & METADATA_SET_MASK;
    let mut status_given = false;
    let mut result = Ok(());
    for field in parts {
        let mut pair = field.splitn(2, |&byte| byte == b'=');
        let key = pair.next().unwrap_or(&[]);
        let value = pair.next().unwrap_or(&[]);
        status_given |= key == b"PS";
        if !field_value(page, key, value) {
            result = Err(DiagnosticKind::BadArgument {
                command: [key.first().copied().unwrap_or(0), key.get(1).copied().unwrap_or(0)],
            });
        }
    }
    if !status_given {
        if national <= EDITTF_LAST_NATIONAL {
            page.set_national_index(national);
        } else {
            result = result.and(Err(DiagnosticKind::UnmappedLanguage { code: national }));
        }
    }
    result
}

/// Applies one `key=value` field. Unknown keys are ignored. Returns false
/// when a known field could not be read.
fn field_value(page: &mut PageFile, key: &[u8], value: &[u8]) -> bool {
    match key {
        b"PN" => {
            let address = value
                .get(..3)
                .and_then(hex_value)
                .and_then(|mpp| u16::try_from(mpp).ok())
                .and_then(PageAddress::from_mpp);
            let subpage = match value.get(3..) {
                None | Some([]) => Some(0),
                Some(digits) => core::str::from_utf8(digits).ok().and_then(|digits| digits.parse::<u8>().ok()),
            };
            if let (Some(address), Some(subpage)) = (address, subpage) {
                page.address = address;
                page.subpage = subpage;
                true
            } else {
                false
            }
        }
        b"SC" => hex_value(value)
            .and_then(|value| u16::try_from(value).ok())
            .map(|value| page.subcode = SubCode::new(value))
            .is_some(),
        b"PS" => {
            hex_value(value).and_then(|value| u16::try_from(value).ok()).map(|value| page.status = value).is_some()
        }
        b"RE" => hex_value(value)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|&value| value <= NIBBLE_MASK)
            .map(|value| page.region = Some(value))
            .is_some(),
        b"X270" => fastext_field(page, value),
        b"X26" => enhancement_field(page, value),
        b"X25" => {
            let mut codes = [0u8; COLUMNS];
            if value.len() != EDITTF_ROW_CHARACTERS || !unpack_codes(value, &mut codes) {
                return false;
            }
            page.set_row(usize::from(PACKET_ROW_25), &codes);
            true
        }
        _ => true,
    }
}

/// `X270`: six links of three hexadecimal page digits and four sub-code
/// digits, and an optional flag character for "display row 24".
fn fastext_field(page: &mut PageFile, value: &[u8]) -> bool {
    let (links, flag) = value.split_at(value.len().min(FASTEXT_LINKS * EDITTF_LINK_CHARACTERS));
    if links.len() != FASTEXT_LINKS * EDITTF_LINK_CHARACTERS {
        return false;
    }
    for (slot, link) in page.fastext.iter_mut().zip(links.chunks(EDITTF_LINK_CHARACTERS)) {
        let (Some(mpp), Some(sub)) = (link.get(..3).and_then(hex_value), link.get(3..).and_then(hex_value)) else {
            return false;
        };
        let mpp = u16::try_from(mpp).unwrap_or(0);
        let subcode = SubCode::new(u16::try_from(sub).unwrap_or(0));
        let null = (mpp as u8) == LINK_NULL_PAGE && subcode.value() == LINK_NULL_SUBCODE;
        *slot = if null { None } else { PageAddress::from_mpp(mpp).map(|address| FastextLink { address, subcode }) };
    }
    page.fastext_row_24 = flag.first() != Some(&NO_ROW_24_FLAG);
    true
}

/// `X26`: triplets of three base64 characters, most significant first,
/// filling packets of thirteen in designation order.
fn enhancement_field(page: &mut PageFile, value: &[u8]) -> bool {
    let (triplets, rest) = value.as_chunks::<EDITTF_TRIPLET_CHARACTERS>();
    if !rest.is_empty() {
        return false;
    }
    for (index, packet_triplets) in triplets.chunks(TRIPLETS_PER_PACKET).enumerate() {
        let Ok(designation) = u8::try_from(index) else { return false };
        let mut packet = EnhancementPacket { designation, triplets: [FILLER; TRIPLETS_PER_PACKET] };
        for (slot, characters) in packet.triplets.iter_mut().zip(packet_triplets) {
            let mut bits = 0u32;
            for &character in characters {
                let Some(value) = base64_value(character) else { return false };
                bits = (bits << BASE64_BITS) | value;
            }
            *slot = Triplet::from_value(bits & TRIPLET_DATA_MASK);
        }
        if !page.enhancements.push(packet) {
            return false;
        }
    }
    true
}

fn write_hex(value: u32, digits: u32, out: &mut dyn FnMut(&[u8])) {
    for index in (0..digits).rev() {
        let digit = (value >> index.saturating_mul(4)) as u8;
        out(&[hex_digit(digit)]);
    }
}

/// Writes a page as an edit.tf address (REQ-INP-016): metadata, the 25
/// rows, and the `PN`, `SC`, `PS`, `X270`, `X26`, `X25` and `RE` fields the
/// page needs. Reading the result reproduces the page.
pub fn write(page: &PageFile, out: &mut dyn FnMut(&[u8])) {
    let national = page.control().national_index();
    let metadata = if national <= EDITTF_LAST_NATIONAL { national } else { 0 };
    out(&[hex_digit(metadata), b':']);
    let mut codes = [0u8; ROWS * COLUMNS];
    for (row, target) in codes.chunks_mut(COLUMNS).enumerate() {
        if let Some(source) = page.row(row) {
            target.copy_from_slice(source);
        }
    }
    pack_codes(&codes, out);
    out(b":PN=");
    out(&page.address.ascii());
    if page.subpage != 0 {
        let tens = page.subpage.checked_div(10).and_then(|tens| tens.checked_rem(10)).unwrap_or(0);
        let units = page.subpage.checked_rem(10).unwrap_or(0);
        out(&[b'0'.saturating_add(tens), b'0'.saturating_add(units)]);
    }
    out(b":SC=");
    write_hex(u32::from(page.subcode.value()), 4, out);
    out(b":PS=");
    write_hex(u32::from(page.status), 4, out);
    if page.fastext.iter().any(Option::is_some) {
        out(b":X270=");
        for link in page.fastext {
            let (mpp, subcode) = link
                .map_or((u16::from(MAGAZINE_ZERO_MEANS) << 8 | u16::from(LINK_NULL_PAGE), LINK_NULL_SUBCODE), |link| {
                    (link.address.mpp(), link.subcode.value())
                });
            write_hex(u32::from(mpp), 3, out);
            write_hex(u32::from(subcode), 4, out);
        }
        out(&[if page.fastext_row_24 { ROW_24_FLAG } else { NO_ROW_24_FLAG }]);
    }
    if !page.enhancements.is_empty() {
        out(b":X26=");
        let last = page.enhancements.as_slice().iter().map(|packet| packet.designation).max().unwrap_or(0);
        for designation in 0..=last {
            let packet = page.enhancements.as_slice().iter().find(|packet| packet.designation == designation);
            let triplets = packet.map_or([FILLER; TRIPLETS_PER_PACKET], |packet| packet.triplets);
            for triplet in triplets {
                let value = triplet.value();
                // Three characters, the most significant six bits first.
                for shift in TRIPLET_SHIFTS {
                    let index = usize::try_from((value >> shift) & BASE64_MASK).unwrap_or(0);
                    out(BASE64URL_ALPHABET.get(index..=index).unwrap_or(&[]));
                }
            }
        }
    }
    if page.has_row(usize::from(PACKET_ROW_25)) {
        out(b":X25=");
        pack_codes(page.row(usize::from(PACKET_ROW_25)).map_or(&[][..], |row| &row[..]), out);
    }
    if let Some(region) = page.region {
        out(b":RE=");
        out(&[hex_digit(region)]);
    }
}

/// A reader of edit.tf addresses, one per line.
#[derive(Clone, Debug)]
pub struct EditTfReader {
    line: [u8; LINE_BYTES],
    len: usize,
    overflow: bool,
    line_number: u32,
    line_offset: u64,
    position: u64,
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
        Self {
            line: [0; LINE_BYTES],
            len: 0,
            overflow: false,
            line_number: 0,
            line_offset: 0,
            position: 0,
            page: PageFile::new(),
        }
    }

    fn end_line(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.line_number = self.line_number.saturating_add(1);
        let line = self.line;
        let text = line.get(..self.len).unwrap_or(&[]);
        let diagnostic = |kind| Item::Diagnostic(Diagnostic::at_line(self.line_offset, self.line_number, kind));
        if self.overflow {
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
        self.len = 0;
        self.overflow = false;
    }
}

impl Reader for EditTfReader {
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
    }
}

#[cfg(test)]
mod tests;
