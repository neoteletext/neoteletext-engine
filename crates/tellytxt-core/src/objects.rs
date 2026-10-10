//! Objects and the pages that carry them (area OBJ, layer L3; clauses 10.5,
//! 10.6 and 13; decisions E.51 to E.53).
//!
//! This module finds objects for the enhancement processor:
//!
//! - [`links`]: the enhancement pages a page uses. Its own X/27/4 and, at
//!   Level 3.5, X/27/5 take precedence role by role; the Magazine
//!   Organization Table of its magazine gives the rest, the fallback bits
//!   and the default objects (REQ-OBJ-030, REQ-OBJ-031, REQ-OBJ-032,
//!   REQ-OBJ-040);
//! - [`find_local`] and [`find_public`]: an object's definition in the
//!   page's own packets X/26 (REQ-OBJ-003), or through the pointer tables of
//!   a Public or Global Public Object Page (REQ-OBJ-004), checked for type
//!   and usage (REQ-OBJ-005);
//! - [`prescan`]: an object's body, where it ends and the last position it
//!   addresses, before it runs (REQ-OBJ-006, REQ-OBJ-020);
//! - [`Body::fetch`]: the triplets of local enhancement data and of object
//!   bodies, one at a time.
//!
//! It reads stored pages only through the read-only lookup of the
//! acquisition layer (REQ-ACQ-020), and decodes their bytes only with the
//! packet layer's decoders and [`TripletFunction`] (REQ-PKT-010,
//! REQ-PKT-017, REQ-PKT-018). Nothing here allocates; every loop is bounded
//! by the 507 triplets of an object page or the 208 of a page's X/26.

use crate::acq::{EnhancementLookup, EnhancementPage, StoredPage};
use crate::config::PresentationLevel;
use crate::consts::{
    COMPOSITIONAL_LINKS, DEFAULT_OBJECTS, DESIGNATION_CODES, LEVEL_BIT_25, LEVEL_BIT_35, LINK_FUNCTION_DRCS,
    LINK_FUNCTION_GDRCS, LINK_FUNCTION_GPOP, LINK_FUNCTION_POP, LINK_NULL_PAGE, LINKS_FIXED_ROLES, LINKS_X27_5_USED,
    LOCAL_TRIPLETS, MOT_DRCS_LINKS_LEVEL_25, MOT_DRCS_LINKS_LEVEL_35, MOT_OBJECT_LINKS_LEVEL_25,
    MOT_OBJECT_LINKS_LEVEL_35, MOT_OBJECT_LINKS_PER_PACKET, OBJECT_BODY_TRIPLETS, OBJECT_POINTER_FIRST_PACKET,
    OBJECT_POINTER_GROUP_TRIPLETS, OBJECT_POINTER_LAST_PACKET, OBJECT_POINTER_UNUSED, ROW_ADDRESS_BASE,
    ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24, ROWS, TRIPLET_BYTES, TRIPLETS_PER_PACKET,
};
use crate::ecc::hamming2418;
use crate::packet::{
    CompositionalLink, ObjectKind, ObjectNumber, ObjectPacket, ObjectSource, Triplet, TripletFunction, TripletLocation,
    mot_drcs_link, mot_object_link, mot_page_entry, mot_position, triplet_location,
};
use crate::types::{FallbackFields, Magazine, PageNumber};

/// A link to an object or DRCS page: its magazine and page number. The
/// sub-page is named by each invocation (decision E.52).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PageLink {
    /// The magazine of the linked page.
    pub magazine: Magazine,
    /// The page number.
    pub page: PageNumber,
}

/// A default object of the Magazine Organization Table (clause 13.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct DefaultInvocation {
    /// GPOP or POP.
    pub source: ObjectSource,
    /// The object type.
    pub kind: ObjectKind,
    /// The object number, with N8 = 0 (REQ-OBJ-040).
    pub number: ObjectNumber,
    /// The object page: the table's own GPOP or POP link, within whose
    /// sub-pages clause 13.8 says the object exists.
    pub page: PageLink,
}

