//! The page encoder (REQ-ARCH-004, REQ-INP-019 and REQ-INP-024): turns a
//! [`PageFile`] into correctly coded packets in the order of Annex B.2,
//! so that page files take the same decoding path as broadcasts.
//!
//! The rows of a page are coded as its page coding says (Table 3,
//! REQ-INP-023): display pages and DRCS pages as seven bits with odd parity,
//! object pages as a Hamming 8/4 byte and thirteen Hamming 24/18 triplets,
//! and the Magazine Organization Table as Hamming 8/4 nibbles. A page file
//! holds every row as forty 7-bit codes, which for the last two codings are
//! the packed characters of the TTI form ([`pack_triplets`], [`nibble_row`]).

use neoteletext_core::consts::{
    ALPHA_BLACK, CLOCK_COLUMN, CLUT_ENTRIES, CLUT_LEVEL_25_FIRST, CODE_MASK_7BIT, COLOUR_BLUE_BIT, COLOUR_GREEN_BIT,
    COLOUR_MAP_DEFAULTS, COLOUR_RED_BIT, COLOUR_WHITE, COLOUR_WORDS, COLUMNS, DATA_BYTES, DESIGNATION_CODES,
    DESIGNATION_PAGE_DATA, HEADER_DISPLAY_BYTES, HEADER_DISPLAY_COLUMN, MODE_G2, MODE_SET_ACTIVE_POSITION,
    MODE_TERMINATION, NIBBLE_MASK, PACKET_ENHANCEMENT, PACKET_FIRST_ROW, PACKET_LINKS, PACKET_MAGAZINE_ENHANCEMENT,
    PACKET_PAGE_ENHANCEMENT, PACKET_ROW_24, PACKET_ROW_25, PAGE_FUNCTION_BASIC, PAGE_FUNCTION_DRCS,
    PAGE_FUNCTION_GDRCS, PAGE_FUNCTION_GPOP, PAGE_FUNCTION_MOT, PAGE_FUNCTION_POP, PAGE_MOT, PAGE_TIME_FILLING,
    ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24, SPACE, TRIPLETS_PER_PACKET,
};
use neoteletext_core::ecc::{
    PageChecksum, hamming84, hamming84_encode, hamming2418, hamming2418_encode, with_odd_parity,
};
use neoteletext_core::packet::{Packet, PageFormat, Triplet, encode};
use neoteletext_core::types::{
    ColourWords, DefaultColourFields, Designations, FixedVec, Magazine, PageNumber, PresentationFields, Rgb12,
    SidePanelFields, SubCode,
};

use crate::consts::{
    NOT_TEXT_PAGE_MARKER, PAGE_CODING_NIBBLES, PAGE_CODING_TRIPLETS, TTI_PACKED_CHARACTER_BASE, TTI_PACKED_CHARACTERS,
    TTI_TRIPLET_CHARACTER_BITS, TTI_TRIPLET_CHARACTER_MASK,
};
use crate::page::{FASTEXT_LINKS, LinkKey, PageFile, RawBody, RawPacket};

pub use neoteletext_core::consts::{DESIGNATION_GROUP_SHIFT, G2_RIGHT_GUILLEMET};
use neoteletext_core::consts::{DESIGNATION_LATIN_ENGLISH, NO_SECOND_G0};
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
/// Header bytes the page checksum covers: bytes 14 to 37, the display
/// columns before the clock (clause 9.6.1, decision E.11).
pub const CHECKSUM_HEADER_BYTES: usize = CLOCK_COLUMN - HEADER_DISPLAY_COLUMN;
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

/// The packed `OL` line of a packet that carries a designation code and
/// thirteen triplets (X/26, X/27/4 to X/27/15, X/28, M/29): the inverse of
/// [`raw_packet`] for those packets, so that received packets can be
/// written as TTI (REQ-INP-023). `None` when the address or the
/// designation code is uncorrectable; an uncorrectable triplet is written
/// as 0.
#[must_use]
pub fn packed_line_of(packet: &Packet) -> Option<RawPacket> {
    let address = neoteletext_core::packet::decode_address(packet).ok()?;
    let data = packet.data();
    let (&first, body) = data.split_first()?;
    let designation = hamming84(first)?;
    let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
    for (triplet, chunk) in triplets.iter_mut().zip(body.as_chunks::<3>().0) {
        *triplet = Triplet::from_value(hamming2418(*chunk).unwrap_or(0));
    }
    Some(RawPacket { number: address.number, designation, body: RawBody::Triplets(triplets) })
}

