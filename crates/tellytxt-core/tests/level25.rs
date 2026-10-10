//! The enhancement processor of Levels 2.5 and 3.5 (area L25, REQ-FSM-011,
//! REQ-FSM-012; REQ-TST-015 and REQ-TST-016): pages built with the encoder,
//! fed through the decoder, and checked cell by cell.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

mod common;

use common::{
    Receiver, background, build, build_25, build_35, char_of, damage, display_page, end, foreground, g0, grid_of,
    local_data, row_0, set_of, set_position, text, x26, x28,
};
use tellytxt_core::charset::{CharSet, G0, G0Set, G2Set};
use tellytxt_core::colour::{ColourMap, colour_index};
use tellytxt_core::config::{Config, PresentationLevel};
use tellytxt_core::consts::{
    ALPHA_BLACK, ALPHA_RED, ALPHA_WHITE, ALPHA_YELLOW, BLACK_BACKGROUND, C5_NEWSFLASH, CONCEAL, CONTIGUOUS_MOSAICS,
    DOUBLE_HEIGHT, DOUBLE_WIDTH, FLASH, LATIN_AT_SIGN, MOSAIC_RED, NEW_BACKGROUND, NORMAL_SIZE, SEPARATED_MOSAICS,
    START_BOX, STEADY,
};
use tellytxt_core::level1::{Cell, CharSource, FlashMode, FlashRate, FontStyle, Glyph, Grid, Size, Width};
use tellytxt_core::packet::{ColourData, DisplayAttributes, FontStyleData, Packet, TripletFunction};
use tellytxt_core::types::{DefaultColourFields, Designations, PresentationFields, Rgb12};

const ALPHA_GREEN: u8 = 0x02;
const ALPHA_BLUE: u8 = 0x04;
const L25: PresentationLevel = PresentationLevel::Level25;
const L35: PresentationLevel = PresentationLevel::Level35;

/// Page 100 with Level 1 rows and local enhancement data.
fn page(rows: &[(u8, &[u8])], local: &[TripletFunction]) -> Vec<Packet> {
    display_page(0, rows, local_data(local))
}

fn display(column: u8, attributes: DisplayAttributes) -> TripletFunction {
    TripletFunction::DisplayAttributes { column, attributes }
}

fn flash(column: u8, mode: u8, rate: u8) -> TripletFunction {
    TripletFunction::AdditionalFlash { column, mode, rate, reserved: 0 }
}

fn latin_g0() -> CharSet {
    CharSet::G0(G0 { set: G0Set::Latin, national: None })
}

/// REQ-L25-010, REQ-L25-011, REQ-TST-015: every character mode, the "@"
/// of mode 10000, data below 2/0, and undefined positions.
#[test]
fn every_character_mode_writes_its_set() {
    let local = [
        set_position(2, 0),
        TripletFunction::G1Character { column: 1, code: 0x7F },
        TripletFunction::G1Character { column: 2, code: b'A' },
        TripletFunction::G3Character { column: 3, code: 0x51, level_15_form: true },
        TripletFunction::G3Character { column: 4, code: 0x52, level_15_form: false },
        g0(5, b'B'),
        TripletFunction::G2Character { column: 6, code: 0x24 },
        TripletFunction::G0Diacritic { column: 7, code: b'e', diacritic: 2 },
        TripletFunction::G0Diacritic { column: 8, code: 0x2A, diacritic: 0 },
        TripletFunction::G0Character { column: 9, code: 0x2A },
        g0(10, 0x1F),
        TripletFunction::G3Character { column: 11, code: 0x6E, level_15_form: false },
        TripletFunction::G2Character { column: 12, code: 0x59 },
    ];
    let (grid, report) = build(&page(&[(2, b"0123456789ABCDEF")], &local), L25);
    let cells = &grid.cells[2];
    assert_eq!(cells[1].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false }, "G1 column 7");
    assert_eq!(set_of(&cells[2]), Some(latin_g0()), "G1 columns 4 and 5 are the G0 set (Table 47 note 3)");
    assert_eq!(char_of(&cells[2]), 'A');
    assert_eq!((set_of(&cells[3]), set_of(&cells[4])), (Some(CharSet::G3), Some(CharSet::G3)), "both G3 forms");
    assert_eq!(char_of(&cells[5]), 'B');
    assert_eq!(set_of(&cells[6]), Some(CharSet::G2(G2Set::Latin)));
    assert!(matches!(cells[7].glyph, Glyph::Char(character) if character.diacritic == 2 && character.code == b'e'));
    assert!(matches!(cells[8].glyph, Glyph::Char(character) if character.code == LATIN_AT_SIGN), "the @ of 10000");
    assert_eq!(char_of(&cells[9]), '*', "mode 01001 with 2/A is the asterisk");
    assert_eq!(char_of(&cells[10]), 'A', "data below 2/0 writes nothing: the Level 1 'A' stays");
    assert_eq!((cells[11].glyph, cells[12].glyph), (Glyph::Space, Glyph::Space), "undefined G3 and G2 positions");
    assert_eq!(report.unwrap().undefined_characters, 2);
    assert!(cells[1..10].iter().all(|cell| cell.attributes.source() == CharSource::Local));
    assert_eq!(cells[13].attributes.source(), CharSource::Level1);
}