/// The enhancement pages a displayed page uses (REQ-OBJ-030, REQ-OBJ-031).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct EnhancementLinks {
    /// The Global Public Object Page.
    pub gpop: Option<PageLink>,
    /// The Public Object Page.
    pub pop: Option<PageLink>,
    /// The Global DRCS page.
    pub gdrcs: Option<PageLink>,
    /// The normal DRCS page.
    pub drcs: Option<PageLink>,
    /// The fallback bits of the page's POP link in the table, for side
    /// panels and Black Background Colour Substitution; `None` without a POP
    /// link (decision E.53).
    pub fallback: Option<FallbackFields>,
    /// The default objects in the order of Annex G.5: GPOP 1, GPOP 2, POP 1,
    /// POP 2 (decision E.53).
    pub default_objects: [Option<DefaultInvocation>; DEFAULT_OBJECTS],
    /// The level the page is processed at. A Level 3.5 decoder processes a
    /// page at Level 2.5 when the table lacks X/22 or X/24 (clause 10.6.1).
    pub level: PresentationLevel,
}

impl EnhancementLinks {
    /// No enhancement pages, at a level.
    #[must_use]
    pub const fn none(level: PresentationLevel) -> Self {
        Self {
            gpop: None,
            pop: None,
            gdrcs: None,
            drcs: None,
            fallback: None,
            default_objects: [None; DEFAULT_OBJECTS],
            level,
        }
    }

    /// The object page of a public or global source.
    #[must_use]
    pub const fn object_page(&self, source: ObjectSource) -> Option<PageLink> {
        match source {
            ObjectSource::Public => self.pop,
            ObjectSource::Global => self.gpop,
            ObjectSource::Local | ObjectSource::Illegal => None,
        }
    }
}

/// Roles of the links, in the order of their function codes (Table 15).
const ROLES: usize = 4;

/// True when a link of X/27/4 or X/27/5 may be used at a level: Format 1
/// markers as Table 15 requires, not null, and a page validity that
/// includes the level (decision E.52).
fn usable(link: CompositionalLink, level: PresentationLevel) -> bool {
    let bit = if level.is_level35() { LEVEL_BIT_35 } else { LEVEL_BIT_25 };
    link.is_format_1() && !link.is_null() && link.validity & bit != 0
}

/// The roles that the page's own X/27/4 and X/27/5 give, indexed by link
/// function (REQ-OBJ-030, decision E.52).
fn compositional(page: &StoredPage, level: PresentationLevel) -> [Option<PageLink>; ROLES] {
    let mut roles = [None; ROLES];
    let mut accept = |link: &CompositionalLink| {
        if !usable(*link, level) {
            return;
        }
        // Table 16 note: a duplicate setting is invalid, so the first wins.
        if let Some(slot) = roles.get_mut(usize::from(link.function)) {
            slot.get_or_insert(PageLink { magazine: link.magazine, page: link.page });
        }
    };
    let [x27_4, x27_5] = page.compositional_links;
    if let Some(links) = x27_4 {
        // Links 0 to 3 have fixed roles; a link whose function bits do not
        // match its position is ignored (Table 16).
        for (position, link) in links.iter().enumerate().take(LINKS_FIXED_ROLES) {
            if let Some(link) = link
                && usize::from(link.function) == position
            {
                accept(link);
            }
        }
        if level.is_level35() {
            for link in links.iter().skip(LINKS_FIXED_ROLES).take(COMPOSITIONAL_LINKS).flatten() {
                accept(link);
            }
        }
    }
    if level.is_level35()
        && let Some(links) = x27_5
    {
        for link in links.iter().take(LINKS_X27_5_USED).flatten() {
            accept(link);
        }
    }
    roles
}

/// A table link to page FF is not used (Tables 21 and 23).
fn table_link(magazine: Magazine, page: PageNumber) -> Option<PageLink> {
    (page.0 != LINK_NULL_PAGE).then_some(PageLink { magazine, page })
}

/// What the Magazine Organization Table gives a page.
struct TableLinks {
    roles: [Option<PageLink>; ROLES],
    fallback: Option<FallbackFields>,
    defaults: [Option<DefaultInvocation>; DEFAULT_OBJECTS],
    level: PresentationLevel,
}

