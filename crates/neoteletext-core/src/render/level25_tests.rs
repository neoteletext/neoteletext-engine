//! Tests of the rendering of Levels 2.5 and 3.5: the resolved colour mode,
//! side panels, double width and size, underline, invert, flash and
//! conceal, G3 drawing and the flash timing
//! (REQ-RND-030 to REQ-RND-040). Every grid is built by hand, so these
//! tests depend on nothing but the grid (REQ-RND-001).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
#![cfg_attr(
    not(feature = "side-panels"),
    allow(dead_code, unused_imports, reason = "helpers of the tests whose capability is compiled out (decision E.71)")
)]

// The module is compiled for tests only (see `render.rs`); the attribute
// also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;
use crate::charset::{CharRef, CharSet, G0, G0Set, NationalOption};
use crate::colour::SidePanels;
use crate::consts::{G3_CODES, G3_FIRST_CODE, PAGE_WIDTH};
use crate::level1::CellAttributes;
use std::vec;
use std::vec::Vec;

const RED: u8 = 1;
const BLUE: u8 = 4;
const CYAN: u8 = 6;
const WHITE: u8 = 7;
const T: u8 = COLOUR_TRANSPARENT;

/// A case the palette mode must refuse: its name and how to make it.
type RefusalCase = (&'static str, fn(&mut Grid));
/// Invert, flash mode, conceal, reveal and flash unit, with the dots' and
/// gaps' indices they give.
type ColourCase = (bool, Option<FlashMode>, bool, bool, u8, (u8, u8));

fn latin() -> CharSet {
    CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) })
}

fn text(code: u8) -> Glyph {
    Glyph::Char(CharRef::new(latin(), code))
}

fn g3(code: u8) -> Glyph {
    Glyph::Char(CharRef::new(CharSet::G3, code))
}

fn options(mode: CellMode, unit: u8, reveal: bool) -> RenderOptions {
    RenderOptions {
        mode,
        rounding: true,
        reveal,
        colour: ColourMode::Resolved(FlashPhase::at_unit(unit)),
        underline: true,
    }
}

fn large(unit: u8) -> RenderOptions {
    options(CellMode::Saa5050, unit, false)
}

fn line_of(grid: &Grid, cell: &Cell, row: usize, column: usize, line: usize, options: RenderOptions) -> CellLine {
    cell_line(cell, grid, row, column, line, &GlyphSet::bedstead(), options)
}

/// A whole screen at the grid's row width.
fn screen(grid: &Grid, options: RenderOptions) -> Vec<u8> {
    let width = row_width(grid);
    let mut out = vec![0u8; width * ROWS * options.row_height()];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut out, stride: width };
    render_screen(grid, ALL_ROWS, &GlyphSet::bedstead(), options, &mut buffer).unwrap();
    out
}

/// The off units of each rate, as decision E.48 lists them.
const OFF_UNITS: [(FlashRate, [u8; 4]); 3] = [
    (FlashRate::FastPhase1, [0, 1, 6, 7]),
    (FlashRate::FastPhase2, [2, 3, 8, 9]),
    (FlashRate::FastPhase3, [4, 5, 10, 11]),
];

/// REQ-RND-040, decision E.48: every rate and phase in every unit.
#[test]
fn flash_rates_follow_decision_e48_in_every_unit() {
    for unit in 0..FLASH_UNITS {
        assert_eq!(flash_on(FlashRate::Slow, unit), unit <= 8, "slow, unit {unit}");
        for (rate, off) in OFF_UNITS {
            assert_eq!(flash_on(rate, unit), !off.contains(&unit), "{rate:?}, unit {unit}");
        }
        let phase = FlashPhase::at_unit(unit);
        for rate in [FlashRate::Slow, FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3] {
            assert_eq!(phase.is_on(rate), flash_on(rate, unit));
            assert!(FlashPhase::STEADY.is_on(rate));
        }
    }
    // Units wrap modulo 12.
    assert_eq!(FlashPhase::at_unit(12), FlashPhase::at_unit(0));
}

/// REQ-RND-040: the caller's time to the flash unit, computed once per
/// frame.
#[test]
fn flash_unit_from_time() {
    let unit = |us| flash_unit(Instant(us));
    assert_eq!((unit(0), unit(83_333), unit(83_334)), (0, 0, 1));
    assert_eq!((unit(999_999), unit(1_000_000), unit(1_500_000)), (11, 0, 6));
    assert_eq!(unit(u64::MAX), unit(u64::MAX % 1_000_000));
    assert_eq!(FlashPhase::at(Instant(250_000)), FlashPhase::at_unit(3));
}