/// REQ-L25-010, REQ-L25-012: national options never apply to triplet
/// characters, while the Level 1 page keeps its own.
#[test]
fn national_options_apply_to_level_1_only() {
    // C14 set: national option index 1, German in group 0000.
    let packets = display_page(1 << 14, &[(1, b"[[")], local_data(&[set_position(1, 0), g0(1, b'[')]));
    let grid = build_25(&packets);
    assert_eq!((char_of(&grid.cells[1][0]), char_of(&grid.cells[1][1])), ('Ä', '['));
}

/// REQ-L25-011: a G1 character written by a triplet is separated by the
/// non-spacing attribute only; that attribute also separates Level 1
/// mosaics.
#[test]
fn g1_mosaics_follow_the_non_spacing_separated_attribute() {
    let underline = DisplayAttributes { underline: true, ..DisplayAttributes::default() };
    let local = [
        set_position(3, 0),
        TripletFunction::G1Character { column: 3, code: 0x7F },
        display(5, underline),
        TripletFunction::G1Character { column: 6, code: 0x7F },
    ];
    let row = [MOSAIC_RED, SEPARATED_MOSAICS, 0x7F, b' ', b' ', b' ', b' ', 0x7F];
    let grid = build_25(&page(&[(3, &row)], &local));
    let cells = &grid.cells[3];
    assert_eq!(cells[2].glyph, Glyph::Mosaic { pattern: 0x3F, separated: true }, "Level 1 1/A");
    assert_eq!(cells[3].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false }, "1/A ignored (note 12)");
    assert_eq!(cells[6].glyph, Glyph::Mosaic { pattern: 0x3F, separated: true });
    assert_eq!(cells[7].glyph, Glyph::Mosaic { pattern: 0x3F, separated: true }, "Level 1 mosaic separated too");
}

/// REQ-L25-020, REQ-TST-016: the three contention examples of Annex C.2,
/// and the same-cell case of decision E.47.
#[test]
fn contention_examples_of_annex_c2() {
    let row: &[u8] = &[b' ', b' ', b' ', b' ', ALPHA_RED, b'A'];
    let blue = colour_index(0, 4);
    let yellow = colour_index(0, 3);
    let red = colour_index(0, 1);
    // Example 1: Foreground Blue at n − 1 takes effect before alpha red.
    let grid = build_25(&page(&[(1, row)], &[set_position(1, 0), foreground(4, blue)]));
    assert_eq!(grid.cells[1][5].foreground, red, "rule 1: red");
    // Example 2: both take effect at n; Foreground Blue is placed later.
    let grid = build_25(&page(&[(1, row)], &[set_position(1, 0), foreground(5, blue)]));
    assert_eq!(grid.cells[1][5].foreground, blue, "rule 2: blue");
    // Example 3: X/26/0 blue and X/26/1 yellow at n: the later wins.
    let packets = display_page(
        0,
        &[(1, row)],
        vec![
            x26(0, &[set_position(1, 0), foreground(5, blue)]),
            x26(1, &[set_position(1, 0), foreground(5, yellow), end()]),
        ],
    );
    assert_eq!(build_25(&packets).cells[1][5].foreground, yellow, "rule 3: yellow");
    // Decision E.47: a set-at Level 1 attribute and an X/26 attribute at the
    // same cell: the X/26 attribute wins.
    let row: &[u8] = &[ALPHA_RED, NEW_BACKGROUND, b'A'];
    let grid = build_25(&page(&[(1, row)], &[set_position(1, 0), background(1, blue)]));
    assert_eq!((grid.cells[1][1].background, grid.cells[1][2].background), (blue, blue));
}

/// REQ-L25-003, REQ-L25-052, decision E.46: only Full Row Colour, Set
/// Active Position and Address Display Row 0 move the row.
#[test]
fn only_three_row_triplets_set_the_active_row() {
    let colour = ColourData { index: 0, high: 0b01 };
    let local = [
        TripletFunction::FullRowColour { address: 45, colour },
        g0(3, b'a'),
        set_position(6, 12),
        g0(13, b'b'),
        // A column of 40 or more keeps the column.
        TripletFunction::SetActivePosition { address: 47, column: 45 },
        TripletFunction::G1Character { column: 14, code: 0x1F },
        g0(15, b'c'),
        // Full Screen Colour, Programme Delivery Control and reserved row
        // modes leave the row, whatever their address (Annex A.3 would not).
        TripletFunction::FullScreenColour(ColourData { index: 0, high: 0b11 }),
        TripletFunction::ProgrammeDeliveryRow { mode: 0b01000, address: 50, data: 0 },
        TripletFunction::ReservedRow { mode: 0b00011, address: 52, data: 0 },
        g0(16, b'd'),
        TripletFunction::AddressDisplayRow0 { address: 50, colour },
        g0(17, b'e'),
    ];
    let more = [row_0(0, 0b01), g0(18, b'f'), set_position(24, 2), g0(3, b'g'), end()];
    let grid = build_25(&display_page(0, &[], vec![x26(0, &local), x26(1, &more)]));
    assert_eq!(char_of(&grid.cells[5][3]), 'a', "Full Row Colour: row 5");
    assert_eq!(char_of(&grid.cells[6][13]), 'b', "Set Active Position");
    assert_eq!(char_of(&grid.cells[7][15]), 'c', "column kept, row 7");
    assert_eq!(char_of(&grid.cells[7][16]), 'd', "row unchanged by other row triplets");
    assert_eq!(char_of(&grid.cells[7][17]), 'e', "Address Display Row 0 with address 50 is reserved");
    assert_eq!(char_of(&grid.cells[0][18]), 'f', "address 63: row 0");
    assert_eq!(text(&grid, 24), " ".repeat(40), "row 24 is not displayed under Annex H");
}