/// Reads the Magazine Organization Table of the page's magazine for the
/// page (REQ-OBJ-031, decision E.52).
fn table(lookup: &EnhancementLookup<'_>, magazine: Magazine, page: PageNumber, level: PresentationLevel) -> TableLinks {
    let mut out = TableLinks { roles: [None; ROLES], fallback: None, defaults: [None; DEFAULT_OBJECTS], level };
    let Some(mot) = lookup.magazine_organization_table(magazine) else { return out };
    // Clause 10.6.1: a Level 3.5 decoder uses X/22 to X/24, and with X/22 or
    // X/24 missing uses X/19 to X/21 and processes the page at Level 2.5.
    let [first_35, second_35] = MOT_OBJECT_LINKS_LEVEL_35;
    let [first_25, second_25] = MOT_OBJECT_LINKS_LEVEL_25;
    let level_35 = level.is_level35() && mot.row(first_35).is_some() && mot.row(MOT_DRCS_LINKS_LEVEL_35).is_some();
    if level.is_level35() && !level_35 {
        out.level = PresentationLevel::Level25;
    }
    let (first, second, drcs_packet) = if level_35 {
        // Decision E.52: with only X/23 missing, POP links 4 to 7 come from
        // the Level 2.5 links of X/20.
        let second = if mot.row(second_35).is_some() { second_35 } else { second_25 };
        (first_35, second, MOT_DRCS_LINKS_LEVEL_35)
    } else {
        (first_25, second_25, MOT_DRCS_LINKS_LEVEL_25)
    };
    // The byte pair of page xFF is ignored (decision E.52).
    if page.0 == LINK_NULL_PAGE {
        return out;
    }
    let position = mot_position(page);
    let Some(entry) = mot.row(position.packet).and_then(|row| mot_page_entry(row, position.index)) else {
        return out;
    };
    let object_link = |number: usize| {
        let (packet, link) = if number < MOT_OBJECT_LINKS_PER_PACKET {
            (first, number)
        } else {
            (second, number.saturating_sub(MOT_OBJECT_LINKS_PER_PACKET))
        };
        mot.row(packet).and_then(|row| mot_object_link(row, link))
    };
    // Decision E.52: the global flags are acquisition hints; the GPOP and
    // GDRCS are reached through the magazine's links whatever they say.
    let gpop = object_link(0);
    let pop = (entry.pop_link != 0).then(|| object_link(usize::from(entry.pop_link))).flatten();
    let drcs_link = |number: u8| mot.row(drcs_packet).and_then(|row| mot_drcs_link(row, usize::from(number)));
    let gdrcs = drcs_link(0);
    let drcs = (entry.drcs_link != 0).then(|| drcs_link(entry.drcs_link)).flatten();

    let mut defaults = out.defaults.iter_mut();
    for (link, source) in [(gpop, ObjectSource::Global), (pop, ObjectSource::Public)] {
        let Some(link) = link else { continue };
        let Some(page_link) = table_link(link.magazine, link.page) else { continue };
        for object in link.default_objects {
            let invocation = object.map(|object| DefaultInvocation {
                source,
                kind: object.kind,
                number: ObjectNumber(u16::from(object.number)),
                page: page_link,
            });
            if let Some(slot) = defaults.next() {
                *slot = invocation;
            }
        }
        if source == ObjectSource::Public {
            // Decision E.53: the fallback bits come from the page's POP link.
            out.fallback = Some(link.fallback);
        }
    }
    out.roles = [
        gpop.and_then(|link| table_link(link.magazine, link.page)),
        pop.and_then(|link| table_link(link.magazine, link.page)),
        gdrcs.and_then(|link| table_link(link.magazine, link.page)),
        drcs.and_then(|link| table_link(link.magazine, link.page)),
    ];
    out
}

