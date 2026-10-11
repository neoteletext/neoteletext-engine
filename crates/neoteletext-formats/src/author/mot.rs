//! The Magazine Organization Table (clause 10.6, Figure 20, Tables 20 to
//! 23; REQ-INP-025): byte pairs placed at the position of their page
//! number, object links with default objects, DRCS links and the totals.

use std::boxed::Box;
use std::collections::BTreeMap;

use neoteletext_core::consts::{
    DATA_BYTES, DATA_START, LINK_NULL_PAGE, MOT_DRCS_LINK_BYTES, MOT_DRCS_LINK_FIRST_BYTE, MOT_DRCS_LINKS,
    MOT_DRCS_LINKS_LEVEL_25, MOT_DRCS_LINKS_LEVEL_35, MOT_OBJECT_LINK_BYTES, MOT_OBJECT_LINK_FIRST_BYTE,
    MOT_OBJECT_LINKS_LEVEL_25, MOT_OBJECT_LINKS_LEVEL_35, MOT_OBJECT_LINKS_PER_PACKET, MOT_TOTALS_BYTE,
    PACKET_FIRST_BYTE_NUMBER, PAGE_FUNCTION_MOT, PAGE_MOT,
};
use neoteletext_core::packet::{MotDrcsLink, MotObjectLink, MotPageEntry, MotTotals, encode, mot_position};
use neoteletext_core::types::{FallbackFields, Magazine, PageAddress, PageNumber, SidePanelFields, SubCode};

use super::{AuthorError, Result, enhancement_subcode, new_page};
use crate::consts::PAGE_CODING_NIBBLES;
use crate::encode::nibble_row;
use crate::page::{PageFile, PageFunction};

/// The packet byte number of the first data byte, byte 6: the tables give
/// positions as packet byte numbers.
const DATA_FIRST_BYTE_NUMBER: usize = PACKET_FIRST_BYTE_NUMBER + DATA_START;
/// Links of each kind at one level: the global link and links 1 to 7
/// (Tables 20 and 22).
const LINKS: usize = 8;

/// The level whose links a table packet carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MotLevel {
    /// Packets X/19 to X/21.
    Level25,
    /// Packets X/22 to X/24.
    Level35,
    /// The same link in both, as a service that sends X/22 to X/24 must
    /// do for every link a Level 3.5 decoder is to find (clause 10.6.1).
    Both,
}

impl MotLevel {
    /// The indices of the link sets this level writes.
    const fn indices(self) -> &'static [usize] {
        match self {
            Self::Level25 => &[0],
            Self::Level35 => &[1],
            Self::Both => &[0, 1],
        }
    }
}

/// The links of one level.
#[derive(Clone, Copy, Debug, Default)]
struct Links {
    objects: [Option<MotObjectLink>; LINKS],
    drcs: [Option<MotDrcsLink>; LINKS],
    totals: Option<MotTotals>,
}

/// A Magazine Organization Table: page mFE, sub-page 0.
#[derive(Clone, Debug)]
pub struct Mot {
    magazine: Magazine,
    update: u8,
    entries: BTreeMap<PageNumber, MotPageEntry>,
    levels: [Links; 2],
    omitted: u32,
}

/// An object link with no default objects and no fallback (Table 21).
#[must_use]
pub const fn object_link(target: PageAddress, subpage_count: u8) -> MotObjectLink {
    MotObjectLink {
        magazine: target.magazine,
        x27_flag: false,
        page: target.page,
        subpage_count,
        fallback: FallbackFields {
            side_panels: SidePanelFields { left: false, right: false, status_flag: true, left_columns: 0 },
            black_background_substitution: false,
        },
        default_objects: [None; 2],
    }
}

/// A DRCS link (Table 23).
#[must_use]
pub const fn drcs_link(target: PageAddress, subpage_count: u8) -> MotDrcsLink {
    MotDrcsLink { magazine: target.magazine, x27_flag: false, page: target.page, subpage_count }
}

impl Mot {
    /// An empty table for a magazine.
    #[must_use]
    pub const fn new(magazine: Magazine) -> Self {
        Self { magazine, update: 0, entries: BTreeMap::new(), levels: [Links::EMPTY; 2], omitted: 0 }
    }

    /// Sets the update indicator S2 (Annex A.1).
    pub fn update_indicator(&mut self, update: u8) -> &mut Self {
        self.update = update;
        self
    }

    /// The byte pair of a page: its POP and DRCS link numbers and global
    /// flags (clauses 10.6.2 and 10.6.3).
    pub fn page(&mut self, page: PageNumber, entry: MotPageEntry) -> &mut Self {
        self.entries.insert(page, entry);
        self
    }

    /// Object link `number` of a level: 0 is the GPOP link, 1 to 7 the POP
    /// links (Table 20).
    pub fn object_link(&mut self, level: MotLevel, number: usize, link: MotObjectLink) -> Result<&mut Self> {
        for &index in level.indices() {
            let links = self.levels.get_mut(index).ok_or(AuthorError::BadPosition)?;
            *links.objects.get_mut(number).ok_or(AuthorError::BadPosition)? = Some(link);
        }
        Ok(self)
    }

