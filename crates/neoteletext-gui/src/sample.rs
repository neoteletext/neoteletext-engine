//! A font's sample (REQ-BRW-010, decision E.74 item 7): every glyph of the
//! font in code-point order, then sample text where the rows leave room.
//! Drawn by the interface from the checked glyph table, never through a
//! decoder: a loaded font may hold glyphs no teletext character set
//! reaches, so a teletext page could not show them all.

use crate::picture::Picture;
use neoteletext_core::consts::{CELL_FIELD_LINES, CELL_HEIGHT, CELL_WIDTH, COLUMNS, GLYPH_DOTS, ROWS};
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