/// REQ-L25-004: every column mode except the reserved ones and Programme
/// Delivery Control sets the column; a character below 2/0 writes nothing.
#[test]
fn column_triplets_set_the_column() {
    use tellytxt_core::packet::{ObjectKind, ObjectSource};
    // A local Passive object writes "X" at its origin; it is invoked after
    // each column triplet, so its "X" shows where the column went.
    let object = [common::define(ObjectKind::Passive, 0b11, common::local_number(1, 0)), g0(0, b'X'), end()];
    let invoke = common::invoke(ObjectSource::Local, ObjectKind::Passive, common::local_number(1, 0));
    let cases: [(TripletFunction, u8); 5] = [
        (TripletFunction::ReservedColumn { column: 30, mode: 0b00100, data: 0 }, 2),
        (TripletFunction::ReservedColumn { column: 30, mode: 0b01010, data: 0 }, 2),
        (TripletFunction::ProgrammeDeliveryColumn { column: 30, data: 0 }, 2),
        (g0(30, 0x10), 30),
        (TripletFunction::FontStyle { column: 30, style: FontStyleData::default() }, 30),
    ];
    for (function, column) in cases {
        let local = [set_position(4, 2), function, invoke, end()];
        let grid = build_25(&display_page(0, &[], vec![x26(0, &local), x26(1, &object)]));
        assert_eq!(char_of(&grid.cells[4][usize::from(column)]), 'X', "{function:?}");
    }
}

/// REQ-L25-005, decision E.46: a damaged triplet is skipped without moving
/// the position; an out-of-order triplet is applied where it points.
#[test]
fn damaged_and_out_of_order_triplets() {
    let local = [set_position(2, 0), g0(5, b'a'), set_position(9, 0), g0(6, b'b'), g0(3, b'c'), end()];
    let packets = display_page(0, &[], vec![damage(x26(0, &local), 2)]);
    let (grid, report) = build(&packets, L25);
    assert_eq!(char_of(&grid.cells[2][6]), 'b', "the skipped Set Active Position left row 2");
    assert_eq!(char_of(&grid.cells[2][3]), 'c', "out of order: applied");
    assert_eq!(report.unwrap().damaged_triplets, 1);
}

/// REQ-L25-012, REQ-TST-015: a Modified G0 and G2 designation lasts to the
/// end of its row; reserved values are ignored; Level 2.5 allows one pair
/// besides the default, Level 3.5 any number.
#[test]
fn modified_designation_lasts_to_the_end_of_the_row() {
    // 0100000 Cyrillic 1, 0110111 Greek, 1010101 Hebrew; 0101000 reserved.
    let local = [
        set_position(1, 0),
        TripletFunction::ModifiedDesignation { column: 2, designation: 0b010_0000 },
        g0(3, b'B'),
        TripletFunction::G2Character { column: 4, code: 0x41 },
        set_position(2, 0),
        g0(3, b'B'),
        TripletFunction::ModifiedDesignation { column: 4, designation: 0b011_0111 },
        g0(5, b'B'),
        TripletFunction::ModifiedDesignation { column: 6, designation: 0b010_1000 },
        g0(7, b'B'),
    ];
    let set = |grid: &Grid, row: usize, column: usize| set_of(&grid.cells[row][column]);
    let cyrillic = Some(CharSet::G0(G0 { set: G0Set::Cyrillic1, national: None }));
    let greek = Some(CharSet::G0(G0 { set: G0Set::Greek, national: None }));
    for level in [L25, L35] {
        let grid = build(&page(&[], &local), level).0;
        assert_eq!(set(&grid, 1, 3), cyrillic, "{level:?}");
        assert_eq!(set(&grid, 1, 4), Some(CharSet::G2(G2Set::Cyrillic)));
        assert_eq!(set(&grid, 2, 3), Some(latin_g0()), "a new row returns to the default");
        let third = if level == L35 { greek } else { Some(latin_g0()) };
        assert_eq!(set(&grid, 2, 5), third, "a third pair at {level:?}");
        assert_eq!(set(&grid, 2, 7), third, "a reserved value changes nothing");
    }
}

/// REQ-L25-021: Foreground and Background Colour last to the end of the
/// row, ignore reserved data and do not change the mosaic mode.
#[test]
fn colour_triplets_set_colour_map_indices() {
    let local = [
        set_position(4, 0),
        foreground(2, 20),
        background(3, 27),
        TripletFunction::ForegroundColour { column: 8, colour: ColourData { index: 5, high: 0b01 } },
    ];
    let grid = build_25(&page(&[(4, &[MOSAIC_RED, b' ', b' ', 0x7F, b'A'])], &local));
    let cells = &grid.cells[4];
    assert_eq!((cells[1].foreground, cells[2].foreground, cells[39].foreground), (1, 20, 20));
    assert_eq!((cells[2].background, cells[3].background, cells[39].background), (0, 27, 27));
    assert_eq!(cells[3].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false }, "still mosaic mode");
    assert!(matches!(cells[4].glyph, Glyph::Char(_)), "blast-through capital");
    assert_eq!(cells[9].foreground, 20, "reserved D6 D5: the column only");
}

