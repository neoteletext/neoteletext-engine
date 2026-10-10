//! Public and Global Public Object Pages (clause 10.5.1, Figure 19;
//! REQ-INP-025): objects defined by function, with the pointer tables
//! computed from them.

use std::collections::BTreeMap;
use std::vec::Vec;

use tellytxt_core::consts::{
    OBJECT_DEFINITION_FIRST_PACKET, OBJECT_POINTER_BITS, OBJECT_POINTER_DATA_BIT, OBJECT_POINTER_UNUSED,
    OBJECT_TRIPLET_LAST, PAGE_FUNCTION_GPOP, PAGE_FUNCTION_POP, TRIPLETS_PER_PACKET,
};
use tellytxt_core::packet::{
    ObjectKind, ObjectNumber, ObjectSource, ObjectUsage, Triplet, TripletFunction, TripletLocation, encode,
    triplet_location,
};
use tellytxt_core::types::PageAddress;

use super::triplets::{ObjectRef, Triplets};
use super::{AuthorError, Result, enhancement_subcode, new_page};
use crate::consts::{
    OBJECTS_PER_POINTER_PACKET, OBJECTS_PER_TYPE, PAGE_CODING_TRIPLETS, TERMINATION_FINAL_LAST, TERMINATION_FINAL_MORE,
    TERMINATION_INTERMEDIATE_LAST, TERMINATION_INTERMEDIATE_MORE,
};
use crate::encode::packed_row;
use crate::page::{EnhancementPacket, PageFile, PageFunction};

/// The highest S1 of a sub-page: four bits (Annex A.1).
const SUBPAGE_LAST: u8 = 0xF;
/// Shifts of the fields of a public object number (Table 28): N4, N6 N5
/// and N8 N7.
const NUMBER_ODD_SHIFT: u32 = 4;
const NUMBER_GROUP_SHIFT: u32 = 5;
const NUMBER_PACKET_SHIFT: u32 = 7;
/// Pointers in one pointer triplet: the even-numbered and odd-numbered
/// object (clause 10.5.1.2).
const POINTERS_PER_TRIPLET: usize = 2;
/// Pointer triplets of the three types in each group of a pointer packet:
/// Active, Adaptive, Passive (clause 10.5.1.2).
const TYPES: usize = 3;

/// The offset of a type within a group of pointer triplets.
const fn type_offset(kind: ObjectKind) -> usize {
    match kind {
        ObjectKind::Active => 0,
        ObjectKind::Adaptive => 1,
        ObjectKind::Passive => 2,
    }
}

/// One object of a sub-page.
#[derive(Clone, Debug)]
struct Definition {
    kind: ObjectKind,
    usage: ObjectUsage,
    index: usize,
    body: Triplets,
}

/// One sub-page: its objects in the order of their definitions, and
/// pointers written over the computed ones.
#[derive(Clone, Debug, Default)]
struct SubPage {
    objects: Vec<Definition>,
    overrides: Vec<(ObjectKind, usize, u16)>,
}

/// A Public (POP) or Global Public (GPOP) Object Page.
#[derive(Clone, Debug)]
pub struct ObjectPage {
    address: PageAddress,
    global: bool,
    update: u8,
    subpages: BTreeMap<u8, SubPage>,
}

/// The object number of the object with `index` (0 to 31) of its type on
/// sub-page `s1` (Table 28): N3 to N0 the sub-page, N8 N7 the pointer
/// packet, N6 N5 the group of pointer triplets and N4 the half.
#[must_use]
pub fn public_number(s1: u8, index: usize) -> ObjectNumber {
    let packet = index.checked_div(OBJECTS_PER_POINTER_PACKET).unwrap_or(0) as u16;
    let within = index.checked_rem(OBJECTS_PER_POINTER_PACKET).unwrap_or(0);
    let group = within.checked_div(POINTERS_PER_TRIPLET).unwrap_or(0) as u16;
    let odd = within.checked_rem(POINTERS_PER_TRIPLET).unwrap_or(0) as u16;
    ObjectNumber(
        packet << NUMBER_PACKET_SHIFT
            | group << NUMBER_GROUP_SHIFT
            | odd << NUMBER_ODD_SHIFT
            | u16::from(s1 & SUBPAGE_LAST),
    )
}

/// Where the pointer of an object lies: the pointer packet (1 to 4), the
/// triplet (1 to 12) and whether it is the odd half (clause 10.5.1.2).
#[must_use]
pub fn pointer_place(kind: ObjectKind, index: usize) -> (u8, usize, bool) {
    let packet = index.checked_div(OBJECTS_PER_POINTER_PACKET).unwrap_or(0);
    let within = index.checked_rem(OBJECTS_PER_POINTER_PACKET).unwrap_or(0);
    let group = within.checked_div(POINTERS_PER_TRIPLET).unwrap_or(0);
    let triplet = 1usize.saturating_add(group.saturating_mul(TYPES)).saturating_add(type_offset(kind));
    (u8::try_from(packet).unwrap_or(0).saturating_add(1), triplet, within.checked_rem(POINTERS_PER_TRIPLET) == Some(1))
}