/// The character that carries six bits of a packed TTI line (vbit2's form,
/// REQ-INP-023): bit 6 set, so every character is printable.
const fn packed_character(value: u32) -> u8 {
    TTI_PACKED_CHARACTER_BASE | (value & TTI_TRIPLET_CHARACTER_MASK) as u8
}

/// The 39 packed characters of thirteen triplets: three characters each,
/// six bits per character, the least significant first (REQ-INP-023).
#[must_use]
pub fn pack_triplets(triplets: &[Triplet; TRIPLETS_PER_PACKET]) -> [u8; TTI_PACKED_CHARACTERS] {
    let mut out = [TTI_PACKED_CHARACTER_BASE; TTI_PACKED_CHARACTERS];
    for (chunk, triplet) in out.as_chunks_mut::<3>().0.iter_mut().zip(triplets) {
        let value = triplet.value();
        *chunk = [
            packed_character(value),
            packed_character(value >> TTI_TRIPLET_CHARACTER_BITS),
            packed_character(value >> (2 * TTI_TRIPLET_CHARACTER_BITS)),
        ];
    }
    out
}

/// The thirteen triplets of up to 39 packed characters; missing characters
/// count as 0. The inverse of [`pack_triplets`].
#[must_use]
pub fn unpack_triplets(characters: &[u8]) -> [Triplet; TRIPLETS_PER_PACKET] {
    let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
    let (chunks, _) = characters.as_chunks::<3>();
    for (triplet, &[first, second, third]) in triplets.iter_mut().zip(chunks) {
        let value = (u32::from(first) & TTI_TRIPLET_CHARACTER_MASK)
            | ((u32::from(second) & TTI_TRIPLET_CHARACTER_MASK) << TTI_TRIPLET_CHARACTER_BITS)
            | ((u32::from(third) & TTI_TRIPLET_CHARACTER_MASK) << (2 * TTI_TRIPLET_CHARACTER_BITS));
        *triplet = Triplet::from_value(value);
    }
    triplets
}

/// How the rows X/1 to X/25 of a page are coded (Table 3, REQ-INP-023).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum RowCoding {
    /// Seven data bits and odd parity (coding 000): display pages and DRCS
    /// pages. Codings this encoder cannot write (8-bit data, and the mixed
    /// codings 100 and 101) are sent this way too.
    Parity,
    /// Coding 010: byte 6 Hamming 8/4, then thirteen Hamming 24/18
    /// triplets. Object pages.
    Triplets,
    /// Coding 011: every byte Hamming 8/4. The Magazine Organization Table.
    Nibbles,
}

impl RowCoding {
    /// The coding of a page coding value (Table 3).
    #[must_use]
    pub const fn from_coding(coding: u8) -> Self {
        match coding {
            PAGE_CODING_TRIPLETS => Self::Triplets,
            PAGE_CODING_NIBBLES => Self::Nibbles,
            _ => Self::Parity,
        }
    }
}

/// The coding of a page's rows: from its page function line (`PF`, or the
/// format of X/28/0, X/28/3 or X/28/4), and for page mFE without one the
/// Hamming 8/4 coding that clause 10.6.1 gives every Magazine Organization
/// Table.
#[must_use]
pub fn row_coding(page: &PageFile) -> RowCoding {
    match page.function {
        Some(function) => RowCoding::from_coding(function.coding),
        None if page.address.page.0 == PAGE_MOT => RowCoding::Nibbles,
        None => RowCoding::Parity,
    }
}

/// True for a page "intended for further processing" (Annex B.2 item 3):
/// one whose page function is not "basic Level 1 page", or page mFE.
#[must_use]
pub fn is_enhancement_page(page: &PageFile) -> bool {
    page.address.page.0 == PAGE_MOT || page.function.is_some_and(|function| function.function != PAGE_FUNCTION_BASIC)
}

