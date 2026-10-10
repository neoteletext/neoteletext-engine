//! Objects at Levels 2.5 and 3.5 (area OBJ, REQ-TST-016, REQ-TST-017,
//! REQ-TST-019): local, public and global objects built with the encoder,
//! found through X/27/4 and the Magazine Organization Table, and run by the
//! enhancement processor.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

mod common;

use common::{
    DISPLAY, Mot, ObjectDef, Receiver, build, build_25, build_35, char_of, define, display_page, drcs_link, end,
    foreground, g0, invoke, link, local_data, local_number, mot_packets, null_link, object_link, object_page,
    object_page_data, object_page_packets, origin, page_grid, public_number, row_0, set_position, text, text_at, x26,
    x27, x28,
};
use tellytxt_core::colour::{SidePanels, colour_index};
use tellytxt_core::config::{Config, PresentationLevel};
use tellytxt_core::consts::{
    LINK_FUNCTION_DRCS, LINK_FUNCTION_GDRCS, LINK_FUNCTION_GPOP, LINK_FUNCTION_POP, OBJECT_INVOCATIONS_PER_TYPE,
    OBJECTS_OPEN_AT_ONCE,
};
use tellytxt_core::enhance::MAX_WALK_STEPS;
use tellytxt_core::level1::CharSource;
use tellytxt_core::objects::{self, PageLink};
use tellytxt_core::packet::{
    ColourData, DefaultObject, MotPageEntry, ObjectKind, ObjectNumber, ObjectSource, Packet, TripletFunction,
};
use tellytxt_core::types::{FallbackFields, PageAddress, PresentationFields, SidePanelFields, SubCode};

const ALPHA_GREEN: u8 = 0x02;
const ALPHA_BLUE: u8 = 0x04;
const ALPHA_RED: u8 = 0x01;
const L25: PresentationLevel = PresentationLevel::Level25;
const L35: PresentationLevel = PresentationLevel::Level35;
const POP: u16 = 0x1A0;
const GPOP: u16 = 0x1A1;

/// X/27/4 naming the POP 1A0 and the GPOP 1A1 for both levels.
fn object_links() -> Packet {
    x27(
        4,
        [
            link(LINK_FUNCTION_GPOP, 0b11, GPOP),
            link(LINK_FUNCTION_POP, 0b11, POP),
            null_link(LINK_FUNCTION_GDRCS),
            null_link(LINK_FUNCTION_DRCS),
            null_link(0),
            null_link(0),
        ],
    )
}

/// A local object in packet X/26/1 or later: its definition and body.
fn local_object(kind: ObjectKind, designation: u8, body: &[TripletFunction]) -> Vec<TripletFunction> {
    let mut all = vec![define(kind, 0b11, local_number(designation, 0))];
    all.extend_from_slice(body);
    all.push(TripletFunction::Termination { kind: 0b101 });
    all
}

/// Page 100 with local data in X/26/0 and local objects from X/26/1, each
/// object starting a new packet.
fn page_with_objects(rows: &[(u8, &[u8])], local: &[TripletFunction], objects: &[Vec<TripletFunction>]) -> Vec<Packet> {
    let mut others = vec![object_links()];
    let mut data = local.to_vec();
    data.push(end());
    others.push(x26(0, &data));
    // Each object starts a packet; one longer than a packet runs on into the
    // next, so it must be the last.
    let mut designation = 1u8;
    for object in objects {
        for chunk in object.chunks(13) {
            others.push(x26(designation, chunk));
            designation += 1;
        }
        if object.is_empty() {
            others.push(x26(designation, &[]));
            designation += 1;
        }
    }
    display_page(0, rows, others)
}

/// The Level 1 row of the clause 13.9 example (columns 0 to 27).
fn example_row() -> Vec<u8> {
    let mut row = vec![ALPHA_BLUE];
    row.extend_from_slice(b"THE FAST DOG AND");
    row.push(ALPHA_RED);
    row.extend_from_slice(b"CAT");
    row.push(ALPHA_GREEN);
    row.extend_from_slice(b"RAN IN");
    row
}

/// The object of the clause 13.9 example: "LAZY", "COW" after Foreground
/// Pink, "FOX", and "SAT" after Foreground Yellow. Pink is CLUT 2 entry 0
/// (REQ-TST-016).
fn example_body() -> Vec<TripletFunction> {
    let mut body = text_at(5, b"LAZY");
    body.push(foreground(10, PINK));
    body.extend(text_at(10, b"COW"));
    body.extend(text_at(18, b"FOX"));
    body.push(foreground(22, YELLOW));
    body.extend(text_at(22, b"SAT"));
    body
}

const PINK: u8 = colour_index(2, 0);
const YELLOW: u8 = colour_index(0, 3);
const BLUE: u8 = colour_index(0, 4);
const RED: u8 = colour_index(0, 1);
const GREEN: u8 = colour_index(0, 2);
const WHITE: u8 = colour_index(0, 7);