/// REQ-RND-040, REQ-L25-027: an incremental run of phases 1, 2, 3, 1, ...
/// makes the dark cell step rightwards every two units; a decremental run
/// (1, 3, 2, ...) steps leftwards.
#[test]
fn incremental_and_decremental_runs() {
    let phases = [FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3];
    let mut grid = Grid::EMPTY;
    for column in 0..6 {
        let up = phases[column % 3];
        let down = phases[(3 - column % 3) % 3];
        for (row, rate) in [(1, up), (2, down)] {
            let cell = &mut grid.cells[row][column];
            *cell = Cell { glyph: g3(0x3F), foreground: WHITE, background: BLUE, flash: true, ..Cell::BLANK };
            cell.attributes = CellAttributes::DEFAULT.with_flash_rate(rate);
        }
    }
    let dark = |grid: &Grid, row: usize, unit: u8| -> Vec<usize> {
        (0..6)
            .filter(|&column| line_of(grid, &grid.cells[row][column], row, column, 0, large(unit)).foreground == BLUE)
            .collect()
    };
    for (pair, expected) in [(0u8, 0usize), (1, 1), (2, 2)] {
        for unit in [2 * pair, 2 * pair + 1, 2 * pair + 6, 2 * pair + 7] {
            assert_eq!(dark(&grid, 1, unit), vec![expected, expected + 3], "incremental, unit {unit}");
            let leftwards = (3 - expected) % 3;
            assert_eq!(dark(&grid, 2, unit), vec![leftwards, leftwards + 3], "decremental, unit {unit}");
        }
    }
}

/// A grid that only Level 1 could have made: CLUT 0 colours, normal slow
/// flash, conceal, boxes, mosaics, text and double height.
fn level1_grid() -> Grid {
    let mut grid = Grid::EMPTY;
    for (column, cell) in grid.cells[3].iter_mut().enumerate() {
        let code = 0x41 + (column % 26) as u8;
        *cell = Cell {
            glyph: if column % 3 == 0 {
                Glyph::Mosaic { pattern: code & 0x3F, separated: column % 2 == 0 }
            } else {
                text(code)
            },
            foreground: (column % 8) as u8,
            background: ((column + 3) % 8) as u8,
            flash: column % 4 == 1,
            conceal: column % 5 == 2,
            boxed: column % 6 == 0,
            ..Cell::BLANK
        };
    }
    grid.cells[5][4] = Cell { glyph: text(b'H'), size: Size::DoubleTop, ..Cell::BLANK };
    grid.cells[6][4] = Cell { glyph: text(b'H'), size: Size::DoubleBottom, ..Cell::BLANK };
    grid.cells[7][1] = Cell { glyph: text(b'Q'), flash: true, conceal: true, foreground: RED, ..Cell::BLANK };
    grid
}

/// REQ-RND-031: on Level 1 pages the resolved mode gives exactly the
/// colours of the palette mode, at every flash unit and both reveal
/// settings, with the slow rate standing in for the timing of decision
/// E.12; also on a boxed page.
#[test]
fn resolved_mode_matches_palette_mode_on_level1_pages() {
    let identity = [0u8, 1, 2, 3, 4, 5, 6, 7, T];
    for boxed_mode in [false, true] {
        let mut grid = level1_grid();
        grid.boxed_mode = boxed_mode;
        for mode in [CellMode::Saa5050, CellMode::Saa5246a] {
            for reveal in [false, true] {
                let palette_options =
                    RenderOptions { mode, rounding: true, reveal, colour: ColourMode::Palette, underline: true };
                let indices = screen(&grid, palette_options);
                for unit in 0..FLASH_UNITS {
                    let mut palette = [0u8; PALETTE_SIZE];
                    fill_palette(&mut palette, &identity, flash_on(FlashRate::Slow, unit), reveal);
                    let expected: Vec<u8> = indices.iter().map(|&index| palette[usize::from(index)]).collect();
                    let resolved = screen(&grid, options(mode, unit, reveal));
                    assert!(expected == resolved, "boxed {boxed_mode}, {mode:?}, reveal {reveal}, unit {unit}");
                }
            }
        }
    }
}

/// REQ-RND-031, decision E.55: the palette mode is refused for each case
/// that needs the resolved mode, by every drawing call.
#[test]
fn palette_mode_is_refused_where_it_cannot_draw() {
    let glyphs = GlyphSet::bedstead();
    let palette = RenderOptions {
        mode: CellMode::Saa5050,
        rounding: true,
        reveal: false,
        colour: ColourMode::Palette,
        underline: true,
    };
    let refused = |grid: &Grid| {
        let mut out = vec![0u8; row_width(grid) * CELL_HEIGHT];
        let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut out, stride: row_width(grid) };
        let row = render_row(grid, 2, Field::Both, &glyphs, palette, &mut buffer);
        let line = render_line(grid, 2 * CELL_HEIGHT, Zoom::Off, &glyphs, palette, IndexFormat::Bits8, &mut out);
        assert_eq!(row, line);
        assert_eq!(row.is_err(), !palette_mode_allowed(grid));
        row == Err(RenderError::NeedsResolvedMode)
    };
    assert!(!refused(&level1_grid()), "a Level 1 page draws in the palette mode");
    let cases: [RefusalCase; 6] = [
        ("foreground outside CLUT 0", |grid| grid.cells[2][5].foreground = 9),
        ("background outside CLUT 0", |grid| grid.cells[2][5].background = 31),
        ("fast flash", |grid| {
            grid.cells[2][5].flash = true;
            grid.cells[2][5].attributes = CellAttributes::DEFAULT.with_flash_rate(FlashRate::FastPhase2);
        }),
        ("inverted flash", |grid| {
            grid.cells[2][5].flash = true;
            grid.cells[2][5].attributes = CellAttributes::DEFAULT.with_flash_mode(FlashMode::Inverted);
        }),
        ("adjacent-CLUT flash", |grid| {
            grid.cells[2][5].flash = true;
            grid.cells[2][5].attributes = CellAttributes::DEFAULT.with_flash_mode(FlashMode::AdjacentClut);
        }),
        ("a side-panel cell outside CLUT 0", |grid| {
            grid.side_panels = SidePanels { left: 4, right: 0 };
            grid.panel_cell_mut(2, 70).unwrap().background = 12;
        }),
    ];
    // A build without a capability cannot draw it, so does not refuse it
    // (decision E.71).
    let compiled = |name: &&str| cfg!(feature = "side-panels") || *name != "a side-panel cell outside CLUT 0";
    for (name, case) in cases.into_iter().filter(|(name, _)| compiled(name)) {
        let mut grid = level1_grid();
        case(&mut grid);
        assert!(refused(&grid), "{name}");
    }
    // Flash with the default slow normal attributes is Level 1 flash.
    let mut grid = level1_grid();
    grid.cells[2][5].flash = true;
    assert!(!refused(&grid));
}

