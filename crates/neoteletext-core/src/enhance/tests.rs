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
use crate::config::Row24Policy;
use crate::consts::{
    ALPHA_BLACK, ALPHA_RED, ALPHA_YELLOW, BLACK_BACKGROUND, C6_SUBTITLE, C7_SUPPRESS_HEADER, C10_INHIBIT_DISPLAY,
    COLOUR_BLACK, COLOUR_WHITE, COMPOSITIONAL_LINKS, CONCEAL, CONTIGUOUS_MOSAICS, DATA_BYTES, DATA_START,
    DESIGNATION_GROUP_SHIFT, DOUBLE_HEIGHT, DOUBLE_WIDTH, END_BOX, ESC, FLASH, FLASH_MODE_ADJACENT_CLUT,
    FLASH_MODE_INVERT_PHASE, FLASH_MODE_NORMAL, FLASH_RATE_DECREMENTAL, FLASH_RATE_INCREMENTAL, FLASH_RATE_PHASE_2,
    FLASH_RATE_PHASE_3, FLASH_RATE_SLOW, G3_UNDEFINED, HEADER_DISPLAY_BYTES, HOLD_MOSAICS, LATIN_G2_UNDEFINED,
    LINK_NULL_PAGE, MAGAZINES, MOSAIC_RED, MOSAIC_WHITE, MOT_OBJECT_LINK_BYTES, MOT_OBJECT_LINKS_LEVEL_25,
    NEW_BACKGROUND, NORMAL_SIZE, PACKET_ENHANCEMENT, PAGE_CODING_PARITY, PAGE_FUNCTION_BASIC, PAGE_FUNCTION_POP,
    PAGE_MOT, PAGE_TIME_FILLING, RELEASE_MOSAICS, ROW_ADDRESS_BASE, ROW_ADDRESS_LAST, ROW_ADDRESS_ROW_24,
    ROW_COLOUR_DOWN_TO_BOTTOM, ROW_COLOUR_THIS_ROW, SEPARATED_MOSAICS, START_BOX, STEADY, TRIPLET_BYTES,
    TRIPLETS_PER_PACKET,
};
use crate::ecc::with_odd_parity;
use crate::level1::{CharSource, FlashMode, FlashRate, Glyph, Grid, Size, Width, build_grid};
use crate::packet::{
    ColourData, CompositionalLink, DisplayAttributes, ObjectKind, ObjectNumber, ObjectSource, Packet, PacketMeta,
    PageFormat, TripletFunction, encode,
};
use crate::types::{
    ControlBits, DefaultColourFields, Designations, Instant, Magazine, PageAddress, PresentationFields, SubCode,
};

use super::attributes::{ATTRIBUTES, Flash, Rate, flash_of, flash_rate};
use super::characters::{enhancement_glyph, is_text};
use super::machine::traced_action;
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
    encode::header(address.magazine, address.page, SubCode::new(0), ControlBits(control), &display).unwrap()
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

/// X/28/0 format 1 of a basic page with these presentation fields.
fn x28_0(fields: &PresentationFields) -> Packet {
    let basic = PageFormat { function: PAGE_FUNCTION_BASIC, coding: PAGE_CODING_PARITY };
    encode::page_enhancement(magazine_1(), 0, basic, fields)
}

/// The default colour fields of X/28/0 (Table 4 triplet 13).
fn colour_fields(row: u8, substitution: bool, remapping: u8) -> PresentationFields {
    PresentationFields {
        colours: Some(DefaultColourFields { screen: 0, row, black_background_substitution: substitution, remapping }),
        ..PresentationFields::default()
    }
}

fn colour(index: u8) -> ColourData {
    ColourData { index, high: 0 }
}

/// A Display Attributes triplet (Table 29 mode 01100).
fn display_at(column: u8, attributes: DisplayAttributes) -> TripletFunction {
    TripletFunction::DisplayAttributes { column, attributes }
}

/// REQ-L25-021, REQ-L25-020: Foreground and Background Colour triplets set
/// a Colour Map index from their cell to the end of the row, until a later
/// attribute of the same type; reserved data only sets the column; at one
/// cell a triplet wins against a set-at Level 1 attribute of its type
/// (decision E.47).
// Needs its capability compiled in (decision E.105).
#[cfg(feature = "nonspacing-attributes")]
#[test]
fn non_spacing_colours_last_to_the_next_attribute_of_their_type() {
    let codes =
        [ALPHA_RED, b'A', b'B', b'C', b'D', ALPHA_YELLOW, b'E', b'F', BLACK_BACKGROUND, b'G', NEW_BACKGROUND, b'H'];
    let local = [
        position(2, 0),
        // CLUT 2 entry 4 and CLUT 1 entry 4 (Table 30).
        TripletFunction::ForegroundColour { column: 2, colour: colour(20) },
        TripletFunction::BackgroundColour { column: 3, colour: colour(12) },
        TripletFunction::ForegroundColour { column: 7, colour: ColourData { index: 5, high: 0b01 } },
        TripletFunction::BackgroundColour { column: 8, colour: colour(4) },
    ];
    let (grid, _) = build_25(&page(0, &[(2, &codes)], local_data(&local)));
    let colours: Vec<(u8, u8)> = grid.cells[2][..12].iter().map(|cell| (cell.foreground, cell.background)).collect();
    let red = ALPHA_RED;
    let yellow = ALPHA_YELLOW;
    let expected = [
        (COLOUR_WHITE, COLOUR_BLACK),
        (red, COLOUR_BLACK),
        (20, COLOUR_BLACK),
        (20, 12),
        (20, 12),
        (20, 12),
        (yellow, 12),
        (yellow, 12),
        (yellow, 4),
        (yellow, 4),
        // 1/D: the background takes the Level 1 foreground's entry.
        (yellow, yellow),
        (yellow, yellow),
    ];
    assert_eq!(colours, expected);
}

