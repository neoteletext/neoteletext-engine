//! Authoring Level 2.5 and 3.5 test pages and streams by function rather
//! than by bit pattern (REQ-INP-025; feature `std`).
//!
//! Each builder produces [`PageFile`]s, which the TTI writer (REQ-INP-023)
//! and the page encoder (REQ-INP-024) turn into files and packets:
//!
//! - [`Triplets`]: local enhancement data and object bodies, every
//!   function of Tables 28 and 29 by name;
//! - [`DisplayPage`]: a Level 1 page with its packets X/26 laid out (Local
//!   Objects included, their numbers computed), X/27/4, X/27/5, X/28/0
//!   and X/28/4;
//! - [`ObjectPage`]: a POP or GPOP with its pointer tables computed;
//! - [`Mot`]: the Magazine Organization Table;
//! - [`MagazinePackets`]: M/29/0 and M/29/4 on a page mFF;
//! - [`Stream`]: a set of pages, written as T42 in the order of Annex B.7
//!   or as TTI files.
//!
//! The authored test stream of REQ-TST-024 is in [`streams`].

mod display;
mod mot;
mod objects;
pub mod streams;
mod triplets;

use std::boxed::Box;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::string::String;
use std::vec::Vec;

use neoteletext_core::consts::{
    C4_ERASE, DESIGNATION_LEVEL_35_DATA, DESIGNATION_PAGE_DATA, HEADER_DISPLAY_BYTES, PACKET_MAGAZINE_ENHANCEMENT,
    PAGE_TIME_FILLING, SPACE, TRIPLETS_PER_PACKET,
};
use neoteletext_core::ecc::with_odd_parity;
use neoteletext_core::packet::{Packet, Triplet, encode};
use neoteletext_core::types::{
    ColourWords, ControlBits, DefaultColourFields, Designations, Magazine, PageAddress, PageNumber, PresentationFields,
    Rgb12, SidePanelFields, SubCode,
};

pub use display::{
    DisplayPage, LinkFunction, PAGE_TRIPLETS, VALID_BOTH, VALID_LEVEL_25, VALID_LEVEL_35, link, no_link,
};
pub use mot::{Mot, MotLevel, object_link};
pub use objects::{ObjectPage, pointer_place, public_number};
pub use triplets::{FlashMode, FlashRate, LocalObject, ObjectRef, RowScope, Triplets, row_address};

use crate::consts::TTI_PS_TRANSMIT;
use crate::encode::{default_clut_2_and_3, encode_page, packed_line_of};
use crate::page::{PageFile, RawBody, RawPacket};
use crate::write::{in_transmission_order, write_tti};

/// Why a page could not be built.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum AuthorError {
    /// The local enhancement data and Local Objects need more than the
    /// 208 triplets of sixteen packets X/26.
    LocalDataFull,
    /// The objects of a sub-page need more than its 507 definition
    /// triplets (clause 10.5.1.3).
    ObjectSubPageFull,
    /// More than 32 objects of one type on a sub-page, or an index used
    /// twice.
    TooManyObjects,
    /// A sub-page number above 15.
    BadSubPage,
    /// A row, column or link position outside its range.
    BadPosition,
    /// An invocation of a Local Object that the page does not define, or
    /// one in a public object.
    UnknownLocalObject,
    /// More packed lines than a page file keeps.
    PageFull,
}

/// The result of a builder.
pub type Result<T> = core::result::Result<T, AuthorError>;

/// The Colour Map index of an entry of a CLUT (decision E.43).
#[must_use]
pub const fn colour(clut: u8, entry: u8) -> u8 {
    neoteletext_core::colour::colour_index(clut, entry)
}

/// A page address from its `mpp` form, such as `0x1A0`.
pub fn address(mpp: u16) -> Result<PageAddress> {
    PageAddress::from_mpp(mpp).ok_or(AuthorError::BadPosition)
}

/// The colour words of CLUTs 0 and 1 in Table 30, as X/28/4 and M/29/4
/// carry them.
#[must_use]
pub fn default_clut_0_and_1() -> ColourWords {
    let mut words = [Rgb12::default(); neoteletext_core::consts::COLOUR_WORDS];
    for (word, &value) in words.iter_mut().zip(neoteletext_core::consts::COLOUR_MAP_DEFAULTS.iter()) {
        *word = Rgb12(value);
    }
    words
}