/// REQ-OBJ-009 to REQ-OBJ-011, REQ-TST-016: the object example of clause
/// 13.9 for each type, from each source (REQ-OBJ-001).
#[test]
fn clause_13_9_object_example() {
    let expected = [
        // THE, LAZY, COW, AND, FOX, SAT, IN.
        (ObjectKind::Active, [BLUE, BLUE, PINK, PINK, RED, YELLOW, YELLOW]),
        (ObjectKind::Adaptive, [BLUE, BLUE, PINK, PINK, PINK, YELLOW, GREEN]),
        (ObjectKind::Passive, [BLUE, WHITE, PINK, BLUE, PINK, YELLOW, GREEN]),
    ];
    let words = [1, 5, 10, 14, 18, 22, 26];
    let row = example_row();
    for (kind, colours) in expected {
        for source in [ObjectSource::Local, ObjectSource::Public, ObjectSource::Global] {
            let number = if source == ObjectSource::Local { local_number(1, 0) } else { public_number(0, 0) };
            let local = [set_position(2, 0), invoke(source, kind, number)];
            let mut packets = page_with_objects(&[(2, &row)], &local, &[local_object(kind, 1, &example_body())]);
            let object = ObjectDef::new(kind, 0, example_body());
            packets.extend(object_page(if source == ObjectSource::Global { GPOP } else { POP }, 0, &[object]));
            let (grid, report) = build(&packets, L25);
            assert_eq!(report.unwrap().objects, 1, "{kind:?} from {source:?}");
            assert_eq!(text(&grid, 2).trim_end(), " THE LAZY COW AND FOX SAT IN", "{kind:?} from {source:?}");
            for (word, (&column, &colour)) in words.iter().zip(colours.iter()).enumerate() {
                for offset in 0..2 {
                    let cell = grid.cells[2][column + offset];
                    assert_eq!(cell.foreground, colour, "{kind:?} from {source:?}: word {word}");
                }
            }
            let plane = match kind {
                ObjectKind::Active => CharSource::Active,
                ObjectKind::Adaptive => CharSource::Adaptive,
                ObjectKind::Passive => CharSource::Passive,
            };
            assert_eq!(grid.cells[2][5].attributes.source(), plane);
            assert_eq!(grid.cells[2][14].attributes.source(), CharSource::Level1);
        }
    }
}

/// REQ-OBJ-011: every invocation of a Passive object starts from its
/// defaults, and its attributes persist across rows.
#[test]
fn passive_objects_start_from_their_defaults() {
    let body = [foreground(0, PINK), g0(0, b'a'), set_position(1, 0), g0(0, b'b')];
    let local = [
        set_position(3, 0),
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(1, 0)),
        set_position(3, 10),
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(1, 0)),
    ];
    let mut object = local_object(ObjectKind::Passive, 1, &body);
    object.insert(1, foreground(0, colour_index(0, 1)));
    let grid = build_25(&page_with_objects(&[(4, &[ALPHA_GREEN, b'x'])], &local, &[object]));
    assert_eq!((grid.cells[3][0].foreground, grid.cells[4][0].foreground), (PINK, PINK), "persists to row 4");
    assert_eq!(grid.cells[3][10].foreground, PINK, "the second invocation starts again from white");
    assert_eq!(grid.cells[4][1].foreground, GREEN, "the page under the object is not affected");
}

/// REQ-OBJ-002, REQ-OBJ-014, REQ-L25-041: the Origin Modifier places an
/// object, also in the side panels (the worked modifier of
/// `research/notes-level25-objects.md` section 13.2: row +2, column +56),
/// the position returns after the object, and elements beyond the object's
/// display area are discarded.
#[test]
fn origin_modifier_and_side_panels() {
    // The left panel only, n = 0: 16 columns, 56 to 71 (decision E.49).
    let panels = PresentationFields {
        side_panels: Some(SidePanelFields { left: true, right: false, status_flag: true, left_columns: 0 }),
        ..PresentationFields::default()
    };
    let object = local_object(ObjectKind::Passive, 1, &[g0(0, b'P'), g0(11, b'Q'), g0(12, b'R')]);
    let local = [
        set_position(3, 4),
        origin(2, 56),
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(1, 0)),
        // The modifier is cancelled: the next object is placed at 3,4.
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(1, 0)),
        // A modifier not followed by an invocation is discarded.
        origin(5, 0),
        g0(30, b'S'),
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(1, 0)),
    ];
    let mut packets = page_with_objects(&[], &local, &[object]);
    packets.insert(1, x28(0, &panels));
    let (grid, report) = build(&packets, L25);
    assert_eq!(grid.side_panels, SidePanels { left: 16, right: 0 });
    let panel = |row: usize, column: usize| char_of(grid.panel_cell(row, column).unwrap());
    assert_eq!((panel(5, 60), panel(5, 71)), ('P', 'Q'), "origin row 3 + 2, column 4 + 56");
    assert_eq!((char_of(&grid.cells[3][4]), char_of(&grid.cells[3][15])), ('P', 'Q'));
    assert_eq!(char_of(&grid.cells[3][16]), 'R', "within the main page");
    assert_eq!(char_of(&grid.cells[3][30]), 'P', "placed at 3,30, over the local 'S'");
    // R of the first object at column 72 is discarded, and so are the Q and
    // R of the third object at 41 and 42, which would cross into the panel
    // area.
    assert_eq!(report.unwrap().discarded_writes, 3);
}