/// REQ-L25-022: at Levels 2.5 and 3.5 the Level 1 colours come from the
/// CLUTs Colour Table Re-mapping selects (Table 4), Alpha Black is entry 0
/// of the foreground CLUT, and Black Background Colour Substitution replaces
/// the default and the 1/C black background with the row colour, but not a
/// black background from 0/0 then 1/D, nor one a triplet sets.
#[test]
fn level_1_colours_are_remapped_and_substituted() {
    const ROW_COLOUR: u8 = 4;
    // Re-mapping 101: foreground CLUT 2, background CLUT 1.
    let fields = colour_fields(ROW_COLOUR, true, 0b101);
    let codes =
        [b'S', ALPHA_RED, b'A', NEW_BACKGROUND, b'B', BLACK_BACKGROUND, b'C', ALPHA_BLACK, NEW_BACKGROUND, b'D'];
    let local = [position(3, 0), TripletFunction::BackgroundColour { column: 11, colour: colour(COLOUR_BLACK) }];
    let mut others = std::vec![x28_0(&fields)];
    others.extend(local_data(&local));
    let (grid, _) = build_25(&page(0, &[(3, &codes)], others));
    let cell = |column: usize| (grid.cells[3][column].foreground, grid.cells[3][column].background);
    let clut = |clut: u8, entry: u8| clut * 8 + entry;
    assert_eq!(cell(0), (clut(2, COLOUR_WHITE), ROW_COLOUR), "start of row");
    assert_eq!(cell(2), (clut(2, ALPHA_RED), ROW_COLOUR));
    assert_eq!(cell(4), (clut(2, ALPHA_RED), clut(1, ALPHA_RED)), "1/D");
    assert_eq!(cell(6), (clut(2, ALPHA_RED), ROW_COLOUR), "1/C substituted");
    assert_eq!(cell(9), (clut(2, COLOUR_BLACK), clut(1, COLOUR_BLACK)), "0/0 then 1/D: not substituted");
    // Without the non-spacing attributes the triplet only sets the column.
    let triplet_black = if ATTRIBUTES { COLOUR_BLACK } else { clut(1, COLOUR_BLACK) };
    assert_eq!(cell(11).1, triplet_black, "a triplet's black is not substituted");
}

/// REQ-L25-023, decision E.47: a Level 1 colour code ends only the Level 1
/// conceal; the triplet's conceal lasts until a triplet clears it, and a 0
/// bit also ends a Level 1 conceal. Invert is recorded in the cell.
// Needs its capability compiled in (decision E.105).
#[cfg(feature = "nonspacing-attributes")]
#[test]
fn conceal_and_invert_of_the_display_attributes() {
    let conceal = DisplayAttributes { conceal: true, ..DisplayAttributes::default() };
    let invert = DisplayAttributes { invert: true, ..DisplayAttributes::default() };
    let codes = [CONCEAL, b'A', ALPHA_YELLOW, b'B', b' ', b'C', ALPHA_RED, b'D', b' ', b'E', b' ', CONCEAL, b'F', b'G'];
    let local = [
        position(4, 0),
        display_at(5, conceal),
        display_at(9, DisplayAttributes::default()),
        display_at(12, DisplayAttributes::default()),
        display_at(13, invert),
    ];
    let (grid, _) = build_25(&page(0, &[(4, &codes)], local_data(&local)));
    let concealed: Vec<bool> = grid.cells[4][..14].iter().map(|cell| cell.conceal).collect();
    let expected = [true, true, true, false, false, true, true, true, true, false, false, true, false, false];
    assert_eq!(concealed, expected);
    assert!(!grid.cells[4][12].attributes.invert() && grid.cells[4][13].attributes.invert());
}

