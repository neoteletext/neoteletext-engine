//! One page as packets in the order of Annex B.2 (REQ-INP-018,
//! REQ-INP-024), with the link row and its guillemet marks.

use neoteletext_core::consts::{
    COLUMNS, DATA_BYTES, DESIGNATION_CODES, G2_RIGHT_GUILLEMET, HEADER_DISPLAY_BYTES, HEADER_DISPLAY_COLUMN, MODE_G2,
    MODE_SET_ACTIVE_POSITION, PACKET_ENHANCEMENT, PACKET_FIRST_ROW, PACKET_LINKS, PACKET_MAGAZINE_ENHANCEMENT,
    PACKET_PAGE_ENHANCEMENT, PACKET_ROW_24, PACKET_ROW_25, ROW_ADDRESS_ROW_24, SPACE, TRIPLETS_PER_PACKET,
};
use neoteletext_core::ecc::with_odd_parity;
use neoteletext_core::packet::{Packet, Triplet, encode};
use neoteletext_core::types::{FixedVec, Magazine, SubCode};

use crate::page::{FASTEXT_LINKS, PageFile};

use super::page_data::page_enhancement_packet;
use super::{
    LinkRow, LinkTargets, PAGE_TRIPLETS, TERMINATION, checksum_word, is_enhancement_page, link_row, page_checksum,
    raw_packet, row_bytes, row_coding,
};

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