/// The enhancement pages a page uses (REQ-OBJ-030 to REQ-OBJ-032,
/// REQ-OBJ-040). Each role comes from the page's own X/27/4 or X/27/5 when
/// they give it, whatever the table's X/27/4,5 flag says, and otherwise
/// from the Magazine Organization Table of the page's magazine. The
/// fallback bits and the default objects come from the table.
#[must_use]
pub fn links(page: &StoredPage, lookup: &EnhancementLookup<'_>, level: PresentationLevel) -> EnhancementLinks {
    let address = page.key.address;
    let table = table(lookup, address.magazine, address.page, level);
    let own = compositional(page, table.level);
    let role = |function: u8| {
        let index = usize::from(function);
        own.get(index).copied().flatten().or_else(|| table.roles.get(index).copied().flatten())
    };
    EnhancementLinks {
        gpop: role(LINK_FUNCTION_GPOP),
        pop: role(LINK_FUNCTION_POP),
        gdrcs: role(LINK_FUNCTION_GDRCS),
        drcs: role(LINK_FUNCTION_DRCS),
        fallback: table.fallback,
        default_objects: table.defaults,
        level: table.level,
    }
}

/// Where a run of triplets comes from: the page's own packets X/26, or an
/// object page's packets 3 to 25 and X/26.
#[derive(Clone, Copy, Debug)]
pub enum Body<'s> {
    /// The displayed page's packets X/26: triplet i is triplet i mod 13 of
    /// the packet with designation code i / 13.
    Local,
    /// An object page sub-page: triplet i is the absolute triplet number of
    /// clause 10.5.1.2.
    Page(EnhancementPage<'s>),
}

/// One triplet of a body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fetch {
    /// A triplet that passed its Hamming 24/18 check.
    Triplet(Triplet),
    /// An uncorrectable triplet.
    Damaged,
    /// The packet holding it was not received.
    Missing,
}

/// The triplet of a stored packet: data byte 0 is the Hamming 8/4 byte 6,
/// then thirteen triplets of three bytes.
fn stored_triplet(bytes: &[u8], triplet: u8) -> Fetch {
    let start = usize::from(triplet).saturating_mul(TRIPLET_BYTES).saturating_add(1);
    match bytes.get(start..start.saturating_add(TRIPLET_BYTES)) {
        Some(&[a, b, c]) => {
            hamming2418([a, b, c]).map_or(Fetch::Damaged, |value| Fetch::Triplet(Triplet::from_value(value)))
        }
        _ => Fetch::Damaged,
    }
}

impl Body<'_> {
    /// The number of triplets the body can address: 208 for X/26, 507 for
    /// an object page.
    #[must_use]
    pub const fn capacity(&self) -> u16 {
        match self {
            Self::Local => LOCAL_TRIPLETS,
            Self::Page(_) => OBJECT_BODY_TRIPLETS,
        }
    }

    /// True for the local enhancement data and local objects.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    /// Triplet `index` of the body.
    #[must_use]
    pub fn fetch(&self, page: &StoredPage, index: u16) -> Fetch {
        let per_packet = TRIPLETS_PER_PACKET as u16;
        match self {
            Self::Local => {
                let designation = usize::from(index.checked_div(per_packet).unwrap_or(0));
                let triplet = usize::from(index.checked_rem(per_packet).unwrap_or(0));
                let received =
                    page.enhancements_present.checked_shr(designation as u32).is_some_and(|bits| bits & 1 == 1);
                if designation >= DESIGNATION_CODES || !received {
                    return Fetch::Missing;
                }
                match page.enhancements.get(designation).and_then(|packet| packet.get(triplet)) {
                    Some(Some(triplet)) => Fetch::Triplet(*triplet),
                    Some(None) => Fetch::Damaged,
                    None => Fetch::Missing,
                }
            }
            Self::Page(sub_page) => {
                let bytes = match triplet_location(index) {
                    Some(TripletLocation::Row { packet, triplet }) => {
                        sub_page.row(packet).map(|bytes| (bytes, triplet))
                    }
                    Some(TripletLocation::Enhancement { designation, triplet }) => {
                        sub_page.enhancement(designation).map(|bytes| (bytes, triplet))
                    }
                    None => None,
                };
                bytes.map_or(Fetch::Missing, |(bytes, triplet)| stored_triplet(bytes, triplet))
            }
        }
    }
}