/// REQ-L25-023, REQ-L25-011, Table 26 notes 11 and 12: the underline bit
/// separates Level 1 mosaics and 1/9 does not cancel it; clearing it also
/// clears a Level 1 1/A; it separates G1 characters of triplets and
/// underlines G0 and G2 characters, never G3 ones.
// Needs its capability compiled in (decision E.105).
#[cfg(feature = "nonspacing-attributes")]
#[test]
fn underline_and_separated_mosaics() {
    let underline = DisplayAttributes { underline: true, ..DisplayAttributes::default() };
    let full = 0x7F;
    let mosaics = [MOSAIC_WHITE, SEPARATED_MOSAICS, full, full, b' ', full, CONTIGUOUS_MOSAICS, full];
    let text = *b"AB C";
    let local = [
        position(5, 0),
        display_at(3, DisplayAttributes::default()),
        display_at(5, underline),
        TripletFunction::G1Character { column: 9, code: full },
        position(6, 0),
        display_at(1, underline),
        TripletFunction::G3Character { column: 2, code: 0x41, level_15_form: false },
    ];
    let (grid, _) = build_25(&page(0, &[(5, &mosaics), (6, &text)], local_data(&local)));
    let separated = |column: usize| match grid.cells[5][column].glyph {
        Glyph::Mosaic { separated, .. } => separated,
        other => panic!("column {column}: {other:?}"),
    };
    assert_eq!([2, 3, 5, 7, 9].map(separated), [true, false, true, true, true]);
    let underlined: Vec<bool> = grid.cells[6][..4].iter().map(|cell| cell.attributes.underline()).collect();
    assert_eq!(underlined, [false, true, false, true]);
}

/// Decision E.48: Level 1 Flash is slow flash in normal mode at Levels 2.5
/// and 3.5, until Steady.
#[test]
fn level_1_flash_is_normal_flash_at_the_level_1_rate() {
    let codes = [FLASH, b'A', STEADY, b'B'];
    let (grid, _) = build_25(&page(0, &[(7, &codes)], local_data(&[position(7, 0), g0_at(10, b'Z')])));
    let cell = grid.cells[7][1];
    assert!(cell.flash);
    assert_eq!((cell.attributes.flash_mode(), cell.attributes.flash_rate()), (FlashMode::Normal, FlashRate::Level1));
    assert!(!grid.cells[7][2].flash && !grid.cells[7][3].flash);
}

/// Decision E.47: the Level 1 box codes act only on a newsflash or subtitle
/// page, Start Box doubled as the configuration asks (REQ-L1 decision E.17).
#[test]
fn level_1_boxes_only_on_boxed_pages() {
    let codes = [START_BOX, START_BOX, b'A', END_BOX, b'B'];
    let boxed: Vec<bool> = [C6_SUBTITLE, 0]
        .into_iter()
        .flat_map(|control| {
            let (grid, _) = build_25(&page(control, &[(8, &codes)], local_data(&[position(8, 0), g0_at(20, b'Z')])));
            [grid.cells[8][2].boxed, grid.cells[8][4].boxed]
        })
        .collect();
    assert_eq!(boxed, [true, false, false, false]);
}

/// REQ-FSM-006 step 2, decision E.50: Hold Mosaics shows the held mosaic at
/// attribute cells; a G3 character of a triplet does not change it, and it
/// never replaces a character of a triplet; Release Mosaics ends it after
/// its cell, and a change of size or of mosaic mode forgets the held one.
#[test]
fn hold_mosaics_beside_enhancement_characters() {
    let full = 0x7F;
    let codes =
        [MOSAIC_RED, HOLD_MOSAICS, full, MOSAIC_WHITE, b'A', MOSAIC_WHITE, MOSAIC_RED, RELEASE_MOSAICS, MOSAIC_WHITE];
    let resets = [MOSAIC_RED, HOLD_MOSAICS, full, ALPHA_RED, MOSAIC_RED, MOSAIC_RED, full, DOUBLE_WIDTH, MOSAIC_RED];
    let local =
        [position(9, 0), TripletFunction::G3Character { column: 4, code: 0x41, level_15_form: false }, g0_at(6, b'Z')];
    let (grid, _) = build_25(&page(0, &[(9, &codes), (10, &resets)], local_data(&local)));
    let held = Glyph::Mosaic { pattern: MOSAIC_FULL, separated: false };
    let row = &grid.cells[9];
    assert_eq!((row[1].glyph, row[3].glyph, row[5].glyph), (Glyph::Space, held, held), "nothing held yet at 1/E");
    assert!(row[3].held && row[5].held);
    assert_eq!(shown(&row[6]), 'Z');
    assert_eq!((row[7].glyph, row[8].glyph), (held, Glyph::Space), "1/F is set-after");
    let row = &grid.cells[10];
    assert_eq!((row[3].glyph, row[5].glyph), (held, Glyph::Space), "alphanumerics forget the held mosaic");
    assert_eq!((row[7].glyph, row[8].glyph), (held, Glyph::Space), "a size change forgets it");
}

/// Clause 15.4, REQ-L25-012: ESC switches the Level 1 page to the second G0
/// set; the characters of triplets never take it.
#[test]
fn esc_selects_the_second_g0_set_for_level_1_only() {
    let fields = PresentationFields {
        designations: Some(Designations { default: 0, second: GREEK }),
        ..PresentationFields::default()
    };
    let mut others = std::vec![x28_0(&fields)];
    others.extend(local_data(&[position(11, 0), g0_at(3, b'b')]));
    let (grid, _) = build_25(&page(0, &[(11, &[b'a', ESC, b'a', b' '])], others));
    let set = |column: usize| match grid.cells[11][column].glyph {
        Glyph::Char(character) => character.set,
        other => panic!("{other:?}"),
    };
    assert_eq!(set(0), CharSet::G0(G0 { set: G0Set::Latin, national: Some(crate::charset::NationalOption::English) }));
    assert!(matches!(set(2), CharSet::G0(G0 { set: G0Set::Greek, .. })));
    assert_eq!(set(3), CharSet::G0(G0 { set: G0Set::Latin, national: None }));
}