/// REQ-L25-040, REQ-L25-041, REQ-TST-016: the three side-panel layouts of
/// clause 12.5.3, the panel defaults, and writes to row 0 discarded.
#[test]
fn side_panel_layouts_of_clause_12_5_3() {
    let layouts = [
        // Full right panel 40 to 55; full left panel 56 to 71; 8 + 8.
        (SidePanelFields { left: false, right: true, status_flag: true, left_columns: 0 }, (0, 16), [40, 55]),
        (SidePanelFields { left: true, right: false, status_flag: true, left_columns: 0 }, (16, 0), [56, 71]),
        (SidePanelFields { left: true, right: true, status_flag: true, left_columns: 8 }, (8, 8), [64, 47]),
    ];
    let colours = tellytxt_core::types::DefaultColourFields {
        screen: 0,
        row: colour_index(3, 2),
        black_background_substitution: false,
        remapping: 0,
    };
    for (fields, widths, columns) in layouts {
        let presentation =
            PresentationFields { side_panels: Some(fields), colours: Some(colours), ..PresentationFields::default() };
        let mut local = vec![];
        for column in columns {
            local.extend([
                set_position(6, 0),
                origin(0, column),
                invoke(ObjectSource::Local, ObjectKind::Active, local_number(1, 0)),
            ]);
        }
        local.extend([
            row_0(0, 0b01),
            origin(0, columns[0]),
            invoke(ObjectSource::Local, ObjectKind::Active, local_number(1, 0)),
        ]);
        let object = local_object(ObjectKind::Active, 1, &[g0(0, b'K')]);
        let mut packets = page_with_objects(&[], &local, &[object]);
        packets.insert(1, x28(0, &presentation));
        let grid = build_25(&packets);
        assert_eq!((grid.side_panels.left, grid.side_panels.right), widths);
        for column in columns {
            let cell = grid.panel_cell(6, usize::from(column)).unwrap();
            assert_eq!(char_of(cell), 'K', "{fields:?} column {column}");
        }
        let blank = grid.panel_cell(7, usize::from(columns[0])).unwrap();
        assert_eq!((char_of(blank), blank.background, blank.foreground), (' ', colour_index(3, 2), WHITE));
        assert!(grid.panel_cell(0, usize::from(columns[0])).is_none(), "row 0 has no panel content");
    }
    // The status flag: at Level 2.5 a flag of 0 shows no panels.
    let fields = SidePanelFields { left: true, right: true, status_flag: false, left_columns: 8 };
    let packets = display_page(
        0,
        &[],
        vec![x28(0, &PresentationFields { side_panels: Some(fields), ..PresentationFields::default() })],
    );
    assert_eq!(build_25(&packets).side_panels, SidePanels::NONE);
    assert_eq!(build_35(&packets).side_panels, SidePanels { left: 8, right: 8 });
}

/// REQ-OBJ-008, REQ-TST-017: every pair of invoker and type, and local
/// objects invoked from public ones.
#[test]
fn invocation_nesting_rules() {
    // Objects 1 to 3 in X/26/1 to X/26/3: Active, Adaptive, Passive, each
    // invoking every type of the others' and writing a letter.
    let all_types = |own: u8| {
        let mut body = vec![g0(own, b'A' + own)];
        for (designation, kind) in [(1, ObjectKind::Active), (2, ObjectKind::Adaptive), (3, ObjectKind::Passive)] {
            body.push(set_position(own + 1, 0));
            body.push(invoke(ObjectSource::Local, kind, local_number(designation, 0)));
        }
        body
    };
    let objects = vec![
        local_object(ObjectKind::Active, 1, &all_types(1)),
        local_object(ObjectKind::Adaptive, 2, &all_types(2)),
        local_object(ObjectKind::Passive, 3, &all_types(3)),
    ];
    let local = [set_position(5, 0), invoke(ObjectSource::Local, ObjectKind::Active, local_number(1, 0))];
    let (_, report) = build(&page_with_objects(&[], &local, &objects), L25);
    let report = report.unwrap();
    // Run: the Active object; from it the Adaptive and a Passive object;
    // from the Adaptive object another Passive object. Forbidden: Active
    // invoking Active (1); Adaptive invoking Active and Adaptive (2); each
    // of the two Passive objects invoking all three types (6).
    assert_eq!(report.objects, 4);
    assert_eq!(report.forbidden_invocations, 1 + 2 + 6);
    // A public object may not invoke a local one.
    let public = ObjectDef::new(
        ObjectKind::Active,
        0,
        vec![invoke(ObjectSource::Local, ObjectKind::Passive, local_number(3, 0))],
    );
    let local = [invoke(ObjectSource::Public, ObjectKind::Active, public_number(0, 0))];
    let mut packets =
        page_with_objects(&[], &local, &[vec![], vec![], local_object(ObjectKind::Passive, 3, &[g0(0, b'x')])]);
    packets.extend(object_page(POP, 0, &[public]));
    let report = build(&packets, L25).1.unwrap();
    assert_eq!((report.objects, report.forbidden_invocations), (1, 1));
}

