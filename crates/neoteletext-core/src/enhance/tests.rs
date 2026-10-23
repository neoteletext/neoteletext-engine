//! Tests of the enhancement processor: the Additional Flash Functions, display areas and planes, enhancement characters, every transition of the walk, and sizes on a segment.
#![cfg_attr(
    not(feature = "additional-flash-functions"),
    allow(
        dead_code,
        unused_imports,
        reason = "helpers of the tests whose capability is compiled out (decision E.105)"
    )
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::charset::{CharRef, CharSet};
use crate::consts::{
    FLASH_MODE_ADJACENT_CLUT, FLASH_MODE_INVERT_PHASE, FLASH_MODE_NORMAL, FLASH_RATE_DECREMENTAL,
    FLASH_RATE_INCREMENTAL, FLASH_RATE_PHASE_2, FLASH_RATE_PHASE_3, FLASH_RATE_SLOW, G3_UNDEFINED,
};
use crate::level1::{CharSource, FlashMode, FlashRate, Glyph, Size, Width};
use crate::packet::{ColourData, ObjectKind, ObjectNumber, ObjectSource, TripletFunction};

use super::attributes::{Flash, Rate, flash_of, flash_rate};
use super::characters::{enhancement_glyph, is_text};
use super::planes::{area_of, scan_index};
use super::row::cell_of;
#[cfg(feature = "additional-flash-functions")]
use crate::consts::FLASH_MODE_STEADY;
use crate::consts::MOSAIC_FULL;

/// REQ-L25-027, decision E.48: the flash a triplet sets, and its
/// reserved values.
// Needs its capability compiled in (decision E.105).
#[cfg(feature = "additional-flash-functions")]
#[test]
fn additional_flash_values() {
    let on = |mode, rate| Some(Flash::On { mode, rate, start: 3 });
    assert_eq!(flash_of(FLASH_MODE_STEADY, FLASH_RATE_PHASE_2, 0, 3), Some(Flash::Steady));
    assert_eq!(flash_of(FLASH_MODE_NORMAL, FLASH_RATE_SLOW, 0, 3), on(FlashMode::Normal, Rate::Fixed(FlashRate::Slow)));
    assert_eq!(
        flash_of(FLASH_MODE_INVERT_PHASE, FLASH_RATE_PHASE_3, 0, 3),
        on(FlashMode::InvertPhase, Rate::Fixed(FlashRate::FastPhase3))
    );
    assert_eq!(
        flash_of(FLASH_MODE_ADJACENT_CLUT, FLASH_RATE_INCREMENTAL, 0, 3),
        on(FlashMode::AdjacentClut, Rate::Incremental)
    );
    assert_eq!(flash_of(FLASH_MODE_NORMAL, FLASH_RATE_DECREMENTAL, 0, 3), on(FlashMode::Normal, Rate::Decremental));
    assert_eq!(flash_of(FLASH_MODE_NORMAL, 0b110, 0, 3), None);
    assert_eq!(flash_of(FLASH_MODE_NORMAL, 0b111, 0, 3), None);
    assert_eq!(flash_of(FLASH_MODE_NORMAL, FLASH_RATE_SLOW, 0b01, 3), None);
    let phases: [FlashRate; 4] = core::array::from_fn(|step| flash_rate(Rate::Decremental, 10, 10 + step));
    assert_eq!(phases, [FlashRate::FastPhase1, FlashRate::FastPhase3, FlashRate::FastPhase2, FlashRate::FastPhase1]);
    assert_eq!(flash_rate(Rate::Incremental, 10, 9), FlashRate::FastPhase1, "left of the start");
}

/// Decision E.49: the display areas of a row.
#[test]
fn display_areas() {
    let panels = SidePanels { left: 6, right: 10 };
    let areas = [0, 39, 40, 49, 50, 65, 66, 71, 72].map(|column| area_of(column, panels));
    let expected = [
        Area::Main,
        Area::Main,
        Area::Right,
        Area::Right,
        Area::Outside,
        Area::Outside,
        Area::Left,
        Area::Left,
        Area::Outside,
    ];
    assert_eq!(areas, expected);
    assert_eq!(area_of(40, SidePanels::NONE), Area::Outside);
    assert_eq!(scan_index(1, 0), SCREEN_COLUMNS);
}

