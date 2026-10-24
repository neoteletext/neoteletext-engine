//! Tests of the enhancement processor: the Additional Flash Functions, display areas and planes, enhancement characters, every transition of the walk, and sizes on a segment; then pages built through the decoder, as a receiver builds them.
#![cfg_attr(
    not(feature = "additional-flash-functions"),
    allow(
        dead_code,
        unused_imports,
        reason = "helpers of the tests whose capability is compiled out (decision E.105)"
    )
)]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, clippy::panic)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use std::boxed::Box;
use std::string::String;
use std::vec::Vec;

use crate::acq::{Decoder, EnhancementPacket, EnhancementStore, EnhancementSubPage};
use crate::charset::{CharRef, CharSet, to_unicode};
use crate::consts::{
    DATA_BYTES, DATA_START, DESIGNATION_GROUP_SHIFT, FLASH_MODE_ADJACENT_CLUT, FLASH_MODE_INVERT_PHASE,
    FLASH_MODE_NORMAL, FLASH_RATE_DECREMENTAL, FLASH_RATE_INCREMENTAL, FLASH_RATE_PHASE_2, FLASH_RATE_PHASE_3,
    FLASH_RATE_SLOW, G3_UNDEFINED, HEADER_DISPLAY_BYTES, LATIN_G2_UNDEFINED, MAGAZINES, PACKET_ENHANCEMENT,
    PAGE_TIME_FILLING, ROW_ADDRESS_BASE, ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24, TRIPLET_BYTES, TRIPLETS_PER_PACKET,
};
use crate::ecc::with_odd_parity;
use crate::level1::{CharSource, FlashMode, FlashRate, Glyph, Grid, Size, Width, build_grid};
use crate::packet::{ColourData, ObjectKind, ObjectNumber, ObjectSource, Packet, PacketMeta, TripletFunction, encode};
use crate::types::{ControlBits, Instant, Magazine, PageAddress, SubCode};

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

// ---------------------------------------------------------------------------
// Pages built through the decoder, as a receiver builds them.
// ---------------------------------------------------------------------------

/// The page every built test shows: page 100.
const PAGE: u16 = 0x100;

/// A reserved row triplet (Table 28 mode 00010), which does nothing
/// (REQ-L25-052): the filler of unused triplets.
const FILLER: TripletFunction = TripletFunction::ReservedRow { mode: 0b00010, address: 41, data: 0 };

/// The Termination Marker that ends the local enhancement data (Table 28
/// mode 11111, address 63).
const END: TripletFunction = TripletFunction::Termination { kind: 0b111 };

/// A designation value of Tables 32 to 34: group (bits 3 to 6) and entry
/// (bits 0 to 2).
const fn designation(group: u8, entry: u8) -> u8 {
    group << DESIGNATION_GROUP_SHIFT | entry
}

/// Greek: group 0110, entry 111 (Tables 33 and 34).
const GREEK: u8 = designation(0b0110, 0b111);
/// Cyrillic 1 with the Cyrillic G2 set: group 0100, entry 000 (Table 34).
const CYRILLIC_1: u8 = designation(0b0100, 0b000);
/// Latin with the Latin G2 set: group 0000 (Table 34).
const LATIN: u8 = designation(0b0000, 0b000);

fn magazine_1() -> Magazine {
    Magazine::new(1).unwrap()
}

/// A header of page `mpp`, sub-code 0, with control bits `control`.
fn header(mpp: u16, control: u16) -> Packet {
    let address = PageAddress::from_mpp(mpp).unwrap();
    let display = [with_odd_parity(b' '); HEADER_DISPLAY_BYTES];
    encode::header(address.magazine, address.page, SubCode::new(0), ControlBits(control), &display)
}

/// A display row of magazine 1 from 7-bit codes, padded with spaces.
fn display_row(number: u8, codes: &[u8]) -> Packet {
    let mut bytes = [with_odd_parity(b' '); DATA_BYTES];
    for (slot, &code) in bytes.iter_mut().zip(codes) {
        *slot = with_odd_parity(code);
    }
    encode::row(magazine_1(), number, &bytes)
}

