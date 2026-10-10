//! Display pages: the Level 1 page, its local enhancement data and Local
//! Objects, its compositional links X/27/4 and X/27/5, and its page-level
//! presentation data X/28/0, X/28/1 and X/28/4 (REQ-INP-025).

use std::boxed::Box;
use std::vec::Vec;

use tellytxt_core::consts::{
    COLUMNS, COMPOSITIONAL_LINKS, DESIGNATION_CODES, DESIGNATION_DRCS_COLOURS, DESIGNATION_LEVEL_35_DATA,
    DESIGNATION_LINKS_LEVEL_25, DESIGNATION_LINKS_LEVEL_35, DESIGNATION_PAGE_DATA, HEADER_DISPLAY_COLUMN,
    LINK_NULL_PAGE, PACKET_LINKS, PACKET_PAGE_ENHANCEMENT, ROWS, SPACE, TRIPLETS_PER_PACKET,
};
use tellytxt_core::packet::{
    CompositionalLink, DrcsColourData, ObjectKind, ObjectNumber, ObjectSource, ObjectUsage, PageFormat, Triplet,
    TripletFunction, encode,
};
use tellytxt_core::types::{ControlBits, Magazine, PageAddress, PageNumber, PresentationFields, SubCode};

use super::triplets::{LocalObject, ObjectRef, Triplets};
use super::{AuthorError, Result, new_page, raw_line};
use crate::consts::{
    TERMINATION_LOCAL_DATA_END, TERMINATION_LOCAL_DATA_OBJECTS_FOLLOW, TERMINATION_LOCAL_OBJECT_LAST,
    TERMINATION_LOCAL_OBJECT_MORE,
};
use crate::page::{EnhancementPacket, PageFile, PageFunction};

/// Triplets that the sixteen packets X/26 of a page can carry.
pub const PAGE_TRIPLETS: usize = DESIGNATION_CODES * TRIPLETS_PER_PACKET;

/// Shift of a local object's designation code in its number: N7 to N4
/// (Table 28).
const LOCAL_DESIGNATION_SHIFT: u32 = 4;

/// The function of a compositional link (Table 15, bits 2 and 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum LinkFunction {
    /// 00: the Global Public Object Page.
    Gpop,
    /// 01: the Public Object Page.
    Pop,
    /// 10: the Global DRCS page.
    Gdrcs,
    /// 11: the normal DRCS page.
    Drcs,
}

impl LinkFunction {
    /// The two bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        match self {
            Self::Gpop => 0b00,
            Self::Pop => 0b01,
            Self::Gdrcs => 0b10,
            Self::Drcs => 0b11,
        }
    }

    /// The position Table 16 gives this function among X/27/4 links 0 to 3.
    #[must_use]
    pub const fn level_25_position(self) -> usize {
        match self {
            Self::Gpop => 0,
            Self::Pop => 1,
            Self::Gdrcs => 2,
            Self::Drcs => 3,
        }
    }
}

/// Page validity of a link (Table 15, bits 4 and 3): required at Level 2.5
/// only.
pub const VALID_LEVEL_25: u8 = 0b01;
/// Required at Level 3.5 only.
pub const VALID_LEVEL_35: u8 = 0b10;
/// Required at both levels.
pub const VALID_BOTH: u8 = 0b11;

/// A Format 1 compositional link to `target`, with the sub-pages whose S1
/// bit is set in `subpages` (Table 15).
#[must_use]
pub const fn link(function: LinkFunction, validity: u8, target: PageAddress, subpages: u16) -> CompositionalLink {
    CompositionalLink {
        function: function.bits(),
        validity,
        magazine: target.magazine,
        page: target.page,
        subpages,
        marker_first: true,
        marker_second: false,
    }
}

/// The "no particular page" link: page FF with every sub-page flag set
/// (clause 9.6.2).
#[must_use]
pub const fn no_link(function: LinkFunction, magazine: Magazine) -> CompositionalLink {
    link(function, VALID_BOTH, PageAddress { magazine, page: PageNumber(LINK_NULL_PAGE) }, u16::MAX)
}