/// REQ-RND-036, decisions E.47 and E.48: invert, then flash, then conceal,
/// for every combination, in both phases. Red on blue; the slow rate is on
/// in unit 0 and off in unit 9.
#[test]
fn invert_flash_and_conceal_combinations() {
    const ON: u8 = 0;
    const OFF: u8 = 9;
    let flip = |colour: u8| colour ^ ADJACENT_CLUT_FLIP;
    // (invert, flash mode, conceal, reveal, unit) -> (dots, gaps)
    let cases: [ColourCase; 26] = [
        (false, None, false, false, ON, (RED, BLUE)),
        (true, None, false, false, ON, (BLUE, RED)),
        (false, Some(FlashMode::Normal), false, false, ON, (RED, BLUE)),
        (false, Some(FlashMode::Normal), false, false, OFF, (BLUE, BLUE)),
        (false, Some(FlashMode::Inverted), false, false, ON, (BLUE, BLUE)),
        (false, Some(FlashMode::Inverted), false, false, OFF, (RED, BLUE)),
        (false, Some(FlashMode::AdjacentClut), false, false, ON, (RED, BLUE)),
        (false, Some(FlashMode::AdjacentClut), false, false, OFF, (flip(RED), BLUE)),
        (true, Some(FlashMode::Normal), false, false, ON, (BLUE, RED)),
        (true, Some(FlashMode::Normal), false, false, OFF, (RED, RED)),
        (true, Some(FlashMode::Inverted), false, false, ON, (RED, RED)),
        (true, Some(FlashMode::Inverted), false, false, OFF, (BLUE, RED)),
        (true, Some(FlashMode::AdjacentClut), false, false, ON, (BLUE, RED)),
        (true, Some(FlashMode::AdjacentClut), false, false, OFF, (flip(BLUE), RED)),
        (false, None, true, false, ON, (BLUE, BLUE)),
        (false, None, true, true, ON, (RED, BLUE)),
        (true, None, true, false, ON, (RED, RED)),
        (true, None, true, true, ON, (BLUE, RED)),
        (false, Some(FlashMode::Normal), true, false, ON, (BLUE, BLUE)),
        (false, Some(FlashMode::Normal), true, true, ON, (RED, BLUE)),
        (false, Some(FlashMode::Normal), true, true, OFF, (BLUE, BLUE)),
        (false, Some(FlashMode::AdjacentClut), true, false, OFF, (BLUE, BLUE)),
        (false, Some(FlashMode::AdjacentClut), true, true, OFF, (flip(RED), BLUE)),
        (true, Some(FlashMode::Inverted), true, false, OFF, (RED, RED)),
        (true, Some(FlashMode::Inverted), true, true, OFF, (BLUE, RED)),
        (true, Some(FlashMode::AdjacentClut), true, true, OFF, (flip(BLUE), RED)),
    ];
    let grid = Grid::EMPTY;
    for (invert, flash, conceal, reveal, unit, expected) in cases {
        let attributes =
            CellAttributes::DEFAULT.with_invert(invert).with_flash_mode(flash.unwrap_or(FlashMode::Normal));
        let cell = Cell {
            glyph: text(b'W'),
            foreground: RED,
            background: BLUE,
            flash: flash.is_some(),
            conceal,
            attributes,
            ..Cell::BLANK
        };
        let raster = line_of(&grid, &cell, 4, 4, 8, options(CellMode::Saa5050, unit, reveal));
        assert_eq!((raster.foreground, raster.background), expected, "{invert} {flash:?} {conceal} {reveal} {unit}");
        assert_ne!(raster.mask, 0);
    }
}