/// A packet X/26 with designation code `designation`: up to thirteen
/// functions, then fillers.
fn x26(designation: u8, functions: &[TripletFunction]) -> Packet {
    assert!(functions.len() <= TRIPLETS_PER_PACKET);
    let mut triplets = [FILLER.to_triplet(); TRIPLETS_PER_PACKET];
    for (slot, function) in triplets.iter_mut().zip(functions) {
        *slot = function.to_triplet();
    }
    encode::triplet_packet(magazine_1(), PACKET_ENHANCEMENT, designation, &triplets)
}

/// The local enhancement data in as many packets X/26 as it needs, from
/// designation code 0, ended by a Termination Marker.
fn local_data(functions: &[TripletFunction]) -> Vec<Packet> {
    let mut all = functions.to_vec();
    all.push(END);
    all.chunks(TRIPLETS_PER_PACKET).enumerate().map(|(code, chunk)| x26(code as u8, chunk)).collect()
}

/// Corrupts triplet `index` of a packet beyond Hamming 24/18 correction:
/// two bits of each of its bytes flipped.
fn damage(mut packet: Packet, index: usize) -> Packet {
    let start = DATA_START + 1 + TRIPLET_BYTES * index;
    for byte in &mut packet.bytes[start..start + TRIPLET_BYTES] {
        *byte ^= 0b0000_0110;
    }
    packet
}

/// Page 100: its header, its other packets, then its rows.
fn page(control: u16, rows: &[(u8, &[u8])], others: Vec<Packet>) -> Vec<Packet> {
    let mut packets = std::vec![header(PAGE, control)];
    packets.extend(others);
    packets.extend(rows.iter().map(|&(number, codes)| display_row(number, codes)));
    packets
}

fn level(level: PresentationLevel) -> Config {
    Config { level, ..Config::default() }
}

/// Feeds `packets` to a decoder, ends every page with the time-filling
/// headers, and calls `f` with page `mpp`, its magazine's data and the
/// enhancement lookup.
fn with_page<R>(
    mpp: u16,
    packets: &[Packet],
    config: &Config,
    f: impl FnOnce(&StoredPage, &crate::acq::MagazineData, &EnhancementLookup<'_>) -> R,
) -> R {
    let mut pages = std::vec![StoredPage::EMPTY; 4];
    let mut sub_pages = std::vec![EnhancementSubPage::EMPTY; 4];
    let mut stored = std::vec![EnhancementPacket::EMPTY; 64];
    let store = EnhancementStore::new(&mut sub_pages, &mut stored);
    let mut decoder = Decoder::with_enhancement_store(&mut pages, store, *config);
    let mut all = packets.to_vec();
    for magazine in 1..=MAGAZINES as u16 {
        all.push(header(magazine << 8 | u16::from(PAGE_TIME_FILLING), 0));
    }
    for (index, packet) in all.iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64 * 64), &mut |_| {});
    }
    decoder.tick(Instant(1_000_000_000), &mut |_| {});
    let address = PageAddress::from_mpp(mpp).unwrap();
    let magazine = decoder.magazine_data(address.magazine);
    let lookup = decoder.enhancement_lookup();
    f(
        decoder.store().get(address, SubCode::ANY).unwrap_or_else(|| panic!("page {mpp:X} is stored")),
        &magazine,
        &lookup,
    )
}

/// The grid of page 100 at `config`'s level, with the processor's report.
fn build(packets: &[Packet], config: &Config) -> (Box<Grid>, EnhancementReport) {
    build_page(PAGE, packets, config)
}

/// The grid of page `mpp` built by the Level 1 pass and then by [`apply`].
fn build_page(mpp: u16, packets: &[Packet], config: &Config) -> (Box<Grid>, EnhancementReport) {
    with_page(mpp, packets, config, |page, magazine, lookup| {
        let options = DisplayOptions::default();
        let mut grid = Box::new(Grid::EMPTY);
        build_grid(&mut grid, Some(page), magazine, config, &options);
        let report = apply(&mut grid, page, magazine, lookup, config, &options);
        (grid, report)
    })
}