/// REQ-OBJ-008, REQ-OBJ-013: the nesting table and the planes that set
/// colours.
#[test]
fn planes() {
    use ObjectKind::{Active, Adaptive, Passive};
    let table = [
        (Plane::Local, [true, true, true]),
        (Plane::Active, [false, true, true]),
        (Plane::Adaptive, [false, false, true]),
        (Plane::Passive, [false, false, false]),
    ];
    for (plane, allowed) in table {
        assert_eq!([Active, Adaptive, Passive].map(|kind| plane.may_invoke(kind)), allowed, "{plane:?}");
    }
    assert!(Plane::Local.sets_colours() && Plane::Active.sets_colours());
    assert!(!Plane::Adaptive.sets_colours() && !Plane::Passive.sets_colours());
}

/// REQ-L25-010, REQ-L25-011, decision E.50: enhancement glyphs.
#[test]
fn enhancement_glyphs() {
    let latin = (G0Set::Latin, G2Set::Latin);
    let full = Glyph::Mosaic { pattern: MOSAIC_FULL, separated: true };
    assert_eq!(enhancement_glyph(EnhChar::G1(0x7F), latin, true).0, full);
    let greek = CharSet::G0(G0 { set: G0Set::Greek, national: None });
    let (glyph, undefined) = enhancement_glyph(EnhChar::G1(b'Q'), (G0Set::Greek, G2Set::Greek), false);
    assert_eq!((glyph, undefined), (Glyph::Char(CharRef::new(greek, b'Q')), false));
    assert_eq!(enhancement_glyph(EnhChar::G2(0x59), latin, false), (Glyph::Space, true));
    assert!(!enhancement_glyph(EnhChar::G2(0x59), (G0Set::Cyrillic1, G2Set::Cyrillic), false).1, "Latin G2 only");
    for code in G3_UNDEFINED {
        assert_eq!(enhancement_glyph(EnhChar::G3(code), latin, false), (Glyph::Space, true));
    }
    assert!(is_text(enhancement_glyph(EnhChar::G2(0x41), latin, false).0));
    assert!(!is_text(enhancement_glyph(EnhChar::G3(0x41), latin, false).0));
}

