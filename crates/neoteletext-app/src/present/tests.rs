use super::*;
use neoteletext_core::charset::{CharRef, CharSet, G0, G0Set, NationalOption};
use neoteletext_core::consts::{ALPHA_YELLOW, COLUMNS, ROWS, SPACE, STATUS_COLUMNS};
use neoteletext_core::level1::CellAttributes;

fn grid_with(text: &str) -> Grid {
    let mut grid = Grid::EMPTY;
    let g0 = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) });
    for (cell, byte) in grid.cells[1].iter_mut().zip(text.bytes()) {
        cell.glyph = Glyph::Char(CharRef::new(g0, byte));
    }
    grid
}

#[test]
fn plain_text_has_25_lines() {
    let text = plain(&grid_with("HELLO"), false, MosaicStyle::Unicode16);
    assert_eq!(text.lines().count(), 25);
    assert_eq!(text.lines().nth(1), Some("HELLO"));
}

#[test]
fn concealed_text_needs_reveal() {
    let mut grid = grid_with("SECRET");
    for cell in &mut grid.cells[1] {
        cell.conceal = true;
    }
    assert_eq!(plain(&grid, false, MosaicStyle::Unicode16).lines().nth(1), Some(""));
    assert_eq!(plain(&grid, true, MosaicStyle::Unicode16).lines().nth(1), Some("SECRET"));
}

/// REQ-RND-041, REQ-CHR-011: G3 characters as L2/19-025 maps them, and
/// the right half of a double-width character as a space.
#[test]
fn g3_and_double_width_in_text() {
    let mut grid = Grid::EMPTY;
    grid.cells[2][0].glyph = Glyph::Char(CharRef::new(CharSet::G3, 0x20));
    grid.cells[2][1].glyph = Glyph::Char(CharRef::new(CharSet::G3, 0x5B));
    grid.cells[2][2].glyph = Glyph::Char(CharRef::new(CharSet::G3, 0x2F));
    let g0 = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) });
    let wide = |width| Cell {
        glyph: Glyph::Char(CharRef::new(g0, b'W')),
        attributes: CellAttributes::DEFAULT.with_width(width),
        ..Cell::BLANK
    };
    grid.cells[2][4] = wide(Width::LeftHalf);
    grid.cells[2][5] = wide(Width::RightHalf);
    grid.cells[2][6].glyph = Glyph::Char(CharRef::new(g0, b'!'));
    let text = plain(&grid, false, MosaicStyle::Unicode16);
    assert_eq!(text.lines().nth(2), Some("\u{1FB3C}\u{2B62}\u{2592} W !"));
}

#[test]
fn the_receivers_rows_read_back_as_written() {
    let mut rows = [[SPACE; COLUMNS]; ROWS];
    rows[0][8..19].copy_from_slice(b" example.or");
    rows[3][..6].copy_from_slice(&[ALPHA_YELLOW, b'H', b'E', b'L', b'L', b'O']);
    let mut status = [SPACE; STATUS_COLUMNS];
    status[..4].copy_from_slice(b"P100");
    let text = plain(&receiver_grid(&rows, status), false, MosaicStyle::Unicode16);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].starts_with("P100") && lines[0].contains("example.or"), "{text}");
    assert_eq!(lines[3].trim(), "HELLO");
}