/// REQ-OBJ-012, REQ-TST-017: characters by the priority of clause 13.2:
/// Passive, Adaptive, local data, Active, Level 1.
#[test]
fn display_priority_of_overlapping_objects() {
    let writer = |letter: u8| vec![g0(0, letter), g0(1, letter), g0(2, letter), g0(3, letter)];
    let objects = vec![
        local_object(ObjectKind::Active, 1, &writer(b'a')),
        local_object(ObjectKind::Adaptive, 2, &writer(b'd')),
        local_object(ObjectKind::Passive, 3, &writer(b'p')[..1]),
    ];
    let local = [
        set_position(4, 0),
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(3, 0)),
        invoke(ObjectSource::Local, ObjectKind::Adaptive, local_number(2, 0)),
        g0(2, b'l'),
        g0(3, b'l'),
        set_position(4, 0),
        invoke(ObjectSource::Local, ObjectKind::Active, local_number(1, 0)),
        g0(5, b'l'),
    ];
    let grid = build_25(&page_with_objects(&[(4, b"123456")], &local, &objects));
    assert_eq!(&text(&grid, 4)[..6], "pddd5l", "Passive, then Adaptive over local data and Active");
}

/// REQ-OBJ-013: Full Screen Colour, Full Row Colour and Address Display
/// Row 0 are ignored in Adaptive and Passive objects.
#[test]
fn restricted_triplets_in_adaptive_and_passive_objects() {
    let colours = vec![
        TripletFunction::FullScreenColour(ColourData { index: 9, high: 0 }),
        TripletFunction::FullRowColour { address: 45, colour: ColourData { index: 10, high: 0 } },
        g0(1, b'r'),
    ];
    for (kind, screen, row_5) in
        [(ObjectKind::Active, 9, 10), (ObjectKind::Adaptive, 0, 0), (ObjectKind::Passive, 0, 0)]
    {
        let local = [set_position(2, 0), invoke(ObjectSource::Local, kind, local_number(1, 0))];
        let grid = build_25(&page_with_objects(&[], &local, &[local_object(kind, 1, &colours)]));
        assert_eq!((grid.screen_colour, grid.row_colours[7]), (screen, row_5), "{kind:?}");
        let written_row = if kind == ObjectKind::Active { 7 } else { 2 };
        assert_eq!(char_of(&grid.cells[written_row][1]), 'r', "{kind:?}: the row move is ignored too");
    }
}

/// REQ-OBJ-003, REQ-OBJ-005, REQ-TST-017: local definitions that cannot be
/// used.
#[test]
fn unusable_local_definitions_are_skipped() {
    let object = |usage| vec![define(ObjectKind::Passive, usage, local_number(1, 0)), g0(0, b'o'), end()];
    let cases = [
        (local_number(1, 0), object(0b11), true),
        (local_number(1, 0), object(0b01), true),
        (ObjectNumber(0x100 | 0x10), object(0b11), false),
        (local_number(1, 13), object(0b11), false),
        (local_number(4, 0), object(0b11), false),
        (local_number(1, 0), vec![define(ObjectKind::Active, 0b11, local_number(1, 0)), g0(0, b'o')], false),
    ];
    for (index, (number, object, shown)) in cases.into_iter().enumerate() {
        let local = [set_position(2, 0), invoke(ObjectSource::Local, ObjectKind::Passive, number)];
        let grid = build_25(&page_with_objects(&[], &local, &[object]));
        assert_eq!(char_of(&grid.cells[2][0]) == 'o', shown, "case {index}");
    }
    // Usage 01 is for Level 2.5 only.
    let local = [set_position(2, 0), invoke(ObjectSource::Local, ObjectKind::Passive, local_number(1, 0))];
    let grid = build_35(&page_with_objects(&[], &local, &[object(0b01)]));
    assert_eq!(char_of(&grid.cells[2][0]), ' ', "not at Level 3.5");
}