/// The forty transmitted bytes of a row given as 7-bit codes, coded as
/// `coding` says.
#[must_use]
pub fn row_bytes(coding: RowCoding, codes: &[u8; COLUMNS]) -> [u8; DATA_BYTES] {
    let mut bytes = [0; DATA_BYTES];
    match coding {
        RowCoding::Parity => {
            for (byte, &code) in bytes.iter_mut().zip(codes) {
                *byte = with_odd_parity(code);
            }
        }
        RowCoding::Nibbles => {
            for (byte, &code) in bytes.iter_mut().zip(codes) {
                *byte = hamming84_encode(code & NIBBLE_MASK);
            }
        }
        RowCoding::Triplets => {
            let (first, rest) = codes.split_first().map_or((0, &[][..]), |(first, rest)| (*first, rest));
            let triplets = unpack_triplets(rest);
            if let Some((byte_6, body)) = bytes.split_first_mut() {
                *byte_6 = hamming84_encode(first & NIBBLE_MASK);
                for (chunk, triplet) in body.as_chunks_mut::<3>().0.iter_mut().zip(&triplets) {
                    *chunk = hamming2418_encode(triplet.value());
                }
            }
        }
    }
    bytes
}

/// The 7-bit codes of a row as a page file holds them, from its forty
/// received bytes: the inverse of [`row_bytes`]. A byte that fails its
/// check is written as a space for parity-coded rows and as 0 otherwise,
/// so that the result can always be written and read again.
#[must_use]
pub fn row_codes(coding: RowCoding, bytes: &[u8; DATA_BYTES]) -> [u8; COLUMNS] {
    let mut codes = [SPACE; COLUMNS];
    match coding {
        RowCoding::Parity => {
            for (code, &byte) in codes.iter_mut().zip(bytes) {
                let (value, passed) = neoteletext_core::ecc::check_parity(byte);
                *code = if passed { value } else { SPACE };
            }
        }
        RowCoding::Nibbles => {
            for (code, &byte) in codes.iter_mut().zip(bytes) {
                *code = TTI_PACKED_CHARACTER_BASE | hamming84(byte).unwrap_or(0);
            }
        }
        RowCoding::Triplets => {
            let (first, rest) = bytes.split_first().map_or((0, &[][..]), |(first, rest)| (*first, rest));
            let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
            for (triplet, chunk) in triplets.iter_mut().zip(rest.as_chunks::<3>().0) {
                *triplet = Triplet::from_value(hamming2418(*chunk).unwrap_or(0));
            }
            codes = packed_row(hamming84(first).unwrap_or(0), &triplets);
        }
    }
    codes
}

/// A row of an object page (coding 010) as 7-bit codes: byte 6's 4-bit
/// value in the low bits of the first character, then the packed triplets.
#[must_use]
pub fn packed_row(byte_6: u8, triplets: &[Triplet; TRIPLETS_PER_PACKET]) -> [u8; COLUMNS] {
    let mut codes = [TTI_PACKED_CHARACTER_BASE; COLUMNS];
    if let Some((first, rest)) = codes.split_first_mut() {
        *first = TTI_PACKED_CHARACTER_BASE | (byte_6 & NIBBLE_MASK);
        for (slot, character) in rest.iter_mut().zip(pack_triplets(triplets)) {
            *slot = character;
        }
    }
    codes
}

/// A row of a Magazine Organization Table (coding 011) as 7-bit codes:
/// each 4-bit value in the low bits of a character 4/0 to 4/F.
#[must_use]
pub fn nibble_row(values: &[u8; DATA_BYTES]) -> [u8; COLUMNS] {
    values.map(|value| TTI_PACKED_CHARACTER_BASE | (value & NIBBLE_MASK))
}