    /// DRCS link `number` of a level: 0 is the GDRCS link, 1 to 7 the
    /// DRCS links (Table 22).
    pub fn drcs_link(&mut self, level: MotLevel, number: usize, link: MotDrcsLink) -> Result<&mut Self> {
        for &index in level.indices() {
            let links = self.levels.get_mut(index).ok_or(AuthorError::BadPosition)?;
            *links.drcs.get_mut(number).ok_or(AuthorError::BadPosition)? = Some(link);
        }
        Ok(self)
    }

    /// The totals of enhancement pages of a level (clause 10.6.6).
    pub fn totals(&mut self, level: MotLevel, totals: MotTotals) -> &mut Self {
        for &index in level.indices() {
            if let Some(links) = self.levels.get_mut(index) {
                links.totals = Some(totals);
            }
        }
        self
    }

    /// Leaves packet X/`packet` out of the table, for the tests of missing
    /// packets (clause 10.6.1, decision E.52).
    pub fn omit_packet(&mut self, packet: u8) -> &mut Self {
        self.omitted |= 1u32.checked_shl(u32::from(packet)).unwrap_or(0);
        self
    }

    /// The page file. Packets X/1 to X/14 are sent where they hold a byte
    /// pair; the link packets of a level are sent where that level has any
    /// link or totals. Unset links are coded with page FF, "not used".
    pub fn build(&self) -> Result<Box<PageFile>> {
        let mut rows: BTreeMap<u8, [u8; DATA_BYTES]> = BTreeMap::new();
        for (&page, &entry) in &self.entries {
            let position = mot_position(page);
            let row = rows.entry(position.packet).or_insert([0; DATA_BYTES]);
            let [objects, drcs] = encode::mot_page_entry(entry);
            for (offset, value) in [objects, drcs].into_iter().enumerate() {
                *row.get_mut(position.index.saturating_add(offset)).ok_or(AuthorError::BadPosition)? = value;
            }
        }
        let unused = object_link(unused_address(self.magazine), 0);
        let unused_drcs = drcs_link(unused_address(self.magazine), 0);
        let packets = [
            (MOT_OBJECT_LINKS_LEVEL_25, MOT_DRCS_LINKS_LEVEL_25),
            (MOT_OBJECT_LINKS_LEVEL_35, MOT_DRCS_LINKS_LEVEL_35),
        ];
        for (links, (object_packets, drcs_packet)) in self.levels.iter().zip(packets) {
            let used = links.objects.iter().any(Option::is_some)
                || links.drcs.iter().any(Option::is_some)
                || links.totals.is_some();
            if !used {
                continue;
            }
            for (number, link) in links.objects.iter().enumerate() {
                let Some(&packet) = object_packets.get(number.checked_div(MOT_OBJECT_LINKS_PER_PACKET).unwrap_or(0))
                else {
                    continue;
                };
                let slot = number.checked_rem(MOT_OBJECT_LINKS_PER_PACKET).unwrap_or(0);
                let start = MOT_OBJECT_LINK_FIRST_BYTE
                    .saturating_add(slot.saturating_mul(MOT_OBJECT_LINK_BYTES))
                    .saturating_sub(DATA_FIRST_BYTE_NUMBER);
                let values: [u8; MOT_OBJECT_LINK_BYTES] = encode::mot_object_link(&link.unwrap_or(unused));
                write(rows.entry(packet).or_insert([0; DATA_BYTES]), start, &values)?;
            }
            let row = rows.entry(drcs_packet).or_insert([0; DATA_BYTES]);
            for (number, link) in links.drcs.iter().enumerate().take(MOT_DRCS_LINKS) {
                let start = MOT_DRCS_LINK_FIRST_BYTE
                    .saturating_add(number.saturating_mul(MOT_DRCS_LINK_BYTES))
                    .saturating_sub(DATA_FIRST_BYTE_NUMBER);
                write(row, start, &encode::mot_drcs_link(&link.unwrap_or(unused_drcs)))?;
            }
            if let Some(totals) = links.totals {
                write(row, MOT_TOTALS_BYTE.saturating_sub(DATA_FIRST_BYTE_NUMBER), &encode::mot_totals(&totals))?;
            }
        }
        let mut page =
            Box::new(new_page(PageAddress { magazine: self.magazine, page: PageNumber(PAGE_MOT) }, SubCode::new(0)));
        page.function = Some(PageFunction { function: PAGE_FUNCTION_MOT, coding: PAGE_CODING_NIBBLES });
        let mut last_packet = 0u8;
        for (&packet, values) in &rows {
            if self.omitted.checked_shr(u32::from(packet)).is_some_and(|bits| bits & 1 == 1) {
                continue;
            }
            page.set_row(usize::from(packet), &nibble_row(values));
            last_packet = packet;
        }
        // Annex A.1: S1 of the table is always 0.
        page.subcode = enhancement_subcode(last_packet, self.update, 0);
        Ok(page)
    }
}

/// Page FF of a magazine: a link that is not used.
const fn unused_address(magazine: Magazine) -> PageAddress {
    PageAddress { magazine, page: PageNumber(LINK_NULL_PAGE) }
}

fn write(row: &mut [u8; DATA_BYTES], start: usize, values: &[u8]) -> Result<()> {
    let target = row.get_mut(start..start.saturating_add(values.len())).ok_or(AuthorError::BadPosition)?;
    target.copy_from_slice(values);
    Ok(())
}

impl Links {
    const EMPTY: Self = Self { objects: [None; LINKS], drcs: [None; LINKS], totals: None };
}
