//! Plain text (REQ-RND-008) and ANSI terminal output (REQ-RND-009), at
//! every level (REQ-RND-041): rows are composed with their side panels,
//! G3 characters convert as REQ-CHR-011 says, and colours come from the
//! page's Colour Map.
//!
//! Every character written passes the allow-list of REQ-SEC-010, and the
//! only escape sequences are the fixed ones built here.

pub use neoteletext_app::present::{MosaicStyle, plain};
use neoteletext_app::present::{cell_text, row_cells};
use neoteletext_core::colour::{ColourIndex, ColourMap};
use neoteletext_core::consts::PRIMARY_FULL;
use neoteletext_core::glyphs::{CharClass, class};
use neoteletext_core::level1::{Cell, Glyph, Grid};
use std::fmt::Write as _;

/// Terminal colour depth (REQ-RND-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColourDepth {
    /// 24-bit colour.
    TrueColour,
    /// The eight ANSI colours, whose numbering equals teletext's.
    Eight,
}

/// The nearest of the eight Level 1 colours to a colour (REQ-RND-041): each
/// primary on when it is at least half intensity, which is the nearest by
/// distance in each primary.
fn nearest_of_eight([red, green, blue]: [u8; 3]) -> u8 {
    const HALF: u8 = PRIMARY_FULL / 2;
    u8::from(red > HALF) | u8::from(green > HALF) << 1 | u8::from(blue > HALF) << 2
}

/// A colour as the terminal is told it: an RGB triple, or the terminal's
/// default for the picture.
fn colour_of(map: &ColourMap, index: ColourIndex, screen: ColourIndex) -> Option<[u8; 3]> {
    // Transparent cells are written in the screen colour (REQ-RND-041).
    let index = if ColourMap::is_transparent(index) { screen } else { index };
    (!ColourMap::is_transparent(index)).then(|| map.rgb888(index))
}

/// What a cell's escape sequence depends on.
// Each flag is one SGR attribute of the fixed list.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Style {
    foreground: Option<[u8; 3]>,
    background: Option<[u8; 3]>,
    flash: bool,
    underline: bool,
    bold: bool,
    italic: bool,
}

impl Style {
    /// The style of a cell, or of the screen colour where a panel has no
    /// content. Invert swaps the colours (decision E.47).
    fn of(cell: Option<&Cell>, grid: &Grid) -> Self {
        let screen = grid.screen_colour;
        let Some(cell) = cell else {
            let colour = colour_of(&grid.colour_map, screen, screen);
            return Self {
                foreground: colour,
                background: colour,
                flash: false,
                underline: false,
                bold: false,
                italic: false,
            };
        };
        let (foreground, background) = if cell.attributes.invert() {
            (cell.background, cell.foreground)
        } else {
            (cell.foreground, cell.background)
        };
        let picture = grid.mix || (grid.boxed_mode && !cell.boxed);
        let text = matches!(cell.glyph, Glyph::Char(character) if class(character) == CharClass::Text);
        let font = cell.attributes.font();
        Self {
            foreground: colour_of(&grid.colour_map, foreground, screen),
            background: if picture { None } else { colour_of(&grid.colour_map, background, screen) },
            flash: cell.flash,
            underline: text && cell.attributes.underline(),
            bold: text && font.bold,
            italic: text && font.italic,
        }
    }

    /// Writes the selection. The sequences come from a fixed set: SGR 0, 1,
    /// 3, 4, 5, 38;2, 48;2, 30 to 37, 39, 40 to 47 and 49.
    fn write(self, depth: ColourDepth, out: &mut String) {
        out.push_str("\x1b[0");
        for (on, code) in [(self.bold, ";1"), (self.italic, ";3"), (self.underline, ";4"), (self.flash, ";5")] {
            if on {
                out.push_str(code);
            }
        }
        match (depth, self.foreground) {
            (_, None) => out.push_str(";39"),
            (ColourDepth::TrueColour, Some([r, g, b])) => {
                let _ = write!(out, ";38;2;{r};{g};{b}");
            }
            (ColourDepth::Eight, Some(rgb)) => {
                let _ = write!(out, ";{}", 30 + u16::from(nearest_of_eight(rgb)));
            }
        }
        match (depth, self.background) {
            (_, None) => out.push_str(";49"),
            (ColourDepth::TrueColour, Some([r, g, b])) => {
                let _ = write!(out, ";48;2;{r};{g};{b}");
            }
            (ColourDepth::Eight, Some(rgb)) => {
                let _ = write!(out, ";{}", 40 + u16::from(nearest_of_eight(rgb)));
            }
        }
        out.push('m');
    }
}