/// An object found and checked, ready to scan and run.
#[derive(Clone, Copy, Debug)]
pub struct ObjectRef<'s> {
    /// Its type.
    pub kind: ObjectKind,
    /// Where its triplets are.
    pub body: Body<'s>,
    /// The index of its first triplet: the one after the definition.
    pub first: u16,
}

/// Why an invocation finds no object (REQ-OBJ-003 to REQ-OBJ-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Absent {
    /// Source 00, or a local object whose number has N8 set or names
    /// triplet 13 to 15 (decision E.51).
    BadNumber,
    /// The page names no object page for the source.
    NoLink,
    /// The object page or its sub-page is not stored, or a packet it needs
    /// is missing.
    NotReceived,
    /// The pointer packet does not carry pointers, or the pointer is
    /// unused or not a valid triplet address (decision E.51).
    NoPointer,
    /// The triplet found is not an Object Definition, or is of another type
    /// (decision E.51).
    NotDefinition,
    /// The definition's usage bits exclude the presentation level.
    WrongLevel,
}

/// True when an Object Definition's usage bits include the level
/// (REQ-OBJ-005): 01 Level 2.5, 10 Level 3.5, 11 both.
fn usage_includes(usage: crate::packet::ObjectUsage, level: PresentationLevel) -> bool {
    if level.is_level35() { usage.level_35() } else { usage.level_25() }
}

/// Checks the triplet a definition was looked for at (REQ-OBJ-005). The
/// object number bits are not compared with the invocation's (decision
/// E.51).
fn check_definition<'s>(
    body: Body<'s>,
    page: &StoredPage,
    index: u16,
    kind: ObjectKind,
    level: PresentationLevel,
) -> Result<ObjectRef<'s>, Absent> {
    match body.fetch(page, index) {
        Fetch::Triplet(triplet) => match triplet.function() {
            TripletFunction::ObjectDefinition { usage, kind: defined, .. } if defined == kind => {
                if usage_includes(usage, level) {
                    Ok(ObjectRef { kind, body, first: index.saturating_add(1) })
                } else {
                    Err(Absent::WrongLevel)
                }
            }
            _ => Err(Absent::NotDefinition),
        },
        Fetch::Damaged => Err(Absent::NotDefinition),
        Fetch::Missing => Err(Absent::NotReceived),
    }
}

/// Finds a local object (REQ-OBJ-003): N7 to N4 name the packet X/26 and N3
/// to N0 the triplet. N8 shall be 0, and triplets 13 to 15 do not exist.
pub fn find_local(
    page: &StoredPage,
    kind: ObjectKind,
    number: ObjectNumber,
    level: PresentationLevel,
) -> Result<ObjectRef<'static>, Absent> {
    let triplet = number.triplet();
    if number.n8() || usize::from(triplet) >= TRIPLETS_PER_PACKET {
        return Err(Absent::BadNumber);
    }
    let index =
        u16::from(number.designation()).saturating_mul(TRIPLETS_PER_PACKET as u16).saturating_add(u16::from(triplet));
    check_definition(Body::Local, page, index, kind, level)
}

/// The type's offset within a pointer group: 0 Active, 1 Adaptive, 2
/// Passive (Table 28).
const fn type_offset(kind: ObjectKind) -> u8 {
    match kind {
        ObjectKind::Active => 0,
        ObjectKind::Adaptive => 1,
        ObjectKind::Passive => 2,
    }
}

