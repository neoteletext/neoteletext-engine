//! The page encoder (REQ-ARCH-004 and REQ-INP-019): turns a
//! [`PageFile`] into correctly coded packets in the order of Annex B.2,
//! so that page files take the same decoding path as broadcasts.

use tellytxt_core::consts::{
    ALPHA_BLACK, CLOCK_COLUMN, CODE_MASK_7BIT, COLOUR_BLUE_BIT, COLOUR_GREEN_BIT, COLOUR_RED_BIT, COLOUR_WHITE,
    COLUMNS, DATA_BYTES, DESIGNATION_CODES, HEADER_DISPLAY_BYTES, HEADER_DISPLAY_COLUMN, MODE_G2,
    MODE_SET_ACTIVE_POSITION, MODE_TERMINATION, NIBBLE_MASK, PACKET_FIRST_ROW, PACKET_LINKS,
    PACKET_MAGAZINE_ENHANCEMENT, PACKET_PAGE_ENHANCEMENT, PACKET_ROW_24, PACKET_ROW_25, ROW_ADDRESS_LAST,
    ROW_ADDRESS_ROW_24, SPACE, TRIPLETS_PER_PACKET,
};
use tellytxt_core::ecc::{PageChecksum, hamming84_encode, with_odd_parity};
use tellytxt_core::packet::{Packet, Triplet, encode};
use tellytxt_core::types::{FixedVec, Magazine, PageNumber, SubCode};

use crate::consts::TTI_PACKED_CHARACTERS;
use crate::page::{FASTEXT_LINKS, LinkKey, PageFile, RawBody, RawPacket};

/// The right guillemet "»" in the Latin G2 set (EN 300 706 Table 37, code
/// 3/B), the receiver's mark for a link that leaves the service
/// (REQ-NAV-011). Not yet in `tellytxt_core::consts`; a test checks it
/// against the core's character table.
pub const G2_RIGHT_GUILLEMET: u8 = 0x3B;
/// The Level 1 character written where the guillemet goes, for receivers
/// without X/26: ">" is the same in every Latin national option.
pub const GUILLEMET_FALLBACK: u8 = b'>';
/// Columns per link prompt: four equal prompts (REQ-NAV-011).
pub const PROMPT_COLUMNS: usize = COLUMNS / 4;
/// Label characters of a same-service prompt: the prompt less its colour
/// attribute (Appendix E.1).
pub const LABEL_CHARACTERS: usize = PROMPT_COLUMNS - 1;
/// Label characters of an other-service prompt, which also holds the mark.
pub const OTHER_SERVICE_LABEL_CHARACTERS: usize = LABEL_CHARACTERS - 1;
/// Table 32 note 1: the character set group occupies bits 3 to 6 of a
/// designation value, above the national option index. The core keeps this
/// private to its character tables.
pub(crate) const DESIGNATION_GROUP_SHIFT: u32 = 3;
/// Header bytes the page checksum covers: bytes 14 to 37, the display
/// columns before the clock (clause 9.6.1, decision E.11).
pub const CHECKSUM_HEADER_BYTES: usize = CLOCK_COLUMN - HEADER_DISPLAY_COLUMN;
/// Table 33: 1111111 means "no second G0 set required". Private in the core.
const NO_SECOND_G0: u8 = 0x7F;
/// Table 4: page function is triplet 1 bits 1 to 4 and page coding bits 5
/// to 7.
const PAGE_CODING_SHIFT: u32 = 4;
/// Triplets a page can carry in its sixteen X/26 packets.
const PAGE_TRIPLETS: usize = DESIGNATION_CODES * TRIPLETS_PER_PACKET;

/// The six X/27/0 link targets handed to the packet builder.
type LinkTargets = [Option<(Magazine, PageNumber, SubCode)>; FASTEXT_LINKS];

/// The Termination Marker (clause 12.3.1, Table 27).
const TERMINATION: Triplet = Triplet { address: ROW_ADDRESS_LAST, mode: MODE_TERMINATION, data: 0 };

/// The receiver-style row 24 of a page with Link Extension links
/// (Appendix E.2 rule 4 and REQ-NAV-011).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LinkRow {
    /// The row's 7-bit codes.
    pub codes: [u8; COLUMNS],
    /// Columns that hold the other-service mark, to be written as the G2
    /// guillemet by X/26.
    pub marks: FixedVec<u8, 4>,
}