/// REQ-L25-025, REQ-L25-026, Annex C.3: spacing double height shows lower
/// halves in the row below, which elsewhere shows the background above and
/// no foreground, ignores its Level 1 bytes, and still shows the characters
/// of triplets; row 23 never doubles (rule 1).
#[test]
fn spacing_double_height_and_the_row_below() {
    let rows: [(u8, &[u8]); 4] = [
        (5, &[DOUBLE_HEIGHT, b'A', b'B', NORMAL_SIZE, ALPHA_RED, NEW_BACKGROUND]),
        (6, b"xyzxyzxyzxyz"),
        (21, &[DOUBLE_HEIGHT, b'X']),
        (23, &[DOUBLE_HEIGHT, b'Y']),
    ];
    let (grid, _) = build_25(&page(0, &rows, local_data(&[position(6, 0), g0_at(10, b'E')])));
    let cells = &grid.cells;
    assert_eq!((cells[5][1].size, cells[6][1].size), (Size::DoubleTop, Size::DoubleBottom));
    assert_eq!(cells[6][1].glyph, cells[5][1].glyph);
    assert_eq!((cells[6][7].glyph, cells[6][7].background), (Glyph::Space, ALPHA_RED), "the fill");
    assert_eq!((shown(&cells[6][10]), cells[6][10].attributes.source()), ('E', CharSource::Local));
    assert_eq!((cells[22][1].size, cells[23][1].size), (Size::DoubleBottom, Size::Normal), "rule 1");
    assert_eq!(grid.double_height_rows, 1 << 5 | 1 << 21);
}

/// REQ-L25-026, Annex C.3 rules 2 and 3: non-spacing double height does
/// not suppress the row below, where its lower half wins against a later
/// double width.
// Needs its capability compiled in (decision E.105).
#[cfg(feature = "nonspacing-attributes")]
#[test]
fn non_spacing_double_height_keeps_the_row_below() {
    let double_height = DisplayAttributes { double_height: true, ..DisplayAttributes::default() };
    let double_width = DisplayAttributes { double_width: true, ..DisplayAttributes::default() };
    let local = [
        position(8, 0),
        display_at(2, double_height),
        g0_at(2, b'H'),
        display_at(3, DisplayAttributes::default()),
        position(9, 0),
        display_at(1, double_width),
        g0_at(1, b'W'),
    ];
    let (grid, _) = build_25(&page(0, &[(8, b"........"), (9, b"abcdef")], local_data(&local)));
    let cells = &grid.cells;
    assert_eq!((cells[8][2].size, cells[9][2].size, shown(&cells[9][2])), (Size::DoubleTop, Size::DoubleBottom, 'H'));
    assert_eq!(cells[9][1].attributes.width(), Width::Normal, "rule 3: the lower half wins");
    assert_eq!(text(&grid, 9, 0..=0), "a", "the row below non-spacing double height is shown");
    assert_eq!(grid.double_height_rows, 1 << 8);
}

/// REQ-L25-032, REQ-L25-033, decision E.45: Full Screen Colour, Full Row
/// Colour "this row" and "down to the bottom" in transmission order,
/// reserved row selections and reserved screen data ignored, Address
/// Display Row 0 for row 0; the fill reaches row 24 only when it is shown.
#[test]
fn screen_and_row_colours() {
    let row_colour = |row: u8, index: u8, high: u8| TripletFunction::FullRowColour {
        address: ROW_ADDRESS_BASE + row,
        colour: ColourData { index, high },
    };
    let local = [
        TripletFunction::FullScreenColour(colour(5)),
        TripletFunction::FullScreenColour(ColourData { index: 9, high: 0b01 }),
        row_colour(3, 6, ROW_COLOUR_DOWN_TO_BOTTOM),
        row_colour(5, 7, ROW_COLOUR_THIS_ROW),
        row_colour(7, 1, 0b01),
        TripletFunction::AddressDisplayRow0 { address: ROW_ADDRESS_LAST, colour: colour(2) },
    ];
    let mut config = level(PresentationLevel::Level25);
    for (row24, row_24_colour) in [(Row24Policy::AnnexH, 5), (Row24Policy::WhenPresent, 6)] {
        config.row24 = row24;
        let (grid, _) = build(&page(0, &[(1, b"ROW 1"), (24, b"ROW 24")], local_data(&local)), &config);
        assert_eq!(grid.screen_colour, 5);
        let mut expected = [6; ROWS];
        expected[..3].copy_from_slice(&[2, 5, 5]);
        expected[5] = 7;
        expected[ROWS - 1] = row_24_colour;
        assert_eq!(grid.row_colours, expected, "{row24:?}");
    }
}