/// REQ-OBJ-004, REQ-OBJ-006, REQ-TST-017: public objects found through the
/// pointer tables, and each case that makes one absent.
#[test]
fn public_object_pointers() {
    let writer = |letter: u8| vec![g0(0, letter)];
    let objects = [
        ObjectDef::new(ObjectKind::Passive, 0, writer(b'a')),
        // The worked number of the research notes: S1 = 2 elsewhere, here
        // object 3 (packet 1, group 1, odd half).
        ObjectDef::new(ObjectKind::Passive, 3, writer(b'b')),
        // Object 9: pointer packet 2.
        ObjectDef::new(ObjectKind::Passive, 9, writer(b'c')),
        ObjectDef::new(ObjectKind::Active, 1, writer(b'x')),
    ];
    let run = |pointer_edit: &dyn Fn(&mut common::ObjectPageData), number: ObjectNumber, drop: Option<u8>| {
        let mut data = object_page_data(0, &objects);
        pointer_edit(&mut data);
        let mut pages = object_page_packets(POP, 0, &data);
        if let Some(number) = drop {
            pages.retain(|packet| tellytxt_core::packet::decode_address(packet).unwrap().number != number);
        }
        let local = [set_position(2, 0), invoke(ObjectSource::Public, ObjectKind::Passive, number)];
        let mut packets = page_with_objects(&[], &local, &[]);
        packets.extend(pages);
        let grid = build_25(&packets);
        char_of(&grid.cells[2][0])
    };
    let keep = |_: &mut common::ObjectPageData| {};
    assert_eq!(run(&keep, public_number(0, 0), None), 'a');
    assert_eq!(run(&keep, public_number(0, 3), None), 'b');
    assert_eq!(run(&keep, public_number(0, 9), None), 'c');
    assert_eq!(run(&keep, public_number(0, 5), None), ' ', "pointer not used: 1FF");
    assert_eq!(run(&keep, public_number(1, 0), None), ' ', "no such sub-page");
    assert_eq!(run(&keep, public_number(0, 9), Some(2)), ' ', "pointer packet missing");
    assert_eq!(run(&keep, public_number(0, 0), Some(3)), ' ', "the definition's packet missing");
    // Pointer 507 is not a triplet address; pointer 1 points at a body
    // triplet, not a definition; the Active definition's pointer used for a
    // Passive invocation is a definition of another type.
    let set = |value: u16| move |data: &mut common::ObjectPageData| data.pointers[0][3].0 = value;
    assert_eq!(run(&set(507), public_number(0, 0), None), ' ');
    assert_eq!(run(&set(1), public_number(0, 0), None), ' ');
    assert_eq!(run(&set(9), public_number(0, 0), None), ' ', "triplet 9 is the Active definition");
}

/// REQ-OBJ-006: a body that runs across packets 3 to 25 and into X/26.
#[test]
fn object_bodies_cross_packets_and_reach_x26() {
    let filler: Vec<TripletFunction> =
        (0..300).map(|_| TripletFunction::ReservedRow { mode: 0b00010, address: 41, data: 0 }).collect();
    let long = ObjectDef::new(ObjectKind::Passive, 0, filler.iter().copied().chain(text_at(0, b"END")).collect());
    let local = [set_position(2, 0), invoke(ObjectSource::Public, ObjectKind::Passive, public_number(0, 0))];
    let mut packets = page_with_objects(&[], &local, &[]);
    packets.extend(object_page(POP, 0, &[long]));
    assert!(packets.iter().any(|packet| tellytxt_core::packet::decode_address(packet).unwrap().number == 26));
    let grid = build_25(&packets);
    assert_eq!(&text(&grid, 2)[..3], "END");
}

/// REQ-OBJ-020, REQ-TST-017: the limits of Annex A.8, reached and exceeded.
#[test]
fn limits_of_annex_a8() {
    // 130 Passive objects, each at its own cell, invoked by one Active
    // object: 127 run.
    let mut body = vec![];
    for index in 0..130u8 {
        if index % 40 == 0 {
            body.push(set_position(1 + index / 40, 0));
        }
        body.push(g0(index % 40, 0x10));
        body.push(invoke(ObjectSource::Public, ObjectKind::Passive, public_number(0, 0)));
    }
    let objects =
        [ObjectDef::new(ObjectKind::Active, 0, body), ObjectDef::new(ObjectKind::Passive, 0, vec![g0(0, b'p')])];
    let local = [invoke(ObjectSource::Public, ObjectKind::Active, public_number(0, 0))];
    let mut packets = page_with_objects(&[], &local, &[]);
    packets.extend(object_page(POP, 0, &objects));
    let (grid, report) = build(&packets, L25);
    let shown = grid.cells.iter().flatten().filter(|cell| char_of(cell) == 'p').count();
    assert_eq!(shown, usize::from(OBJECT_INVOCATIONS_PER_TYPE));
    assert_eq!(report.unwrap().limited_invocations, 3);
    // Twenty objects invoked at one position, each open there: fifteen run.
    let invocation = invoke(ObjectSource::Local, ObjectKind::Passive, local_number(2, 0));
    let mut local = vec![set_position(2, 0)];
    local.extend(core::iter::repeat_n(invocation, 12));
    let more: Vec<TripletFunction> = core::iter::repeat_n(invocation, 8).collect();
    let object = local_object(ObjectKind::Passive, 2, &[g0(0, b'o')]);
    let packets = display_page(0, &[], vec![x26(0, &local), x26(1, &more), x26(2, &object)]);
    let report = build(&packets, L25).1.unwrap();
    assert_eq!((report.objects, report.limited_invocations), (OBJECTS_OPEN_AT_ONCE as u32, 5));
    // Objects that write nothing, opened one after another at their own
    // cells: the sixteenth in succession is ignored.
    let invocation = invoke(ObjectSource::Local, ObjectKind::Passive, local_number(3, 0));
    let silent = local_object(ObjectKind::Passive, 3, &[]);
    let mut local = vec![];
    for column in 0..18 {
        local.extend([g0(column, 0x10), invocation]);
    }
    let packets =
        display_page(0, &[], vec![x26(0, &local[..13]), x26(1, &local[13..26]), x26(2, &local[26..]), x26(3, &silent)]);
    let report = build(&packets, L25).1.unwrap();
    assert_eq!((report.objects, report.limited_invocations), (15, 3));
}