/// The colour of a key's prompt: red, green, yellow and cyan (REQ-NAV-011).
const fn key_colour(key: LinkKey) -> u8 {
    match key {
        LinkKey::Red => COLOUR_RED_BIT,
        LinkKey::Green => COLOUR_GREEN_BIT,
        LinkKey::Yellow => COLOUR_RED_BIT | COLOUR_GREEN_BIT,
        LinkKey::Cyan => COLOUR_GREEN_BIT | COLOUR_BLUE_BIT,
        LinkKey::Index => COLOUR_WHITE,
    }
}

/// Draws row 24 from a page's `LN` links, or `None` when the page has no
/// valid `LN` links. A link to another service is drawn in white, with the
/// "»" mark, as Appendix E.2 rule 4 asks for broadcast.
#[must_use]
pub fn link_row(page: &PageFile) -> Option<LinkRow> {
    if !page.uses_link_lines() || !page.links_valid {
        return None;
    }
    let mut row = LinkRow { codes: [SPACE; COLUMNS], marks: FixedVec::new(0) };
    for link in page.links.as_slice().iter().filter(|link| link.key.is_coloured()) {
        let start = link.key.fastext_index().saturating_mul(PROMPT_COLUMNS);
        let Some(prompt) = row.codes.get_mut(start..start.saturating_add(PROMPT_COLUMNS)) else { continue };
        let (colour, label_start, limit) = if link.is_other_service() {
            (COLOUR_WHITE, 2, OTHER_SERVICE_LABEL_CHARACTERS)
        } else {
            (key_colour(link.key), 1, LABEL_CHARACTERS)
        };
        if let Some(attribute) = prompt.first_mut() {
            *attribute = ALPHA_BLACK | colour;
        }
        if link.is_other_service() {
            if let Some(mark) = prompt.get_mut(1) {
                *mark = GUILLEMET_FALLBACK;
            }
            let _ = row.marks.push(u8::try_from(start.saturating_add(1)).unwrap_or(0));
        }
        let label = link.label.as_bytes().iter().take(limit);
        for (slot, &byte) in prompt.iter_mut().skip(label_start).zip(label) {
            let code = byte & CODE_MASK_7BIT;
            // A label may not carry attributes into the receiver's row.
            *slot = if code < SPACE { SPACE } else { code };
        }
    }
    Some(row)
}

/// Computes the page checksum of clause 9.6.1 (REQ-ECC-007): header bytes
/// 14 to 37, then bytes 6 to 45 of packets X/1 to X/25, absent packets as
/// spaces. Every byte is applied with its parity bit, as transmitted.
#[must_use]
pub fn page_checksum(header_display: &[u8; HEADER_DISPLAY_BYTES], rows: &[Option<[u8; DATA_BYTES]>]) -> u16 {
    let mut checksum = PageChecksum::new();
    // Bytes 14 to 37 are the first 24 display bytes; the clock bytes 38 to
    // 45 change every second and are left out.
    checksum.update_all(header_display.get(..CHECKSUM_HEADER_BYTES).unwrap_or(&[]));
    let space = with_odd_parity(SPACE);
    for row in rows.iter().take(usize::from(PACKET_ROW_25)) {
        match row {
            Some(bytes) => checksum.update_all(bytes),
            None => {
                for _ in 0..DATA_BYTES {
                    checksum.update(space);
                }
            }
        }
    }
    checksum.value()
}

/// The 16-bit word handed to the X/27/0 builder, which writes it big-endian
/// into bytes 44 and 45. Every decision about the transmitted order of the
/// checksum lives here.
#[must_use]
pub const fn checksum_word(checksum: u16) -> u16 {
    checksum
}

/// Builds a packet from a stored packed line.
#[must_use]
pub fn raw_packet(magazine: Magazine, raw: &RawPacket) -> Packet {
    match &raw.body {
        RawBody::Triplets(triplets) => encode::triplet_packet(magazine, raw.number, raw.designation, triplets),
        RawBody::Nibbles(values) => {
            let body: [u8; TTI_PACKED_CHARACTERS] = values.map(|value| hamming84_encode(value & NIBBLE_MASK));
            encode::raw(magazine, raw.number, raw.designation, &body)
        }
    }
}