/// REQ-L25-029, Table C.1 of Annex C.1 (REQ-TST-016): all sixteen rows,
/// for the gaps and the dots of a cell, and mix mode.
#[test]
fn display_planes_follow_table_c1() {
    const OPAQUE: u8 = 2;
    const ROW: u8 = 5;
    #[derive(Clone, Copy, PartialEq, Debug)]
    enum Shows {
        Colour,
        Video,
        Row,
    }
    use Shows::{Colour, Row, Video};
    // (F/B transparent, row colour transparent, boxed, newsflash) -> result.
    let table = [
        (false, false, false, false, Colour),
        (false, false, false, true, Video),
        (false, false, true, false, Colour),
        (false, false, true, true, Colour),
        (false, true, false, false, Colour),
        (false, true, false, true, Video),
        (false, true, true, false, Colour),
        (false, true, true, true, Colour),
        (true, false, false, false, Row),
        (true, false, false, true, Video),
        (true, false, true, false, Video),
        (true, false, true, true, Row),
        (true, true, false, false, Video),
        (true, true, false, true, Video),
        (true, true, true, false, Video),
        (true, true, true, true, Video),
    ];
    for (fb_transparent, row_transparent, boxed, newsflash, result) in table {
        let mut grid = Grid::EMPTY;
        grid.boxed_mode = newsflash;
        grid.row_colours[6] = if row_transparent { T } else { ROW };
        let colour = if fb_transparent { T } else { OPAQUE };
        let cell = Cell { glyph: g3(0x3F), foreground: colour, background: colour, boxed, ..Cell::BLANK };
        let expected = match result {
            Colour => OPAQUE,
            Video => T,
            Row => ROW,
        };
        let raster = line_of(&grid, &cell, 6, 20, 3, large(0));
        assert_eq!(
            (raster.foreground, raster.background),
            (expected, expected),
            "{fb_transparent} {row_transparent} {boxed} {newsflash}"
        );
    }
    // Mix mode makes backgrounds the picture, not the dots.
    let mut grid = Grid::EMPTY;
    grid.mix = true;
    let cell = Cell { glyph: g3(0x3F), foreground: RED, background: BLUE, ..Cell::BLANK };
    let raster = line_of(&grid, &cell, 6, 20, 3, large(0));
    assert_eq!((raster.foreground, raster.background), (RED, T));
}

/// Decision E.45, REQ-SEC-005: the row colour never shows in row 0,
/// columns 0 to 7, even where the cell's colour is transparent.
#[test]
fn row_colour_never_colours_the_status_area() {
    let mut grid = Grid::EMPTY;
    grid.row_colours[0] = CYAN;
    let cell = Cell { background: T, ..Cell::BLANK };
    for column in 0..COLUMNS {
        let expected = if column < RECEIVER_COLUMNS { T } else { CYAN };
        assert_eq!(line_of(&grid, &cell, 0, column, 0, large(0)).background, expected, "column {column}");
    }
}

/// REQ-RND-030: indices above 31 keep their low five bits, so the resolved
/// mode always writes Colour Map indices.
#[test]
fn resolved_indices_are_colour_map_indices() {
    let grid = Grid::EMPTY;
    let cell = Cell { glyph: g3(0x3F), foreground: 0xFF, background: 0x3F, ..Cell::BLANK };
    let raster = line_of(&grid, &cell, 2, 2, 0, large(0));
    assert_eq!((raster.foreground, raster.background), (31, 31));
}

/// A grid whose cells say where they are: the cell of source column c
/// shows the contiguous mosaic c mod 64 in white on colour c / 64, so
/// the renderer's choice of source column can be read back from its pixels.
/// The screen colour is cyan, which no label uses.
fn labelled_grid(left: u8, right: u8) -> Grid {
    let mut grid = Grid::EMPTY;
    grid.side_panels = SidePanels { left, right };
    grid.screen_colour = CYAN;
    let label = |column: usize| Cell {
        glyph: Glyph::Mosaic { pattern: (column % 64) as u8, separated: false },
        foreground: WHITE,
        background: (column / 64) as u8,
        ..Cell::BLANK
    };
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            grid.cells[row][column] = label(column);
        }
        for column in SIDE_PANEL_FIRST_COLUMN..PANEL_AREA_END {
            if let Some(cell) = grid.panel_cell_mut(row, column) {
                *cell = label(column);
            }
        }
    }
    grid
}

/// The source column each display slot of a row shows, read back from the
/// labels of [`labelled_grid`], or `None` for the screen colour. The row is
/// drawn by the row renderer in the palette mode.
fn columns_shown(grid: &Grid, row: usize) -> Vec<Option<usize>> {
    let width = row_width(grid);
    let mut out = vec![0u8; width * CELL_HEIGHT];
    let palette = RenderOptions {
        mode: CellMode::Saa5050,
        rounding: true,
        reveal: false,
        colour: ColourMode::Palette,
        underline: true,
    };
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut out, stride: width };
    render_row(grid, row, Field::Both, &GlyphSet::bedstead(), palette, &mut buffer).unwrap();
    (0..width / CELL_WIDTH)
        .map(|slot| {
            let pixel = |line: usize, dot: usize| out[line * width + slot * CELL_WIDTH + dot];
            if pixel(0, 0) == CYAN {
                return None;
            }
            // Block rows start on lines 0, 6 and 14 (decision E.14); the
            // left block is dot 0 and the right block dot 11.
            let (mut pattern, mut high) = (0usize, 0usize);
            for (block, line) in [0, 6, 14].into_iter().enumerate() {
                for (side, dot) in [0, 11].into_iter().enumerate() {
                    if pixel(line, dot) == WHITE {
                        pattern |= 1 << (2 * block + side);
                    } else {
                        high = usize::from(pixel(line, dot));
                    }
                }
            }
            Some(pattern + 64 * high)
        })
        .collect()
}