/// The fields of an X/28/0 with only the default colours group.
fn colours(screen: u8, row: u8, substitution: bool, remapping: u8) -> PresentationFields {
    PresentationFields {
        designations: Some(Designations { default: 0, second: 0x7F }),
        colours: Some(DefaultColourFields { screen, row, black_background_substitution: substitution, remapping }),
        ..PresentationFields::default()
    }
}

/// REQ-L25-022, REQ-TST-015: each of the eight re-mapping values, with
/// substitution on and off.
#[test]
fn remapping_and_black_background_substitution() {
    const CLUTS: [(u8, u8); 8] = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 1), (2, 2), (2, 3)];
    let row: &[u8] =
        &[b'A', ALPHA_RED, b'B', BLACK_BACKGROUND, b'C', ALPHA_BLACK, NEW_BACKGROUND, b'D', b' ', b' ', b' ', b'E'];
    for (value, (foreground_clut, background_clut)) in CLUTS.into_iter().enumerate() {
        for substitution in [false, true] {
            let others = vec![
                x28(0, &colours(0, 20, substitution, value as u8)),
                x26(0, &[set_position(5, 0), background(10, 0), end()]),
            ];
            let grid = build_25(&display_page(0, &[(5, row)], others));
            let cells = &grid.cells[5];
            let black = if substitution { 20 } else { colour_index(background_clut, 0) };
            let label = format!("re-mapping {value}, substitution {substitution}");
            assert_eq!(cells[0].foreground, colour_index(foreground_clut, 7), "{label}: default white");
            assert_eq!(cells[0].background, black, "{label}: default black");
            assert_eq!(cells[2].foreground, colour_index(foreground_clut, 1), "{label}: alpha red");
            assert_eq!(cells[4].background, black, "{label}: 1/C");
            assert_eq!(cells[7].background, colour_index(background_clut, 0), "{label}: 0/0 then 1/D");
            assert_eq!(cells[7].foreground, colour_index(foreground_clut, 0), "{label}: alpha black");
            assert_eq!(cells[11].background, 0, "{label}: a triplet's black is never substituted");
        }
    }
}

/// REQ-L25-023, decision E.47: each display attribute bit, set and
/// cleared, against the Level 1 attribute of its type.
#[test]
fn display_attributes_against_level_1() {
    let on = |f: fn(&mut DisplayAttributes)| {
        let mut attributes = DisplayAttributes::default();
        f(&mut attributes);
        attributes
    };
    let off = DisplayAttributes::default();
    // Conceal: a colour code ends only Level 1 conceal; a 0 bit ends both.
    let local = [set_position(1, 0), display(2, on(|a| a.conceal = true)), display(12, off)];
    let row: &[u8] = &[b' ', b' ', b'A', ALPHA_RED, b'B', b' ', b' ', b' ', b' ', b' ', CONCEAL, b'C', b'D'];
    let grid = build_25(&page(&[(1, row)], &local));
    let concealed: Vec<bool> = grid.cells[1][..13].iter().map(|cell| cell.conceal).collect();
    let mut expected = vec![false, false, true, true, true, true, true, true, true, true, true, true, false];
    assert_eq!(concealed, expected, "non-spacing conceal survives 0/1");
    let row: &[u8] = &[CONCEAL, b'A', ALPHA_RED, b'B'];
    let grid = build_25(&page(&[(1, row)], &[set_position(1, 0), display(1, off)]));
    expected = vec![true, false, false, false];
    assert_eq!(grid.cells[1][..4].iter().map(|cell| cell.conceal).collect::<Vec<_>>(), expected, "a 0 bit ends 1/8");
    // Underline: on G0 text, not cancelled by 1/9; a 0 bit clears 1/A.
    let local = [set_position(1, 0), display(1, on(|a| a.underline = true)), display(6, off)];
    let row: &[u8] = &[b'A', b'B', CONTIGUOUS_MOSAICS, b'C', MOSAIC_RED, SEPARATED_MOSAICS, 0x7F];
    let grid = build_25(&page(&[(1, row)], &local));
    let cells = &grid.cells[1];
    assert!(!cells[0].attributes.underline() && cells[1].attributes.underline() && cells[3].attributes.underline());
    assert_eq!(cells[6].glyph, Glyph::Mosaic { pattern: 0x3F, separated: false }, "the 0 bit cleared 1/A");
    // Invert.
    let grid = build_25(&page(&[(1, b"AB")], &[set_position(1, 0), display(1, on(|a| a.invert = true))]));
    assert!(!grid.cells[1][0].attributes.invert() && grid.cells[1][1].attributes.invert());
}

/// REQ-L25-029, decision E.47: a window on an ordinary page comes only
/// from the boxing bit; Level 1 Start Box opens none there; on a newsflash
/// page both box.
#[test]
fn boxing_and_windows() {
    let boxing = DisplayAttributes { boxing: true, ..DisplayAttributes::default() };
    let row: &[u8] = &[START_BOX, START_BOX, b'A', b'B'];
    let local = [set_position(2, 0), display(3, boxing)];
    let grid = build_25(&page(&[(2, row)], &local));
    let boxed: Vec<bool> = grid.cells[2][..5].iter().map(|cell| cell.boxed).collect();
    assert_eq!(boxed, [false, false, false, true, true], "window from the triplet only");
    let packets = display_page(C5_NEWSFLASH, &[(2, row)], local_data(&[]));
    let grid = build_25(&packets);
    let boxed: Vec<bool> = grid.cells[2][..4].iter().map(|cell| cell.boxed).collect();
    assert_eq!(boxed, [false, true, true, true], "Level 1 boxes on a newsflash page");
}

