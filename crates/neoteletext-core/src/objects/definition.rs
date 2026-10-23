//! An object's definition, found and checked (REQ-OBJ-003 to REQ-OBJ-006),
//! and the prescan of its body (REQ-OBJ-020).

use super::{Body, Fetch, PageLink};
use crate::acq::{EnhancementLookup, StoredPage};
use crate::config::PresentationLevel;
use crate::consts::{
    OBJECT_BODY_TRIPLETS, OBJECT_POINTER_FIRST_PACKET, OBJECT_POINTER_GROUP_TRIPLETS, OBJECT_POINTER_LAST_PACKET,
    OBJECT_POINTER_UNUSED, ROW_ADDRESS_BASE, ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24, ROWS, TRIPLETS_PER_PACKET,
};
use crate::packet::{ObjectKind, ObjectNumber, ObjectPacket, TripletFunction, TripletLocation, triplet_location};

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