/// The rows as transmitted: 7-bit codes with odd parity, row 24 replaced by
/// the link row where there is one.
fn transmitted_rows(page: &PageFile, row_24: Option<&LinkRow>) -> [Option<[u8; DATA_BYTES]>; PACKET_ROW_25 as usize] {
    let mut rows = [None; PACKET_ROW_25 as usize];
    for (index, slot) in rows.iter_mut().enumerate() {
        let number = index.saturating_add(usize::from(PACKET_FIRST_ROW));
        let codes = if number == usize::from(PACKET_ROW_24)
            && let Some(link_row) = row_24
        {
            Some(&link_row.codes)
        } else if page.has_row(number) {
            page.row(number)
        } else {
            None
        };
        *slot = codes.map(|codes| codes.map(with_odd_parity));
    }
    rows
}

/// The six X/27/0 links of a page and its "display row 24" flag, or `None`
/// when the page has no links.
fn fastext_links(page: &PageFile) -> Option<(LinkTargets, bool)> {
    let mut links = [None; FASTEXT_LINKS];
    if page.uses_link_lines() {
        if !page.links_valid {
            return None;
        }
        // Appendix E.2 rule 4: links to other services are written as null.
        for link in page.links.as_slice().iter().filter(|link| !link.is_other_service()) {
            if let Some(slot) = links.get_mut(link.key.fastext_index()) {
                *slot = Some((link.address.magazine, link.address.page, link.subcode.unwrap_or(SubCode::ANY)));
            }
        }
        return Some((links, true));
    }
    if page.fastext.iter().all(Option::is_none) {
        return None;
    }
    for (slot, link) in links.iter_mut().zip(page.fastext) {
        *slot = link.map(|link| (link.address.magazine, link.address.page, link.subcode));
    }
    Some((links, page.fastext_row_24))
}

/// The page's X/26 triplets with the guillemet marks of a link row written
/// at the end of row 24, repacked by designation code. `None` when there is
/// no room, in which case the page's own packets are used unchanged.
fn triplets_with_marks(
    page: &PageFile,
    marks: &[u8],
) -> Option<FixedVec<[Triplet; TRIPLETS_PER_PACKET], DESIGNATION_CODES>> {
    let mut all: FixedVec<Triplet, PAGE_TRIPLETS> = FixedVec::new(TERMINATION);
    let mut ordered: FixedVec<(u8, [Triplet; TRIPLETS_PER_PACKET]), DESIGNATION_CODES> =
        FixedVec::new((0, [TERMINATION; TRIPLETS_PER_PACKET]));
    for packet in page.enhancements.as_slice() {
        let _ = ordered.push((packet.designation, packet.triplets));
    }
    ordered.as_mut_slice().sort_unstable_by_key(|(designation, _)| *designation);
    let mut in_row_24 = false;
    'packets: for (_, triplets) in ordered.as_slice() {
        for &triplet in triplets {
            if triplet == TERMINATION {
                break 'packets;
            }
            if triplet.is_row_address() {
                // The receiver draws row 24 itself; the page's own changes to
                // it are dropped (REQ-NAV-011).
                in_row_24 = triplet.address == ROW_ADDRESS_ROW_24;
            }
            if !in_row_24 && !all.push(triplet) {
                return None;
            }
        }
    }
    let ok = all.push(Triplet { address: ROW_ADDRESS_ROW_24, mode: MODE_SET_ACTIVE_POSITION, data: 0 })
        && marks.iter().all(|&column| all.push(Triplet { address: column, mode: MODE_G2, data: G2_RIGHT_GUILLEMET }))
        && all.push(TERMINATION);
    if !ok {
        return None;
    }
    let mut packets = FixedVec::new([TERMINATION; TRIPLETS_PER_PACKET]);
    for chunk in all.as_slice().chunks(TRIPLETS_PER_PACKET) {
        let mut packet = [TERMINATION; TRIPLETS_PER_PACKET];
        for (slot, &triplet) in packet.iter_mut().zip(chunk) {
            *slot = triplet;
        }
        let _ = packets.push(packet);
    }
    Some(packets)
}