/// REQ-L25-025, REQ-TST-015: double width, double size and the rules of
/// Annex C.3.
#[test]
fn sizes_follow_annex_c3() {
    let wide = DisplayAttributes { double_width: true, ..DisplayAttributes::default() };
    let size = DisplayAttributes { double_width: true, double_height: true, ..DisplayAttributes::default() };
    let tall = DisplayAttributes { double_height: true, ..DisplayAttributes::default() };
    let local = [
        set_position(3, 0),
        display(5, wide),
        display(9, DisplayAttributes::default()),
        display(39, wide),
        set_position(23, 0),
        display(1, size),
        set_position(5, 0),
        display(11, tall),
        display(12, DisplayAttributes::default()),
        set_position(6, 0),
        display(10, wide),
    ];
    let rows: &[(u8, &[u8])] = &[
        (3, b"     ABCDEFGH                          Z"),
        (5, b"           X"),
        (6, b"          PQRS"),
        (23, b" T"),
        (10, &[DOUBLE_WIDTH, b'A', b'B', b'C']),
    ];
    let grid = build_25(&page(rows, &local));
    let width = |row: usize, column: usize| grid.cells[row][column].attributes.width();
    assert_eq!((width(3, 5), width(3, 6), width(3, 7)), (Width::LeftHalf, Width::RightHalf, Width::LeftHalf));
    assert_eq!((char_of(&grid.cells[3][6]), char_of(&grid.cells[3][7])), ('A', 'C'), "B is hidden");
    assert_eq!(width(3, 39), Width::Normal, "rule 1: not in column 39");
    assert_eq!((grid.cells[23][1].size, width(23, 1)), (Size::Normal, Width::LeftHalf), "rule 3: width only");
    assert_eq!(grid.cells[5][11].size, Size::DoubleTop);
    assert_eq!((grid.cells[6][11].size, char_of(&grid.cells[6][11])), (Size::DoubleBottom, 'X'));
    assert_eq!(char_of(&grid.cells[6][12]), 'R', "non-spacing double height does not suppress the row below");
    assert_eq!(width(6, 10), Width::Normal, "rule 3: the lower half above wins over double width");
    assert_eq!((width(10, 1), width(10, 2), char_of(&grid.cells[10][3])), (Width::LeftHalf, Width::RightHalf, 'C'));
}

/// REQ-L25-026: the row below spacing double height shows lower halves,
/// the local background elsewhere, and enhancement characters, with the
/// start-of-row defaults.
#[test]
fn row_below_spacing_double_height() {
    let local = [set_position(6, 0), g0(20, b'E'), foreground(21, 3), g0(22, b'F')];
    let rows: &[(u8, &[u8])] =
        &[(5, &[ALPHA_RED, DOUBLE_HEIGHT, b'B', ALPHA_YELLOW, b'G', NORMAL_SIZE]), (6, b"XYZXYZXYZ")];
    let grid = build_25(&page(rows, &local));
    assert_eq!((grid.cells[6][2].size, char_of(&grid.cells[6][2])), (Size::DoubleBottom, 'B'));
    assert_eq!(char_of(&grid.cells[6][7]), ' ', "Level 1 content of the lower row is ignored");
    assert_eq!((char_of(&grid.cells[6][20]), grid.cells[6][20].foreground), ('E', 7), "start-of-row white");
    assert_eq!(grid.cells[6][22].foreground, 3);
    assert!(grid.row_has_double_height(5) && !grid.row_has_double_height(6));
}

/// REQ-L25-027, decision E.48: every flash mode and rate, the phases of
/// incremental and decremental flash, reserved values, and Level 1 Flash.
#[test]
fn additional_flash_functions() {
    let local = [
        set_position(1, 0),
        flash(1, 0b01, 0b000),
        flash(3, 0b10, 0b010),
        flash(5, 0b11, 0b001),
        flash(7, 0b00, 0b000),
        flash(10, 0b01, 0b100),
        flash(20, 0b01, 0b101),
        flash(30, 0b01, 0b110),
        TripletFunction::AdditionalFlash { column: 32, mode: 0b01, rate: 0, reserved: 0b10 },
    ];
    let grid = build_25(&page(&[(1, &[b'A'; 40]), (2, &[FLASH, b'A', b'B', STEADY, b'C'])], &local));
    let cell = |column: usize| grid.cells[1][column];
    let state = |cell: Cell| (cell.flash, cell.attributes.flash_mode(), cell.attributes.flash_rate());
    assert!(!state(cell(0)).0);
    assert_eq!(state(cell(1)), (true, FlashMode::Normal, FlashRate::Slow));
    assert_eq!(state(cell(3)), (true, FlashMode::Inverted, FlashRate::FastPhase2));
    assert_eq!(state(cell(5)), (true, FlashMode::AdjacentClut, FlashRate::FastPhase1));
    assert!(!cell(7).flash, "steady");
    let phases =
        |first: usize| (first..first + 4).map(|column| cell(column).attributes.flash_rate()).collect::<Vec<_>>();
    let (p1, p2, p3) = (FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3);
    assert_eq!(phases(10), [p1, p2, p3, p1], "incremental");
    assert_eq!(phases(20), [p1, p3, p2, p1], "decremental");
    assert_eq!(phases(30), [p3, p2, p1, p3], "reserved values change nothing: decremental continues");
    // Level 1 Flash: slow, normal, until Steady.
    let level_1: Vec<bool> = grid.cells[2][..5].iter().map(|cell| cell.flash).collect();
    assert_eq!(level_1, [false, true, true, false, false]);
    assert_eq!(grid.cells[2][1].attributes.flash_rate(), FlashRate::Slow);
}

