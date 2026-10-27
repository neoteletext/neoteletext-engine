//! Tests of controls and their looks (decision E.127).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::consts::{MODE_TERMINATION, ROW_ADDRESS_BASE, TRIPLETS_PER_PACKET};
use crate::level1::Grid;
use crate::packet::Triplet;
use std::vec::Vec;

fn page_with(triplets: &[Triplet]) -> StoredPage {
    let mut page = StoredPage::EMPTY;
    for (index, triplet) in triplets.iter().enumerate() {
        let designation = index / TRIPLETS_PER_PACKET;
        page.enhancements[designation][index % TRIPLETS_PER_PACKET] = Some(*triplet);
        page.enhancements_present |= 1 << designation;
    }
    page
}

fn row(row: u8) -> Triplet {
    TripletFunction::SetActivePosition { address: ROW_ADDRESS_BASE + row, column: 0 }.to_triplet()
}

fn control(column: u8, width: u8, joined: bool) -> Triplet {
    Triplet { address: column, mode: MODE_CONTROL, data: width | if joined { CONTROL_JOINED } else { 0 } }
}

fn list(found: &Controls) -> Vec<(u8, u8, u8, u8)> {
    found.list.as_slice().iter().map(|c| (c.row, c.column, c.width, c.height)).collect()
}

/// REQ-INT-117: four buttons on one row are four controls, each its own
/// column and width, even where two touch.
#[test]
fn four_buttons_on_a_row_are_four_controls() {
    let page = page_with(&[
        row(20),
        control(1, 9, false),
        control(10, 9, false),
        control(21, 9, false),
        control(31, 9, false),
    ]);
    let found = controls(&page);
    assert_eq!(list(&found), [(20, 1, 9, 1), (20, 10, 9, 1), (20, 21, 9, 1), (20, 31, 9, 1)]);
    assert_eq!(found.dropped, 0);
    assert_eq!(found.at(20, 9), Some(0));
    assert_eq!(found.at(20, 10), Some(1), "touching controls stay apart");
    assert_eq!(found.at(20, 30), None, "the gap is no control");
    assert_eq!(found.at(21, 1), None);
}

/// REQ-INT-117: a joined run continues the control above at the same
/// column and width; a list's rows, each its own triplet, stay apart.
#[test]
fn a_joined_run_makes_a_control_taller() {
    let page = page_with(&[row(5), control(2, 30, false), row(6), control(2, 30, true), row(7), control(2, 30, false)]);
    assert_eq!(list(&controls(&page)), [(5, 2, 30, 2), (7, 2, 30, 1)]);
}

/// REQ-INT-118: a triplet that breaks a rule is dropped and counted, and
/// the rest are kept.
#[test]
fn a_control_that_breaks_a_rule_is_dropped_and_counted() {
    let page = page_with(&[
        row(3),
        control(0, 10, false),
        control(5, 10, false), // over the first
        control(35, 6, false), // past column 39
        control(20, 0, false), // no width
        control(20, 5, true),  // joined to nothing above
        row(24),
        control(0, 5, false), // the Fastext row
    ]);
    let found = controls(&page);
    assert_eq!(list(&found), [(3, 0, 10, 1)]);
    assert_eq!(found.dropped, 5);
    // Row 0: the header is no control's.
    let header = page_with(&[
        Triplet { address: ROW_ADDRESS_LAST, mode: crate::consts::MODE_ADDRESS_ROW_0, data: 0 },
        control(0, 5, false),
    ]);
    assert_eq!(controls(&header).dropped, 1);
}

/// REQ-INT-118: at most `MAX_CONTROLS`.
#[test]
fn a_page_has_at_most_max_controls() {
    let mut triplets = Vec::new();
    for at in 1..=17u8 {
        triplets.extend([row(at), control(0, 10, false), control(20, 10, false)]);
    }
    let all = controls(&page_with(&triplets));
    assert_eq!(all.list.len(), MAX_CONTROLS);
    assert_eq!(all.dropped, 34 - 32, "17 rows of two: the last two dropped");
    assert_eq!(all.list.as_slice()[MAX_CONTROLS - 1].row, 16);
}

/// REQ-INT-117: the local enhancement data ends at a Termination Marker,
/// so a control after it, as in an object, is none of the page's.
#[test]
fn controls_end_with_the_local_enhancement_data() {
    let end = Triplet { address: ROW_ADDRESS_LAST, mode: MODE_TERMINATION, data: 0b111 };
    assert!(controls(&page_with(&[row(4), end, control(0, 5, false)])).list.is_empty());
}

/// REQ-INT-119: hover moves each colour to the adjacent CLUT; pressed also
/// exchanges foreground and background, and both leave the cells beside
/// the control alone.
#[test]
fn hover_flips_the_clut_and_pressed_also_inverts() {
    let mut grid = Grid::EMPTY;
    for cell in &mut grid.cells[4] {
        cell.foreground = 4; // CLUT 0 blue
        cell.background = 22; // CLUT 2 entry 6
    }
    let area = Control { row: 4, column: 2, width: 3, height: 1 };
    let mut hovered = grid;
    apply_look(&mut hovered, area, Look::Hover);
    assert_eq!((hovered.cells[4][2].foreground, hovered.cells[4][2].background), (12, 30), "CLUT 1 and CLUT 3");
    assert!(!hovered.cells[4][2].attributes.invert());
    assert_eq!(hovered.cells[4][5], grid.cells[4][5], "the cell after it");
    assert_eq!(hovered.cells[4][1], grid.cells[4][1], "the cell before it");
    let mut pressed = grid;
    apply_look(&mut pressed, area, Look::Pressed);
    assert_eq!((pressed.cells[4][3].foreground, pressed.cells[4][3].background), (12, 30));
    assert!(pressed.cells[4][3].attributes.invert());
    let mut inverted = grid;
    inverted.cells[4][3].attributes = inverted.cells[4][3].attributes.with_invert(true);
    apply_look(&mut inverted, area, Look::Pressed);
    assert!(!inverted.cells[4][3].attributes.invert(), "an inverted cell is turned back");
}

/// Decision E.127: the control triplet is still a reserved mode to the
/// enhancement processor, so it changes nothing drawn (REQ-L25-052).
#[test]
fn the_control_triplet_is_reserved_column_mode_00101() {
    assert_eq!(MODE_CONTROL, 0b00101);
    assert!(matches!(
        control(3, 9, false).function(),
        TripletFunction::ReservedColumn { column: 3, mode: MODE_CONTROL, data: 9 }
    ));
}