/// Fields of X/28/0 format 1 or M/29/0 that change nothing: the Latin
/// default designation with national option `national`, no second G0 set,
/// no side panels, CLUTs 2 and 3 of Table 30, screen and row colour 0, no
/// substitution or re-mapping. Change the groups a page needs.
#[must_use]
pub fn neutral_fields(national: u8) -> PresentationFields {
    PresentationFields {
        designations: Some(Designations { default: national, second: neoteletext_core::consts::NO_SECOND_G0 }),
        side_panels: Some(SidePanelFields { left: false, right: false, status_flag: true, left_columns: 0 }),
        colour_words: Some(default_clut_2_and_3()),
        colours: Some(DefaultColourFields { screen: 0, row: 0, black_background_substitution: false, remapping: 0 }),
    }
}

/// [`neutral_fields`] for X/28/4 and M/29/4, whose colour words are CLUTs
/// 0 and 1.
#[must_use]
pub fn neutral_fields_level_35(national: u8) -> PresentationFields {
    PresentationFields { colour_words: Some(default_clut_0_and_1()), ..neutral_fields(national) }
}

/// An empty page file at an address, sent with C4 (erase page) set and
/// with the TTI "transmit page" status bit, which vbit2 needs.
fn new_page(address: PageAddress, subcode: SubCode) -> PageFile {
    let mut page = PageFile::new();
    page.address = address;
    page.subcode = subcode;
    page.status = TTI_PS_TRANSMIT;
    page.set_control(ControlBits(C4_ERASE));
    page
}

/// The sub-code of an enhancement page, `nnXs` (Annex A.1; REQ-NUM-022):
/// the last packet of an uninterrupted transmission, the update indicator
/// S2 and the sub-page S1, laid out by the one definition in `limits`.
#[must_use]
pub fn enhancement_subcode(last_packet: u8, update: u8, s1: u8) -> SubCode {
    SubCode::new(neoteletext_core::limits::enhancement_sub_code(last_packet, update, s1))
}

/// The packed `OL` line of a packet just built by the core's encoders,
/// which always decodes; the fallback only satisfies the type.
fn raw_line(packet: &Packet, number: u8, designation: u8) -> RawPacket {
    packed_line_of(packet).unwrap_or(RawPacket {
        number,
        designation,
        body: RawBody::Triplets([Triplet::default(); TRIPLETS_PER_PACKET]),
    })
}

/// Adds a packed line to a page.
fn push_raw(page: &mut PageFile, raw: RawPacket) -> Result<()> {
    if page.raw_packets.push(raw) { Ok(()) } else { Err(AuthorError::PageFull) }
}

/// The magazine packets M/29/0 and M/29/4 of a magazine, written
/// on its page mFF (REQ-INP-023).
#[derive(Clone, Copy, Debug)]
pub struct MagazinePackets {
    magazine: Magazine,
    data: Option<PresentationFields>,
    level_35: Option<PresentationFields>,
}

impl MagazinePackets {
    /// No packets for a magazine.
    #[must_use]
    pub const fn new(magazine: Magazine) -> Self {
        Self { magazine, data: None, level_35: None }
    }

    /// M/29/0 (Table 11).
    pub fn data(&mut self, fields: PresentationFields) -> &mut Self {
        self.data = Some(fields);
        self
    }

    /// M/29/4 (Table 13), whose colour words are CLUTs 0 and 1.
    pub fn level_35(&mut self, fields: PresentationFields) -> &mut Self {
        self.level_35 = Some(fields);
        self
    }