/// REQ-L25-028, REQ-TST-015: Font Style at Level 3.5 only, from its column
/// over R further rows (the zvbi observation of a row count of 2), replaced
/// by a later triplet, and for G0 and G2 characters only.
#[test]
fn font_style_extends_over_rows_at_level_35() {
    let styled = FontStyleData { bold: true, italic: true, proportional: true, rows: 2 };
    let local = [
        set_position(3, 0),
        TripletFunction::FontStyle { column: 4, style: styled },
        set_position(4, 0),
        TripletFunction::FontStyle { column: 10, style: FontStyleData::default() },
    ];
    let rows: &[(u8, &[u8])] =
        &[(3, &[b'A'; 40]), (4, &[b'A'; 40]), (5, &[b'A'; 40]), (6, &[b'A'; 40]), (7, &[MOSAIC_RED, 0x7F])];
    let all = FontStyle { bold: true, italic: true, proportional: true };
    let grid = build_35(&page(rows, &local));
    let font = |row: usize, column: usize| grid.cells[row][column].attributes.font();
    assert_eq!((font(3, 3), font(3, 4), font(3, 39)), (FontStyle::default(), all, all));
    assert_eq!((font(4, 4), font(4, 10)), (all, FontStyle::default()), "replaced from column 10");
    assert_eq!((font(5, 4), font(6, 4)), (all, FontStyle::default()), "two rows only");
    let grid = build_25(&page(rows, &local));
    assert_eq!(grid.cells[3][4].attributes.font(), FontStyle::default(), "Level 2.5 ignores it");
    // Only G0 and G2 characters take a style.
    let local = [set_position(7, 0), TripletFunction::FontStyle { column: 0, style: styled }];
    let grid = build_35(&page(rows, &local));
    assert_eq!(grid.cells[7][1].attributes.font(), FontStyle::default(), "not a mosaic");
}

/// REQ-L25-030, REQ-L25-031, REQ-TST-016: the page's Colour Map from X/28/0
/// and, at Level 3.5 only, X/28/4; the Table 30 defaults otherwise.
#[test]
fn colour_map_from_page_packets() {
    let words = |red: u8| core::array::from_fn(|entry| Rgb12::new(red, entry as u8, 0));
    let page_data = PresentationFields { colour_words: Some(words(9)), ..colours(0, 0, false, 0) };
    let page_35 = PresentationFields { colour_words: Some(words(4)), ..colours(0, 0, false, 0) };
    let packets = display_page(0, &[(1, b"A")], vec![x28(0, &page_data), x28(4, &page_35)]);
    for level in [L25, L35] {
        let grid = build(&packets, level).0;
        assert_eq!(grid.colour_map.entry(colour_index(2, 3)), Rgb12::new(9, 3, 0), "{level:?}");
        let clut_0 = if level == L35 { Rgb12::new(4, 1, 0) } else { ColourMap::DEFAULT.entry(1) };
        assert_eq!(grid.colour_map.entry(1), clut_0, "{level:?}");
        assert!(ColourMap::is_transparent(colour_index(1, 0)));
    }
    let grid = build_25(&display_page(0, &[(1, b"A")], vec![]));
    assert_eq!(grid.colour_map, ColourMap::DEFAULT, "Table 30");
}

/// REQ-L25-032 to REQ-L25-034, REQ-TST-016, decision E.45: the precedence
/// lists of Annex A.5 and the order of row colour commands.
#[test]
fn screen_and_row_colours() {
    let full_row = |row: u8, index: u8, high: u8| TripletFunction::FullRowColour {
        address: if row == 24 { 40 } else { 40 + row },
        colour: ColourData { index, high },
    };
    let local = [
        TripletFunction::FullScreenColour(ColourData { index: 17, high: 0 }),
        row_0(21, 0b00),
        full_row(5, 18, 0b00),
        full_row(10, 19, 0b11),
        full_row(12, 20, 0b00),
        full_row(14, 22, 0b01),
        g0(1, b'x'),
    ];
    let grid = build_25(&page(&[], &local));
    assert_eq!(grid.screen_colour, 17);
    let expected: Vec<u8> =
        [21, 17, 17, 17, 17, 18, 17, 17, 17, 17, 19, 19, 20, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 17].to_vec();
    assert_eq!(grid.row_colours.to_vec(), expected);
    assert_eq!(char_of(&grid.cells[14][1]), 'x', "a reserved selection still sets the position");
    // Without a Full Screen Colour triplet, the default row colour applies;
    // a reserved Full Screen Colour is ignored.
    let local = [TripletFunction::FullScreenColour(ColourData { index: 9, high: 0b10 })];
    let packets = display_page(0, &[], vec![x28(0, &colours(11, 12, false, 0)), x26(0, &[local[0], end()])]);
    let grid = build_25(&packets);
    assert_eq!((grid.screen_colour, grid.row_colours[3]), (11, 12));
}