/// Finds a public or global object through its object page (REQ-OBJ-004):
/// the sub-page S1 = N3 to N0, pointer packet 1 + N8 N7, pointer triplet
/// 1 + 3 × N6 N5 + type, and the half that N4 selects.
pub fn find_public<'s>(
    lookup: &EnhancementLookup<'s>,
    link: Option<PageLink>,
    page: &StoredPage,
    kind: ObjectKind,
    number: ObjectNumber,
    level: PresentationLevel,
) -> Result<ObjectRef<'s>, Absent> {
    let link = link.ok_or(Absent::NoLink)?;
    let sub_page = lookup.sub_page(link.magazine, link.page, number.subpage()).ok_or(Absent::NotReceived)?;
    let pointer_packet = OBJECT_POINTER_FIRST_PACKET.saturating_add(number.pointer_packet_index());
    let pointers = sub_page.row(pointer_packet).map(ObjectPacket::new).ok_or(Absent::NotReceived)?;
    if !pointers.carries_pointers() {
        return Err(Absent::NoPointer);
    }
    let triplet = usize::from(number.pointer_group())
        .saturating_mul(usize::from(OBJECT_POINTER_GROUP_TRIPLETS))
        .saturating_add(usize::from(type_offset(kind)))
        .saturating_add(1);
    let pointer = pointers.pointer(triplet, number.pointer_odd());
    if pointer == OBJECT_POINTER_UNUSED {
        return Err(Absent::NoPointer);
    }
    // Decision E.51: values 507 to 510 and pointers into a pointer packet
    // address no definition.
    match triplet_location(pointer) {
        None => return Err(Absent::NoPointer),
        Some(TripletLocation::Row { packet, .. }) if packet <= OBJECT_POINTER_LAST_PACKET => {
            let carries = sub_page.row(packet).is_some_and(|row| ObjectPacket::new(row).carries_pointers());
            if carries {
                return Err(Absent::NoPointer);
            }
        }
        Some(_) => {}
    }
    check_definition(Body::Page(sub_page), page, pointer, kind, level)
}

/// The row an address of a row triplet sets: 40 means row 24, 41 to 63
/// rows 1 to 23 (clause 12.3.2).
#[must_use]
pub const fn row_of(address: u8) -> u8 {
    if address == ROW_ADDRESS_ROW_24 { (ROWS - 1) as u8 } else { address.saturating_sub(ROW_ADDRESS_BASE) }
}

/// What a scan of an object body finds (REQ-OBJ-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Extent {
    /// The index one past its last triplet: the Object Definition or
    /// Termination Marker that ends it, or the end of the data.
    pub end: u16,
    /// The last position it addresses in scan order, relative to its
    /// origin, `(row, column)`; `None` when it addresses no cell.
    pub last: Option<(u8, u8)>,
}

