//! A font's sample (REQ-BRW-010, decision E.74 item 7): every glyph of the
//! font in code-point order, then sample text where the rows leave room.
//! Drawn by the interface from the checked glyph table, never through a
//! decoder: a loaded font may hold glyphs no teletext character set
//! reaches, so a teletext page could not show them all.

use crate::picture::Picture;
use neoteletext_core::consts::{CELL_FIELD_LINES, CELL_HEIGHT, CELL_WIDTH, COLUMNS, GLYPH_DOTS, ROWS};
use neoteletext_core::charset::{CharSet, G0, G0Set};
use neoteletext_core::glyphs::conformance::assigned;
use neoteletext_core::glyphs::{GlyphEntry, LoadedFont, PLACEHOLDER};
use neoteletext_core::render::font_line_20;
use neoteletext_core::types::FontGlyph;

/// Red, green, blue and alpha.
const RGBA: usize = 4;
/// The glyphs' colour.
const INK: [u8; RGBA] = [0xFF, 0xFF, 0xFF, 0xFF];
/// The sample text's colour.
const TEXT_INK: [u8; RGBA] = [0xFF, 0xFF, 0x00, 0xFF];
/// A missing character's placeholder, dimmed.
const MISSING_INK: [u8; RGBA] = [0x60, 0x60, 0x60, 0xFF];
/// Alternate cell backgrounds, so each glyph's extent shows.
const PAPER: [[u8; RGBA]; 2] = [[0x00, 0x00, 0x00, 0xFF], [0x18, 0x18, 0x30, 0xFF]];
/// Share of a line's letters the font must draw for the line to be shown.
const COVERAGE_PERCENT: usize = 90;
/// Sample lines, each shown when the font draws most of its letters.
const SAMPLES: [&str; 9] = [
    "The quick brown fox jumps over the lazy dog",
    "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG",
    "0123456789 !\"#$%&'()*+,-./:;<=>?@[\\]^_{|}~",
    "Съешь же ещё этих мягких французских булок",
    "СЪЕШЬ ЖЕ ЕЩЁ ЭТИХ МЯГКИХ ФРАНЦУЗСКИХ БУЛОК",
    "Ξεσκεπάζω την ψυχοφθόρα βδελυγμία",
    "ΞΕΣΚΕΠΑΖΩ ΤΗΝ ΨΥΧΟΦΘΟΡΑ ΒΔΕΛΥΓΜΙΑ",
    "דג סקרן שט בים מאוכזב ולפתע מצא חברה",
    "£ ½ ¼ ¾ ÷ ← → ↑ ↓ ‖ ■",
];

/// Bedstead's glyphs in the 12 by 10 form of a loaded font: below the
/// cell's blank top line, each dot two pixels wide with the leftmost dot
/// column blank, as the renderer draws them.
#[must_use]
pub fn bedstead_glyphs(entries: &[GlyphEntry]) -> Vec<FontGlyph> {
    entries.iter().map(|entry| FontGlyph { code_point: entry.code_point, rows: twelve_by_ten(&entry.rows) }).collect()
}

fn twelve_by_ten(rows: &[u8]) -> [u16; CELL_FIELD_LINES] {
    let mut out = [0u16; CELL_FIELD_LINES];
    for (line, &dots) in out.iter_mut().skip(1).zip(rows) {
        *line = (0..GLYPH_DOTS).filter(|dot| (dots >> dot) & 1 == 1).fold(0, |acc, dot| acc | (0b11 << (dot * 2)));
    }
    out
}

/// What one cell of the sample shows.
#[derive(Clone, Copy)]
struct Cell {
    rows: [u16; CELL_FIELD_LINES],
    ink: [u8; RGBA],
}