/// REQ-RND-033, decision E.49, clause 12.5.3 (REQ-TST-016): the row is
/// composed of the left panel, the main page and the right panel, for every
/// width; row 0's panel area shows the screen colour.
// Needs its capability compiled in (decision E.71).
#[cfg(feature = "side-panels")]
#[test]
fn side_panels_compose_each_row() {
    for (left, right) in [(0u8, 0u8), (16, 0), (0, 16), (8, 8), (5, 11), (11, 5), (1, 0), (0, 1), (16, 16)] {
        let grid = labelled_grid(left, right);
        let (left, right) = (usize::from(left), usize::from(right));
        assert_eq!(row_columns(&grid), 40 + left + right);
        assert_eq!(row_width(&grid), (40 + left + right) * CELL_WIDTH);
        let expected: Vec<Option<usize>> = (72 - left..72).chain(0..40).chain(40..40 + right).map(Some).collect();
        for row in [1, 12, 24] {
            assert_eq!(columns_shown(&grid, row), expected, "{left} {right} row {row}");
        }
        let row0: Vec<Option<usize>> =
            (0..left).map(|_| None).chain((0..40).map(Some)).chain((0..right).map(|_| None)).collect();
        assert_eq!(columns_shown(&grid, 0), row0, "row 0, {left} {right}");
    }
    // Widths are clamped at 16 columns each.
    let grid = labelled_grid(200, 17);
    assert_eq!(row_columns(&grid), 72);
}

/// REQ-RND-033: the three layouts of clause 12.5.3 at their widest, drawn
/// in the resolved mode at full width, with REQ-RND-017's independence:
/// whole screen, row by row and line by line agree.
// Needs its capability compiled in (decision E.71).
#[cfg(feature = "side-panels")]
#[test]
fn panel_rows_render_identically_in_any_order() {
    for (left, right) in [(16u8, 0u8), (0, 16), (8, 8)] {
        let mut grid = Grid::EMPTY;
        grid.side_panels = SidePanels { left, right };
        grid.screen_colour = 3;
        grid.cells[4][0].glyph = text(b'A');
        grid.panel_cell_mut(4, 71).unwrap().glyph = text(b'L');
        grid.panel_cell_mut(4, 40).unwrap().glyph = text(b'R');
        grid.panel_cell_mut(9, 50).unwrap().glyph = g3(0x25);
        let width = row_width(&grid);
        assert_eq!(width, 672);
        let options = large(5);
        let whole = screen(&grid, options);
        let glyphs = GlyphSet::bedstead();
        for row in (0..ROWS).rev() {
            let mut single = vec![0u8; width * CELL_HEIGHT];
            let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut single, stride: width };
            render_row(&grid, row, Field::Both, &glyphs, options, &mut buffer).unwrap();
            assert_eq!(&whole[row * CELL_HEIGHT * width..(row + 1) * CELL_HEIGHT * width], &single[..]);
        }
        for y in 0..ROWS * CELL_HEIGHT {
            let mut line = vec![0u8; width];
            render_line(&grid, y, Zoom::Off, &glyphs, options, IndexFormat::Bits8, &mut line).unwrap();
            assert_eq!(&whole[y * width..(y + 1) * width], &line[..]);
        }
        // Row 0's panel area is the screen colour on every line.
        let left_pixels = usize::from(left) * CELL_WIDTH;
        assert!(whole[..left_pixels].iter().all(|&pixel| pixel == 3));
        // A buffer of only 480 pixels is too small.
        let mut narrow = vec![0u8; PAGE_WIDTH];
        let result = render_line(&grid, 0, Zoom::Off, &glyphs, options, IndexFormat::Bits8, &mut narrow);
        assert_eq!(result, Err(RenderError::BufferTooSmall));
    }
}

/// A grid without panel rows shows the screen colour in its panel area.
// Needs its capability compiled in (decision E.71).
#[cfg(feature = "side-panels")]
#[test]
fn panels_without_storage_show_the_screen_colour() {
    let mut grid: Grid<0> = Grid::empty();
    grid.side_panels = SidePanels { left: 2, right: 0 };
    grid.screen_colour = CYAN;
    let mut out = vec![0u8; 42 * CELL_WIDTH];
    render_line(&grid, 100, Zoom::Off, &GlyphSet::bedstead(), large(0), IndexFormat::Bits8, &mut out).unwrap();
    assert!(out[..2 * CELL_WIDTH].iter().all(|&pixel| pixel == CYAN));
    assert!(out[2 * CELL_WIDTH..].iter().all(|&pixel| pixel == 0));
}

/// The screen colour is the picture on a boxed page and in mix mode
/// (decision E.45).
#[test]
fn border_index_follows_the_screen_colour() {
    let mut grid = Grid::EMPTY;
    grid.screen_colour = 0x2A;
    assert_eq!(border_index(&grid), 0x0A);
    grid.mix = true;
    assert_eq!(border_index(&grid), T);
    grid.mix = false;
    grid.boxed_mode = true;
    assert_eq!(border_index(&grid), T);
}

/// The normal-width mask of a cell on a line.
fn normal_mask(grid: &Grid, cell: Cell, line: usize, options: RenderOptions) -> u16 {
    let plain = Cell { size: Size::Normal, attributes: cell.attributes.with_width(Width::Normal), ..cell };
    line_of(grid, &plain, 5, 5, line, options).mask
}