/// REQ-OBJ-041: an enhancement page shows no rows of text: a page whose
/// X/28/0 names an object page function; row 0 stays. The Magazine
/// Organization Table is never in the page store at all, even under the
/// diagnostic setting, since a reserved page number is read only as its
/// function (REQ-NUM-010).
#[test]
fn enhancement_pages_show_no_text() {
    let config = Config { store_hexadecimal_pages: true, ..level(PresentationLevel::Level25) };
    let pop = PageFormat { function: PAGE_FUNCTION_POP, coding: PAGE_CODING_PARITY };
    let x28 = encode::page_enhancement(magazine_1(), 0, pop, &PresentationFields::default());
    // The first reception makes the page an enhancement page; a decimal
    // page is then stored in both stores (REQ-ACQ-016).
    let pop_page = [page(0, &[(1, b"OBJECTS")], std::vec![x28]), page(0, &[(1, b"OBJECTS")], std::vec![x28])].concat();
    let mot = PageAddress { magazine: magazine_1(), page: crate::types::PageNumber(PAGE_MOT) }.mpp();
    let (grid, report) = build_page(PAGE, &pop_page, &config);
    assert!(grid.cells[1..].iter().flatten().all(|cell| *cell == Cell::BLANK));
    assert_eq!((grid.double_height_rows, report.steps), (0, 0));
    let mut pages = std::vec![StoredPage::EMPTY; 4];
    let mut decoder = Decoder::new(&mut pages, config);
    for (index, packet) in [header(mot, 0), display_row(1, b"TABLE"), header(PAGE, 0)].iter().enumerate() {
        decoder.push(packet, &PacketMeta::default(), Instant(index as u64), &mut |_| {});
    }
    assert!(!decoder.store().contains(PageAddress::from_mpp(mot).unwrap()));
}

/// REQ-OBJ-041: a page that a link names, and that is therefore also kept
/// as an enhancement page, is still shown when its own X/28/0 gives the
/// page function of a basic page, so that a wrong link never hides it.
#[test]
fn a_display_page_named_by_a_link_is_still_shown() {
    let config = level(PresentationLevel::Level25);
    let address = PageAddress::from_mpp(PAGE).unwrap();
    let to_itself = CompositionalLink {
        function: 0b00,
        validity: 0b11,
        magazine: address.magazine,
        page: address.page,
        subpages: 0xFFFF,
        marker_first: true,
        marker_second: false,
    };
    let reception = || {
        let links = encode::compositional_links(magazine_1(), 4, &[to_itself; COMPOSITIONAL_LINKS]);
        page(0, &[(1, b"SHOWN")], std::vec![links, x28_0(&PresentationFields::default())])
    };
    // The first reception names the page; the second is kept in both stores.
    let packets = [reception(), reception()].concat();
    with_page(PAGE, &packets, &config, |_, _, lookup| {
        assert!(lookup.sub_page(address.magazine, address.page, 0).is_some(), "kept as an enhancement page");
    });
    let (grid, _) = build(&packets, &config);
    assert_eq!(text(&grid, 1, 0..=4), "SHOWN");
}

/// REQ-L25-001: rows the Level 1 grid does not show keep their Level 1
/// cells: row 0 of a page with C7 set, and rows 1 to 24 of a page with C10
/// set; the other rows are still enhanced.
#[test]
fn rows_the_page_hides_keep_their_level_1_cells() {
    let row_0 =
        TripletFunction::AddressDisplayRow0 { address: ROW_ADDRESS_LAST, colour: ColourData { index: 1, high: 0b01 } };
    let local = [row_0, g0_at(10, b'Y'), position(2, 0), g0_at(1, b'Z')];
    let config = level(PresentationLevel::Level25);
    for (control, hidden, enhanced) in [(C7_SUPPRESS_HEADER, 0..1, (2, 1)), (C10_INHIBIT_DISPLAY, 1..ROWS, (0, 10))] {
        let packets = page(control, &[(2, b"ROW 2")], local_data(&local));
        with_page(PAGE, &packets, &config, |page, magazine, lookup| {
            let options = DisplayOptions::default();
            let mut grid = Box::new(Grid::EMPTY);
            build_grid(&mut grid, Some(page), magazine, &config, &options);
            let level_1 = grid.clone();
            let _ = apply(&mut grid, page, magazine, lookup, &config, &options);
            assert_eq!(grid.cells[hidden.clone()], level_1.cells[hidden], "{control:#X}");
            assert_eq!(grid.cells[enhanced.0][enhanced.1].attributes.source(), CharSource::Local, "{control:#X}");
        });
    }
}

