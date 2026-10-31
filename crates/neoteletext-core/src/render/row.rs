//! The row renderer of REQ-RND-017: the writing of cells, lines, rows and
//! the screen into the caller's buffer. Every row is [`PAGE_WIDTH`] pixels
//! wide (REQ-1644).

use super::colours::palette_row_allowed;
use super::options::{ColourMode, RenderOptions};
use super::raster::{CELL_LAST_HALF_DOT, cell_line};
use super::{CellLine, Field, IndexBuffer, IndexFormat, RenderError};
use crate::consts::{CELL_WIDTH, INDEX_4BIT_MAX, PAGE_WIDTH, ROWS};
use crate::glyphs::GlyphSet;
use crate::level1::{Grid, Zoom};

/// Writes one cell line as twelve 8-bit indices.
// Called for every cell of every line, as cell_line is.
#[allow(clippy::inline_always)]
#[inline(always)]
fn fill_cell(target: &mut [u8; CELL_WIDTH], raster: CellLine) {
    for (dot, pixel) in target.iter_mut().enumerate() {
        let on = (raster.mask >> CELL_LAST_HALF_DOT.wrapping_sub(dot)) & 1 == 1;
        *pixel = if on { raster.foreground } else { raster.background };
    }
}

/// Writes one cell line at cell slot `slot` of a pixel row.
fn write_cell(
    out: &mut [u8],
    slot: usize,
    raster: CellLine,
    format: IndexFormat,
    check: bool,
) -> Result<(), RenderError> {
    if check && raster.foreground.max(raster.background) > INDEX_4BIT_MAX {
        return Err(RenderError::IndexTooWide);
    }
    match format {
        IndexFormat::Bits8 => {
            let start = slot.saturating_mul(CELL_WIDTH);
            let target = out.get_mut(start..start.saturating_add(CELL_WIDTH)).ok_or(RenderError::BufferTooSmall)?;
            let target = <&mut [u8; CELL_WIDTH]>::try_from(target).map_err(|_| RenderError::BufferTooSmall)?;
            fill_cell(target, raster);
        }
        IndexFormat::Bits4 => {
            let bytes = CELL_WIDTH / 2;
            let start = slot.saturating_mul(bytes);
            let target = out.get_mut(start..start.saturating_add(bytes)).ok_or(RenderError::BufferTooSmall)?;
            for (pair, byte) in target.iter_mut().enumerate() {
                let left = raster.index(pair.wrapping_mul(2)) & INDEX_4BIT_MAX;
                let right = raster.index(pair.wrapping_mul(2).wrapping_add(1)) & INDEX_4BIT_MAX;
                *byte = (left << 4) | right;
            }
        }
    }
    Ok(())
}

/// Writes one pixel row of a text row into `out`.
fn write_line(
    grid: &Grid,
    row: usize,
    line: usize,
    glyphs: &GlyphSet,
    options: RenderOptions,
    format: IndexFormat,
    out: &mut [u8],
) -> Result<(), RenderError> {
    let needed = match format {
        IndexFormat::Bits8 => PAGE_WIDTH,
        IndexFormat::Bits4 => PAGE_WIDTH / 2,
    };
    let out = out.get_mut(..needed).ok_or(RenderError::BufferTooSmall)?;
    if row >= ROWS {
        return Err(RenderError::OutOfRange);
    }
    let cells = grid.cells.get(row).ok_or(RenderError::OutOfRange)?;
    let check = format == IndexFormat::Bits4 && matches!(options.colour, ColourMode::Resolved(_));
    if format == IndexFormat::Bits8 && !check {
        // The hot path of every port: whole cells of the buffer, with no
        // per-pixel bounds checks.
        for (column, (cell, target)) in cells.iter().zip(out.as_chunks_mut::<CELL_WIDTH>().0).enumerate() {
            fill_cell(target, cell_line(cell, grid, row, column, line, glyphs, options));
        }
    } else {
        for (column, cell) in cells.iter().enumerate() {
            let raster = cell_line(cell, grid, row, column, line, glyphs, options);
            write_cell(out, column, raster, format, check)?;
        }
    }
    Ok(())
}