/// Each half-dot of a normal-width line, drawn twice, across 24 dots.
fn doubled(mask: u16) -> u32 {
    (0..CELL_WIDTH).fold(0u32, |acc, dot| if (mask >> dot) & 1 == 1 { acc | (0b11 << (2 * dot)) } else { acc })
}

/// REQ-RND-034: the left and right halves of a double-width character
/// show the normal-width line, each half-dot drawn twice, for text with
/// rounding, mosaics and G3; double size adds the vertical halves.
#[test]
fn double_width_and_double_size() {
    let grid = Grid::EMPTY;
    let glyphs = [text(b'W'), Glyph::Mosaic { pattern: 0x2D, separated: true }, g3(0x22)];
    for mode in [CellMode::Saa5050, CellMode::Saa5246a] {
        let options = options(mode, 0, false);
        let height = options.row_height();
        for glyph in glyphs {
            for (size, vertical) in [(Size::Normal, 0), (Size::DoubleTop, 1), (Size::DoubleBottom, 2)] {
                let half =
                    |width| Cell { glyph, size, attributes: CellAttributes::DEFAULT.with_width(width), ..Cell::BLANK };
                for line in 0..height {
                    let source = match vertical {
                        0 => line,
                        1 => line / 2,
                        _ => height / 2 + line / 2,
                    };
                    let normal = normal_mask(&grid, half(Width::Normal), source, options);
                    let left = line_of(&grid, &half(Width::LeftHalf), 5, 5, line, options).mask;
                    let right = line_of(&grid, &half(Width::RightHalf), 5, 6, line, options).mask;
                    let joined = (u32::from(left) << CELL_WIDTH) | u32::from(right);
                    assert_eq!(joined, doubled(normal), "{glyph:?} {size:?} {mode:?} line {line}");
                }
            }
        }
    }
}

/// REQ-RND-034, REQ-L25-025: the renderer draws each half where the grid
/// puts it; a pair split by the panel boundary still shows its halves, and
/// a double-width cell in a side panel is drawn like one on the page.
// Needs its capability compiled in (decision E.71).
#[cfg(feature = "side-panels")]
#[test]
fn double_width_halves_beside_the_panel_boundary() {
    let mut grid = Grid::EMPTY;
    grid.side_panels = SidePanels { left: 0, right: 16 };
    let left =
        Cell { glyph: text(b'M'), attributes: CellAttributes::DEFAULT.with_width(Width::LeftHalf), ..Cell::BLANK };
    let right = Cell { attributes: CellAttributes::DEFAULT.with_width(Width::RightHalf), ..left };
    grid.cells[3][39] = left;
    *grid.panel_cell_mut(3, 40).unwrap() = right;
    *grid.panel_cell_mut(3, 44).unwrap() = left;
    *grid.panel_cell_mut(3, 45).unwrap() = right;
    let options = large(0);
    let width = row_width(&grid);
    for line in 0..CELL_HEIGHT {
        let mut out = vec![0u8; width];
        render_line(
            &grid,
            3 * CELL_HEIGHT + line,
            Zoom::Off,
            &GlyphSet::bedstead(),
            options,
            IndexFormat::Bits8,
            &mut out,
        )
        .unwrap();
        let normal = normal_mask(&grid, left, line, options);
        let expected: Vec<u8> =
            (0..2 * CELL_WIDTH).rev().map(|bit| if (doubled(normal) >> bit) & 1 == 1 { WHITE } else { 0 }).collect();
        assert_eq!(&out[39 * CELL_WIDTH..41 * CELL_WIDTH], &expected[..], "across the boundary, line {line}");
        assert_eq!(&out[44 * CELL_WIDTH..46 * CELL_WIDTH], &expected[..], "inside the panel, line {line}");
    }
}

/// REQ-RND-035, decision E.47: underline is the last line of the cell,
/// full width, on G0 and G2 characters only, and on the bottom half of a
/// double-height character; the underline bit on a mosaic means separated
/// mosaics and draws no line.
#[test]
fn underline_on_the_last_line() {
    let grid = Grid::EMPTY;
    let underlined = CellAttributes::DEFAULT.with_underline(true);
    let g2 = CharSet::G2(crate::charset::G2Set::Latin);
    for mode in [CellMode::Saa5050, CellMode::Saa5246a] {
        let options = options(mode, 0, false);
        let last = options.row_height() - 1;
        for glyph in [text(b'g'), text(b' '), Glyph::Char(CharRef::new(g2, 0x23))] {
            let cell = Cell { glyph, attributes: underlined, ..Cell::BLANK };
            assert_eq!(line_of(&grid, &cell, 1, 1, last, options).mask, CELL_MASK, "{glyph:?} {mode:?}");
            let plain = Cell { attributes: CellAttributes::DEFAULT, ..cell };
            for line in 0..last {
                assert_eq!(line_of(&grid, &cell, 1, 1, line, options), line_of(&grid, &plain, 1, 1, line, options));
            }
            let top = Cell { size: Size::DoubleTop, ..cell };
            let bottom = Cell { size: Size::DoubleBottom, ..cell };
            let plain_top = Cell { attributes: CellAttributes::DEFAULT, ..top };
            for line in 0..=last {
                assert_eq!(
                    line_of(&grid, &top, 1, 1, line, options),
                    line_of(&grid, &plain_top, 1, 1, line, options),
                    "top half"
                );
            }
            assert_eq!(line_of(&grid, &bottom, 2, 1, last, options).mask, CELL_MASK);
            assert_eq!(line_of(&grid, &bottom, 2, 1, last - 1, options).mask, CELL_MASK, "the enlarged underline");
        }
        // Never on G3, a mosaic or an empty cell.
        for glyph in [g3(0x20), Glyph::Space] {
            let cell = Cell { glyph, attributes: underlined, ..Cell::BLANK };
            let plain = Cell { attributes: CellAttributes::DEFAULT, ..cell };
            assert_eq!(line_of(&grid, &cell, 1, 1, last, options), line_of(&grid, &plain, 1, 1, last, options));
        }
        let separated =
            Cell { glyph: Glyph::Mosaic { pattern: 0x3F, separated: true }, attributes: underlined, ..Cell::BLANK };
        assert_eq!(line_of(&grid, &separated, 1, 1, last, options).mask, 0, "separated mosaics, no underline");
    }
}

