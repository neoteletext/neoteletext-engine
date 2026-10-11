//! Converting between formats (REQ-INP-014, REQ-INP-018), and stored
//! pages back to page files for `ttx convert --to tti`: display pages with
//! their packets 26 to 28, enhancement pages coded by their page coding,
//! and each magazine's packets M/29 on its page mFF (REQ-INP-023).

use neoteletext_core::acq::{EnhancementPage, MagazineData, StoredPage};
use neoteletext_core::config::ParityPolicy;
use neoteletext_core::consts::{
    CODE_MASK_7BIT, COLUMNS, COMPOSITIONAL_LINKS, DESIGNATION_CODES, DESIGNATION_LEVEL_35_DATA,
    DESIGNATION_LINKS_LEVEL_25, DESIGNATION_PAGE_DATA, HEADER_DISPLAY_COLUMN, LINK_NULL_PAGE, MAGAZINES,
    PACKET_FIRST_ROW, PACKET_MAGAZINE_ENHANCEMENT, PACKET_PAGE_ENHANCEMENT, PACKET_ROW_25, PAGE_FUNCTION_POP, PAGE_MOT,
    PAGE_TIME_FILLING, SPACE, STORED_ROWS, TRIPLETS_PER_PACKET,
};
use neoteletext_core::ecc::{hamming2418, has_odd_parity};
use neoteletext_core::packet::{CompositionalLink, Packet, PageFormat, Triplet, encode};
use neoteletext_core::types::{Magazine, PageAddress, PageNumber, SubCode};
use neoteletext_formats::encode::{RowCoding, packed_line_of, row_codes, transmission_class};
use neoteletext_formats::page::{EnhancementPacket, FastextLink, PageFile, PageFunction};

use crate::input::{self, Options};
use crate::session::Session;
use crate::{Failure, exit};
use neoteletext_formats::Item;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Fastext keys carried by a stored X/27/0: links 0 to 5.
const LINKS: usize = 6;
/// Page coding 010 (Table 3), assumed for an enhancement page that sent
/// no page function and whose rows are not all parity bytes.
const CODING_TRIPLETS: u8 = 0x2;

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
    if let Some(data) = page.drcs_colours {
        push_line(file, &encode::drcs_colours(magazine, PACKET_PAGE_ENHANCEMENT, &data));
    }
    if let Some(data) = page.page_data_level35 {
        let format = data.format.unwrap_or(basic);
        push_line(file, &encode::page_enhancement(magazine, DESIGNATION_LEVEL_35_DATA, format, &data.fields));
        file.function = Some(PageFunction { function: format.function, coding: format.coding });
    }
}

/// The page function of an enhancement page: its X/28/0 or X/28/3 when one
/// was received, none for page mFE (whose coding is known), otherwise a
/// guess from the stored bytes: rows that all pass parity are taken as a
/// DRCS page's D-bytes and need no function, others as an object page's
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
/// in the TTI form of its page coding, its packets X/26 and X/28/3.
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
    if let Some(modes) = page.drcs_modes() {
        let format = modes.format.or(page.format()).unwrap_or(PageFormat { function: 0, coding: 0 });
        push_line(&mut file, &encode::drcs_modes(key.address.magazine, format, modes));
    }
    file
}

/// The magazine packets M/29/0, M/29/1 and M/29/4 of a magazine on its
/// page mFF, or `None` when it has none (REQ-INP-023).
#[must_use]
pub fn magazine_page_file(magazine: Magazine, data: &MagazineData) -> Option<PageFile> {
    if data.data.is_none() && data.drcs_colours.is_none() && data.level35.is_none() {
        return None;
    }
    let address = PageAddress { magazine, page: PageNumber(PAGE_TIME_FILLING) };
    let mut file = PageFile { address, subcode: SubCode::new(0), ..PageFile::default() };
    if let Some(fields) = data.data {
        push_line(&mut file, &encode::magazine_enhancement(magazine, DESIGNATION_PAGE_DATA, &fields));
    }
    if let Some(colours) = data.drcs_colours {
        push_line(&mut file, &encode::drcs_colours(magazine, PACKET_MAGAZINE_ENHANCEMENT, &colours));
    }
    if let Some(fields) = data.level35 {
        push_line(&mut file, &encode::magazine_enhancement(magazine, DESIGNATION_LEVEL_35_DATA, &fields));
    }
    Some(file)
}

/// Every page of a session as page files in the order of Annex B.7, so
/// that reading the written file stores enhancement pages before the pages
/// that use them: magazine packets, tables, DRCS pages, object pages, then
/// display pages. With a filter, only that page's sub-pages.
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

/// Conversion targets (REQ-CLI-001 `convert`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// T42 packets.
    T42,
    /// TTI page files.
    Tti,
}