/// REQ-OBJ-030, REQ-TST-017: links of X/27/4 and X/27/5 and each case that
/// makes one unusable.
#[test]
fn compositional_links_by_role() {
    let decode = |packets: &[Packet], level| {
        let config = Config { level, ..Config::default() };
        Receiver::new().run(packets, config, |decoder| {
            let page = *decoder.store().get(PageAddress::from_mpp(DISPLAY).unwrap(), SubCode::ANY).unwrap();
            objects::links(&page, &decoder.enhancement_lookup(), level)
        })
    };
    let to = |mpp: u16| {
        let address = PageAddress::from_mpp(mpp).unwrap();
        Some(PageLink { magazine: address.magazine, page: address.page })
    };
    let mut wrong_marker = link(LINK_FUNCTION_DRCS, 0b11, 0x1B3);
    wrong_marker.marker_first = false;
    let x27_4 = x27(
        4,
        [
            link(LINK_FUNCTION_GPOP, 0b01, 0x1A1),
            // Link 1 must be a POP link: a GDRCS function there is ignored.
            link(LINK_FUNCTION_GDRCS, 0b11, 0x2A2),
            link(LINK_FUNCTION_GDRCS, 0b10, 0x1B1),
            wrong_marker,
            link(LINK_FUNCTION_POP, 0b10, 0x1A5),
            link(LINK_FUNCTION_GPOP, 0b10, 0x1A6),
        ],
    );
    let x27_5 = x27(
        5,
        [link(LINK_FUNCTION_DRCS, 0b11, 0x1B5), null_link(0), null_link(0), null_link(0), null_link(0), null_link(0)],
    );
    let packets = display_page(0, &[], vec![x27_4, x27_5]);
    let at_25 = decode(&packets, L25);
    assert_eq!((at_25.gpop, at_25.pop, at_25.gdrcs, at_25.drcs), (to(0x1A1), None, None, None));
    let at_35 = decode(&packets, L35);
    assert_eq!(at_35.gpop, to(0x1A6), "link 0 is for Level 2.5 only; link 5 gives the GPOP");
    assert_eq!((at_35.pop, at_35.gdrcs, at_35.drcs), (to(0x1A5), to(0x1B1), to(0x1B5)));
}

/// The table of magazine 1 linking page 100 to POP link 1 and DRCS link 2.
fn table(level_25: bool, level_35: bool, omit: &[u8]) -> Mot {
    let fallback = FallbackFields {
        side_panels: SidePanelFields { left: true, right: false, status_flag: true, left_columns: 0 },
        black_background_substitution: true,
    };
    let mut mot = Mot {
        entries: vec![(0x00, MotPageEntry { global_objects: false, pop_link: 1, global_drcs: false, drcs_link: 2 })],
        gpop: Some(object_link(0x3A1, FallbackFields::default(), [None; 2])),
        gdrcs: Some(drcs_link(0x1B1)),
        level_25,
        level_35,
        omit: omit.to_vec(),
        ..Mot::default()
    };
    mot.pops[0] = Some(object_link(POP, fallback, [None; 2]));
    mot.drcs[1] = Some(drcs_link(0x1B2));
    mot
}