/// Decision E.88: with the underline option off, no G0 or G2 character is
/// underlined, in either half of double height, and separated mosaics,
/// which share the attribute bit, are drawn exactly as before.
#[test]
fn the_underline_option_turns_underline_off_and_leaves_separated_mosaics() {
    let grid = Grid::EMPTY;
    let underlined = CellAttributes::DEFAULT.with_underline(true);
    let g2 = CharSet::G2(crate::charset::G2Set::Latin);
    for mode in [CellMode::Saa5050, CellMode::Saa5246a] {
        let on = options(mode, 0, false);
        let off = on.with_underline(false);
        assert!(on.underline && !off.underline);
        let last = on.row_height() - 1;
        for glyph in [text(b'g'), text(b' '), Glyph::Char(CharRef::new(g2, 0x23))] {
            for size in [Size::Normal, Size::DoubleTop, Size::DoubleBottom] {
                let cell = Cell { glyph, size, attributes: underlined, ..Cell::BLANK };
                let plain = Cell { attributes: CellAttributes::DEFAULT, ..cell };
                let row = if size == Size::DoubleBottom { 2 } else { 1 };
                for line in 0..=last {
                    assert_eq!(
                        line_of(&grid, &cell, row, 1, line, off),
                        line_of(&grid, &plain, row, 1, line, off),
                        "{glyph:?} {size:?} {mode:?} line {line}"
                    );
                }
            }
        }
        for separated in [false, true] {
            let mosaic = Glyph::Mosaic { pattern: 0x3F, separated };
            let cell = Cell { glyph: mosaic, attributes: underlined, ..Cell::BLANK };
            for line in 0..=last {
                assert_eq!(
                    line_of(&grid, &cell, 1, 1, line, off),
                    line_of(&grid, &cell, 1, 1, line, on),
                    "mosaic line {line}"
                );
            }
        }
    }
}

/// REQ-RND-037: every G3 position draws in both cell sizes, from the
/// generated data, without rounding and without panicking; the undefined
/// positions and 5/F are spaces.
#[test]
fn every_g3_position_draws() {
    let grid = Grid::EMPTY;
    let glyphs = GlyphSet::bedstead();
    for offset in 0..G3_CODES {
        let code = G3_FIRST_CODE + offset as u8;
        let data = glyphs.g3(code);
        for mode in [CellMode::Saa5050, CellMode::Saa5246a] {
            for rounding in [false, true] {
                let options = RenderOptions { rounding, ..options(mode, 0, false) };
                let cell = Cell { glyph: g3(code), ..Cell::BLANK };
                for line in 0..options.row_height() {
                    let expected = match mode {
                        CellMode::Saa5050 => data.large[line],
                        CellMode::Saa5246a => data.small[line],
                    };
                    assert_eq!(line_of(&grid, &cell, 1, 1, line, options).mask, expected, "{code:#04x} {mode:?}");
                }
            }
        }
    }
    for code in [0x5F, 0x6E, 0x6F, 0x7E, 0x7F, 0x00, 0x80, 0xFF] {
        assert_eq!(*glyphs.g3(code), crate::glyphs::G3Glyph::BLANK, "{code:#04x}");
    }
}

/// REQ-RND-037: shapes checked against Table 48's figure. 2/5 and 6/5 are
/// the two halves of the cell cut by its diagonal; 3/F is the full block;
/// 5/0 and 5/1 are the light lines through the centre; 5/D points up;
/// 4/E is a disc.
#[test]
fn g3_shapes_match_the_figure() {
    let glyphs = GlyphSet::bedstead();
    let small = |code| glyphs.g3(code).small;
    let lower_left = small(0x25);
    assert_eq!(lower_left[9] & 0x800, 0x800, "2/5 fills the bottom-left corner");
    assert_eq!(lower_left[0] & 0x001, 0, "and not the top-right");
    for line in 0..10 {
        assert_eq!(small(0x25)[line] | small(0x65)[line], CELL_MASK, "2/5 and 6/5 tile the cell");
        assert_eq!(small(0x25)[line] & small(0x65)[line], 0);
    }
    assert_eq!(small(0x3F), [CELL_MASK; 10]);
    assert_eq!(small(0x50), [0b0000_0110_0000; 10]);
    assert_eq!(small(0x51)[4..6], [CELL_MASK, CELL_MASK]);
    let up = small(0x5D);
    assert!(up[0].count_ones() < up[3].count_ones(), "5/D widens downwards from its point");
    let disc = small(0x4E);
    assert!(disc[0] == 0 && disc[5] == CELL_MASK && disc[1].count_ones() < disc[4].count_ones());
}