    /// The page mFF that carries the packets.
    pub fn build(&self) -> Result<Box<PageFile>> {
        let magazine = self.magazine;
        let mut page =
            Box::new(new_page(PageAddress { magazine, page: PageNumber(PAGE_TIME_FILLING) }, SubCode::new(0)));
        // A time-filling page is not erased; it only carries the packets.
        page.set_control(ControlBits(0));
        let number = PACKET_MAGAZINE_ENHANCEMENT;
        if let Some(fields) = self.data {
            let packet = encode::magazine_enhancement(magazine, DESIGNATION_PAGE_DATA, &fields);
            push_raw(&mut page, raw_line(&packet, number, DESIGNATION_PAGE_DATA))?;
        }
        if let Some(fields) = self.level_35 {
            let packet = encode::magazine_enhancement(magazine, DESIGNATION_LEVEL_35_DATA, &fields);
            push_raw(&mut page, raw_line(&packet, number, DESIGNATION_LEVEL_35_DATA))?;
        }
        Ok(page)
    }
}

/// A set of pages: written as a T42 stream in the order of Annex B.7, or
/// as TTI files (REQ-INP-023, REQ-INP-024).
#[derive(Clone, Debug, Default)]
pub struct Stream {
    // Boxed: a page file is about 21 kilobytes.
    pages: Vec<Box<PageFile>>,
}

impl Stream {
    /// An empty stream.
    #[must_use]
    pub const fn new() -> Self {
        Self { pages: Vec::new() }
    }

    /// Adds a page, as the builders return it.
    pub fn push(&mut self, page: Box<PageFile>) -> &mut Self {
        self.pages.push(page);
        self
    }

    /// Adds pages.
    pub fn extend(&mut self, pages: impl IntoIterator<Item = PageFile>) -> &mut Self {
        self.pages.extend(pages.into_iter().map(Box::new));
        self
    }

    /// The pages in the order they were added.
    #[must_use]
    pub fn pages(&self) -> Vec<&PageFile> {
        self.pages.iter().map(|page| &**page).collect()
    }

    /// The pages in the order of Annex B.7: magazine packets, Magazine
    /// Organization Tables, object pages, then Level 1 pages.
    #[must_use]
    pub fn in_transmission_order(&self) -> Vec<&PageFile> {
        in_transmission_order(&self.pages())
    }

    /// The packets of one transmission of every page, in the order of
    /// Annex B.7, followed by a time-filling header (page FF) in each
    /// magazine used, so that the last page of each magazine is complete
    /// (clause 7.2.1).
    #[must_use]
    pub fn packets(&self) -> Vec<Packet> {
        let mut packets = Vec::new();
        let mut magazines = BTreeSet::new();
        for page in self.in_transmission_order() {
            magazines.insert(page.address.magazine);
            encode_page(page, &mut |packet| packets.push(packet));
        }
        let display = [with_odd_parity(SPACE); HEADER_DISPLAY_BYTES];
        for magazine in magazines {
            packets.extend(encode::header(
                magazine,
                PageNumber(PAGE_TIME_FILLING),
                SubCode::new(0),
                ControlBits(0),
                &display,
            ));
        }
        packets
    }

    /// The stream as T42 bytes.
    #[must_use]
    pub fn t42(&self) -> Vec<u8> {
        self.packets().iter().flat_map(|packet| packet.bytes).collect()
    }

    /// Writes the stream as T42.
    pub fn write_t42(&self, out: &mut dyn Write) -> io::Result<()> {
        out.write_all(&self.t42())
    }

    /// Every page as one TTI file, in the order of Annex B.7, so that a
    /// reader that encodes pages as it reads them stores the enhancement
    /// pages before the pages that use them.
    pub fn tti(&self) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        write_tti(&self.in_transmission_order(), &mut out)?;
        Ok(out)
    }

    /// One TTI file per page address, named `mpp.tti`, holding its
    /// sub-pages in order.
    pub fn tti_files(&self) -> io::Result<Vec<(String, Vec<u8>)>> {
        let mut addresses: Vec<PageAddress> = Vec::new();
        for page in self.in_transmission_order() {
            if !addresses.contains(&page.address) {
                addresses.push(page.address);
            }
        }
        let mut files = Vec::new();
        for address in addresses {
            let pages: Vec<&PageFile> = self.pages().into_iter().filter(|page| page.address == address).collect();
            let mut out = Vec::new();
            write_tti(&pages, &mut out)?;
            let name: String = address.ascii().iter().map(|&byte| char::from(byte)).chain(".tti".chars()).collect();
            files.push((name, out));
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests;