impl ObjectPage {
    /// An empty object page: a GPOP when `global`, otherwise a POP.
    #[must_use]
    pub const fn new(address: PageAddress, global: bool) -> Self {
        Self { address, global, update: 0, subpages: BTreeMap::new() }
    }

    /// The page's address.
    #[must_use]
    pub const fn address(&self) -> PageAddress {
        self.address
    }

    /// Sets the update indicator S2 of every sub-page (Annex A.1).
    pub fn update_indicator(&mut self, update: u8) -> &mut Self {
        self.update = update;
        self
    }

    /// Defines the next object of a type on sub-page `s1`, and returns the
    /// reference that invokes it.
    pub fn define(&mut self, s1: u8, kind: ObjectKind, usage: ObjectUsage, body: Triplets) -> Result<ObjectRef> {
        let index = (0..OBJECTS_PER_TYPE)
            .find(|&index| {
                self.subpages
                    .get(&s1)
                    .is_none_or(|sub| !sub.objects.iter().any(|object| object.kind == kind && object.index == index))
            })
            .ok_or(AuthorError::TooManyObjects)?;
        self.define_at(s1, kind, usage, index, body)
    }

    /// Defines an object with a chosen index (0 to 31) among its type on
    /// sub-page `s1`.
    pub fn define_at(
        &mut self,
        s1: u8,
        kind: ObjectKind,
        usage: ObjectUsage,
        index: usize,
        body: Triplets,
    ) -> Result<ObjectRef> {
        if s1 > SUBPAGE_LAST {
            return Err(AuthorError::BadSubPage);
        }
        if index >= OBJECTS_PER_TYPE {
            return Err(AuthorError::TooManyObjects);
        }
        let sub = self.subpages.entry(s1).or_default();
        if sub.objects.iter().any(|object| object.kind == kind && object.index == index) {
            return Err(AuthorError::TooManyObjects);
        }
        if body.elements.iter().any(|element| matches!(element, super::triplets::Element::Local { .. })) {
            // Clause 13.6 NOTE: a public object never invokes a Local Object.
            return Err(AuthorError::UnknownLocalObject);
        }
        sub.objects.push(Definition { kind, usage, index, body });
        let source = if self.global { ObjectSource::Global } else { ObjectSource::Public };
        Ok(ObjectRef { source, kind, number: public_number(s1, index) })
    }

    /// Writes `value` over the computed pointer of an object, for unused,
    /// reserved and damaged pointers (decision E.51).
    pub fn override_pointer(&mut self, s1: u8, kind: ObjectKind, index: usize, value: u16) -> &mut Self {
        self.subpages.entry(s1 & SUBPAGE_LAST).or_default().overrides.push((kind, index, value));
        self
    }

    /// The page files, one per sub-page in ascending S1 (Figure 19).
    pub fn build(&self) -> Result<Vec<PageFile>> {
        let last = self.subpages.keys().next_back().copied();
        self.subpages.iter().map(|(&s1, sub)| self.build_subpage(s1, sub, Some(s1) == last)).collect()
    }