/// REQ-FSM-009, REQ-FSM-011: every row of the walk's table, checked for
/// its next state and action.
#[test]
fn every_transition_of_the_walk() {
    use TripletFunction as F;
    use WalkAction as A;
    use WalkEvent as E;
    use WalkState as S;
    let colour = ColourData { index: 3, high: 0 };
    let number = ObjectNumber(0x15);
    let pending = S::ModifierPending { row_offset: 2, column_offset: 56 };
    let invocation = F::ObjectInvocation { source: ObjectSource::Local, kind: ObjectKind::Passive, number };
    let column = F::G0Character { column: 4, code: b'A' };
    let attribute = F::ForegroundColour { column: 4, colour };
    let rows: [(S, E, S, A); 25] = [
        (S::Start, E::Begin { display: true }, S::Walking, A::BeginLocal),
        (S::Start, E::Begin { display: false }, S::PlacingDefaults, A::None),
        (S::Walking, E::Triplet(None), S::Walking, A::Skip),
        (S::Walking, E::Triplet(Some(F::Termination { kind: 7 })), S::Returning, A::EndSource),
        (S::Walking, E::EndOfData, S::Returning, A::EndSource),
        (
            S::Walking,
            E::Triplet(Some(F::ObjectDefinition {
                usage: crate::packet::ObjectUsage(3),
                kind: ObjectKind::Active,
                number,
            })),
            S::Returning,
            A::EndSource,
        ),
        (
            S::Walking,
            E::Triplet(Some(F::FullRowColour { address: 45, colour })),
            S::Walking,
            A::RowColour { row: 5, colour },
        ),
        (
            S::Walking,
            E::Triplet(Some(F::SetActivePosition { address: 40, column: 7 })),
            S::Walking,
            A::Position { row: 24, column: 7 },
        ),
        (
            S::Walking,
            E::Triplet(Some(F::AddressDisplayRow0 { address: 63, colour })),
            S::Walking,
            A::RowColour { row: 0, colour },
        ),
        (S::Walking, E::Triplet(Some(F::AddressDisplayRow0 { address: 50, colour })), S::Walking, A::None),
        (S::Walking, E::Triplet(Some(F::FullScreenColour(colour))), S::Walking, A::ScreenColour(colour)),
        // Decision E.95: DRCS Mode is a reserved row triplet, and DRCS
        // Character Invocation only sets the column.
        (S::Walking, E::Triplet(Some(F::ReservedRow { mode: 0b11000, address: 41, data: 0x72 })), S::Walking, A::None),
        (
            S::Walking,
            E::Triplet(Some(F::UnsupportedColumn { column: 3, mode: 0b01101, data: 0x40 })),
            S::Walking,
            A::Column(F::UnsupportedColumn { column: 3, mode: 0b01101, data: 0x40 }),
        ),
        (S::Walking, E::Triplet(Some(F::OriginModifier { row_offset: 2, column_offset: 56 })), pending, A::None),
        (
            pending,
            E::Triplet(Some(invocation)),
            S::Walking,
            A::Invoke {
                source: ObjectSource::Local,
                kind: ObjectKind::Passive,
                number,
                row_offset: 2,
                column_offset: 56,
            },
        ),
        (pending, E::Triplet(Some(column)), S::Walking, A::Column(column)),
        (pending, E::Triplet(None), S::Walking, A::Skip),
        (
            S::Walking,
            E::Triplet(Some(invocation)),
            S::Walking,
            A::Invoke {
                source: ObjectSource::Local,
                kind: ObjectKind::Passive,
                number,
                row_offset: 0,
                column_offset: 0,
            },
        ),
        (S::Walking, E::Triplet(Some(F::ReservedRow { mode: 2, address: 41, data: 0 })), S::Walking, A::None),
        (S::Walking, E::Triplet(Some(F::ProgrammeDeliveryColumn { column: 3, data: 0 })), S::Walking, A::None),
        (S::Walking, E::Triplet(Some(attribute)), S::Walking, A::Column(attribute)),
        (S::Returning, E::Returned(Ended::Object), S::Walking, A::Pop),
        (S::Returning, E::Returned(Ended::Local), S::Finished, A::None),
        (S::Returning, E::Returned(Ended::Default), S::PlacingDefaults, A::ResetPosition),
        (S::PlacingDefaults, E::DefaultObject { found: true }, S::Walking, A::StartDefault),
    ];
    for (state, event, next, action) in rows {
        assert_eq!(walk_transition(state, event), (next, action), "{state:?} {event:?}");
    }
    assert_eq!(walk_transition(S::PlacingDefaults, E::DefaultObject { found: false }), (S::Finished, A::None));
    // Events that cannot arrive change nothing.
    assert_eq!(walk_transition(S::Finished, E::Begin { display: true }), (S::Finished, A::None));
    assert_eq!(walk_transition(S::Returning, E::EndOfData), (S::Returning, A::None));
}

/// Annex C.2 rule 3: the later of two changes at one cell wins, type by
/// type.
#[test]
fn changes_at_one_cell() {
    let first = AttrChange { foreground: Some(1), background: Some(2), ..AttrChange::default() };
    let second = AttrChange { foreground: Some(3), ..AttrChange::default() };
    let both = first.then(second);
    assert_eq!((both.foreground, both.background), (Some(3), Some(2)));
}

/// REQ-L25-025: Annex C.3 on one segment: lower halves claimed for the
/// next row, and no doubling where it cannot be shown.
#[test]
fn sizes_on_a_segment() {
    let attrs = Attrs { double_height: true, double_width: true, ..Attrs::PASSIVE };
    let resolved = cell_of(Glyph::Space, &attrs, CharSource::Local, 0);
    let mut cells = RowCells::new();
    cells.resolved[38] = Some(resolved);
    cells.resolved[39] = Some(resolved);
    cells.place(38, 40, true);
    let placed = |cells: &RowCells, column: usize| cells.cells[column].unwrap();
    assert_eq!(placed(&cells, 38).size, Size::DoubleTop);
    assert_eq!(placed(&cells, 38).attributes.width(), Width::LeftHalf);
    assert_eq!(placed(&cells, 39).attributes.width(), Width::RightHalf);
    assert!(cells.claims[38] && cells.claims[39]);
    let mut cells = RowCells::new();
    cells.resolved[39] = Some(resolved);
    cells.place(0, 40, false);
    assert_eq!((placed(&cells, 39).size, placed(&cells, 39).attributes.width()), (Size::Normal, Width::Normal));
}