/// The sample of a font whose glyphs are sorted by code point, drawn as
/// its pages are: a doubled dot matrix rounded (REQ-FNT-008, issue #209).
#[must_use]
pub fn sample(glyphs: &[FontGlyph]) -> Picture {
    let rounding = LoadedFont::new(glyphs).is_ok_and(|font| font.is_dot_matrix());
    let mut lines: Vec<Vec<Cell>> = glyphs
        .chunks(COLUMNS)
        .map(|row| row.iter().map(|glyph| Cell { rows: glyph.rows, ink: INK }).collect())
        .collect();
    let text = sample_text(glyphs);
    // The text only where the glyphs leave room in the page's rows.
    if !text.is_empty() && lines.len() + 1 + text.len() <= ROWS {
        lines.push(Vec::new());
        lines.extend(text);
    }
    let rows = lines.len().max(ROWS);
    let width = COLUMNS * CELL_WIDTH;
    let height = rows * CELL_HEIGHT;
    let mut rgba = vec![0u8; width * height * RGBA];
    for (row, cells) in lines.iter().enumerate() {
        for (column, cell) in cells.iter().enumerate() {
            let paper = PAPER[(row + column) % PAPER.len()];
            for line in 0..CELL_HEIGHT {
                let bits = font_line_20(&cell.rows, line, rounding);
                for x in 0..CELL_WIDTH {
                    let lit = (bits >> (CELL_WIDTH - 1 - x)) & 1 == 1;
                    let at = ((row * CELL_HEIGHT + line) * width + column * CELL_WIDTH + x) * RGBA;
                    if let Some(pixel) = rgba.get_mut(at..at + RGBA) {
                        pixel.copy_from_slice(if lit { &cell.ink } else { &paper });
                    }
                }
            }
        }
    }
    Picture { width, height, rgba }
}

/// The sample lines the font draws most of, wrapped at word boundaries to
/// the page's width.
fn sample_text(glyphs: &[FontGlyph]) -> Vec<Vec<Cell>> {
    let find = |character: char| {
        glyphs.binary_search_by_key(&u32::from(character), |glyph| glyph.code_point).ok().map(|at| glyphs[at].rows)
    };
    let placeholder = twelve_by_ten(&PLACEHOLDER);
    let mut out = Vec::new();
    for sample in SAMPLES {
        let letters: Vec<char> = sample.chars().filter(|c| !c.is_whitespace()).collect();
        let drawn = letters.iter().filter(|&&c| find(c).is_some()).count();
        if drawn * 100 < letters.len() * COVERAGE_PERCENT {
            continue;
        }
        for line in wrap(sample) {
            out.push(
                line.chars()
                    .map(|c| match (c, find(c)) {
                        (' ', _) => Cell { rows: [0; CELL_FIELD_LINES], ink: TEXT_INK },
                        (_, Some(rows)) => Cell { rows, ink: TEXT_INK },
                        (_, None) => Cell { rows: placeholder, ink: MISSING_INK },
                    })
                    .collect(),
            );
        }
    }
    out
}

/// A sample line broken at spaces into lines of at most the page's width.
fn wrap(text: &str) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split(' ') {
        let current = lines.last_mut().map_or(0, |line| line.chars().count());
        if current > 0 && current + 1 + word.chars().count() > COLUMNS {
            lines.push(String::new());
        }
        if let Some(line) = lines.last_mut() {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    lines
}

/// One character set's grid: its name, the set, and the glyphs that fill
/// it (the chosen font's, or Bedstead's).
pub struct GridSet<'a> {
    /// The heading.
    pub name: &'a str,
    /// The character set.
    pub set: CharSet,
    /// The glyphs, sorted by code point.
    pub glyphs: &'a [FontGlyph],
}

/// The first and last code positions of a set, 2/0 and 7/F.
const SPACE_CODE: u8 = 0x20;
const LAST_CODE: u8 = 0x7F;
/// Paper behind a position the standard leaves unassigned.
const UNASSIGNED_PAPER: [u8; RGBA] = [0x10, 0x10, 0x10, 0xFF];
/// Paper behind a Latin national option position.
const NATIONAL_PAPER: [u8; RGBA] = [0x00, 0x30, 0x50, 0xFF];
/// Paper behind a hole: an assigned position the font lacks.
const HOLE_PAPER: [u8; RGBA] = [0x50, 0x00, 0x00, 0xFF];
/// Labels' colour.
const LABEL_INK: [u8; RGBA] = [0x00, 0xFF, 0xFF, 0xFF];
/// Columns of one grid: a row label, then three for each code column.
const GRID_COLUMNS: usize = COLUMNS / 2;
/// The code columns 2 to 7 and rows 0 to F of a character set.
const CODE_COLUMNS: u8 = 6;
const CODE_ROWS: u8 = 16;
const FIRST_COLUMN: u8 = 2;

/// A cell to draw at a row and column of the picture.
struct Placed {
    row: usize,
    column: usize,
    rows: [u16; CELL_FIELD_LINES],
    ink: [u8; RGBA],
    paper: [u8; RGBA],
    rounding: bool,
}