/// REQ-OBJ-031, REQ-TST-017: the Magazine Organization Table at both
/// levels, with each Level 3.5 packet missing in turn, magazines read as
/// absolute, and the fallback of the POP link.
#[test]
fn magazine_organization_table_links() {
    let decode = |mot: &Mot, level| {
        let mut packets = mot_packets(mot);
        packets.extend(display_page(0, &[], vec![]));
        let config = Config { level, ..Config::default() };
        Receiver::new().run(&packets, config, |decoder| {
            let page = *decoder.store().get(PageAddress::from_mpp(DISPLAY).unwrap(), SubCode::ANY).unwrap();
            objects::links(&page, &decoder.enhancement_lookup(), level)
        })
    };
    let to = |mpp: u16| {
        let address = PageAddress::from_mpp(mpp).unwrap();
        Some(PageLink { magazine: address.magazine, page: address.page })
    };
    let links = decode(&table(true, false, &[]), L25);
    assert_eq!((links.gpop, links.pop, links.gdrcs, links.drcs), (to(0x3A1), to(POP), to(0x1B1), to(0x1B2)));
    assert_eq!(links.gpop.unwrap().magazine.number(), 3, "the table's magazine is absolute");
    assert!(links.fallback.unwrap().black_background_substitution);
    assert_eq!(links.level, L25);
    // Level 3.5 with X/22 to X/24: the Level 3.5 links.
    assert_eq!(decode(&table(false, true, &[]), L35).pop, to(POP));
    assert_eq!(decode(&table(false, true, &[]), L35).level, L35);
    // X/22 or X/24 missing: the Level 2.5 links, processed at Level 2.5.
    for missing in [22, 24] {
        let links = decode(&table(true, true, &[missing]), L35);
        assert_eq!((links.pop, links.level), (to(POP), L25), "X/{missing} missing");
    }
    // Only X/23 missing: POP links 4 to 7 come from X/20.
    let mut mot = table(true, true, &[23]);
    mot.entries[0].1.pop_link = 5;
    mot.pops[4] = Some(object_link(0x1A5, FallbackFields::default(), [None; 2]));
    let links = decode(&mot, L35);
    assert_eq!((links.pop, links.level), (to(0x1A5), L35));
    // A Level 2.5 decoder ignores X/22 to X/24.
    assert_eq!(decode(&table(false, true, &[]), L25).pop, None);
    // The page's own X/27/4 takes precedence.
    let mut packets = mot_packets(&table(true, false, &[]));
    packets.extend(display_page(0, &[], vec![object_links()]));
    let own = Receiver::new().run(&packets, Config::default(), |decoder| {
        let page = *decoder.store().get(PageAddress::from_mpp(DISPLAY).unwrap(), SubCode::ANY).unwrap();
        objects::links(&page, &decoder.enhancement_lookup(), L25)
    });
    assert_eq!((own.gpop, own.pop, own.gdrcs), (to(GPOP), to(POP), to(0x1B1)), "X/27/4 first, role by role");
}

/// REQ-OBJ-040, REQ-TST-017: default objects in the order GPOP 1, GPOP 2,
/// POP 1, POP 2, placed only on pages without local display data.
#[test]
fn default_objects() {
    let default = |kind, index| Some(DefaultObject { kind, number: public_number(0, index).0 as u8 });
    let mut mot = table(true, false, &[]);
    mot.gpop = Some(object_link(
        GPOP,
        FallbackFields::default(),
        [default(ObjectKind::Passive, 0), default(ObjectKind::Active, 1)],
    ));
    mot.pops[0] = Some(object_link(POP, FallbackFields::default(), [default(ObjectKind::Passive, 0), None]));
    let gpop = object_page(
        GPOP,
        0,
        &[
            ObjectDef::new(ObjectKind::Passive, 0, vec![g0(3, b'g'), g0(9, b'g'), g0(10, b'g')]),
            ObjectDef::new(ObjectKind::Active, 1, vec![set_position(3, 0), g0(5, b'h')]),
        ],
    );
    let pop = object_page(POP, 0, &[ObjectDef::new(ObjectKind::Passive, 0, vec![g0(10, b'p')])]);
    let run = |others: Vec<Packet>| {
        let mut packets = mot_packets(&mot);
        packets.extend(gpop.clone());
        packets.extend(pop.clone());
        packets.extend(display_page(0, &[(1, b"LEVEL1")], others));
        build_25(&packets)
    };
    let grid = run(vec![]);
    // Clause 13.8: origin row 0, column 0; the receiver's columns are kept
    // (REQ-L25-050); POP 1 is invoked after GPOP 1 and wins (Annex A.7.1).
    let plain = build_25(&display_page(0, &[], vec![]));
    assert_eq!(grid.cells[0][3], plain.cells[0][3], "column 3 belongs to the receiver");
    assert_eq!((char_of(&grid.cells[0][9]), char_of(&grid.cells[0][10])), ('g', 'p'));
    assert_eq!(char_of(&grid.cells[3][5]), 'h', "GPOP 2, an Active object");
    // Programme Delivery Control only: the default objects stay.
    let pdc = x26(0, &[TripletFunction::ProgrammeDeliveryRow { mode: 0b01000, address: 41, data: 0 }, end()]);
    assert_eq!(char_of(&run(vec![pdc]).cells[3][5]), 'h');
    // Local display data: no default objects.
    let local = local_data(&[set_position(2, 0), g0(0, b'l')]);
    let grid = run(local);
    assert_eq!((char_of(&grid.cells[3][5]), char_of(&grid.cells[2][0])), (' ', 'l'));
}

/// REQ-OBJ-032: an object sub-page beyond the table's count is used when
/// stored.
#[test]
fn sub_pages_beyond_the_count_are_used() {
    let mot = table(true, false, &[]);
    let mut packets = mot_packets(&mot);
    packets.extend(object_page(POP, 3, &[ObjectDef::new(ObjectKind::Passive, 0, vec![g0(0, b's')])]));
    packets.extend(display_page(
        0,
        &[],
        local_data(&[set_position(2, 0), invoke(ObjectSource::Public, ObjectKind::Passive, public_number(3, 0))]),
    ));
    assert_eq!(char_of(&build_25(&packets).cells[2][0]), 's');
}