/// REQ-RND-017: in the resolved mode the 4-bit format holds indices up to
/// 15 and refuses wider ones.
#[test]
fn four_bit_format_in_the_resolved_mode() {
    let mut grid = Grid::EMPTY;
    grid.cells[2][3] = Cell { glyph: g3(0x3F), foreground: 15, background: 9, ..Cell::BLANK };
    let glyphs = GlyphSet::bedstead();
    let mut out = vec![0u8; PAGE_WIDTH / 2];
    render_line(&grid, 2 * CELL_HEIGHT, Zoom::Off, &glyphs, large(0), IndexFormat::Bits4, &mut out).unwrap();
    assert_eq!(&out[3 * 6..4 * 6], &[0xFF; 6]);
    grid.cells[2][3].foreground = 16;
    let result = render_line(&grid, 2 * CELL_HEIGHT, Zoom::Off, &glyphs, large(0), IndexFormat::Bits4, &mut out);
    assert_eq!(result, Err(RenderError::IndexTooWide));
    // The palette mode keeps its masking, as at Level 1.
    let palette = RenderOptions { colour: ColourMode::Palette, ..large(0) };
    grid.cells[2][3].foreground = 7;
    grid.cells[2][3].background = 1;
    grid.cells[2][3].flash = true;
    assert!(render_line(&grid, 2 * CELL_HEIGHT, Zoom::Off, &glyphs, palette, IndexFormat::Bits4, &mut out).is_ok());
}

/// REQ-RND-032 (REQ-TST-018 for the counting): moving from each unit to
/// the next reports exactly the rows whose flashing cells change; a reveal
/// change reports the rows with concealed cells; a changed cell or a
/// rebuilt row reports only its own row.
#[test]
fn change_tracking_in_the_resolved_mode() {
    let mut grid = Grid::EMPTY;
    let rates = [
        (2, FlashRate::Slow, FlashMode::Normal),
        (4, FlashRate::FastPhase1, FlashMode::Normal),
        (6, FlashRate::FastPhase2, FlashMode::Inverted),
        (8, FlashRate::FastPhase3, FlashMode::AdjacentClut),
    ];
    for (row, rate, mode) in rates {
        grid.cells[row][10] = Cell {
            glyph: text(b'X'),
            foreground: RED,
            background: BLUE,
            flash: true,
            attributes: CellAttributes::DEFAULT.with_flash_rate(rate).with_flash_mode(mode),
            ..Cell::BLANK
        };
    }
    // Flashing cells that look the same in both phases: a space, and red
    // on red.
    grid.cells[12][0] = Cell { flash: true, ..Cell::BLANK };
    grid.cells[13][0] = Cell { glyph: text(b'X'), foreground: RED, background: RED, flash: true, ..Cell::BLANK };
    // A flashing concealed cell changes nothing until revealed.
    grid.cells[14][0] = Cell { glyph: text(b'X'), foreground: RED, flash: true, conceal: true, ..Cell::BLANK };
    grid.cells[16][3] = Cell { glyph: text(b'X'), conceal: true, ..Cell::BLANK };
    let mut tracker = ChangeTracker::new();
    assert_eq!(tracker.changed_rows(&grid, large(11)), ALL_ROWS);
    for unit in 0..FLASH_UNITS {
        let previous = (unit + FLASH_UNITS - 1) % FLASH_UNITS;
        let expected = rates.iter().fold(0u32, |bits, &(row, rate, _)| {
            if flash_on(rate, unit) == flash_on(rate, previous) { bits } else { bits | 1 << row }
        });
        assert_eq!(tracker.changed_rows(&grid, large(unit)), expected, "unit {previous} to {unit}");
    }
    assert_eq!(tracker.changed_rows(&grid, large(11)), 0, "same unit, nothing");
    tracker.changed_rows(&grid, large(0));
    // Reveal: the rows with concealed cells.
    let revealed = options(CellMode::Saa5050, 0, true);
    assert_eq!(tracker.changed_rows(&grid, revealed), 1 << 14 | 1 << 16);
    // A flashing concealed cell flashes once revealed.
    assert_eq!(tracker.changed_rows(&grid, options(CellMode::Saa5050, 9, true)) & 1 << 14, 1 << 14);
    tracker.changed_rows(&grid, large(0));
    // A changed cell changes only its row.
    grid.cells[20][5].glyph = text(b'D');
    assert_eq!(tracker.changed_rows(&grid, large(0)), 1 << 20, "the cell changed");
    // A side-panel cell and a row colour belong to their rows.
    grid.side_panels = SidePanels { left: 4, right: 0 };
    assert_eq!(tracker.changed_rows(&grid, large(0)), ALL_ROWS, "the page layout changed");
    grid.panel_cell_mut(9, 70).unwrap().foreground = 3;
    grid.row_colours[11] = 5;
    assert_eq!(tracker.changed_rows(&grid, large(0)), 1 << 9 | 1 << 11);
}