/// The 96 positions of one or two character sets as the standard draws its
/// tables (REQ-BRW-010, decision E.75 item 4): columns 2 to 7, rows 0 to F,
/// each position in the set's font, with holes, unassigned positions and
/// national option positions marked, and sample text below where it fits.
#[must_use]
pub fn grid(left: &GridSet<'_>, right: Option<&GridSet<'_>>, rounding_on: bool) -> Picture {
    let bedstead = bedstead_glyphs(neoteletext_core::glyphs::GlyphSet::bedstead().entries());
    let mut cells: Vec<Placed> = Vec::new();
    let text = |row: usize, column: usize, words: &str, ink: [u8; RGBA], cells: &mut Vec<Placed>| {
        for (offset, character) in words.chars().enumerate() {
            let at = column + offset;
            if at >= COLUMNS {
                break;
            }
            if let Some(rows) = find(&bedstead, character) {
                cells.push(Placed { row, column: at, rows, ink, paper: PAPER[0], rounding: true });
            }
        }
    };
    let mut left_rounding = false;
    for (index, part) in std::iter::once(left).chain(right).enumerate() {
        let origin = index * GRID_COLUMNS;
        // A dot matrix's rows are all pixel pairs (REQ-FNT-008); counted
        // here, since Bedstead's table is longer than a loaded font's.
        let pair_bits = 0b0101_0101_0101u16;
        let rounding = rounding_on
            && part.glyphs.iter().all(|glyph| glyph.rows.iter().all(|row| (row ^ (row >> 1)) & pair_bits == 0));
        if index == 0 {
            left_rounding = rounding;
        }
        let space_blank = find(part.glyphs, ' ').is_some_and(|rows| rows.iter().all(|&row| row == 0));
        let drawn = (SPACE_CODE..=LAST_CODE)
            .filter(|&code| match assigned(part.set, code) {
                None | Some(' ') => space_blank,
                Some(character) => find(part.glyphs, character).is_some_and(|rows| rows.iter().any(|&row| row != 0)),
            })
            .count();
        let (set_name, font_name) = part.name.split_once(' ').unwrap_or((part.name, ""));
        let heading: String = set_name.chars().take(GRID_COLUMNS - 1).collect();
        text(0, origin, &heading, INK, &mut cells);
        let line: String = format!("{drawn}/96 {font_name}").chars().take(GRID_COLUMNS - 1).collect();
        text(1, origin, &line, if drawn == 96 { INK } else { MISSING_INK }, &mut cells);
        for column in 0..CODE_COLUMNS {
            let label = char::from_digit(u32::from(FIRST_COLUMN + column), 16).unwrap_or('?');
            text(2, origin + 2 + usize::from(column) * 3, &label.to_string(), LABEL_INK, &mut cells);
        }
        let base = CharSet::G0(G0 { set: G0Set::Latin, national: None });
        let national = matches!(part.set, CharSet::G0(G0 { set: G0Set::Latin, national: Some(_) }));
        for row in 0..CODE_ROWS {
            let label = char::from_digit(u32::from(row), 16).unwrap_or('?').to_ascii_uppercase();
            text(3 + usize::from(row), origin, &label.to_string(), LABEL_INK, &mut cells);
            for column in 0..CODE_COLUMNS {
                let code = (FIRST_COLUMN + column) * CODE_ROWS + row;
                let at = origin + 2 + usize::from(column) * 3;
                let (rows, ink, paper) = match assigned(part.set, code) {
                    None => (dotted(), MISSING_INK, UNASSIGNED_PAPER),
                    Some(character) => {
                        let paper = if national && assigned(base, code) != Some(character) { NATIONAL_PAPER } else { PAPER[1] };
                        match find(part.glyphs, character) {
                            Some(rows) => (rows, INK, paper),
                            None => (twelve_by_ten(&PLACEHOLDER), MISSING_INK, HOLE_PAPER),
                        }
                    }
                };
                cells.push(Placed { row: 3 + usize::from(row), column: at, rows, ink, paper, rounding });
            }
        }
    }
    // Sample text in the left set's font, in the rows below the grids.
    let first_text_row = 3 + usize::from(CODE_ROWS) + 1;
    for (offset, line) in sample_text(left.glyphs).into_iter().take(ROWS - first_text_row).enumerate() {
        for (column, cell) in line.into_iter().enumerate() {
            cells.push(Placed {
                row: first_text_row + offset,
                column,
                rows: cell.rows,
                ink: cell.ink,
                paper: PAPER[0],
                rounding: left_rounding,
            });
        }
    }
    paint(&cells)
}