/// REQ-OBJ-041: the Magazine Organization Table and object and DRCS pages
/// are never shown as text.
#[test]
fn enhancement_pages_are_not_shown_as_text() {
    let mut packets = mot_packets(&table(true, false, &[]));
    // A decimal page that a link names and whose X/28/3 says DRCS.
    packets.extend(display_page(
        0,
        &[],
        vec![x27(
            4,
            [
                null_link(0),
                null_link(1),
                null_link(2),
                link(LINK_FUNCTION_DRCS, 0b11, 0x150),
                null_link(0),
                null_link(0),
            ],
        )],
    ));
    packets.extend(common::drcs_page(0x150, 0, &[(0, common::ptu([0xFFF; 10]))], Some([0; 48])));
    let config = Config { store_hexadecimal_pages: true, ..Config::default() };
    Receiver::new().run(&packets, config, |decoder| {
        for mpp in [0x1FE, 0x150] {
            if decoder.store().get(PageAddress::from_mpp(mpp).unwrap(), SubCode::ANY).is_none() {
                continue;
            }
            let grid = page_grid(decoder, mpp, &config).0;
            assert!(grid.cells[1..].iter().flatten().all(|cell| char_of(cell) == ' '), "page {mpp:03X}");
        }
    });
}

/// REQ-OBJ-020, REQ-TST-019, REQ-SEC-012: a worst-case page, with objects
/// invoked to the limits, the deepest nesting, objects filling an object
/// page and all sixteen X/26 packets, finishes within the stated bound,
/// uses only the memory given, and never panics. Active objects reach 100:
/// the local data holds no more invocations at distinct positions, and
/// Annex A.8 allows only fifteen objects open at one position; Adaptive and
/// Passive objects reach their limit of 127.
#[test]
fn worst_case_page_is_bounded() {
    let packets = common::worst_case_page();
    let started = std::time::Instant::now();
    let (grid, report) = build(&packets, L35);
    let report = report.unwrap();
    eprintln!("worst case: {} triplets read in {:?}, {} objects", report.steps, started.elapsed(), report.objects);
    assert!(report.steps <= MAX_WALK_STEPS, "{} > {MAX_WALK_STEPS}", report.steps);
    assert_eq!(report.objects, 100 + 2 * u32::from(OBJECT_INVOCATIONS_PER_TYPE));
    assert_eq!(report.limited_invocations, (200 - 127) + (2 * 127 - 127));
    assert_eq!(char_of(&grid.cells[1][0]), 'w', "the Passive objects show");
}

/// REQ-OBJ-020, REQ-TST-016: the open area of Figure A.1 runs in scan
/// order from the invocation to the last position the object addresses,
/// across rows and through the side-panel columns: an object invoked at
/// row 2 and placed in the right panel down to row 6 is open at row 4, so
/// only fourteen more objects can be opened there.
#[test]
fn open_area_of_figure_a1() {
    let panels = PresentationFields {
        side_panels: Some(SidePanelFields { left: false, right: true, status_flag: true, left_columns: 0 }),
        ..PresentationFields::default()
    };
    // X/26/2: the wide object, writing at its origin and one row down, three
    // columns on. X/26/3: a small object writing at its origin.
    let wide =
        [define(ObjectKind::Passive, 0b11, local_number(2, 0)), g0(0, b'W'), set_position(1, 3), g0(3, b'W'), end()];
    let small = [define(ObjectKind::Passive, 0b11, local_number(3, 0)), g0(0, b's'), end()];
    let small_invocation = invoke(ObjectSource::Local, ObjectKind::Passive, local_number(3, 0));
    let mut local = vec![
        set_position(2, 30),
        origin(3, 12),
        invoke(ObjectSource::Local, ObjectKind::Passive, local_number(2, 0)),
        set_position(4, 5),
    ];
    local.extend(core::iter::repeat_n(small_invocation, 9));
    let more: Vec<TripletFunction> = core::iter::repeat_n(small_invocation, 6).collect();
    let others = vec![x28(0, &panels), x26(0, &local), x26(1, &more), x26(2, &wide), x26(3, &small)];
    let (grid, report) = build(&display_page(0, &[], others), L25);
    let report = report.unwrap();
    assert_eq!(char_of(grid.panel_cell(5, 42).unwrap()), 'W', "origin 2 + 3, 30 + 12");
    assert_eq!(char_of(grid.panel_cell(6, 45).unwrap()), 'W', "the last position addressed");
    assert_eq!(char_of(&grid.cells[4][5]), 's');
    assert_eq!((report.objects, report.limited_invocations), (1 + 14, 1));
}

/// REQ-L25-051: an object whose page is missing is skipped; the rest of
/// the page shows.
#[test]
fn missing_object_pages_leave_the_page() {
    let local =
        [set_position(2, 0), invoke(ObjectSource::Global, ObjectKind::Passive, public_number(0, 0)), g0(9, b'k')];
    let (grid, report) = build(&page_with_objects(&[(2, b"STILL HERE")], &local, &[]), L25);
    assert_eq!(text(&grid, 2).trim_end(), "STILL HERk");
    assert_eq!(report.unwrap().absent_objects, 1);
}