/// REQ-L25-001: at Level 1.5 the processor does nothing.
#[test]
fn the_processor_runs_only_at_levels_2_5_and_3_5() {
    let config = level(PresentationLevel::Level15);
    let packets = page(0, &[(1, b"ROW 1")], local_data(&[position(1, 0), g0_at(1, b'Z')]));
    with_page(PAGE, &packets, &config, |page, magazine, lookup| {
        let options = DisplayOptions::default();
        let mut grid = Box::new(Grid::EMPTY);
        build_grid(&mut grid, Some(page), magazine, &config, &options);
        let level_1 = grid.clone();
        let report = apply(&mut grid, page, magazine, lookup, &config, &options);
        assert_eq!(grid.cells, level_1.cells);
        assert_eq!((report.steps, report.faults), (0, crate::acq::EnhancementFaults::NONE));
    });
}

/// Group 1111, which Table 34 and neoteletext's own sets leave reserved.
const RESERVED_DESIGNATION: u8 = designation(0b1111, 0b000);

/// The transitions of the page-level run, as `apply_traced` reports them.
fn traced(packets: &[Packet], trace: bool) -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    let config = Config { trace, ..level(PresentationLevel::Level25) };
    with_page(PAGE, packets, &config, |page, magazine, lookup| {
        let options = DisplayOptions::default();
        let mut grid = Box::new(Grid::EMPTY);
        build_grid(&mut grid, Some(page), magazine, &config, &options);
        let mut entries = Vec::new();
        let mut receive = |entry: crate::trace::TraceEntry| {
            assert_eq!(entry.machine, crate::trace::Machine::EnhancementWalk);
            entries.push((entry.from, entry.event, entry.action, entry.to));
        };
        let mut sink = crate::trace::TraceSink { time: Instant(0), entries: &mut receive };
        let _ = apply_traced(&mut grid, page, magazine, lookup, &config, &options, &mut sink);
        entries
    })
}

/// REQ-FSM-010, REQ-FSM-011, REQ-L25-051: the walk reports every transition
/// with the table's state names, and names a fault in place of the action
/// that met it; without the configuration's trace it reports nothing.
#[test]
fn the_walk_is_traced() {
    let row_0 =
        TripletFunction::AddressDisplayRow0 { address: ROW_ADDRESS_LAST, colour: ColourData { index: 1, high: 0b01 } };
    let reserved_designation = TripletFunction::ModifiedDesignation { column: 11, designation: RESERVED_DESIGNATION };
    let first = damage(x26(0, &[g0_at(1, b'A'), row_0, g0_at(3, b'X'), g0_at(10, b'Y'), reserved_designation, END]), 0);
    let packets = page(0, &[(1, b"ROW 1")], std::vec![first]);
    let (walking, returning) = ("Walking", "Returning");
    let expected = [
        ("Start", "walk begins, with a display triplet", "begin the local enhancement data", walking),
        (walking, "damaged triplet", "skip the damaged triplet", walking),
        (walking, row_0.name(), "move to the row; row colour", walking),
        (walking, "G0 character", "discard the write (REQ-OBJ-014, REQ-L25-050)", walking),
        (walking, "G0 character", "set the column; place the character or attribute", walking),
        (walking, reserved_designation.name(), "ignore the designation (REQ-L25-012)", walking),
        (walking, END.name(), "end the source", returning),
        (returning, "the local enhancement data ended", "none", "Finished"),
    ];
    assert_eq!(traced(&packets, true), expected);
    assert_eq!(traced(&packets, false), []);
    // REQ-OBJ-040, decision E.53: data with no display triplet places the
    // default objects; with none, the walk ends.
    let quiet = page(0, &[(1, b"ROW 1")], local_data(&[FILLER]));
    let expected = [
        ("Start", "walk begins, with no display triplet", "none", "Placing default objects"),
        ("Placing default objects", "no more default objects", "none", "Finished"),
    ];
    assert_eq!(traced(&quiet, true), expected);
    // Local data without a Termination Marker ends with its data.
    let unended = page(0, &[(1, b"ROW 1")], std::vec![x26(0, &[g0_at(1, b'A')])]);
    let entries = traced(&unended, true);
    let expected = [
        ("Walking", "end of the source's data", "end the source", "Returning"),
        ("Returning", "the local enhancement data ended", "none", "Finished"),
    ];
    assert_eq!(entries[entries.len() - 2..], expected);
}

/// True when no two names are the same.
fn distinct(names: &[&str]) -> bool {
    let mut sorted = names.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).all(|pair| pair[0] != pair[1])
}

