//! Levels 2.5 and 3.5 through the image writers: golden raster checksums of
//! hand-built demonstration grids (REQ-TST-024, REQ-TST-030), and the
//! dimensions and palettes of PNG and PPM images with side panels
//! (REQ-RND-007, REQ-RND-030, REQ-RND-033).
//!
//! The grids are built directly, so the checksums depend only on the
//! renderer (REQ-RND-001). `tests/golden/raster-level25.txt` carries a
//! format version: a deliberate rendering change raises it and regenerates
//! the file in the same change, with `NEOTELETEXT_UPDATE_GOLDEN=1`
//! (REQ-TST-030). With `NEOTELETEXT_SAMPLES` naming a directory, the test also
//! writes each demonstration as a PNG there, for inspection by eye.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::arithmetic_side_effects, missing_docs)]

use neoteletext_core::charset::{CharRef, CharSet, G0, G0Set, NationalOption};
use neoteletext_core::colour::{SidePanels, colour_index};
use neoteletext_core::config::CellMode;
use neoteletext_core::consts::{CELL_HEIGHT, CELL_WIDTH, COLUMNS, ROWS};
use neoteletext_core::glyphs::GlyphSet;
use neoteletext_core::level1::{Cell, CellAttributes, FlashMode, FlashRate, Glyph, Grid, Size, Width};
use neoteletext_core::render::{
    ALL_ROWS, ColourMode, FlashPhase, IndexBuffer, IndexFormat, RenderOptions, render_screen, row_width,
};
use neoteletext_image::Sink;
use neoteletext_image::palette::{Rgba, colour_map_palette};
use neoteletext_image::scale::{scale_indices, scaled_size};
use std::fmt::Write as _;
use std::path::PathBuf;

/// Raise when Level 2.5 and 3.5 rendering changes on purpose (REQ-TST-030).
const FORMAT_VERSION: u32 = 5;

const BLACK: u8 = 0;
const RED: u8 = 1;
const GREEN: u8 = 2;
const YELLOW: u8 = 3;
const BLUE: u8 = 4;
const MAGENTA: u8 = 5;
const CYAN: u8 = 6;
const WHITE: u8 = 7;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3))
}

fn latin() -> CharSet {
    CharSet::G0(G0 { set: G0Set::Latin, national: Some(NationalOption::English) })
}

/// Writes text from a column, with colours and attributes, into the main
/// page or, for columns 40 to 71, the side-panel area.
fn write(
    grid: &mut Grid,
    row: usize,
    column: usize,
    text: &str,
    foreground: u8,
    background: u8,
    attributes: CellAttributes,
) {
    for (offset, byte) in text.bytes().enumerate() {
        let cell =
            Cell { glyph: Glyph::Char(CharRef::new(latin(), byte)), foreground, background, attributes, ..Cell::BLANK };
        put(grid, row, column + offset, cell);
    }
}

fn put(grid: &mut Grid, row: usize, column: usize, cell: Cell) {
    if column < COLUMNS {
        grid.cells[row][column] = cell;
    } else if let Some(slot) = grid.panel_cell_mut(row, column) {
        *slot = cell;
    }
}

fn fill_row(grid: &mut Grid, row: usize, background: u8) {
    for column in 0..72 {
        put(grid, row, column, Cell { background, ..Cell::BLANK });
    }
}

