//! Tests of the renderer: rounding, mosaics, the palette, row independence
//! and change tracking. The tests of the rendering of Levels 2.5 and 3.5
//! and of loaded fonts are in tests/level25.rs and tests/fonts.rs.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

// The module is compiled for tests only (see the parent file); the
// attribute also marks this file as test code for `cargo xtask check-layers`.
#[cfg(test)]
use super::*;

use crate::config::{CellMode, Config};
use crate::consts::{PALETTE_CONCEAL_BASE, PALETTE_FLASH_BASE, PALETTE_SIZE, PALETTE_TRANSPARENT};
use crate::glyphs::GlyphSet;
use crate::level1::{Cell, Glyph, Grid, Zoom};
use crate::types::Instant;

use super::raster::{CELL_MASK, SEPARATED_COLUMNS, font_line_20, glyph_line_20, mosaic_block, mosaic_line, widen};
use super::row::PANEL_AREA_END;
use crate::charset::{CharRef, CharSet, G0, G0Set, NationalOption};
use crate::consts::{COLOUR_BLUE_BIT, COLOUR_RED_BIT, PAGE_WIDTH};
use std::vec;
use std::vec::Vec;

fn options() -> RenderOptions {
    RenderOptions {
        mode: CellMode::Saa5050,
        rounding: true,
        reveal: false,
        colour: ColourMode::Palette,
        underline: true,
    }
}

#[test]
fn rounding_matches_decision_e13() {
    // A diagonal: row 0 has the dot at bit 0, row 1 at bit 1.
    let glyph = [0b00001, 0b00010, 0, 0, 0, 0, 0, 0, 0];
    // Lower sub-line of glyph row 0 rounds against row 1.
    let lower = glyph_line_20(&glyph, 3, true);
    assert_eq!(lower, widen(0b00001) | (widen(0b00001) << 1) & widen(0b00010) & !(widen(0b00010) << 1));
    // Upper sub-line of row 0 has no neighbour above: no rounding.
    assert_eq!(glyph_line_20(&glyph, 2, true), widen(0b00001));
    assert_eq!(glyph_line_20(&glyph, 3, false), widen(0b00001));
    // Cell line 0 is blank.
    assert_eq!(glyph_line_20(&glyph, 0, true), 0);
}

/// REQ-RND-013: decision E.14's geometry in both cell sizes.
#[test]
fn separated_mosaic_geometry_matches_e14() {
    let full = 0x3F;
    let blank_lines: Vec<usize> = (0..20).filter(|&l| mosaic_line(full, true, l, 20) == 0).collect();
    assert_eq!(blank_lines, vec![4, 5, 12, 13, 18, 19]);
    assert_eq!(mosaic_line(full, true, 0, 20), !SEPARATED_COLUMNS & CELL_MASK);
    let small: Vec<usize> = (0..10).filter(|&l| mosaic_line(full, true, l, 10) == 0).collect();
    assert_eq!(small, vec![2, 6, 9]);
    // Contiguous blocks: 6, 8 and 6 lines.
    assert_eq!(mosaic_block(5, 20).0, 0);
    assert_eq!(mosaic_block(6, 20).0, 1);
    assert_eq!(mosaic_block(13, 20).0, 1);
    assert_eq!(mosaic_block(14, 20).0, 2);
}

#[test]
fn palette_flash_and_reveal() {
    let mut palette = [0u8; PALETTE_SIZE];
    let base = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    fill_palette(&mut palette, &base, true, false);
    let flash = usize::from(PALETTE_FLASH_BASE) + 8 * 3 + 4;
    let conceal = usize::from(PALETTE_CONCEAL_BASE) + 8 * 3 + 4;
    assert_eq!((palette[flash], palette[conceal]), (3, 4));
    fill_palette(&mut palette, &base, false, true);
    assert_eq!((palette[flash], palette[conceal]), (4, 3));
    assert_eq!(palette[usize::from(PALETTE_TRANSPARENT)], 8);
}