/// REQ-FSM-010, REQ-FSM-011: the trace names the states as the table of
/// REQ-FSM-011 does, a triplet by its function, and every other event and
/// every action by a name of its own, so that no two transitions read
/// alike.
#[test]
fn every_state_event_and_action_has_its_own_name() {
    use WalkAction as A;
    use WalkEvent as E;
    use WalkState as S;
    let states = [
        S::Start,
        S::Walking,
        S::ModifierPending { row_offset: 1, column_offset: 2 },
        S::Returning,
        S::PlacingDefaults,
        S::Finished,
    ];
    let table = ["Start", "Walking", "Modifier pending", "Returning", "Placing default objects", "Finished"];
    assert_eq!(states.map(walk_state_name), table);
    let events = [
        E::Begin { display: true },
        E::Begin { display: false },
        E::Triplet(None),
        E::EndOfData,
        E::Returned(Ended::Local),
        E::Returned(Ended::Object),
        E::Returned(Ended::Default),
        E::DefaultObject { found: true },
        E::DefaultObject { found: false },
    ];
    assert!(distinct(&events.map(walk_event_name)));
    assert_eq!(walk_event_name(E::Triplet(Some(END))), END.name());
    let colour = colour(1);
    let number = ObjectNumber(0);
    let actions = [
        A::None,
        A::BeginLocal,
        A::StartDefault,
        A::Skip,
        A::EndSource,
        A::Pop,
        A::ResetPosition,
        A::ScreenColour(colour),
        A::RowColour { row: 1, colour },
        A::Position { row: 1, column: 1 },
        A::Invoke { source: ObjectSource::Local, kind: ObjectKind::Active, number, row_offset: 0, column_offset: 0 },
        A::Column(END),
    ];
    assert!(distinct(&actions.map(walk_action_name)));
}

/// REQ-L25-051: the trace names a fault the step met in place of the
/// action, the first of them in the order of the report; in Placing
/// default objects, finding the next default object may pass over several,
/// which the trace says together.
#[test]
fn faults_are_named_in_place_of_the_action() {
    use crate::acq::EnhancementFaults;
    let none = EnhancementFaults::NONE;
    let one = |set: fn(&mut EnhancementFaults)| {
        let mut faults = none;
        set(&mut faults);
        faults
    };
    let faults = [
        (one(|f| f.forbidden_invocations = 1), "skip the invocation: not permitted (REQ-OBJ-008)"),
        (one(|f| f.limited_invocations = 1), "skip the invocation: a limit of Annex A.8 is reached"),
        (one(|f| f.absent_objects = 1), "skip the invocation: the object is absent"),
        (one(|f| f.discarded_writes = 1), "discard the write (REQ-OBJ-014, REQ-L25-050)"),
        (one(|f| f.ignored_designations = 1), "ignore the designation (REQ-L25-012)"),
    ];
    let column = WalkAction::Column(g0_at(1, b'A'));
    for (after, name) in faults {
        assert_eq!(traced_action(WalkState::Walking, column, &none, &after), name);
    }
    let both = EnhancementFaults { forbidden_invocations: 1, absent_objects: 1, ..none };
    assert_eq!(traced_action(WalkState::Walking, column, &none, &both), faults[0].1);
    // Faults that are not the walk's own are not named.
    let damaged = one(|f| f.damaged_triplets = 1);
    assert_eq!(
        traced_action(WalkState::Walking, WalkAction::Skip, &none, &damaged),
        walk_action_name(WalkAction::Skip)
    );
    let absent = one(|f| f.absent_objects = 2);
    let defaults = WalkState::PlacingDefaults;
    assert_eq!(
        traced_action(defaults, WalkAction::StartDefault, &none, &absent),
        "start the default object, past default objects absent or over a limit"
    );
    assert_eq!(
        traced_action(defaults, WalkAction::None, &none, &absent),
        "none: the default objects left are absent or over a limit"
    );
}

// ---------------------------------------------------------------------------
// Objects (area OBJ), compiled in with the feature `objects`.
// ---------------------------------------------------------------------------

/// A local object's number: the designation code of its packet X/26 and
/// its triplet there (Table 28, REQ-OBJ-003).
#[cfg(feature = "objects")]
const fn local_number(designation: u8, triplet: u8) -> ObjectNumber {
    ObjectNumber((designation as u16) << 4 | triplet as u16)
}

/// The definition of a local object of type `kind` for both levels.
#[cfg(feature = "objects")]
fn define(kind: ObjectKind, number: ObjectNumber) -> TripletFunction {
    TripletFunction::ObjectDefinition { usage: crate::packet::ObjectUsage(0b11), kind, number }
}

#[cfg(feature = "objects")]
fn invoke_local(kind: ObjectKind, number: ObjectNumber) -> TripletFunction {
    TripletFunction::ObjectInvocation { source: ObjectSource::Local, kind, number }
}

/// REQ-OBJ-014, decision E.49: an element of an object that falls outside
/// every enabled display area, in panel columns that are not enabled or
/// beyond column 71, is discarded and counted, as one that crosses from
/// the main page is.
#[cfg(feature = "objects")]
#[test]
fn object_writes_outside_every_display_area_are_discarded() {
    let number = local_number(1, 0);
    let objects = x26(1, &[define(ObjectKind::Passive, number), g0_at(0, b'P'), g0_at(30, b'Q'), END]);
    let at = |column_offset: u8| {
        [TripletFunction::OriginModifier { row_offset: 0, column_offset }, invoke_local(ObjectKind::Passive, number)]
    };
    let local = [[position(2, 0)].as_slice(), &at(10), &at(45), &at(71)].concat();
    let mut others = local_data(&local);
    others.push(objects);
    let (grid, report) = build_25(&page(0, &[(2, b"ROW 2")], others));
    assert_eq!((shown(&grid.cells[2][10]), grid.cells[2][10].attributes.source()), ('P', CharSource::Passive));
    // Column 40 from the main page; columns 45, 75, 71 and 101 with no
    // side panel enabled.
    assert_eq!(report.faults.discarded_writes, 5);
}