/// REQ-L25-050, REQ-SEC-005: nothing written to row 0 columns 0 to 7 by
/// local data reaches the grid.
#[test]
fn receiver_area_is_never_written() {
    let red_background = background(0, 1);
    let local = [row_0(4, 0b11), red_background, g0(0, b'Q'), g0(7, b'Q'), g0(8, b'Q')];
    let (grid, report) = build(&page(&[], &local), L25);
    let level_1 = build(&page(&[], &[]), PresentationLevel::Level1).0;
    assert_eq!(grid.cells[0][..8], level_1.cells[0][..8]);
    assert_eq!(char_of(&grid.cells[0][8]), 'Q');
    assert_eq!(report.unwrap().discarded_writes, 3);
}

/// REQ-L25-052: Programme Delivery Control and reserved triplets have no
/// display effect.
#[test]
fn programme_delivery_control_has_no_display_effect() {
    let local = [
        set_position(2, 3),
        TripletFunction::ProgrammeDeliveryColumn { column: 9, data: 0x41 },
        TripletFunction::ProgrammeDeliveryRow { mode: 0b01100, address: 44, data: 0x41 },
        TripletFunction::ReservedColumn { column: 9, mode: 0b00101, data: 0x41 },
        TripletFunction::ReservedRow { mode: 0b11001, address: 44, data: 0x41 },
    ];
    let with = build_25(&page(&[(2, b"ABCDEFGHIJ")], &local));
    let without = build_25(&page(&[(2, b"ABCDEFGHIJ")], &[]));
    common::assert_same_cells(&with, &without, "PDC");
}

/// REQ-L25-002, REQ-FSM-011: designation codes in ascending order whatever
/// the order of arrival, missing codes skipped, and the walk ended by a
/// Termination Marker or an Object Definition.
#[test]
fn processing_order_of_local_data() {
    use tellytxt_core::packet::ObjectKind;
    let packets = display_page(
        0,
        &[],
        vec![
            x26(3, &[g0(5, b'd'), end(), g0(6, b'e')]),
            x26(1, &[set_position(2, 0), g0(3, b'b')]),
            x26(0, &[set_position(1, 0), g0(1, b'a')]),
        ],
    );
    let grid = build_25(&packets);
    assert_eq!((char_of(&grid.cells[1][1]), char_of(&grid.cells[2][3])), ('a', 'b'));
    assert_eq!(char_of(&grid.cells[2][5]), 'd', "code 2 missing: skipped");
    assert_eq!(char_of(&grid.cells[2][6]), ' ', "after the Termination Marker");
    let definition = common::define(ObjectKind::Active, 0b11, common::local_number(0, 2));
    let grid =
        build_25(&display_page(0, &[], vec![x26(0, &[set_position(1, 0), g0(1, b'a'), definition, g0(2, b'b')])]));
    assert_eq!(text(&grid, 1).trim_end(), " a", "an Object Definition ends the local data");
}

/// REQ-L25-001, REQ-PRF-006, REQ-TST-015: one received page shown at each
/// level without being acquired again; the Level 3.5 features are used at
/// Level 3.5 only.
#[test]
fn levels_change_without_reacquisition() {
    let styled = FontStyleData { bold: true, ..FontStyleData::default() };
    let local = [
        set_position(1, 0),
        TripletFunction::G3Character { column: 1, code: 0x51, level_15_form: true },
        g0(2, b'Z'),
        TripletFunction::FontStyle { column: 3, style: styled },
    ];
    let packets = page(&[(1, b"ABCD")], &local);
    let mut receiver = Receiver::new();
    receiver.run(&packets, Config::default(), |decoder| {
        let at = |level| grid_of(decoder, &Config { level, ..Config::default() }).0;
        let level_1 = at(PresentationLevel::Level1);
        let level_15 = at(PresentationLevel::Level15);
        let level_25 = at(L25);
        let level_35 = at(L35);
        assert_eq!(text(&level_1, 1).trim_end(), "ABCD");
        assert_eq!(set_of(&level_15.cells[1][1]), Some(CharSet::G3));
        assert_eq!(char_of(&level_15.cells[1][2]), 'C', "mode 01001 is not a Level 1.5 mode");
        assert_eq!(char_of(&level_25.cells[1][2]), 'Z');
        assert!(!level_25.cells[1][3].attributes.font().bold && level_35.cells[1][3].attributes.font().bold);
        // The decoder still holds the page as received.
        let again = at(PresentationLevel::Level1);
        common::assert_same_cells(&again, &level_1, "again");
    });
}

/// REQ-L25-051: a page whose enhancement fails still shows its Level 1
/// text; the faults are counted.
#[test]
fn faults_leave_the_level_1_page() {
    use tellytxt_core::packet::{ObjectKind, ObjectSource};
    let missing = common::invoke(ObjectSource::Public, ObjectKind::Active, common::public_number(0, 0));
    let local = [set_position(1, 0), missing, g0(5, b'!')];
    let packets =
        display_page(0, &[(1, b"LEVEL ONE")], vec![damage(x26(0, &[local[0], local[1], local[2], end()]), 3)]);
    let (grid, report) = build(&packets, L25);
    let report = report.unwrap();
    assert_eq!(text(&grid, 1).trim_end(), "LEVEL!ONE");
    assert_eq!((report.damaged_triplets, report.absent_objects), (1, 1));
}