/// REQ-RND-021's test harness: rows drawn alone, in reverse order and
/// line by line match the whole-screen loop.
#[test]
fn rows_render_identically_in_any_order() {
    let mut grid = Grid::EMPTY;
    let g0 = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) });
    grid.cells[2][3].glyph = Glyph::Char(CharRef::new(g0, b'A'));
    grid.cells[2][4].glyph = Glyph::Mosaic { pattern: 0x15, separated: false };
    let glyphs = GlyphSet::bedstead();
    let mut whole = vec![0u8; PAGE_WIDTH * 500];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut whole, stride: PAGE_WIDTH };
    render_screen(&grid, ALL_ROWS, &glyphs, options(), &mut buffer).unwrap();
    for row in (0..ROWS).rev() {
        let mut single = vec![0u8; PAGE_WIDTH * 20];
        let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut single, stride: PAGE_WIDTH };
        render_row(&grid, row, Field::Both, &glyphs, options(), &mut buffer).unwrap();
        assert_eq!(&whole[row * 20 * PAGE_WIDTH..(row + 1) * 20 * PAGE_WIDTH], &single[..]);
    }
    for y in 0..500 {
        let mut line = vec![0u8; PAGE_WIDTH];
        render_line(&grid, y, Zoom::Off, &glyphs, options(), IndexFormat::Bits8, &mut line).unwrap();
        assert_eq!(&whole[y * PAGE_WIDTH..(y + 1) * PAGE_WIDTH], &line[..]);
    }
}

#[test]
fn change_tracking_reports_only_changed_rows() {
    let mut tracker = ChangeTracker::new();
    let mut grid = Grid::EMPTY;
    assert_eq!(tracker.changed_rows(&grid, options()), ALL_ROWS);
    assert_eq!(tracker.changed_rows(&grid, options()), 0);
    grid.cells[5][0].foreground = 3;
    assert_eq!(tracker.changed_rows(&grid, options()), 1 << 5);
    grid.cells[7][0].flash = true;
    grid.cells[7][0].conceal = true;
    tracker.changed_rows(&grid, options());
    let revealed = RenderOptions { reveal: true, ..options() };
    assert_eq!(tracker.changed_rows(&grid, revealed), 1 << 7, "reveal redraws flashing concealed rows only");
}

/// REQ-L1-009, REQ-L1-010, REQ-L1-011: what the flash, conceal and box
/// marks of a cell do to its pixels.
#[test]
fn flash_conceal_and_box_marks_drive_the_palette() {
    // Colours 1 and 4 are the single-gun colours.
    let (red, blue) = (COLOUR_RED_BIT, COLOUR_BLUE_BIT);
    let base = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    let mut grid = Grid::EMPTY;
    let plain = Cell { foreground: red, background: blue, ..Cell::BLANK };
    // Flash: foreground pixels take the background colour in the hidden
    // phase.
    let index = usize::from(foreground_index(&Cell { flash: true, ..plain }, &grid, false));
    for (visible, colour) in [(true, red), (false, blue)] {
        let mut palette = [0u8; PALETTE_SIZE];
        fill_palette(&mut palette, &base, visible, false);
        assert_eq!(palette[index], colour, "flash visible {visible}");
    }
    // Conceal: shown as a space on its background unless revealed.
    let index = usize::from(foreground_index(&Cell { conceal: true, ..plain }, &grid, false));
    for (reveal, colour) in [(false, blue), (true, red)] {
        let mut palette = [0u8; PALETTE_SIZE];
        fill_palette(&mut palette, &base, true, reveal);
        assert_eq!(palette[index], colour, "reveal {reveal}");
    }
    // Box membership changes nothing outside boxed mode (decision E.22).
    let boxed = Cell { boxed: true, ..plain };
    assert_eq!((foreground_index(&plain, &grid, false), background_index(&plain, &grid)), (red, blue));
    assert_eq!((foreground_index(&boxed, &grid, false), background_index(&boxed, &grid)), (red, blue));
    grid.boxed_mode = true;
    let picture = (PALETTE_TRANSPARENT, PALETTE_TRANSPARENT);
    assert_eq!((foreground_index(&plain, &grid, false), background_index(&plain, &grid)), picture);
    assert_eq!((foreground_index(&boxed, &grid, false), background_index(&boxed, &grid)), (red, blue));
}

#[test]
fn flash_timing_is_three_to_one() {
    let config = Config::default();
    assert!(flash_visible(Instant(0), &config));
    assert!(flash_visible(Instant(959_999), &config));
    assert!(!flash_visible(Instant(960_000), &config));
    assert!(flash_visible(Instant(1_280_000), &config));
}

// The tests draw with the shipped glyph set.
mod level25;
// Loaded fonts fall back to the shipped glyph set.
mod fonts;