/// Converts the input to `to`, written to `output`; with a filter, only
/// that page (REQ-INP-014, REQ-INP-018).
pub fn convert(options: &Options, output: &Path, to: Target, filter: Option<PageAddress>) -> Result<(), Failure> {
    let file = File::create(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    match to {
        Target::T42 => {
            let filter = neoteletext_formats::write::PacketFilter {
                magazine: filter.map(|address| address.magazine),
                page: filter.map(|address| address.page),
            };
            let mut writer = neoteletext_formats::write::T42Writer::with_filter(std::io::BufWriter::new(file), filter);
            let mut input = input::open(options)?;
            let mut reader = input::reader(options, true);
            let mut failure = None;
            input::pump(input.as_mut(), reader.as_mut(), &mut |item| {
                if let Item::Packet { packet, .. } = item
                    && let Err(error) = writer.write_packet(&packet)
                {
                    failure = Some(error);
                }
            })?;
            if let Some(error) = failure {
                return Err((exit::INPUT, error.to_string()));
            }
            writer.into_inner().flush().map_err(|error| (exit::INPUT, error.to_string()))
        }
        Target::Tti => {
            let session = input::load(options)?;
            let pages = session_page_files(&session, filter);
            let references: Vec<&PageFile> = pages.iter().collect();
            let mut out = std::io::BufWriter::new(file);
            neoteletext_formats::write::write_tti(&references, &mut out)
                .map_err(|error| (exit::INPUT, error.to_string()))?;
            out.flush().map_err(|error| (exit::INPUT, error.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoteletext_core::config::Config;
    use neoteletext_core::packet::PacketMeta;
    use neoteletext_formats::Item;
    use neoteletext_formats::author::streams;
    use neoteletext_formats::detect::{AutoConfig, AutoReader, Format};
    use neoteletext_formats::item::Reader;

    fn session_of(packets: &[Packet]) -> Session {
        let mut session = Session::new(Config::default(), 512);
        for packet in packets {
            session.push(packet, &PacketMeta::default());
        }
        session.finish();
        session
    }

    /// REQ-INP-023 and REQ-TST-020: the authored stream (REQ-TST-024),
    /// converted from T42 to TTI and read back, gives the same stores:
    /// display pages with their packets 26 to 28, enhancement pages as
    /// received, and the magazine packets.
    #[test]
    fn authored_stream_converts_to_tti_and_back() {
        let stream = streams::stream().unwrap();
        let first = session_of(&stream.packets());
        let files = session_page_files(&first, None);
        let references: Vec<&PageFile> = files.iter().collect();
        let mut text = Vec::new();
        neoteletext_formats::write::write_tti(&references, &mut text).unwrap();
        let mut reader =
            AutoReader::new(AutoConfig { format: Some(Format::Tti), encode_pages: true, ..AutoConfig::default() });
        let mut packets = Vec::new();
        let mut collect = |item: Item<'_>| {
            if let Item::Packet { packet, .. } = item {
                packets.push(packet);
            }
        };
        reader.feed(&text, &mut collect);
        reader.finish(&mut collect);
        let second = session_of(&packets);

        let (left, right) = (first.pages(), second.pages());
        assert_eq!(left.len(), right.len());
        assert!(left.len() >= 30);
        for (a, b) in left.iter().zip(&right) {
            let name = a.key.address.mpp();
            assert_eq!(a.key, b.key);
            assert_eq!(a.cells, b.cells, "{name:X}");
            assert_eq!((a.enhancements, a.enhancements_present), (b.enhancements, b.enhancements_present), "{name:X}");
            assert_eq!(a.compositional_links, b.compositional_links, "{name:X}");
            assert_eq!(
                (a.page_data, a.drcs_colours, a.page_data_level35),
                (b.page_data, b.drcs_colours, b.page_data_level35)
            );
            assert_eq!(a.control, b.control, "{name:X}");
        }
        let stored = |session: &Session| {
            let store = session.decoder().enhancement_store();
            let mut pages: Vec<_> = store
                .sub_pages()
                .map(|page| {
                    let rows: Vec<_> = (1..=25).map(|packet| page.row(packet).copied()).collect();
                    let enhancements: Vec<_> = (0..16).map(|code| page.enhancement(code).copied()).collect();
                    (
                        page.key().address,
                        page.key().s1,
                        page.subcode(),
                        page.format(),
                        page.drcs_modes().copied(),
                        rows,
                        enhancements,
                        page.received(),
                    )
                })
                .collect();
            pages.sort_by_key(|page| (page.0, page.1));
            pages
        };
        let (left, right) = (stored(&first), stored(&second));
        assert_eq!(left.len(), 21);
        assert_eq!(left, right);
        for bits in 0..8 {
            let magazine = Magazine::from_bits(bits);
            assert_eq!(first.decoder().magazine_data(magazine), second.decoder().magazine_data(magazine));
        }
        assert!(first.decoder().magazine_data(Magazine::new(7).unwrap()).data.is_some());
    }

    /// REQ-INP-023: an enhancement page without X/28 is written in the
    /// coding its bytes show, and page mFE in Hamming 8/4.
    #[test]
    fn enhancement_pages_without_a_function_keep_their_bytes() {
        let stream = streams::stream().unwrap();
        // Leave out every X/28 so that only the bytes tell the coding.
        let packets: Vec<Packet> = stream
            .packets()
            .into_iter()
            .filter(|packet| {
                neoteletext_core::packet::decode_address(packet)
                    .map_or(true, |address| address.number != PACKET_PAGE_ENHANCEMENT)
            })
            .collect();
        let session = session_of(&packets);
        let files = session_page_files(&session, None);
        let table = files.iter().find(|file| file.address.mpp() == 0x1FE).unwrap();
        assert_eq!(table.function, None);
        let objects = files.iter().find(|file| file.address.mpp() == 0x1A0).unwrap();
        assert_eq!(objects.function, Some(PageFunction { function: PAGE_FUNCTION_POP, coding: CODING_TRIPLETS }));
    }
}