/// A Local Object waiting to be laid out.
#[derive(Clone, Debug)]
struct LocalDefinition {
    kind: ObjectKind,
    usage: ObjectUsage,
    body: Triplets,
}

/// A Level 1 page with the enhancement data of Levels 2.5 and 3.5.
#[derive(Clone, Debug)]
pub struct DisplayPage {
    // Boxed: a page file is about 21 kilobytes, and a stream builds many.
    page: Box<PageFile>,
    local: Triplets,
    objects: Vec<LocalDefinition>,
    links: [Option<[CompositionalLink; COMPOSITIONAL_LINKS]>; 2],
    page_data: Option<(PageFormat, PresentationFields)>,
    drcs_colours: Option<DrcsColourData>,
    level_35: Option<PresentationFields>,
    raw_enhancements: Vec<EnhancementPacket>,
    terminate: bool,
}

impl DisplayPage {
    /// An empty page at `address` with sub-code `subcode`, sent with C4
    /// (erase page) set.
    #[must_use]
    pub fn new(address: PageAddress, subcode: SubCode) -> Self {
        Self {
            page: Box::new(new_page(address, subcode)),
            local: Triplets::new(),
            objects: Vec::new(),
            links: [None; 2],
            page_data: None,
            drcs_colours: None,
            level_35: None,
            raw_enhancements: Vec::new(),
            terminate: true,
        }
    }

    /// Writes the header text: columns 8 to 39 of row 0.
    pub fn header(&mut self, text: &[u8]) -> &mut Self {
        let mut row = [SPACE; COLUMNS];
        for (slot, &code) in row.iter_mut().skip(HEADER_DISPLAY_COLUMN).zip(text) {
            *slot = code;
        }
        self.page.set_row(0, &row);
        self
    }

    /// Writes one code, a character or a spacing attribute, at a row 1 to
    /// 24 and column 0 to 39 of the Level 1 page.
    pub fn put(&mut self, row: u8, column: u8, code: u8) -> Result<&mut Self> {
        self.text(row, column, &[code])
    }

    /// Writes codes from a row and column of the Level 1 page; they must
    /// fit within the row.
    pub fn text(&mut self, row: u8, column: u8, codes: &[u8]) -> Result<&mut Self> {
        let row = usize::from(row);
        let start = usize::from(column);
        if row == 0 || row >= ROWS || start.saturating_add(codes.len()) > COLUMNS {
            return Err(AuthorError::BadPosition);
        }
        let mut line = self.page.row(row).copied().unwrap_or([SPACE; COLUMNS]);
        for (slot, &code) in line.iter_mut().skip(start).zip(codes) {
            *slot = code;
        }
        self.page.set_row(row, &line);
        Ok(self)
    }

    /// The national option index of C12 to C14 (decision E.21).
    pub fn national(&mut self, index: u8) -> &mut Self {
        self.page.set_national_index(index);
        self
    }

    /// Replaces the control bits.
    pub fn control(&mut self, control: ControlBits) -> &mut Self {
        self.page.set_control(control);
        self
    }

    /// The local enhancement data, written into packets X/26 from X/26/0
    /// triplet 0.
    pub fn local(&mut self) -> &mut Triplets {
        &mut self.local
    }

    /// Whether the local enhancement data ends with a Termination Marker
    /// (the default). Without one, a decoder reads to the end of the last
    /// packet X/26.
    pub fn terminate_local_data(&mut self, terminate: bool) -> &mut Self {
        self.terminate = terminate;
        self
    }

    /// Adds a Local Object, defined after the local enhancement data in
    /// the page's packets X/26 (clause 13.1). Its number is the place of
    /// its definition, known once the page is built.
    pub fn local_object(&mut self, kind: ObjectKind, usage: ObjectUsage, body: Triplets) -> LocalObject {
        let index = self.objects.len();
        self.objects.push(LocalDefinition { kind, usage, body });
        LocalObject { index, kind }
    }