/// The rows as transmitted, coded as the page coding says; on a display
/// page, row 24 is replaced by the link row where there is one.
fn transmitted_rows(page: &PageFile, row_24: Option<&LinkRow>) -> [Option<[u8; DATA_BYTES]>; PACKET_ROW_25 as usize] {
    let coding = row_coding(page);
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
        *slot = codes.map(|codes| row_bytes(coding, codes));
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

/// Encodes a page as packets in the order of Annex B.2 (REQ-INP-018,
/// REQ-INP-024): magazine packets M/29 first, then the header, packets 27
/// and packets 28. A display page then sends packets 26 in ascending
/// designation code and the rows (item 2); a page for further processing
/// ([`is_enhancement_page`]) sends the rows and then packets 26 (item 3).
/// Form commands are never transmitted (Appendix C.6).
pub fn encode_page(page: &PageFile, out: &mut dyn FnMut(Packet)) {
    let magazine = page.address.magazine;
    let further_processing = is_enhancement_page(page);
    // The receiver-style link row belongs to display pages only.
    let link_row = if further_processing { None } else { link_row(page) };
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

    if let Some(packet) = page_enhancement_packet(page) {
        out(packet);
    }
    emit_raw(page, PACKET_PAGE_ENHANCEMENT, out);

    if further_processing {
        emit_rows(magazine, &rows, out);
        emit_enhancements(page, None, out);
    } else {
        emit_enhancements(page, link_row.as_ref(), out);
        emit_rows(magazine, &rows, out);
    }
}

/// Emits the rows that are present in ascending packet number.
fn emit_rows(magazine: Magazine, rows: &[Option<[u8; DATA_BYTES]>], out: &mut dyn FnMut(Packet)) {
    for (index, row) in rows.iter().enumerate() {
        if let Some(bytes) = row {
            let number = u8::try_from(index).unwrap_or(0).saturating_add(PACKET_FIRST_ROW);
            out(encode::row(magazine, number, bytes));
        }
    }
}

/// Emits the packets X/26 in ascending designation code, with the
/// guillemet marks of a link row added where it has any.
fn emit_enhancements(page: &PageFile, link_row: Option<&LinkRow>, out: &mut dyn FnMut(Packet)) {
    let magazine = page.address.magazine;
    let marks = link_row.map_or(&[][..], |row| row.marks.as_slice());
    let repacked = if marks.is_empty() { None } else { triplets_with_marks(page, marks) };
    if let Some(packets) = repacked {
        for (designation, triplets) in packets.as_slice().iter().enumerate() {
            let designation = u8::try_from(designation).unwrap_or(0);
            out(encode::triplet_packet(magazine, PACKET_ENHANCEMENT, designation, triplets));
        }
    } else {
        let mut ordered = page.enhancements;
        ordered.as_mut_slice().sort_unstable_by_key(|packet| packet.designation);
        for packet in ordered.as_slice() {
            out(encode::triplet_packet(magazine, PACKET_ENHANCEMENT, packet.designation, &packet.triplets));
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

/// The colour words of CLUTs 2 and 3 in Table 30, as X/28/0 format 1
/// carries them. A page-level packet that only states the page's
/// designation sends these, so that it does not redefine those CLUTs.
#[must_use]
pub fn default_clut_2_and_3() -> ColourWords {
    let mut words = [Rgb12::default(); COLOUR_WORDS];
    let first = usize::from(CLUT_LEVEL_25_FIRST).saturating_mul(CLUT_ENTRIES);
    for (word, &value) in words.iter_mut().zip(COLOUR_MAP_DEFAULTS.iter().skip(first)) {
        *word = Rgb12(value);
    }
    words
}

/// Packet X/28/0 format 1 made from `RE` and `PF` when the page has no
/// explicit `OL,28` line with designation code 0, or `None` when none is
/// needed:
/// - a page function other than "basic Level 1 page" is sent as Table 6
///   (and Table 5) give it: the function and coding, the marker of bits 8
///   to 18, and reserved triplets 2 to 13 as 0;
/// - a basic page with `RE`, or with a page coding other than 000, is sent
///   as Table 4 gives it: the default designation is the region above the
///   page's national option index; the second G0 set is Latin English for
///   neoteletext's own designations, whose second set that is
///   (REQ-CHR-012), and none for the standard's; every other
///   field at its default (no side panels, the CLUT 2 and 3 colours of
///   Table 30, screen and row colour 0, no substitution or re-mapping), so
///   that the packet changes nothing but the designation;
/// - a basic page with neither needs no packet: its absence means a basic
///   Level 1 page coded with odd parity (REQ-ACQ-014).
fn page_enhancement_packet(page: &PageFile) -> Option<Packet> {
    let explicit =
        page.raw_packets.as_slice().iter().any(|raw| raw.number == PACKET_PAGE_ENHANCEMENT && raw.designation == 0);
    if explicit {
        return None;
    }
    let function = page.function.unwrap_or_default();
    let format = PageFormat { function: function.function & NIBBLE_MASK, coding: function.coding };
    let magazine = page.address.magazine;
    if format.function != PAGE_FUNCTION_BASIC {
        let mut values = [0u32; TRIPLETS_PER_PACKET];
        if let Some(first) = values.first_mut() {
            *first = format.bits() | NOT_TEXT_PAGE_MARKER;
        }
        let triplets = values.map(Triplet::from_value);
        return Some(encode::triplet_packet(magazine, PACKET_PAGE_ENHANCEMENT, DESIGNATION_PAGE_DATA, &triplets));
    }
    if page.region.is_none() && format.coding == 0 {
        return None;
    }
    let region = page.region.unwrap_or(0) & NIBBLE_MASK;
    let default = (region << DESIGNATION_GROUP_SHIFT) | page.control().national_index();
    let fields = PresentationFields {
        designations: Some(Designations { default, second: second_g0_value(default) }),
        side_panels: Some(SidePanelFields { left: false, right: false, status_flag: false, left_columns: 0 }),
        colour_words: Some(default_clut_2_and_3()),
        colours: Some(DefaultColourFields { screen: 0, row: 0, black_background_substitution: false, remapping: 0 }),
    };
    Some(encode::page_enhancement(magazine, DESIGNATION_PAGE_DATA, format, &fields))
}

/// The Table 33 value to send with a default designation: Latin English
/// where the designation implies it (REQ-CHR-012), so that writing it
/// explicitly keeps the page as it reads without the value.
fn second_g0_value(default: u8) -> u8 {
    if neoteletext_core::charset::default_second_g0(default).is_some() {
        DESIGNATION_LATIN_ENGLISH
    } else {
        NO_SECOND_G0
    }
}

/// The order in which a set of pages is sent (Annex B.7, REQ-INP-024):
/// magazine packets, then the Magazine Organization Table, the DRCS
/// pages, the object pages, and last the Level 1 pages.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum TransmissionClass {
    /// A page mFF that carries magazine packets M/29.
    MagazinePackets,
    /// Page mFE, or a page whose function is the Magazine Organization
    /// Table.
    OrganizationTable,
    /// A DRCS or global DRCS page.
    Drcs,
    /// A Public or Global Public Object Page.
    Objects,
    /// Any other page for further processing.
    OtherData,
    /// A Level 1 page.
    Display,
}

/// The class of a page in the order of Annex B.7.
#[must_use]
pub fn transmission_class(page: &PageFile) -> TransmissionClass {
    let carries_magazine_packets = page.address.page.0 == PAGE_TIME_FILLING
        && page.raw_packets.as_slice().iter().any(|raw| raw.number == PACKET_MAGAZINE_ENHANCEMENT);
    if carries_magazine_packets {
        return TransmissionClass::MagazinePackets;
    }
    if page.address.page.0 == PAGE_MOT {
        return TransmissionClass::OrganizationTable;
    }
    match page.function.map(|function| function.function) {
        None | Some(PAGE_FUNCTION_BASIC) => TransmissionClass::Display,
        Some(PAGE_FUNCTION_MOT) => TransmissionClass::OrganizationTable,
        Some(PAGE_FUNCTION_GDRCS | PAGE_FUNCTION_DRCS) => TransmissionClass::Drcs,
        Some(PAGE_FUNCTION_GPOP | PAGE_FUNCTION_POP) => TransmissionClass::Objects,
        Some(_) => TransmissionClass::OtherData,
    }
}

#[cfg(test)]
mod tests;
