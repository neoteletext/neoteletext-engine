//! The enhancement processor's non-spacing attributes (area L25, Table 27;
//! REQ-FSM-011, REQ-FSM-012, REQ-TST-015 and REQ-TST-016): colours, flash,
//! display attributes and sizes placed by X/26 triplets, checked cell by
//! cell. They are the `nonspacing-attributes` feature, off by default
//! (decision E.71), so these tests are built only with it.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

mod common;

use common::{
    background, build, build_25, char_of, colours, display_page, end, foreground, g0, local_data, row_0, set_position,
    x26, x28,
};
use neoteletext_core::colour::colour_index;
use neoteletext_core::config::PresentationLevel;
use neoteletext_core::consts::{
    ALPHA_BLACK, ALPHA_RED, ALPHA_YELLOW, BLACK_BACKGROUND, C5_NEWSFLASH, CONCEAL, CONTIGUOUS_MOSAICS, DOUBLE_HEIGHT,
    DOUBLE_WIDTH, MOSAIC_RED, NEW_BACKGROUND, NORMAL_SIZE, SEPARATED_MOSAICS, START_BOX,
};
use neoteletext_core::level1::{Glyph, Size, Width};
use neoteletext_core::packet::{ColourData, DisplayAttributes, Packet, TripletFunction};

const L25: PresentationLevel = PresentationLevel::Level25;

/// Page 100 with Level 1 rows and local enhancement data.
fn page(rows: &[(u8, &[u8])], local: &[TripletFunction]) -> Vec<Packet> {
    display_page(0, rows, local_data(local))
}

fn display(column: u8, attributes: DisplayAttributes) -> TripletFunction {
    TripletFunction::DisplayAttributes { column, attributes }
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
    assert_eq!(report.unwrap().faults.discarded_writes, 3);
}

/// The Additional Flash Functions (Table 29) are the feature of that name,
/// which builds on the non-spacing attributes.
#[cfg(feature = "additional-flash-functions")]
mod additional_flash_functions {
    use super::*;
    use neoteletext_core::consts::{FLASH, STEADY};
    use neoteletext_core::level1::{Cell, FlashMode, FlashRate};

    fn flash(column: u8, mode: u8, rate: u8) -> TripletFunction {
        TripletFunction::AdditionalFlash { column, mode, rate, reserved: 0 }
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
        assert_eq!(state(cell(3)), (true, FlashMode::InvertPhase, FlashRate::FastPhase2));
        assert_eq!(state(cell(5)), (true, FlashMode::AdjacentClut, FlashRate::FastPhase1));
        assert!(!cell(7).flash, "steady");
        let phases =
            |first: usize| (first..first + 4).map(|column| cell(column).attributes.flash_rate()).collect::<Vec<_>>();
        let (p1, p2, p3) = (FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3);
        assert_eq!(phases(10), [p1, p2, p3, p1], "incremental");
        assert_eq!(phases(20), [p1, p3, p2, p1], "decremental");
        assert_eq!(phases(30), [p3, p2, p1, p3], "reserved values change nothing: decremental continues");
        // Level 1 Flash: normal, at the Level 1 rate, until Steady.
        let level_1: Vec<bool> = grid.cells[2][..5].iter().map(|cell| cell.flash).collect();
        assert_eq!(level_1, [false, true, true, false, false]);
        assert_eq!(grid.cells[2][1].attributes.flash_rate(), FlashRate::Level1);
    }
}