fn build_25(packets: &[Packet]) -> (Box<Grid>, EnhancementReport) {
    build(packets, &level(PresentationLevel::Level25))
}

/// Set Active Position to `row`, `column` (Table 28 mode 00100; address 40
/// is row 24).
fn position(row: u8, column: u8) -> TripletFunction {
    let address = if usize::from(row) == ROWS - 1 { ROW_ADDRESS_ROW_24 } else { ROW_ADDRESS_BASE + row };
    TripletFunction::SetActivePosition { address, column }
}

fn g0_at(column: u8, code: u8) -> TripletFunction {
    TripletFunction::G0Character { column, code }
}

/// What a cell shows, as Unicode text: the base character, a space for no
/// glyph and `#` for a mosaic.
fn shown(cell: &Cell) -> char {
    match cell.glyph {
        Glyph::Char(character) => to_unicode(character).base,
        Glyph::Mosaic { .. } => '#',
        Glyph::Space => ' ',
    }
}

/// The text of `row`, columns `first` to `last`.
fn text(grid: &Grid, row: usize, columns: core::ops::RangeInclusive<usize>) -> String {
    grid.cells[row][columns].iter().map(shown).collect()
}

/// REQ-L25-010, REQ-L25-011, REQ-L25-004: every character mode of Table 29
/// writes at the active position, with the source of the local enhancement
/// data; a code below 2/0 writes nothing; an undefined G2 position shows a
/// space and is counted (decision E.50).
#[test]
fn local_data_writes_every_character_mode() {
    let latin_g0 = CharSet::G0(G0 { set: G0Set::Latin, national: None });
    let local = [
        position(1, 0),
        g0_at(2, b'A'),
        TripletFunction::G0Diacritic { column: 3, code: b'e', diacritic: 2 },
        // Table 29 mode 10000 with data 2/A is "@".
        TripletFunction::G0Diacritic { column: 4, code: 0x2A, diacritic: 0 },
        // 7/F: every sextant.
        TripletFunction::G1Character { column: 5, code: 0x7F },
        // Table 47 note 3: G1 columns 4 and 5 are the G0 set's characters.
        TripletFunction::G1Character { column: 6, code: b'Q' },
        TripletFunction::G2Character { column: 7, code: 0x41 },
        TripletFunction::G3Character { column: 8, code: 0x41, level_15_form: false },
        TripletFunction::G3Character { column: 9, code: 0x42, level_15_form: true },
        g0_at(10, 0x1F),
        TripletFunction::G2Character { column: 11, code: LATIN_G2_UNDEFINED[0] },
    ];
    let packets = page(0, &[(1, b"0123456789ABCDEF")], local_data(&local));
    let (grid, report) = build_25(&packets);
    let row = &grid.cells[1];
    assert_eq!(row[2].glyph, Glyph::Char(CharRef::new(latin_g0, b'A')));
    assert_eq!(row[2].attributes.source(), CharSource::Local);
    assert_eq!(
        to_unicode(match row[3].glyph {
            Glyph::Char(character) => character,
            other => panic!("{other:?}"),
        }),
        crate::charset::Unicode { base: 'e', combining: Some('\u{301}') },
        "mode 10010: acute"
    );
    assert_eq!(shown(&row[4]), '@');
    assert_eq!(row[5].glyph, Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false });
    assert_eq!(row[6].glyph, Glyph::Char(CharRef::new(latin_g0, b'Q')));
    assert_eq!(row[7].glyph, Glyph::Char(CharRef::new(CharSet::G2(G2Set::Latin), 0x41)));
    assert_eq!(row[8].glyph, Glyph::Char(CharRef::new(CharSet::G3, 0x41)));
    assert_eq!(row[9].glyph, Glyph::Char(CharRef::new(CharSet::G3, 0x42)), "mode 00010 at Level 2.5 too");
    assert_eq!((shown(&row[10]), row[10].attributes.source()), ('A', CharSource::Level1), "below 2/0");
    assert_eq!((row[11].glyph, row[11].attributes.source()), (Glyph::Space, CharSource::Local));
    assert_eq!(report.faults.undefined_characters, 1);
    assert_eq!(text(&grid, 1, 0..=1), "01", "the Level 1 characters stay");
}