    /// Sets link `position` (0 to 5) of X/27/4 (`designation` 4) or X/27/5
    /// (5). The other links of the packet are "no particular page".
    pub fn compositional_link(
        &mut self,
        designation: u8,
        position: usize,
        link: CompositionalLink,
    ) -> Result<&mut Self> {
        let index = match designation {
            DESIGNATION_LINKS_LEVEL_25 => 0,
            DESIGNATION_LINKS_LEVEL_35 => 1,
            _ => return Err(AuthorError::BadPosition),
        };
        let magazine = self.page.address.magazine;
        let functions = [LinkFunction::Gpop, LinkFunction::Pop, LinkFunction::Gdrcs, LinkFunction::Drcs];
        let slot = self.links.get_mut(index).ok_or(AuthorError::BadPosition)?;
        let links = slot.get_or_insert_with(|| {
            let mut empty = [no_link(LinkFunction::Gpop, magazine); COMPOSITIONAL_LINKS];
            for (link, function) in empty.iter_mut().zip(functions.iter().cycle()) {
                *link = no_link(*function, magazine);
            }
            empty
        });
        *links.get_mut(position).ok_or(AuthorError::BadPosition)? = link;
        Ok(self)
    }

    /// Sets the X/27/4 link of a function at the position Table 16 fixes
    /// for it, valid at both levels, to sub-pages `subpages` of `target`.
    pub fn link_level_25(&mut self, function: LinkFunction, target: PageAddress, subpages: u16) -> Result<&mut Self> {
        self.compositional_link(
            DESIGNATION_LINKS_LEVEL_25,
            function.level_25_position(),
            link(function, VALID_BOTH, target, subpages),
        )
    }

    /// Packet X/28/0 format 1 for a basic Level 1 page (Table 4).
    pub fn page_data(&mut self, fields: PresentationFields) -> &mut Self {
        self.page_data = Some((PageFormat { function: 0, coding: 0 }, fields));
        self
    }

    /// Packet X/28/0 format 1 with any page function and coding.
    pub fn page_data_with_format(&mut self, format: PageFormat, fields: PresentationFields) -> &mut Self {
        self.page_data = Some((format, fields));
        self
    }

    /// Packet X/28/1, the DRCS colour lookup tables (Table 8).
    pub fn drcs_colours(&mut self, data: DrcsColourData) -> &mut Self {
        self.drcs_colours = Some(data);
        self
    }

    /// Packet X/28/4, the Level 3.5 page data (Table 10): its colour words
    /// are CLUTs 0 and 1.
    pub fn level_35_data(&mut self, fields: PresentationFields) -> &mut Self {
        self.level_35 = Some(fields);
        self
    }

    /// Sends this packet X/26 as written instead of the laid-out local
    /// enhancement data, for damaged or reserved data.
    pub fn raw_enhancement(&mut self, designation: u8, triplets: [Triplet; TRIPLETS_PER_PACKET]) -> &mut Self {
        self.raw_enhancements.push(EnhancementPacket { designation, triplets });
        self
    }

    /// The triplets of the packets X/26: the local data, its Termination
    /// Marker, then each Local Object's definition, body and marker
    /// (clause 13.1 and Table 28). Unused triplets of the last packet
    /// repeat the last marker (clause 12.3.1).
    fn lay_out(&self) -> Result<Vec<Triplet>> {
        // The place of each definition is fixed by the lengths alone, so
        // invocations can be resolved before anything is written.
        // No local data and no objects: no packet X/26 at all, since any
        // X/26 cancels the default objects (clause 13.8).
        let marker = usize::from((self.terminate && !self.local.is_empty()) || !self.objects.is_empty());
        let mut next = self.local.len().saturating_add(marker);
        let mut places = Vec::with_capacity(self.objects.len());
        for object in &self.objects {
            places.push(next);
            next = next.saturating_add(object.body.len()).saturating_add(2);
        }
        if next > PAGE_TRIPLETS {
            return Err(AuthorError::LocalDataFull);
        }
        let number = |place: usize| -> ObjectNumber {
            let designation = place.checked_div(TRIPLETS_PER_PACKET).unwrap_or(0);
            let triplet = place.checked_rem(TRIPLETS_PER_PACKET).unwrap_or(0);
            ObjectNumber(((designation as u16) << LOCAL_DESIGNATION_SHIFT) | triplet as u16)
        };
        let resolve = |object: LocalObject, kind: ObjectKind| -> Result<Triplet> {
            let place = places.get(object.index).copied().ok_or(AuthorError::UnknownLocalObject)?;
            Ok(ObjectRef { source: ObjectSource::Local, kind, number: number(place) }.invocation().to_triplet())
        };
        let mut out = self.local.resolved(&resolve)?;
        let mut last_marker = TERMINATION_LOCAL_DATA_END;
        if !self.objects.is_empty() {
            last_marker = TERMINATION_LOCAL_DATA_OBJECTS_FOLLOW;
        }
        if marker == 1 {
            out.push(TripletFunction::Termination { kind: last_marker }.to_triplet());
        }
        for (index, (object, &place)) in self.objects.iter().zip(&places).enumerate() {
            let definition =
                TripletFunction::ObjectDefinition { usage: object.usage, kind: object.kind, number: number(place) };
            out.push(definition.to_triplet());
            out.extend(object.body.resolved(&resolve)?);
            last_marker = if index.saturating_add(1) == self.objects.len() {
                TERMINATION_LOCAL_OBJECT_LAST
            } else {
                TERMINATION_LOCAL_OBJECT_MORE
            };
            out.push(TripletFunction::Termination { kind: last_marker }.to_triplet());
        }
        if !out.is_empty() {
            let filler = TripletFunction::Termination { kind: last_marker }.to_triplet();
            while !out.len().is_multiple_of(TRIPLETS_PER_PACKET) {
                out.push(filler);
            }
        }
        Ok(out)
    }

