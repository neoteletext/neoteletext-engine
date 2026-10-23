//! Stored pages back to page files for `neottx convert --to tti`: display
//! pages with their packets 26 to 28, enhancement pages coded by their page
//! coding, and each magazine's packets M/29 on its page mFF (REQ-INP-023).

use neoteletext_core::acq::{EnhancementPage, MagazineData, StoredPage};
use neoteletext_core::config::ParityPolicy;
use neoteletext_core::consts::{
    CODE_MASK_7BIT, COLUMNS, COMPOSITIONAL_LINKS, DESIGNATION_CODES, DESIGNATION_LEVEL_35_DATA,
    DESIGNATION_LINKS_LEVEL_25, DESIGNATION_PAGE_DATA, HEADER_DISPLAY_COLUMN, LINK_NULL_PAGE, MAGAZINES,
    PACKET_FIRST_ROW, PACKET_ROW_25, PAGE_FUNCTION_POP, PAGE_MOT, PAGE_TIME_FILLING, SPACE, STORED_ROWS,
    TRIPLETS_PER_PACKET,
};
use neoteletext_core::ecc::{hamming2418, has_odd_parity};
use neoteletext_core::packet::{CompositionalLink, Packet, PageFormat, Triplet, encode};
use neoteletext_core::types::{Magazine, PageAddress, PageNumber, SubCode};
use neoteletext_formats::encode::{RowCoding, packed_line_of, row_codes, transmission_class};
use neoteletext_formats::page::{EnhancementPacket, FastextLink, PageFile, PageFunction};

use crate::session::Session;

/// Fastext keys carried by a stored X/27/0: links 0 to 5.
const LINKS: usize = 6;
/// Page coding 010 (Table 3), assumed for an enhancement page that sent
/// no page function and whose rows are not all parity bytes.
pub(super) const CODING_TRIPLETS: u8 = 0x2;

/// A page file from a stored page: rows as 7-bit codes (parity failures as
/// spaces, decision E.8), control bits, X/26 packets, X/27/0 links, and the
/// packets X/27/4, X/27/5, X/28/0, X/28/1 and X/28/4 as packed lines.
#[must_use]
pub fn page_file(page: &StoredPage) -> PageFile {
    let mut file = PageFile { address: page.key.address, subcode: page.key.subcode, ..PageFile::default() };
    file.set_control(page.control);
    for row in 0..STORED_ROWS {
        if row > 0 && !page.has_row(row as u8) {
            continue;
        }
        let mut codes = [SPACE; COLUMNS];
        let first = if row == 0 { HEADER_DISPLAY_COLUMN } else { 0 };
        for (column, code) in codes.iter_mut().enumerate().skip(first) {
            let (byte, passed) = page.byte(row, column, ParityPolicy::KeepLastGood);
            *code = if passed { byte & CODE_MASK_7BIT } else { SPACE };
        }
        file.set_row(row, &codes);
        file.rows_present |= 1 << row;
    }
    for (designation, triplets) in page.enhancements.iter().enumerate() {
        if page.enhancements_present & (1 << designation) != 0 {
            let mut packet = EnhancementPacket { designation: designation as u8, triplets: Default::default() };
            for (slot, triplet) in packet.triplets.iter_mut().zip(triplets) {
                *slot = triplet.unwrap_or_default();
            }
            let _ = file.enhancements.push(packet);
        }
    }
    if let Some(links) = page.links {
        for (slot, link) in file.fastext.iter_mut().zip(links.links.iter()).take(LINKS) {
            *slot = link.and_then(|link| {
                let mpp = (u16::from(link.magazine.number()) << 8) | u16::from(link.page.0);
                PageAddress::from_mpp(mpp).map(|address| FastextLink { address, subcode: link.subcode })
            });
        }
        file.fastext_row_24 = links.display_row_24;
    }
    page_packets(page, &mut file);
    file
}

/// Adds the packed line of a packet just encoded.
fn push_line(file: &mut PageFile, packet: &Packet) {
    if let Some(line) = packed_line_of(packet) {
        let _ = file.raw_packets.push(line);
    }
}

/// The packets X/27/4, X/27/5 and X/28 of a stored page, encoded again from
/// their decoded fields. A link that was uncorrectable is written as "no
/// link" (page validity 00), and a damaged field group as 0.
fn page_packets(page: &StoredPage, file: &mut PageFile) {
    let magazine = page.key.address.magazine;
    for (designation, links) in (DESIGNATION_LINKS_LEVEL_25..).zip(page.compositional_links) {
        let Some(links) = links else { continue };
        let none = CompositionalLink {
            function: 0,
            validity: 0,
            magazine,
            page: PageNumber(LINK_NULL_PAGE),
            subpages: 0,
            marker_first: true,
            marker_second: false,
        };
        let mut all = [none; COMPOSITIONAL_LINKS];
        for (slot, link) in all.iter_mut().zip(links) {
            *slot = link.unwrap_or(none);
        }
        push_line(file, &encode::compositional_links(magazine, designation, &all));
    }
    let basic = PageFormat { function: 0, coding: 0 };
    if let Some(data) = page.page_data {
        let format = data.format.unwrap_or(basic);
        push_line(file, &encode::page_enhancement(magazine, DESIGNATION_PAGE_DATA, format, &data.fields));
        file.function = Some(PageFunction { function: format.function, coding: format.coding });
    }
    if let Some(data) = page.page_data_level35 {
        let format = data.format.unwrap_or(basic);
        push_line(file, &encode::page_enhancement(magazine, DESIGNATION_LEVEL_35_DATA, format, &data.fields));
        file.function = Some(PageFunction { function: format.function, coding: format.coding });
    }
}