/// Refuses the palette mode for a row that needs the resolved mode.
fn check_mode(grid: &Grid, row: usize, options: RenderOptions) -> Result<(), RenderError> {
    if options.colour == ColourMode::Palette && !palette_row_allowed(grid, row) {
        return Err(RenderError::NeedsResolvedMode);
    }
    Ok(())
}

/// Renders text row `row` into `out` with a row stride of `stride` bytes
/// (REQ-RND-017). In the 12-by-20 mode this draws 20 lines for
/// [`Field::Both`] and 10 for one field. Each line is [`PAGE_WIDTH`] pixels.
/// On an error the buffer's contents are unspecified.
pub fn render_row(
    grid: &Grid,
    row: usize,
    field: Field,
    glyphs: &GlyphSet,
    options: RenderOptions,
    buffer: &mut IndexBuffer<'_>,
) -> Result<(), RenderError> {
    if row >= ROWS {
        return Err(RenderError::OutOfRange);
    }
    check_mode(grid, row, options)?;
    let height = options.row_height();
    let mut target = 0usize;
    for line in 0..height {
        let wanted = match field {
            Field::Both => true,
            Field::First => line % 2 == 0,
            Field::Second => line % 2 == 1,
        };
        if !wanted {
            continue;
        }
        let start = target.saturating_mul(buffer.stride);
        let slice = buffer.data.get_mut(start..).ok_or(RenderError::BufferTooSmall)?;
        write_line(grid, row, line, glyphs, options, buffer.format, slice)?;
        target = target.saturating_add(1);
    }
    Ok(())
}

/// Renders screen line `y` (0 to 499 in the 12-by-20 mode) into `out`,
/// applying page zoom (REQ-L1-020).
pub fn render_line(
    grid: &Grid,
    y: usize,
    zoom: Zoom,
    glyphs: &GlyphSet,
    options: RenderOptions,
    format: IndexFormat,
    out: &mut [u8],
) -> Result<(), RenderError> {
    let height = options.row_height();
    let (row, line) = match zoom {
        Zoom::Off => (y.checked_div(height).unwrap_or(0), y.checked_rem(height).unwrap_or(0)),
        Zoom::Top | Zoom::Bottom => {
            // Rows 0 to 11 or 12 to 23 doubled; each output line shows half
            // a source line, the way double height does.
            let first = if zoom == Zoom::Top { 0 } else { ZOOM_ROWS };
            let doubled = y / 2;
            (first.saturating_add(doubled.checked_div(height).unwrap_or(0)), doubled.checked_rem(height).unwrap_or(0))
        }
    };
    if row >= ROWS {
        return Err(RenderError::OutOfRange);
    }
    check_mode(grid, row, options)?;
    write_line(grid, row, line, glyphs, options, format, out)
}

/// Rows in each half of a zoomed page (REQ-L1-020).
const ZOOM_ROWS: usize = 12;

/// Renders every row, or only changed rows, into a whole-screen framebuffer
/// (REQ-RND-022), the reference caller of REQ-RND-021. `rows` is a bitmask
/// of rows to draw. Each row is [`PAGE_WIDTH`] pixels wide from the start of
/// the framebuffer's lines; filling any border beyond it with
/// [`border_index`](super::border_index) is for the caller.
pub fn render_screen(
    grid: &Grid,
    rows: u32,
    glyphs: &GlyphSet,
    options: RenderOptions,
    framebuffer: &mut IndexBuffer<'_>,
) -> Result<(), RenderError> {
    let height = options.row_height();
    for row in 0..ROWS {
        if rows.checked_shr(row as u32).is_some_and(|bits| bits & 1 == 1) {
            let start = row.saturating_mul(height).saturating_mul(framebuffer.stride);
            let data = framebuffer.data.get_mut(start..).ok_or(RenderError::BufferTooSmall)?;
            let mut slice = IndexBuffer { format: framebuffer.format, data, stride: framebuffer.stride };
            render_row(grid, row, Field::Both, glyphs, options, &mut slice)?;
        }
    }
    Ok(())
}