/// A glyph by character.
fn find(glyphs: &[FontGlyph], character: char) -> Option<[u16; CELL_FIELD_LINES]> {
    glyphs.binary_search_by_key(&u32::from(character), |glyph| glyph.code_point).ok().and_then(|at| glyphs.get(at)).map(|glyph| glyph.rows)
}

/// A faint dot in the middle of the cell: a position the standard leaves
/// unassigned.
fn dotted() -> [u16; CELL_FIELD_LINES] {
    let mut rows = [0u16; CELL_FIELD_LINES];
    rows[CELL_FIELD_LINES / 2] = 0b0000_0110_0000;
    rows
}

/// Paints placed cells on a black page of 40 by 25 cells.
fn paint(cells: &[Placed]) -> Picture {
    let width = COLUMNS * CELL_WIDTH;
    let height = ROWS * CELL_HEIGHT;
    let mut rgba = vec![0u8; width * height * RGBA];
    for chunk in rgba.chunks_exact_mut(RGBA) {
        chunk.copy_from_slice(&PAPER[0]);
    }
    for cell in cells {
        for line in 0..CELL_HEIGHT {
            let bits = font_line_20(&cell.rows, line, cell.rounding);
            for x in 0..CELL_WIDTH {
                let lit = (bits >> (CELL_WIDTH - 1 - x)) & 1 == 1;
                let at = ((cell.row * CELL_HEIGHT + line) * width + cell.column * CELL_WIDTH + x) * RGBA;
                if let Some(pixel) = rgba.get_mut(at..at + RGBA) {
                    pixel.copy_from_slice(if lit { &cell.ink } else { &cell.paper });
                }
            }
        }
    }
    Picture { width, height, rgba }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn glyph(code_point: u32, row: u16) -> FontGlyph {
        FontGlyph { code_point, rows: [row; CELL_FIELD_LINES] }
    }

    /// Every glyph is drawn, 40 to a row, in code-point order.
    #[test]
    fn every_glyph_is_drawn() {
        let glyphs: Vec<FontGlyph> = (0..41).map(|n| glyph(0x2000 + n, 0x800)).collect();
        let picture = sample(&glyphs);
        assert_eq!((picture.width, picture.height), (COLUMNS * CELL_WIDTH, ROWS * CELL_HEIGHT));
        let pixel = |x: usize, y: usize| &picture.rgba[(y * picture.width + x) * RGBA..][..RGBA];
        assert_eq!(pixel(0, 0), INK, "the first glyph's leftmost pixel");
        assert_eq!(pixel(39 * CELL_WIDTH, 0), INK, "the 40th glyph ends the first row");
        assert_eq!(pixel(0, CELL_HEIGHT), INK, "the 41st starts the second");
        assert_eq!(pixel(1, 0), PAPER[0], "unlit pixels show the cell's paper");
    }

    /// The sample text needs the font's letters and room below the glyphs.
    #[test]
    fn sample_text_where_there_is_room() {
        let latin: Vec<FontGlyph> = (0x20..0x7F).map(|c| glyph(c, 0x0F0)).collect();
        let text = sample_text(&latin);
        assert_eq!(text.len(), 6, "two lines each for the two pangrams and for the digits and signs");
        assert!(sample_text(&[glyph(0x41, 0)]).is_empty(), "too few letters for any line");
        // 1,000 glyphs fill 25 rows, leaving no room for text.
        let many: Vec<FontGlyph> = (0..1000).map(|n| glyph(0x20 + n, 0x0F0)).collect();
        assert_eq!(sample(&many).height, ROWS * CELL_HEIGHT);
        assert_eq!(
            wrap("The quick brown fox jumps over the lazy dog"),
            ["The quick brown fox jumps over the lazy", "dog"]
        );
    }

    /// Bedstead's glyphs sit below the blank top line, the leftmost dot
    /// column blank.
    #[test]
    fn bedstead_as_twelve_by_ten() {
        let rows = twelve_by_ten(&[0b1_0001, 0b1_1111]);
        assert_eq!(rows[0], 0, "the blank top line");
        assert_eq!(rows[1], 0b11_0000_0011);
        assert_eq!(rows[2], 0b11_1111_1111);
        assert_eq!(rows[1] >> 10, 0, "the leftmost dot column is blank");
    }
}
