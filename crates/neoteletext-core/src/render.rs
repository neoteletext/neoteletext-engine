//! Rendering (area RND, layer L4).
//!
//! All teletext-specific pixel logic is in [`cell_line`], the cell raster of
//! REQ-RND-016. The row renderer of REQ-RND-017 draws indices into buffers
//! the caller supplies; the library never pushes pixels and keeps no pixel
//! memory (decision E.29).
//!
//! There are two colour modes (decision E.55):
//!
//! - **The palette mode** of REQ-RND-023, for Levels 1 and 1.5: flashing
//!   and concealed pixels get indices of their own, so flash and reveal are
//!   palette changes only.
//! - **The resolved mode** of REQ-RND-031, for Levels 2.5 and 3.5: flash
//!   and conceal are applied at render time from a [`FlashPhase`] and the
//!   reveal setting, and the indices are Colour Map indices 0 to 31, index 8
//!   transparent. The port's palette is the grid's Colour Map.
//!
//! A row is 40 columns wide at every level (REQ-1644).
//!
//! Renderers start from the cell grid and never decode packets
//! (REQ-RND-001). The raster uses integer arithmetic only, so the same grid
//! and settings give the same pixels on every platform (REQ-RND-015).

use crate::consts::{CELL_FIELD_LINES, ROWS};

mod changes;
mod colours;
mod flash;
mod options;
mod raster;
mod row;

pub use changes::ChangeTracker;
pub use colours::{
    background_index, border_index, fill_palette, foreground_index, palette_colour, palette_mode_allowed,
};
pub use flash::{FlashPhase, flash_on, flash_unit, flash_visible};
pub use options::{ColourMode, RenderOptions};
use raster::CELL_LAST_HALF_DOT;
pub use raster::cell_line;
pub use row::{render_line, render_row, render_screen};

/// One line of one cell (REQ-RND-016).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CellLine {
    /// Twelve half-dots, bit 11 the leftmost; a set bit is foreground.
    pub mask: u16,
    /// Index of the dots.
    pub foreground: u8,
    /// Index of the gaps.
    pub background: u8,
}

impl CellLine {
    /// The index of dot `dot` (0 the leftmost).
    #[must_use]
    pub fn index(&self, dot: usize) -> u8 {
        let on = self.mask.checked_shr(CELL_LAST_HALF_DOT.wrapping_sub(dot) as u32).is_some_and(|bits| bits & 1 == 1);
        if on { self.foreground } else { self.background }
    }
}

/// Which lines of a row to draw (REQ-RND-017).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    /// Both fields: 20 lines per row in the 12-by-20 mode.
    Both,
    /// The first field only: even lines.
    First,
    /// The second field only: odd lines.
    Second,
}

/// Errors from the row renderer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderError {
    /// The buffer is too small for the requested row, stride and format.
    BufferTooSmall,
    /// The row or line number is out of range.
    OutOfRange,
    /// The palette mode was asked to draw a grid that needs the resolved
    /// mode: a colour outside CLUT 0 or a flash mode of Level 2.5 (decision
    /// E.55, REQ-RND-031).
    NeedsResolvedMode,
    /// The 4-bit format was asked to hold a Colour Map index above 15.
    IndexTooWide,
}

/// Pixel packing of the caller's buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexFormat {
    /// One index per byte.
    Bits8,
    /// Two indices per byte, the left pixel in the high nibble. In the
    /// palette mode only indices 0 to 15 fit, so flash and conceal use the
    /// base colours; in the resolved mode an index above 15 is an error.
    Bits4,
}

/// A caller-supplied buffer of indices (REQ-RND-017).
#[derive(Debug)]
pub struct IndexBuffer<'a> {
    /// The pixel packing.
    pub format: IndexFormat,
    /// The memory.
    pub data: &'a mut [u8],
    /// Bytes from one pixel row to the next.
    pub stride: usize,
}

/// Every row.
pub const ALL_ROWS: u32 = (1 << ROWS) - 1;

/// Lines of the cell in one field (for documentation of the geometry).
pub const FIELD_LINES: usize = CELL_FIELD_LINES;

#[cfg(all(test, feature = "bedstead"))]
mod tests;