    /// The page file: the Level 1 rows, the packets X/26 laid out, and the
    /// packets 27 and 28 as `OL` lines.
    pub fn build(&self) -> Result<Box<PageFile>> {
        let mut page = self.page.clone();
        if self.raw_enhancements.is_empty() {
            let triplets = self.lay_out()?;
            for (designation, chunk) in (0u8..).zip(triplets.chunks(TRIPLETS_PER_PACKET)) {
                let mut packet = EnhancementPacket { designation, triplets: [Triplet::default(); TRIPLETS_PER_PACKET] };
                for (slot, &triplet) in packet.triplets.iter_mut().zip(chunk) {
                    *slot = triplet;
                }
                if !page.enhancements.push(packet) {
                    return Err(AuthorError::LocalDataFull);
                }
            }
        } else {
            for packet in &self.raw_enhancements {
                if !page.enhancements.push(*packet) {
                    return Err(AuthorError::LocalDataFull);
                }
            }
        }
        let magazine = page.address.magazine;
        for (designation, links) in [DESIGNATION_LINKS_LEVEL_25, DESIGNATION_LINKS_LEVEL_35].into_iter().zip(self.links)
        {
            if let Some(links) = links {
                let packet = encode::compositional_links(magazine, designation, &links);
                super::push_raw(&mut page, raw_line(&packet, PACKET_LINKS, designation))?;
            }
        }
        if let Some((format, fields)) = self.page_data {
            let packet = encode::page_enhancement(magazine, DESIGNATION_PAGE_DATA, format, &fields);
            super::push_raw(&mut page, raw_line(&packet, PACKET_PAGE_ENHANCEMENT, DESIGNATION_PAGE_DATA))?;
            page.function = Some(PageFunction { function: format.function, coding: format.coding });
        }
        if let Some(data) = self.drcs_colours {
            let packet = encode::drcs_colours(magazine, PACKET_PAGE_ENHANCEMENT, &data);
            super::push_raw(&mut page, raw_line(&packet, PACKET_PAGE_ENHANCEMENT, DESIGNATION_DRCS_COLOURS))?;
        }
        if let Some(fields) = self.level_35 {
            let format = self.page_data.map_or(PageFormat { function: 0, coding: 0 }, |(format, _)| format);
            let packet = encode::page_enhancement(magazine, DESIGNATION_LEVEL_35_DATA, format, &fields);
            super::push_raw(&mut page, raw_line(&packet, PACKET_PAGE_ENHANCEMENT, DESIGNATION_LEVEL_35_DATA))?;
            // REQ-INP-023: an X/28/4 line sets the page function too, so the
            // built page says so, as a file read back would.
            page.function = Some(PageFunction { function: format.function, coding: format.coding });
        }
        Ok(page)
    }
}