/// Side panels of clause 12.5.3: 8 columns each side, the panels in their
/// own colours from CLUTs 2 and 3, a G3 frame and a screen colour.
fn side_panels() -> Grid {
    let mut grid = Grid::EMPTY;
    grid.side_panels = SidePanels { left: 8, right: 8 };
    grid.screen_colour = colour_index(3, 1);
    for row in 1..ROWS {
        fill_row(&mut grid, row, colour_index(3, 0));
        for column in 64..72 {
            put(&mut grid, row, column, Cell { background: colour_index(2, (row % 8) as u8), ..Cell::BLANK });
        }
        for column in 40..48 {
            put(&mut grid, row, column, Cell { background: colour_index(2, 7 - (row % 8) as u8), ..Cell::BLANK });
        }
    }
    write(&mut grid, 2, 64, " LEFT  ", WHITE, BLACK, CellAttributes::DEFAULT);
    write(&mut grid, 2, 40, " RIGHT ", WHITE, BLACK, CellAttributes::DEFAULT);
    write(&mut grid, 2, 8, "Side panels: left 8, right 8", YELLOW, colour_index(3, 0), CellAttributes::DEFAULT);
    write(&mut grid, 4, 2, "The main page is 40 columns wide.", WHITE, colour_index(3, 0), CellAttributes::DEFAULT);
    for column in 0..72 {
        let frame = Cell {
            glyph: Glyph::Char(CharRef::new(CharSet::G3, 0x51)),
            foreground: CYAN,
            background: colour_index(3, 0),
            ..Cell::BLANK
        };
        put(&mut grid, 6, column, frame);
    }
    for row in 8..20 {
        for column in 0..72 {
            let code = 0x20 + ((row + column) % 12) as u8;
            let colour = colour_index(((row / 4) % 4) as u8, (column % 8) as u8);
            let cell = Cell {
                glyph: Glyph::Char(CharRef::new(CharSet::G3, code)),
                foreground: colour,
                background: BLACK,
                ..Cell::BLANK
            };
            put(&mut grid, row, column, cell);
        }
    }
    grid
}

/// The 32 colours of the Colour Map as blocks, with transparent index 8
/// showing the row colour below it (Table C.1).
fn colour_map() -> Grid {
    let mut grid = Grid::EMPTY;
    grid.row_colours = [colour_index(2, 4); ROWS];
    write(&mut grid, 1, 2, "The Colour Map: CLUTs 0 to 3", WHITE, BLACK, CellAttributes::DEFAULT);
    for index in 0..32u8 {
        let row = 3 + usize::from(index / 8) * 2;
        let column = 2 + usize::from(index % 8) * 4;
        for offset in 0..3 {
            let block = Cell {
                glyph: Glyph::Char(CharRef::new(CharSet::G3, 0x3F)),
                foreground: index,
                background: index,
                ..Cell::BLANK
            };
            put(&mut grid, row, column + offset, block);
        }
    }
    grid
}

/// Every G3 position (REQ-RND-037), in six rows of sixteen, and the same
/// mosaics joined into shapes at double size.
fn g3_page() -> Grid {
    let mut grid = Grid::EMPTY;
    write(&mut grid, 1, 2, "G3 smooth mosaics and line drawing", WHITE, BLACK, CellAttributes::DEFAULT);
    for code in 0x20u8..0x80 {
        let row = 3 + usize::from((code - 0x20) / 16) * 2;
        let column = 4 + usize::from(code % 16) * 2;
        let cell = Cell {
            glyph: Glyph::Char(CharRef::new(CharSet::G3, code)),
            foreground: YELLOW,
            background: BLUE,
            ..Cell::BLANK
        };
        put(&mut grid, row, column, cell);
    }
    let shape = [[0x2Fu8, 0x2E, 0x23, 0x3F, 0x33, 0x3E, 0x2F], [0x5B, 0x5C, 0x5D, 0x5E, 0x4D, 0x4E, 0x4F]];
    for (line, codes) in shape.iter().enumerate() {
        for (offset, &code) in codes.iter().enumerate() {
            let glyph = Glyph::Char(CharRef::new(CharSet::G3, code));
            let column = 4 + offset * 3;
            for (row, size) in [(16 + line * 3, Size::DoubleTop), (17 + line * 3, Size::DoubleBottom)] {
                put(
                    &mut grid,
                    row,
                    column,
                    Cell {
                        glyph,
                        foreground: WHITE,
                        background: MAGENTA,
                        size,
                        attributes: CellAttributes::DEFAULT.with_width(Width::LeftHalf),
                        ..Cell::BLANK
                    },
                );
                put(
                    &mut grid,
                    row,
                    column + 1,
                    Cell {
                        glyph,
                        foreground: WHITE,
                        background: MAGENTA,
                        size,
                        attributes: CellAttributes::DEFAULT.with_width(Width::RightHalf),
                        ..Cell::BLANK
                    },
                );
            }
        }
    }
    grid
}