/// REQ-L25-050: a write to row 0, columns 0 to 7, is discarded and counted,
/// and the data after it is still processed; row 0 from column 8 can be
/// written.
#[test]
fn the_receiver_area_is_never_written() {
    let row_0 =
        TripletFunction::AddressDisplayRow0 { address: ROW_ADDRESS_LAST, colour: ColourData { index: 1, high: 0b01 } };
    let packets = page(0, &[(1, b"ROW 1")], local_data(&[row_0, g0_at(3, b'X'), g0_at(10, b'Y')]));
    let (grid, report) = build_25(&packets);
    assert_ne!(shown(&grid.cells[0][3]), 'X');
    assert_eq!(grid.cells[0][3].attributes.source(), CharSource::Level1);
    assert_eq!((shown(&grid.cells[0][10]), grid.cells[0][10].attributes.source()), ('Y', CharSource::Local));
    assert_eq!(report.faults.discarded_writes, 1);
}

/// REQ-L25-002, REQ-L25-005: missing designation codes are skipped, and a
/// damaged triplet is skipped and counted without moving the position.
#[test]
fn damaged_triplets_and_missing_packets_are_skipped() {
    let first = damage(x26(0, &[position(2, 0), g0_at(1, b'A'), g0_at(2, b'B'), g0_at(3, b'C')]), 2);
    // Designation code 1 is never sent.
    let third = x26(2, &[g0_at(5, b'D'), END]);
    let packets = page(0, &[(2, b"......")], std::vec![first, third]);
    let (grid, report) = build_25(&packets);
    assert_eq!(text(&grid, 2, 0..=5), ".A.C.D");
    assert_eq!(report.faults.damaged_triplets, 1);
}

/// REQ-L25-012, decision E.50: a Modified G0 and G2 designation lasts to
/// the end of its row and never affects the Level 1 page; at Level 2.5 a
/// third pair of sets is ignored and counted, at Level 3.5 it is used.
#[test]
fn modified_designations_and_the_level_2_5_limit() {
    let designate = |column: u8, designation: u8| TripletFunction::ModifiedDesignation { column, designation };
    let local = [
        position(12, 0),
        designate(1, GREEK),
        g0_at(2, b'a'),
        designate(4, CYRILLIC_1),
        g0_at(5, b'a'),
        designate(7, LATIN),
        g0_at(8, b'a'),
        designate(9, GREEK),
        position(13, 0),
        g0_at(1, b'a'),
    ];
    let packets = page(0, &[(12, b"aaaaaaaaaa")], local_data(&local));
    let set = |grid: &Grid, row: usize, column: usize| match grid.cells[row][column].glyph {
        Glyph::Char(CharRef { set: CharSet::G0(g0), .. }) => g0.set,
        other => panic!("{other:?}"),
    };
    let (grid, report) = build_25(&packets);
    let sets = [2, 3, 5, 8].map(|column| set(&grid, 12, column));
    assert_eq!(sets, [G0Set::Greek, G0Set::Latin, G0Set::Greek, G0Set::Latin], "column 3 is Level 1");
    assert_eq!(set(&grid, 13, 1), G0Set::Latin, "the next row starts with the default sets");
    assert_eq!((report.designations, report.faults.ignored_designations), (3, 1));
    let (grid, report) = build(&packets, &level(PresentationLevel::Level35));
    assert_eq!(set(&grid, 12, 5), G0Set::Cyrillic1);
    assert_eq!((report.designations, report.faults.ignored_designations), (4, 0));
}