    fn build_subpage(&self, s1: u8, sub: &SubPage, final_subpage: bool) -> Result<PageFile> {
        // Pointer packets 1 to 4 as needed by the highest index of any type;
        // definitions start after the last one, and never before packet 3
        // (clause 10.5.1.3).
        let highest = sub.objects.iter().map(|object| object.index).max().unwrap_or(0);
        let pointer_packets = highest.checked_div(OBJECTS_PER_POINTER_PACKET).unwrap_or(0).saturating_add(1);
        let first_packet = (pointer_packets as u8).saturating_add(1).max(OBJECT_DEFINITION_FIRST_PACKET);
        let reserved = usize::from(first_packet.saturating_sub(OBJECT_DEFINITION_FIRST_PACKET));
        let start = reserved.saturating_mul(TRIPLETS_PER_PACKET);

        let mut sequence: Vec<Triplet> = Vec::new();
        let mut pointers: Vec<(ObjectKind, usize, u16)> = Vec::new();
        let local = |_: super::triplets::LocalObject, _: ObjectKind| Err(AuthorError::UnknownLocalObject);
        let count = sub.objects.len();
        for (position, object) in sub.objects.iter().enumerate() {
            let absolute = start.saturating_add(sequence.len());
            pointers.push((object.kind, object.index, u16::try_from(absolute).unwrap_or(OBJECT_POINTER_UNUSED)));
            let number = public_number(s1, object.index);
            sequence.push(
                TripletFunction::ObjectDefinition { usage: object.usage, kind: object.kind, number }.to_triplet(),
            );
            sequence.extend(object.body.resolved(&local)?);
            let more = position.saturating_add(1) < count;
            let kind = match (final_subpage, more) {
                (false, true) => TERMINATION_INTERMEDIATE_MORE,
                (false, false) => TERMINATION_INTERMEDIATE_LAST,
                (true, true) => TERMINATION_FINAL_MORE,
                (true, false) => TERMINATION_FINAL_LAST,
            };
            sequence.push(TripletFunction::Termination { kind }.to_triplet());
        }
        if let Some(&marker) = sequence.last() {
            while !start.saturating_add(sequence.len()).is_multiple_of(TRIPLETS_PER_PACKET) {
                sequence.push(marker);
            }
        }
        if start.saturating_add(sequence.len()) > usize::from(OBJECT_TRIPLET_LAST).saturating_add(1) {
            return Err(AuthorError::ObjectSubPageFull);
        }
        for &(kind, index, value) in &sub.overrides {
            pointers.push((kind, index, value));
        }

        let mut tables = Vec::new();
        for packet in 1..=pointer_packets {
            let mut triplets = [Triplet::default(); TRIPLETS_PER_PACKET];
            for slot in triplets.iter_mut().skip(1) {
                *slot = encode::pointer_triplet(OBJECT_POINTER_UNUSED, OBJECT_POINTER_UNUSED);
            }
            tables.push((u8::try_from(packet).unwrap_or(1), triplets));
        }
        for &(kind, index, value) in &pointers {
            let (packet, triplet, odd) = pointer_place(kind, index);
            let Some((_, triplets)) = tables.iter_mut().find(|(number, _)| *number == packet) else { continue };
            let Some(slot) = triplets.get_mut(triplet) else { continue };
            let current = slot.value();
            let (even, odd_value) = (
                current as u16 & OBJECT_POINTER_UNUSED,
                (current >> OBJECT_POINTER_BITS) as u16 & OBJECT_POINTER_UNUSED,
            );
            *slot = if odd { encode::pointer_triplet(even, value) } else { encode::pointer_triplet(value, odd_value) };
        }

        let mut page = new_page(self.address, tellytxt_core::types::SubCode::new(0));
        let function = if self.global { PAGE_FUNCTION_GPOP } else { PAGE_FUNCTION_POP };
        page.function = Some(PageFunction { function, coding: PAGE_CODING_TRIPLETS });
        let mut last_packet = 0u8;
        for (packet, triplets) in &tables {
            page.set_row(usize::from(*packet), &packed_row(OBJECT_POINTER_DATA_BIT, triplets));
            last_packet = last_packet.max(*packet);
        }
        let mut rows: BTreeMap<u8, [Triplet; TRIPLETS_PER_PACKET]> = BTreeMap::new();
        let mut enhancements: BTreeMap<u8, [Triplet; TRIPLETS_PER_PACKET]> = BTreeMap::new();
        for (offset, &triplet) in sequence.iter().enumerate() {
            let number = u16::try_from(start.saturating_add(offset)).unwrap_or(u16::MAX);
            match triplet_location(number).ok_or(AuthorError::ObjectSubPageFull)? {
                TripletLocation::Row { packet, triplet: index } => {
                    let slots = rows.entry(packet).or_insert([Triplet::default(); TRIPLETS_PER_PACKET]);
                    *slots.get_mut(usize::from(index)).ok_or(AuthorError::ObjectSubPageFull)? = triplet;
                }
                TripletLocation::Enhancement { designation, triplet: index } => {
                    let slots = enhancements.entry(designation).or_insert([Triplet::default(); TRIPLETS_PER_PACKET]);
                    *slots.get_mut(usize::from(index)).ok_or(AuthorError::ObjectSubPageFull)? = triplet;
                }
            }
        }
        for (&packet, triplets) in &rows {
            // Byte 6 of packets 3 and 4 says "definitions" with B1 = 0, and
            // is reserved elsewhere (clause 10.5.1.2).
            page.set_row(usize::from(packet), &packed_row(0, triplets));
            last_packet = last_packet.max(packet);
        }
        for (&designation, triplets) in &enhancements {
            if !page.enhancements.push(EnhancementPacket { designation, triplets: *triplets }) {
                return Err(AuthorError::ObjectSubPageFull);
            }
            last_packet = enhancement_last_packet(designation);
        }
        page.subcode = enhancement_subcode(last_packet, self.update, s1);
        Ok(page)
    }
}

/// The last-packet number of X/26 with a designation code (Annex A.1).
fn enhancement_last_packet(designation: u8) -> u8 {
    crate::consts::LAST_PACKET_ENHANCEMENT_BASE.saturating_add(designation)
}