/// Double width, double height and double size (REQ-RND-034), underline
/// (REQ-RND-035), and invert and conceal (REQ-RND-036).
fn attributes_page() -> Grid {
    let mut grid = Grid::EMPTY;
    let plain = CellAttributes::DEFAULT;
    write(&mut grid, 1, 2, "Size, underline and invert", WHITE, BLACK, plain);
    for (offset, byte) in "DOUBLE".bytes().enumerate() {
        let glyph = Glyph::Char(CharRef::new(latin(), byte));
        let column = 2 + offset * 2;
        for (part, step) in [(Width::LeftHalf, 0), (Width::RightHalf, 1)] {
            let attributes = plain.with_width(part);
            put(&mut grid, 3, column + step, Cell { glyph, foreground: CYAN, attributes, ..Cell::BLANK });
            put(
                &mut grid,
                5,
                column + step,
                Cell { glyph, foreground: YELLOW, size: Size::DoubleTop, attributes, ..Cell::BLANK },
            );
            put(
                &mut grid,
                6,
                column + step,
                Cell { glyph, foreground: YELLOW, size: Size::DoubleBottom, attributes, ..Cell::BLANK },
            );
        }
        put(&mut grid, 5, 20 + offset, Cell { glyph, foreground: GREEN, size: Size::DoubleTop, ..Cell::BLANK });
        put(&mut grid, 6, 20 + offset, Cell { glyph, foreground: GREEN, size: Size::DoubleBottom, ..Cell::BLANK });
    }
    write(&mut grid, 8, 2, "Underlined text", WHITE, BLUE, plain.with_underline(true));
    for column in 20..26 {
        let mosaic = Cell {
            glyph: Glyph::Mosaic { pattern: 0x3F, separated: true },
            foreground: GREEN,
            attributes: plain.with_underline(true),
            ..Cell::BLANK
        };
        put(&mut grid, 8, column, mosaic);
    }
    write(&mut grid, 10, 2, "Inverted text", RED, WHITE, plain.with_invert(true));
    let mut concealed = Grid::EMPTY;
    write(&mut concealed, 0, 0, "Concealed until revealed", MAGENTA, BLACK, plain);
    for (column, cell) in concealed.cells[0].iter().enumerate().take(24) {
        put(&mut grid, 14, 2 + column, Cell { conceal: true, ..*cell });
    }
    grid
}

/// Every flash mode and rate (REQ-RND-036, REQ-RND-040, decision E.48), in
/// a row of its own, with an incremental and a decremental run.
fn flash_page() -> Grid {
    let mut grid = Grid::EMPTY;
    write(&mut grid, 1, 2, "Flash modes and rates", WHITE, BLACK, CellAttributes::DEFAULT);
    let rates = [FlashRate::Slow, FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3];
    let modes = [FlashMode::Normal, FlashMode::Inverted, FlashMode::AdjacentClut];
    for (mode_index, mode) in modes.into_iter().enumerate() {
        for (rate_index, rate) in rates.into_iter().enumerate() {
            let row = 3 + mode_index * 5 + rate_index;
            let attributes = CellAttributes::DEFAULT.with_flash_mode(mode).with_flash_rate(rate);
            write(&mut grid, row, 2, "FLASH", RED, BLUE, attributes);
            for column in 2..7 {
                grid.cells[row][column].flash = true;
            }
            write(&mut grid, row, 10, &format!("{mode:?} {rate:?}"), WHITE, BLACK, CellAttributes::DEFAULT);
        }
    }
    let block = |rate| Cell {
        glyph: Glyph::Char(CharRef::new(CharSet::G3, 0x3F)),
        foreground: YELLOW,
        background: BLACK,
        flash: true,
        attributes: CellAttributes::DEFAULT.with_flash_rate(rate),
        ..Cell::BLANK
    };
    for column in 0..30 {
        let up = [FlashRate::FastPhase1, FlashRate::FastPhase2, FlashRate::FastPhase3][column % 3];
        let down = [FlashRate::FastPhase1, FlashRate::FastPhase3, FlashRate::FastPhase2][column % 3];
        put(&mut grid, 19, 2 + column, block(up));
        put(&mut grid, 21, 2 + column, block(down));
    }
    grid
}