/// The page with ANSI colour (REQ-RND-009, REQ-RND-041). Double height and
/// double width are approximated: top and left halves as normal
/// characters, bottom and right halves as background.
#[must_use]
pub fn ansi(grid: &Grid, reveal: bool, mosaics: MosaicStyle, depth: ColourDepth) -> String {
    let mut out = String::new();
    for row in 0..grid.cells.len() {
        let mut previous: Option<Style> = None;
        for cell in row_cells(grid, row) {
            let style = Style::of(cell, grid);
            if previous != Some(style) {
                style.write(depth, &mut out);
                previous = Some(style);
            }
            match cell {
                Some(cell) => cell_text(cell, grid, reveal, mosaics, &mut out),
                None => out.push(' '),
            }
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoteletext_core::charset::{CharRef, CharSet, G0, G0Set, NationalOption};
    use neoteletext_core::colour::{SidePanels, colour_index};
    use neoteletext_core::consts::COLOUR_TRANSPARENT;
    use neoteletext_core::level1::{CellAttributes, FontStyle, Width};
    use neoteletext_core::types::Rgb12;

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
        let text = ansi(&grid_with(&hostile), false, MosaicStyle::Unicode16, ColourDepth::Eight);
        let bytes = text.as_bytes();
        for (index, &byte) in bytes.iter().enumerate() {
            if byte == 0x1B {
                assert_eq!(bytes.get(index + 1), Some(&b'['), "only our own CSI sequences");
            } else {
                assert!(byte >= 0x20 || byte == b'\n', "control byte {byte:#x} written");
            }
        }
    }

    /// REQ-RND-041 with REQ-RND-033: rows are written with their side
    /// panels, left, main, right; row 0's panel area is blank.
    #[test]
    fn plain_text_composes_side_panels() {
        let mut grid = grid_with("MAIN");
        grid.side_panels = SidePanels { left: 3, right: 2 };
        let g0 = CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) });
        for (column, byte) in [(69, b'L'), (70, b'E'), (71, b'F'), (40, b'R'), (41, b'T')] {
            grid.panel_cell_mut(1, column).unwrap().glyph = Glyph::Char(CharRef::new(g0, byte));
        }
        grid.cells[0][0].glyph = Glyph::Char(CharRef::new(g0, b'0'));
        let text = plain(&grid, false, MosaicStyle::Unicode16);
        let expected = format!("LEFMAIN{}RT", " ".repeat(36));
        assert_eq!(text.lines().nth(1), Some(expected.as_str()));
        assert_eq!(text.lines().next(), Some("   0"));
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

    /// REQ-RND-041: 24-bit colours come from the page's Colour Map, the
    /// 8-colour mode takes the nearest Level 1 colour, transparent cells
    /// take the screen colour, invert swaps the colours, and underline,
    /// bold and italic use their SGR codes on text.
    #[test]
    fn terminal_colours_and_attributes() {
        let mut grid = grid_with("A");
        grid.colour_map.entries[usize::from(colour_index(2, 3))] = Rgb12::new(15, 8, 1);
        grid.screen_colour = colour_index(3, 2);
        let cell = &mut grid.cells[1][0];
        cell.foreground = colour_index(2, 3);
        cell.background = COLOUR_TRANSPARENT;
        let screen = grid.colour_map.rgb888(colour_index(3, 2));
        let text = ansi(&grid, false, MosaicStyle::Unicode16, ColourDepth::TrueColour);
        let line = text.lines().nth(1).unwrap();
        let expected = format!("\x1b[0;38;2;255;136;17;48;2;{};{};{}mA", screen[0], screen[1], screen[2]);
        assert!(line.starts_with(&expected), "{line:?}");
        let eight = ansi(&grid, false, MosaicStyle::Unicode16, ColourDepth::Eight);
        let screen_eight = 40 + u16::from(nearest_of_eight(screen));
        assert!(eight.lines().nth(1).unwrap().starts_with(&format!("\x1b[0;33;{screen_eight}mA")));
        // Invert, underline and the font styles.
        let cell = &mut grid.cells[1][0];
        cell.foreground = 1;
        cell.background = 4;
        cell.attributes = CellAttributes::DEFAULT.with_invert(true).with_underline(true).with_font(FontStyle {
            bold: true,
            italic: true,
            proportional: false,
        });
        let text = ansi(&grid, false, MosaicStyle::Unicode16, ColourDepth::Eight);
        assert!(text.lines().nth(1).unwrap().starts_with("\x1b[0;1;3;4;34;41mA"));
        // A side panel's empty row 0 is the screen colour.
        grid.side_panels = SidePanels { left: 1, right: 0 };
        let text = ansi(&grid, false, MosaicStyle::Unicode16, ColourDepth::Eight);
        let screen_fg = 30 + u16::from(nearest_of_eight(screen));
        assert!(text.starts_with(&format!("\x1b[0;{screen_fg};{screen_eight}m ")), "{text:?}");
    }

    /// The nearest Level 1 colour of every Table 30 entry of CLUT 0 is the
    /// entry itself.
    #[test]
    fn nearest_colour_keeps_clut_0() {
        for index in 0..8u8 {
            assert_eq!(nearest_of_eight(ColourMap::DEFAULT.rgb888(index)), index);
        }
        assert_eq!(nearest_of_eight(ColourMap::DEFAULT.rgb888(colour_index(1, 1))), 0, "half red is dark");
    }
}
