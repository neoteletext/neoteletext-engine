//! Tests of object links, bodies and definitions (area OBJ).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::acq::StoredPage;
use crate::consts::{ROW_ADDRESS_BASE, ROW_ADDRESS_ROW_24};
use crate::packet::{ObjectUsage, Triplet, TripletFunction};

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
    for (usage, at_25, at_35) in [(0b01, true, false), (0b10, false, true), (0b11, true, true), (0b00, false, false)] {
        let page = page_with(&[(0, 0, definition(ObjectKind::Active, usage))]);
        let find = |level| find_local(&page, ObjectKind::Active, ObjectNumber(0), level).is_ok();
        assert_eq!((find(PresentationLevel::Level25), find(PresentationLevel::Level35)), (at_25, at_35), "{usage:02b}");
    }
}

/// REQ-OBJ-006: the body ends at the next definition or termination,
/// crosses packets, and is absent past a missing packet.
#[test]
fn prescan_finds_the_end_and_the_last_position() {
    let column = |column: u8| TripletFunction::G0Character { column, code: b'A' }.to_triplet();
    let row = |row: u8| TripletFunction::SetActivePosition { address: ROW_ADDRESS_BASE + row, column: 3 }.to_triplet();
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