fn demonstrations() -> Vec<(&'static str, Grid)> {
    vec![
        ("side-panels", side_panels()),
        ("colour-map", colour_map()),
        ("g3", g3_page()),
        ("attributes", attributes_page()),
        ("flash", flash_page()),
    ]
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

fn render(grid: &Grid, options: RenderOptions) -> (usize, usize, Vec<u8>) {
    let width = row_width(grid);
    let height = ROWS * options.row_height();
    let mut indices = vec![0u8; width * height];
    let mut buffer = IndexBuffer { format: IndexFormat::Bits8, data: &mut indices, stride: width };
    render_screen(grid, ALL_ROWS, &GlyphSet::bedstead(), options, &mut buffer).unwrap();
    (width, height, indices)
}

fn checksums() -> String {
    let mut out = format!("format {FORMAT_VERSION}\n");
    for (name, grid) in demonstrations() {
        for (size, mode) in [("12x20", CellMode::Saa5050), ("12x10", CellMode::Saa5246a)] {
            let units: Vec<u8> = if name == "flash" { (0..12).collect() } else { vec![0] };
            for unit in units {
                for reveal in [false, true] {
                    let (width, height, indices) = render(&grid, options(mode, unit, reveal));
                    let _ = writeln!(
                        out,
                        "{name} {size} unit={unit} reveal={reveal} {width}x{height} {:016x}",
                        fnv(&indices)
                    );
                }
            }
        }
    }
    out
}

struct VecSink(Vec<u8>);

impl Sink for VecSink {
    type Error = ();
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

fn png(width: usize, height: usize, indices: &[u8], palette: &[Rgba]) -> Vec<u8> {
    let mut sink = VecSink(Vec::new());
    let mut scratch = vec![0u8; 2 * (width + 1)];
    neoteletext_image::png::write(
        &mut sink,
        width,
        height,
        palette,
        &mut |y, row: &mut [u8]| row.copy_from_slice(&indices[y * width..(y + 1) * width]),
        &mut scratch,
    )
    .unwrap();
    sink.0
}

/// The chunks of a PNG file: kind and body.
fn chunks(file: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    let mut position = 8;
    let mut out = Vec::new();
    while position < file.len() {
        let length = u32::from_be_bytes(file[position..position + 4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = file[position + 4..position + 8].try_into().unwrap();
        out.push((kind, file[position + 8..position + 8 + length].to_vec()));
        position += 12 + length;
    }
    out
}

/// REQ-TST-024, REQ-TST-030: golden checksums of the demonstrations in
/// both cell sizes, every flash unit of the flash page, and both reveal
/// settings.
#[test]
fn level25_golden_rasters_match() {
    let path = root().join("tests/golden/raster-level25.txt");
    let actual = checksums();
    if std::env::var_os("NEOTELETEXT_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .expect("tests/golden/raster-level25.txt; generate with NEOTELETEXT_UPDATE_GOLDEN=1");
    assert_eq!(actual, expected, "rendering changed: if deliberate, raise FORMAT_VERSION and regenerate");
}

/// REQ-RND-007, REQ-RND-030, REQ-RND-033: a page of 72 columns (both panels
/// of 16, the most the panel area holds) gives an 864 by 500 PNG with the
/// 32 colours of its Colour Map, index 8 transparent; scaling and aspect
/// correction keep the width; the PPM has the same size.
#[test]
fn images_with_side_panels() {
    let mut grid = side_panels();
    grid.side_panels = SidePanels { left: 16, right: 16 };
    grid.colour_map.entries[17] = neoteletext_core::types::Rgb12::new(1, 2, 3);
    let (width, height, indices) = render(&grid, options(CellMode::Saa5050, 0, false));
    assert_eq!((width, height), (72 * CELL_WIDTH, ROWS * CELL_HEIGHT));
    assert!(indices.iter().all(|&index| index < 32));
    let palette = colour_map_palette(&grid.colour_map, Rgba::TRANSPARENT);
    let file = png(width, height, &indices, &palette);
    let chunks = chunks(&file);
    let header = &chunks[0].1;
    assert_eq!(&chunks[0].0, b"IHDR");
    assert_eq!(u32::from_be_bytes(header[0..4].try_into().unwrap()), 864);
    assert_eq!(u32::from_be_bytes(header[4..8].try_into().unwrap()), 500);
    let plte = &chunks.iter().find(|(kind, _)| kind == b"PLTE").unwrap().1;
    let trns = &chunks.iter().find(|(kind, _)| kind == b"tRNS").unwrap().1;
    assert_eq!((plte.len(), trns.len()), (32 * 3, 32));
    assert_eq!(&plte[17 * 3..18 * 3], &[17, 34, 51], "the page's own colour, times 17");
    assert_eq!(trns[8], 0, "CLUT 1 entry 0 is transparent");
    assert!(trns.iter().enumerate().all(|(index, &alpha)| index == 8 || alpha == 255));
    // Scaling by 2 and the 4:3 correction.
    let (scaled_width, scaled_height) = scaled_size(width, height, 2, 2, true);
    // REQ-RND-004: the width stretched by 16/13, the height by the factor.
    assert_eq!((scaled_width, scaled_height), (864 * 2 * 16 / 13, 1000));
    let mut scaled = vec![0u8; scaled_width * scaled_height];
    scale_indices(&indices, width, height, 2, 2, true, &mut scaled).unwrap();
    let source_column = |x: usize| indices[x * width / scaled_width];
    assert!((0..scaled_width).all(|x| scaled[x] == source_column(x)), "each column from its source column");
    // The PPM takes the fill colour for transparent pixels.
    let mut ppm = Vec::new();
    neoteletext_image::ppm::write(
        &mut VecSinkWriter(&mut ppm),
        &indices,
        width,
        height,
        &palette,
        Rgba::opaque(9, 9, 9),
    )
    .unwrap();
    assert!(ppm.starts_with(b"P6\n864 500\n255\n"));
    assert_eq!(ppm.len(), b"P6\n864 500\n255\n".len() + 864 * 500 * 3);
    // The 12 by 10 cell.
    let (_, small_height, _) = render(&grid, options(CellMode::Saa5246a, 0, false));
    assert_eq!(small_height, 250);
}

/// A sink over a vector, for the PPM writer.
struct VecSinkWriter<'a>(&'a mut Vec<u8>);

impl Sink for VecSinkWriter<'_> {
    type Error = ();
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

/// The PNG writer refuses palettes it cannot write.
#[test]
fn png_palette_bounds() {
    let mut sink = VecSink(Vec::new());
    let mut scratch = vec![0u8; 8];
    let mut row = |_: usize, _: &mut [u8]| {};
    let empty = neoteletext_image::png::write(&mut sink, 2, 1, &[], &mut row, &mut scratch);
    assert_eq!(empty, Err(neoteletext_image::png::PngError::BadSize));
    let long = vec![Rgba::TRANSPARENT; 145];
    let result = neoteletext_image::png::write(&mut sink, 2, 1, &long, &mut row, &mut scratch);
    assert_eq!(result, Err(neoteletext_image::png::PngError::BadSize));
}

/// With `NEOTELETEXT_SAMPLES` set, writes every demonstration as a PNG at
/// twice the size with the 4:3 correction, and the flash page at several
/// units, for inspection by eye.
#[test]
fn write_samples_when_asked() {
    let Some(directory) = std::env::var_os("NEOTELETEXT_SAMPLES") else { return };
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let mut jobs: Vec<(String, Grid, RenderOptions)> = Vec::new();
    for (name, grid) in demonstrations() {
        jobs.push((format!("{name}.png"), grid, options(CellMode::Saa5050, 0, false)));
    }
    let flash = flash_page();
    for unit in [0, 2, 4, 9] {
        jobs.push((format!("flash-unit{unit:02}.png"), flash, options(CellMode::Saa5050, unit, false)));
    }
    jobs.push(("attributes-revealed.png".into(), attributes_page(), options(CellMode::Saa5050, 0, true)));
    jobs.push(("side-panels-12x10.png".into(), side_panels(), options(CellMode::Saa5246a, 0, false)));
    for (file, grid, options) in jobs {
        let (width, height, indices) = render(&grid, options);
        let (scaled_width, scaled_height) = scaled_size(width, height, 2, 2, true);
        let mut scaled = vec![0u8; scaled_width * scaled_height];
        scale_indices(&indices, width, height, 2, 2, true, &mut scaled).unwrap();
        let palette = colour_map_palette(&grid.colour_map, Rgba::opaque(0, 0, 0));
        std::fs::write(directory.join(file), png(scaled_width, scaled_height, &scaled, &palette)).unwrap();
    }
}