/// The page function of an enhancement page: its X/28/0 or X/28/3 when one
/// was received, none for page mFE (whose coding is known), otherwise a
/// guess from the stored bytes: rows that all pass parity are taken as
/// text coding and need no function, others as an object page's
/// Hamming-coded triplets.
fn enhancement_function(page: &EnhancementPage<'_>) -> Option<PageFunction> {
    if let Some(format) = page.format() {
        return Some(PageFunction { function: format.function, coding: format.coding });
    }
    if page.key().address.page.0 == PAGE_MOT {
        return None;
    }
    let parity = (PACKET_FIRST_ROW..=PACKET_ROW_25)
        .filter_map(|packet| page.row(packet))
        .all(|bytes| bytes.iter().all(|&byte| has_odd_parity(byte)));
    if parity { None } else { Some(PageFunction { function: PAGE_FUNCTION_POP, coding: CODING_TRIPLETS }) }
}

/// A page file from a stored enhancement sub-page (REQ-INP-023): its rows
/// in the TTI form of its page coding, and its packets X/26. Its page
/// function goes in the file's `PF` line.
#[must_use]
pub fn enhancement_page_file(page: &EnhancementPage<'_>) -> PageFile {
    let key = page.key();
    let mut file = PageFile { address: key.address, subcode: page.subcode(), ..PageFile::default() };
    file.set_control(page.control());
    file.function = enhancement_function(page);
    let coding = match file.function {
        Some(function) => RowCoding::from_coding(function.coding),
        None if key.address.page.0 == PAGE_MOT => RowCoding::Nibbles,
        None => RowCoding::Parity,
    };
    for packet in PACKET_FIRST_ROW..=PACKET_ROW_25 {
        if let Some(bytes) = page.row(packet) {
            file.set_row(usize::from(packet), &row_codes(coding, bytes));
        }
    }
    for designation in (0u8..).take(DESIGNATION_CODES) {
        let Some(bytes) = page.enhancement(designation) else { continue };
        let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
        for (triplet, chunk) in triplets.iter_mut().zip(bytes[1..].as_chunks::<3>().0) {
            *triplet = Triplet::from_value(hamming2418(*chunk).unwrap_or(0));
        }
        let _ = file.enhancements.push(EnhancementPacket { designation, triplets });
    }
    file
}

/// The magazine packets M/29/0 and M/29/4 of a magazine on its
/// page mFF, or `None` when it has none (REQ-INP-023).
#[must_use]
pub fn magazine_page_file(magazine: Magazine, data: &MagazineData) -> Option<PageFile> {
    if data.data.is_none() && data.level35.is_none() {
        return None;
    }
    let address = PageAddress { magazine, page: PageNumber(PAGE_TIME_FILLING) };
    let mut file = PageFile { address, subcode: SubCode::new(0), ..PageFile::default() };
    if let Some(fields) = data.data {
        push_line(&mut file, &encode::magazine_enhancement(magazine, DESIGNATION_PAGE_DATA, &fields));
    }
    if let Some(fields) = data.level35 {
        push_line(&mut file, &encode::magazine_enhancement(magazine, DESIGNATION_LEVEL_35_DATA, &fields));
    }
    Some(file)
}

/// Every page of a session as page files in the order of Annex B.7, so
/// that reading the written file stores enhancement pages before the pages
/// that use them: magazine packets, tables, object pages, then display
/// pages. With a filter, only that page's sub-pages.
#[must_use]
pub fn session_page_files(session: &Session, filter: Option<PageAddress>) -> Vec<PageFile> {
    let wanted = |address: PageAddress| filter.is_none_or(|page| page == address);
    let decoder = session.decoder();
    let mut files = Vec::new();
    if filter.is_none() {
        for bits in (0u8..).take(MAGAZINES) {
            let magazine = Magazine::from_bits(bits);
            files.extend(magazine_page_file(magazine, &decoder.magazine_data(magazine)));
        }
    }
    files.extend(
        decoder
            .enhancement_store()
            .sub_pages()
            .filter(|page| wanted(page.key().address))
            .map(|page| enhancement_page_file(&page)),
    );
    files.extend(session.pages().into_iter().filter(|page| wanted(page.key.address)).map(page_file));
    files.sort_by_key(transmission_class);
    files
}