/// Encodes a page as packets in the order of Annex B.2 (REQ-INP-018):
/// magazine packets M/29 first, then the header, packets 27, packets 28,
/// packets 26 in ascending designation code and the rows. Form commands are
/// never transmitted (Appendix C.6).
pub fn encode_page(page: &PageFile, out: &mut dyn FnMut(Packet)) {
    let magazine = page.address.magazine;
    let link_row = link_row(page);
    let rows = transmitted_rows(page, link_row.as_ref());

    for raw in page.raw_packets.as_slice().iter().filter(|raw| raw.number == PACKET_MAGAZINE_ENHANCEMENT) {
        out(raw_packet(magazine, raw));
    }

    let header_codes = page.row(0).copied().unwrap_or([SPACE; COLUMNS]);
    let mut display = [with_odd_parity(SPACE); HEADER_DISPLAY_BYTES];
    for (slot, &code) in display.iter_mut().zip(header_codes.iter().skip(HEADER_DISPLAY_COLUMN)) {
        *slot = with_odd_parity(code);
    }
    out(encode::header(magazine, page.address.page, page.subcode, page.control(), &display));

    if let Some((links, row_24)) = fastext_links(page) {
        let checksum = page_checksum(&display, &rows);
        out(encode::links(magazine, &links, row_24, checksum_word(checksum)));
    }
    emit_raw(page, PACKET_LINKS, out);

    let explicit_x28_0 =
        page.raw_packets.as_slice().iter().any(|raw| raw.number == PACKET_PAGE_ENHANCEMENT && raw.designation == 0);
    if !explicit_x28_0 && (page.region.is_some() || page.function.is_some()) {
        out(page_enhancement_packet(page));
    }
    emit_raw(page, PACKET_PAGE_ENHANCEMENT, out);

    let marks = link_row.as_ref().map_or(&[][..], |row| row.marks.as_slice());
    let repacked = if marks.is_empty() { None } else { triplets_with_marks(page, marks) };
    if let Some(packets) = repacked {
        for (designation, triplets) in packets.as_slice().iter().enumerate() {
            let designation = u8::try_from(designation).unwrap_or(0);
            out(encode::triplet_packet(magazine, tellytxt_core::consts::PACKET_ENHANCEMENT, designation, triplets));
        }
    } else {
        let mut ordered = page.enhancements;
        ordered.as_mut_slice().sort_unstable_by_key(|packet| packet.designation);
        for packet in ordered.as_slice() {
            out(encode::triplet_packet(
                magazine,
                tellytxt_core::consts::PACKET_ENHANCEMENT,
                packet.designation,
                &packet.triplets,
            ));
        }
    }

    for (index, row) in rows.iter().enumerate() {
        if let Some(bytes) = row {
            let number = u8::try_from(index).unwrap_or(0).saturating_add(PACKET_FIRST_ROW);
            out(encode::row(magazine, number, bytes));
        }
    }
}

/// Emits the stored packed lines of one packet number in ascending
/// designation code.
fn emit_raw(page: &PageFile, number: u8, out: &mut dyn FnMut(Packet)) {
    let mut ordered = page.raw_packets;
    ordered.as_mut_slice().sort_unstable_by_key(|raw| raw.designation);
    for raw in ordered.as_slice().iter().filter(|raw| raw.number == number) {
        out(raw_packet(page.address.magazine, raw));
    }
}

/// Packet X/28/0 format 1 from `RE` and `PF` (Table 4): the default
/// designation is the region above the page's national option index, with
/// no second G0 set.
fn page_enhancement_packet(page: &PageFile) -> Packet {
    let region = page.region.unwrap_or(0) & NIBBLE_MASK;
    let designation = (region << DESIGNATION_GROUP_SHIFT) | page.control().national_index();
    let [first, second] = encode::designation_triplets(designation, NO_SECOND_G0);
    let function = page.function.unwrap_or_default();
    let first = Triplet::from_value(
        first.value() | u32::from(function.function & NIBBLE_MASK) | (u32::from(function.coding) << PAGE_CODING_SHIFT),
    );
    let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
    if let Some(slot) = triplets.first_mut() {
        *slot = first;
    }
    if let Some(slot) = triplets.get_mut(1) {
        *slot = second;
    }
    encode::triplet_packet(page.address.magazine, PACKET_PAGE_ENHANCEMENT, 0, &triplets)
}

#[cfg(test)]
mod tests;
