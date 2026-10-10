//! Plain text (REQ-RND-008) and ANSI terminal output (REQ-RND-009).
//!
//! Every character written passes the allow-list of REQ-SEC-010, and the
//! only escape sequences are the fixed ones built here.

use std::fmt::Write as _;
use tellytxt_core::charset::{compose, mosaic_to_unicode, to_unicode};
use tellytxt_core::consts::COLOURS;
use tellytxt_core::level1::{Cell, Glyph, Grid, Size};
use tellytxt_core::policy::is_safe_output_char;
use tellytxt_image::palette::Rgba;

/// How mosaics are written.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MosaicStyle {
    /// Separated mosaics as the Unicode 16 separated sextants (REQ-CHR-005).
    Unicode16,
    /// Separated mosaics drawn with the contiguous forms, for older fonts.
    Legacy,
}

/// Whether a cell's foreground is visible at all.
fn visible(cell: &Cell, grid: &Grid, reveal: bool) -> bool {
    !(grid.boxed_mode && !cell.boxed) && !(cell.conceal && !reveal) && cell.size != Size::DoubleBottom
}

/// The text of one cell: a base character and an optional combining mark,
/// precomposed where Unicode allows (decision E.26).
fn cell_text(cell: &Cell, grid: &Grid, reveal: bool, mosaics: MosaicStyle, out: &mut String) {
    if !visible(cell, grid, reveal) {
        out.push(' ');
        return;
    }
    match cell.glyph {
        Glyph::Space => out.push(' '),
        Glyph::Mosaic { pattern, separated } => {
            out.push(mosaic_to_unicode(pattern, separated && mosaics == MosaicStyle::Unicode16));
        }
        Glyph::Char(character) => {
            let unicode = to_unicode(character);
            let base = if is_safe_output_char(unicode.base) { unicode.base } else { ' ' };
            match unicode.combining {
                Some(mark) => match compose(base, mark) {
                    Some(precomposed) => out.push(precomposed),
                    None => {
                        out.push(base);
                        if is_safe_output_char(mark) {
                            out.push(mark);
                        }
                    }
                },
                None => out.push(base),
            }
        }
    }
}

/// The page as 25 lines of UTF-8 (REQ-RND-008).
#[must_use]
pub fn plain(grid: &Grid, reveal: bool, mosaics: MosaicStyle) -> String {
    let mut out = String::new();
    for row in &grid.cells {
        let mut line = String::new();
        for cell in row {
            cell_text(cell, grid, reveal, mosaics, &mut line);
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// Terminal colour depth (REQ-RND-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColourDepth {
    /// 24-bit colour.
    TrueColour,
    /// The eight ANSI colours, whose numbering equals teletext's.
    Eight,
}

/// Writes the colour selection for a cell. The sequences come from a fixed
/// set: SGR 0, 5, 38;2, 48;2, 30 to 37, 40 to 47 and 49.
fn colours(cell: &Cell, grid: &Grid, depth: ColourDepth, palette: &[Rgba; COLOURS], out: &mut String) {
    let transparent = grid.mix || (grid.boxed_mode && !cell.boxed);
    let foreground = palette.get(usize::from(cell.foreground)).copied().unwrap_or_default();
    let background = palette.get(usize::from(cell.background)).copied().unwrap_or_default();
    out.push_str("\x1b[0");
    if cell.flash {
        out.push_str(";5");
    }
    match depth {
        ColourDepth::TrueColour => {
            let _ = write!(out, ";38;2;{};{};{}", foreground.r, foreground.g, foreground.b);
            if transparent {
                out.push_str(";49");
            } else {
                let _ = write!(out, ";48;2;{};{};{}", background.r, background.g, background.b);
            }
        }
        ColourDepth::Eight => {
            let _ = write!(out, ";{}", 30 + u16::from(cell.foreground & 7));
            if transparent {
                out.push_str(";49");
            } else {
                let _ = write!(out, ";{}", 40 + u16::from(cell.background & 7));
            }
        }
    }
    out.push('m');
}

/// The page with ANSI colour (REQ-RND-009). Double height is approximated:
/// top halves as normal characters, bottom halves as background.
#[must_use]
pub fn ansi(grid: &Grid, reveal: bool, mosaics: MosaicStyle, depth: ColourDepth, palette: &[Rgba; COLOURS]) -> String {
    let mut out = String::new();
    for row in &grid.cells {
        let mut previous: Option<(u8, u8, bool, bool)> = None;
        for cell in row {
            let key = (cell.foreground, cell.background, cell.flash, grid.boxed_mode && !cell.boxed);
            if previous != Some(key) {
                colours(cell, grid, depth, palette, &mut out);
                previous = Some(key);
            }
            cell_text(cell, grid, reveal, mosaics, &mut out);
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tellytxt_core::charset::{CharRef, CharSet, G0, G0Set, NationalOption};

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

    #[test]
    fn page_control_codes_never_reach_the_terminal() {
        // Every control code a page can carry, in every cell.
        let hostile: String = (0u8..0x20).map(char::from).chain("\u{7f}".chars()).collect();
        let text = ansi(&grid_with(&hostile), false, MosaicStyle::Unicode16, ColourDepth::Eight, &[Rgba::default(); 8]);
        let bytes = text.as_bytes();
        for (index, &byte) in bytes.iter().enumerate() {
            if byte == 0x1B {
                assert_eq!(bytes.get(index + 1), Some(&b'['), "only our own CSI sequences");
            } else {
                assert!(byte >= 0x20 || byte == b'\n', "control byte {byte:#x} written");
            }
        }
    }
}