/// Scans an object body before it runs (REQ-OBJ-006): it ends at the next
/// Object Definition or Termination Marker, or after 507 triplets, and is
/// absent when a packet before its end was not received. The active
/// position is followed as the walk will follow it, so that the last
/// position addressed bounds the object's open area (Annex A.8). In
/// Adaptive and Passive objects, Full Row Colour and Address Display Row 0
/// are ignored entirely (decision E.51), so they move nothing here either.
pub fn prescan(object: &ObjectRef<'_>, page: &StoredPage) -> Result<Extent, Absent> {
    let restricted = matches!(object.kind, ObjectKind::Adaptive | ObjectKind::Passive);
    let limit = object.first.saturating_add(OBJECT_BODY_TRIPLETS).min(object.body.capacity());
    let (mut row, mut column) = (0u8, 0u8);
    let mut last: Option<(u8, u8)> = None;
    let mut index = object.first;
    while index < limit {
        let triplet = match object.body.fetch(page, index) {
            Fetch::Triplet(triplet) => triplet,
            Fetch::Damaged => {
                index = index.saturating_add(1);
                continue;
            }
            Fetch::Missing => return Err(Absent::NotReceived),
        };
        let function = triplet.function();
        match function {
            TripletFunction::Termination { .. } | TripletFunction::ObjectDefinition { .. } => break,
            TripletFunction::FullRowColour { address, .. } if !restricted => (row, column) = (row_of(address), 0),
            TripletFunction::AddressDisplayRow0 { address: ROW_ADDRESS_LAST, .. } if !restricted => {
                (row, column) = (0, 0);
            }
            TripletFunction::SetActivePosition { address, column: data } => {
                row = row_of(address);
                if usize::from(data) < crate::consts::COLUMNS {
                    column = data;
                }
            }
            _ => {
                let Some(new_column) = function.active_column() else {
                    index = index.saturating_add(1);
                    continue;
                };
                column = new_column;
            }
        }
        // Every triplet that moves the active position addresses it.
        if last.is_none_or(|before| (row, column) > before) {
            last = Some((row, column));
        }
        index = index.saturating_add(1);
    }
    Ok(Extent { end: index.min(limit), last })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::packet::ObjectUsage;

    fn definition(kind: ObjectKind, usage: u8) -> Triplet {
        TripletFunction::ObjectDefinition { usage: ObjectUsage(usage), kind, number: ObjectNumber(0) }.to_triplet()
    }

    fn page_with(triplets: &[(usize, usize, Triplet)]) -> StoredPage {
        let mut page = StoredPage::EMPTY;
        for &(designation, index, triplet) in triplets {
            page.enhancements[designation][index] = Some(triplet);
            page.enhancements_present |= 1 << designation;
        }
        page
    }

    /// REQ-OBJ-003: N7 to N4 and N3 to N0 name the definition; N8 set,
    /// triplets 13 to 15 and missing packets are skipped.
    #[test]
    fn local_objects_are_found_by_designation_and_triplet() {
        let page = page_with(&[(1, 5, definition(ObjectKind::Passive, 0b11))]);
        let number = ObjectNumber(0x15);
        let found = find_local(&page, ObjectKind::Passive, number, PresentationLevel::Level25).unwrap();
        assert_eq!(found.first, 13 + 5 + 1);
        assert!(found.body.is_local());
        assert_eq!(
            find_local(&page, ObjectKind::Active, number, PresentationLevel::Level25).err(),
            Some(Absent::NotDefinition)
        );
        assert_eq!(
            find_local(&page, ObjectKind::Passive, ObjectNumber(0x115), PresentationLevel::Level25).err(),
            Some(Absent::BadNumber)
        );
        assert_eq!(
            find_local(&page, ObjectKind::Passive, ObjectNumber(0x1D), PresentationLevel::Level25).err(),
            Some(Absent::BadNumber)
        );
        assert_eq!(
            find_local(&page, ObjectKind::Passive, ObjectNumber(0x25), PresentationLevel::Level25).err(),
            Some(Absent::NotReceived)
        );
    }

    /// REQ-OBJ-005: the usage bits must include the level.
    #[test]
    fn usage_bits_select_the_level() {
        for (usage, at_25, at_35) in
            [(0b01, true, false), (0b10, false, true), (0b11, true, true), (0b00, false, false)]
        {
            let page = page_with(&[(0, 0, definition(ObjectKind::Active, usage))]);
            let find = |level| find_local(&page, ObjectKind::Active, ObjectNumber(0), level).is_ok();
            assert_eq!(
                (find(PresentationLevel::Level25), find(PresentationLevel::Level35)),
                (at_25, at_35),
                "{usage:02b}"
            );
        }
    }

    /// REQ-OBJ-006: the body ends at the next definition or termination,
    /// crosses packets, and is absent past a missing packet.
    #[test]
    fn prescan_finds_the_end_and_the_last_position() {
        let column = |column: u8| TripletFunction::G0Character { column, code: b'A' }.to_triplet();
        let row =
            |row: u8| TripletFunction::SetActivePosition { address: ROW_ADDRESS_BASE + row, column: 3 }.to_triplet();
        let end = TripletFunction::Termination { kind: 0 }.to_triplet();
        let mut entries = std::vec![(0, 0, definition(ObjectKind::Active, 0b11)), (0, 1, column(9)), (0, 2, row(4))];
        for index in 3..13 {
            entries.push((0, index, column(2)));
        }
        entries.push((1, 0, column(5)));
        entries.push((1, 1, end));
        let page = page_with(&entries);
        let object = find_local(&page, ObjectKind::Active, ObjectNumber(0), PresentationLevel::Level25).unwrap();
        let extent = prescan(&object, &page).unwrap();
        assert_eq!(extent, Extent { end: 14, last: Some((4, 5)) });
        // Without packet X/26/1 the body runs into a missing packet.
        let mut short = page;
        short.enhancements_present = 1;
        assert_eq!(prescan(&object, &short), Err(Absent::NotReceived));
    }

    /// Clause 12.3.2: row address 40 is row 24.
    #[test]
    fn row_addresses() {
        assert_eq!(row_of(ROW_ADDRESS_ROW_24), 24);
        assert_eq!(row_of(41), 1);
        assert_eq!(row_of(63), 23);
    }
}