/// REQ-L25-006: what a cell records: colours as Colour Map indices, source
/// and the Level 2.5 attributes, all at their defaults on a plain page.
#[test]
fn plain_page_matches_level_1() {
    let rows: &[(u8, &[u8])] = &[(1, b"PLAIN TEXT"), (2, &[ALPHA_GREEN, b'G', ALPHA_BLUE, b'B', ALPHA_WHITE, b'W'])];
    let enhanced = build_25(&display_page(0, rows, vec![]));
    let level_1 = build(&display_page(0, rows, vec![]), PresentationLevel::Level1).0;
    common::assert_same_cells(&enhanced, &level_1, "no enhancement data, no re-mapping: the Level 1 page");
}

/// REQ-L25-010, decision E.50: a G3 character written by a triplet does
/// not change the held mosaic, and Hold Mosaics never replaces it.
#[test]
fn hold_mosaics_and_enhancement_characters() {
    use tellytxt_core::consts::HOLD_MOSAICS;
    let row: &[u8] = &[MOSAIC_RED, 0x7F, HOLD_MOSAICS, MOSAIC_RED, b' '];
    let local = [set_position(1, 0), TripletFunction::G3Character { column: 2, code: 0x51, level_15_form: false }];
    let grid = build_25(&page(&[(1, row)], &local));
    assert_eq!(set_of(&grid.cells[1][2]), Some(CharSet::G3), "not replaced by the held mosaic");
    let held = Glyph::Mosaic { pattern: 0x3F, separated: false };
    assert_eq!((grid.cells[1][3].glyph, grid.cells[1][3].held), (held, true), "the held mosaic is unchanged");
}

/// REQ-FSM-011, REQ-FSM-009: transitions not covered elsewhere. A
/// Termination Marker ends an object and the walk returns to the invoker's
/// position; a damaged triplet in the Modifier pending state is skipped
/// without moving the position and discards the offsets.
#[test]
fn walk_transitions() {
    use tellytxt_core::packet::{ObjectKind, ObjectSource};
    let object = [
        common::define(ObjectKind::Passive, 0b11, common::local_number(1, 0)),
        g0(0, b'o'),
        TripletFunction::Termination { kind: 0b100 },
        g0(1, b'x'),
    ];
    let invoke = common::invoke(ObjectSource::Local, ObjectKind::Passive, common::local_number(1, 0));
    let local = [
        set_position(3, 2),
        common::origin(1, 10),
        invoke,
        g0(5, b'l'),
        common::origin(2, 0),
        g0(30, 0x10),
        invoke,
        end(),
    ];
    let packets = display_page(0, &[], vec![damage(x26(0, &local), 5), x26(1, &object)]);
    let grid = build_25(&packets);
    assert_eq!(char_of(&grid.cells[4][12]), 'o', "placed with the modifier");
    assert_eq!(char_of(&grid.cells[4][13]), ' ', "the Termination Marker ended the object");
    assert_eq!(char_of(&grid.cells[3][5]), 'o', "the second object at 3,5: position restored, offsets discarded");
    assert_eq!((char_of(&grid.cells[5][5]), char_of(&grid.cells[3][30])), (' ', ' '));
}

/// REQ-TST-015, REQ-L25-050, REQ-FSM-008: every row mode and every column
/// mode of Tables 28 and 29, with each class of data value, reserved ones
/// included, at both levels: the page builds, the receiver's area is
/// untouched, and a character written after it lands where the position
/// rules say.
#[test]
fn every_mode_with_every_class_of_data() {
    use tellytxt_core::packet::Triplet;
    let reference = build_25(&page(&[], &[]));
    for level in [L25, L35] {
        for row_group in [false, true] {
            for mode in 0..32u8 {
                for data in [0x00, 0x1F, 0x20, 0x41, 0x5F, 0x7F] {
                    let address = if row_group { 46 } else { 12 };
                    let triplet = Triplet { address, mode, data }.function();
                    let local = [set_position(3, 2), triplet, g0(30, b'Z')];
                    let (grid, _) = build(&page(&[], &local), level);
                    let label = format!("{level:?} row group {row_group} mode {mode:05b} data {data:#04x}");
                    assert_eq!(grid.cells[0][..8], reference.cells[0][..8], "{label}");
                    // Only the row triplets that set the row move it:
                    // Full Row Colour and Set Active Position to row 6, and
                    // Object Invocation never.
                    let moves_row = row_group && matches!(mode, 0b00001 | 0b00100);
                    // Decision E.46: an Object Definition ends the local
                    // data (the Termination Marker needs address 63).
                    let ends = row_group && (0b10101..=0b10111).contains(&mode);
                    let expected_row = if moves_row { 6 } else { 3 };
                    // A designation may change the set; the code stays.
                    let glyph = grid.cells[expected_row][30].glyph;
                    if !ends {
                        assert!(
                            matches!(glyph, Glyph::Char(character) if character.code == b'Z'),
                            "{label}: {glyph:?}"
                        );
                    }
                }
            }
        }
    }
}