/// REQ-L25-012, Annex C.4 rule 2: in an Adaptive or Passive object a
/// Modified G0 and G2 designation lasts to the end of the object, across
/// its rows, and not into the data that invoked it.
#[cfg(feature = "objects")]
#[test]
fn a_designation_in_an_object_lasts_to_its_end() {
    let greek = TripletFunction::ModifiedDesignation { column: 0, designation: GREEK };
    let mut objects = Vec::new();
    for (designation, kind) in [(1, ObjectKind::Passive), (2, ObjectKind::Adaptive)] {
        let body =
            [define(kind, local_number(designation, 0)), greek, g0_at(1, b'a'), position(1, 0), g0_at(2, b'b'), END];
        objects.push(x26(designation, &body));
    }
    let local = [
        position(2, 0),
        invoke_local(ObjectKind::Passive, local_number(1, 0)),
        g0_at(10, b'c'),
        position(5, 0),
        invoke_local(ObjectKind::Adaptive, local_number(2, 0)),
        g0_at(10, b'c'),
    ];
    let mut others = local_data(&local);
    others.extend(objects);
    let (grid, _) = build_25(&page(0, &[], others));
    let set = |row: usize, column: usize| match grid.cells[row][column].glyph {
        Glyph::Char(CharRef { set: CharSet::G0(g0), .. }) => g0.set,
        other => panic!("row {row} column {column}: {other:?}"),
    };
    let passive = [(2, 1), (3, 2), (2, 10)].map(|(row, column)| set(row, column));
    let adaptive = [(5, 1), (6, 2), (5, 10)].map(|(row, column)| set(row, column));
    for sets in [passive, adaptive] {
        assert_eq!(sets, [G0Set::Greek, G0Set::Greek, G0Set::Latin]);
    }
}

/// REQ-OBJ-006, decision E.51: an object whose body runs into a packet
/// that was not received is absent: nothing of it is placed, and it is
/// counted.
#[cfg(feature = "objects")]
#[test]
fn an_object_with_a_missing_packet_is_absent() {
    // Triplets 11 and 12 of designation code 1; code 2 is not sent, and no
    // Termination Marker comes before it.
    let number = local_number(1, 11);
    let objects = x26(
        1,
        &[
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            FILLER,
            define(ObjectKind::Passive, number),
            g0_at(3, b'P'),
        ],
    );
    let mut others = local_data(&[position(2, 0), invoke_local(ObjectKind::Passive, number)]);
    others.push(objects);
    let (grid, report) = build_25(&page(0, &[(2, b"ROW 2")], others));
    assert_eq!(text(&grid, 2, 0..=4), "ROW 2");
    assert_eq!((report.faults.absent_objects, report.objects), (1, 0));
}

/// REQ-OBJ-040, clause 13.8: a page with no display triplet places the
/// default objects of its Magazine Organization Table; one whose object
/// page is not received is absent, counted, and named in the trace.
#[cfg(feature = "objects")]
#[test]
fn an_absent_default_object_is_counted() {
    use crate::packet::{DefaultObject, MotObjectLink, MotPageEntry, mot_position};
    let gpop = MotObjectLink {
        magazine: magazine_1(),
        x27_flag: false,
        page: crate::types::PageNumber(0xA1),
        subpage_count: 0,
        fallback: crate::types::FallbackFields::default(),
        default_objects: [Some(DefaultObject { kind: ObjectKind::Active, number: 0 }), None],
    };
    let unused = MotObjectLink { page: crate::types::PageNumber(LINK_NULL_PAGE), default_objects: [None; 2], ..gpop };
    let mut links = [0; DATA_BYTES];
    for (slot, link) in links.as_chunks_mut::<MOT_OBJECT_LINK_BYTES>().0.iter_mut().zip([gpop, unused, unused, unused])
    {
        slot.copy_from_slice(&encode::mot_object_link(&link));
    }
    let position = mot_position(PageAddress::from_mpp(PAGE).unwrap().page);
    let mut entries = [0; DATA_BYTES];
    let entry = MotPageEntry { global_objects: true, pop_link: 0 };
    entries[position.index..position.index + 2].copy_from_slice(&encode::mot_page_entry(entry));
    let mot = PageAddress { magazine: magazine_1(), page: crate::types::PageNumber(PAGE_MOT) }.mpp();
    let mut packets = std::vec![
        header(mot, 0),
        encode::nibble_packet(magazine_1(), position.packet, &entries),
        encode::nibble_packet(magazine_1(), MOT_OBJECT_LINKS_LEVEL_25[0], &links),
    ];
    packets.extend(page(0, &[(1, b"ROW 1")], Vec::new()));
    let (_, report) = build_25(&packets);
    assert_eq!((report.faults.absent_objects, report.objects), (1, 0));
    let entries = traced(&packets, true);
    assert_eq!(entries.last().unwrap().2, "none: the default objects left are absent or over a limit");
}
