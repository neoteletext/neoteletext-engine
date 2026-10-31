//! The processor's entry points (REQ-L25-001): which pages it shows, the
//! page-level run, and the rebuilding of the rows.

use super::{Collector, Inputs, RowCells, Rows, Walker};

use crate::acq::{EnhancementKey, EnhancementLookup, EnhancementReport, MagazineData, PageBuild, StoredPage};
use crate::colour::{self, ColourIndex, ColourMap, PagePresentation, PresentationSources};
use crate::config::{Config, PresentationLevel};
use crate::consts::{
    C5_NEWSFLASH, C6_SUBTITLE, C7_SUPPRESS_HEADER, C10_INHIBIT_DISPLAY, COLUMNS, PAGE_FUNCTION_DRCS,
    PAGE_FUNCTION_GDRCS, PAGE_FUNCTION_GPOP, PAGE_FUNCTION_MOT, PAGE_FUNCTION_POP, PAGE_MOT, RECEIVER_COLUMNS, ROWS,
};
use crate::level1::{Cell, DisplayOptions, Grid, Size, resolve_designation, row_24_shown};
use crate::objects::{self, EnhancementLinks};
use crate::trace::TraceSink;

/// True when the page is an enhancement page and so never shown as text
/// (REQ-OBJ-041): the Magazine Organization Table, or a page whose stored
/// enhancement copy names an object, DRCS or table page function. A page
/// named only by a link is still shown, so that a wrong link never hides a
/// display page.
fn is_enhancement_page(page: &StoredPage, lookup: &EnhancementLookup<'_>) -> bool {
    let address = page.key.address;
    if address.page.0 == PAGE_MOT {
        return true;
    }
    let s1 = EnhancementKey::new(address, page.key.subcode).s1;
    lookup.sub_page(address.magazine, address.page, s1).and_then(|sub_page| sub_page.format()).is_some_and(|format| {
        matches!(
            format.function,
            PAGE_FUNCTION_GPOP | PAGE_FUNCTION_POP | PAGE_FUNCTION_GDRCS | PAGE_FUNCTION_DRCS | PAGE_FUNCTION_MOT
        )
    })
}

/// Builds a page's grid at Level 2.5 or 3.5 (REQ-L25-001 and the areas
/// L25, OBJ and DRC): `grid` holds the page's Level 1 grid, built by
/// `level1::build_grid` with the same page, magazine data, configuration
/// and options. At other levels it does nothing.
///
/// Rows are rebuilt from their Level 1 bytes and the enhancement data; row
/// 0 keeps the receiver's status area, and rows the Level 1 grid does not
/// show (a suppressed header, inhibited display, a row 24 that is not
/// displayed or that the receiver draws) keep their Level 1 cells.
pub fn apply(
    grid: &mut Grid,
    page: &StoredPage,
    magazine: &MagazineData,
    lookup: &EnhancementLookup<'_>,
    config: &Config,
    options: &DisplayOptions,
) -> EnhancementReport {
    process(grid, page, magazine, lookup, config, options, None)
}

/// As [`apply`], and when `Config::trace` is on, reports every transition
/// of the page-level run of the walk to `trace` (REQ-FSM-010). A fault the
/// walk meets (REQ-L25-051) is named as the action of the transition that
/// met it.
pub fn apply_traced(
    grid: &mut Grid,
    page: &StoredPage,
    magazine: &MagazineData,
    lookup: &EnhancementLookup<'_>,
    config: &Config,
    options: &DisplayOptions,
    trace: &mut TraceSink<'_>,
) -> EnhancementReport {
    process(grid, page, magazine, lookup, config, options, config.trace.then_some(trace))
}

/// [`apply`], with `trace` `None` when tracing is off.
fn process(
    grid: &mut Grid,
    page: &StoredPage,
    magazine: &MagazineData,
    lookup: &EnhancementLookup<'_>,
    config: &Config,
    options: &DisplayOptions,
    trace: Option<&mut TraceSink<'_>>,
) -> EnhancementReport {
    let build = PageBuild::of(page, config.level);
    if !config.level.is_enhanced() {
        return EnhancementReport::new(build, config.level);
    }
    let links = objects::links(page, lookup, config.level);
    let level = links.level;
    let config = Config { level, ..*config };
    // REQ-OBJ-041: an enhancement page shows no rows of text.
    if is_enhancement_page(page, lookup) {
        for row in grid.cells.iter_mut().skip(1) {
            row.fill(Cell::BLANK);
        }
        grid.double_height_rows = 0;
        return EnhancementReport::new(build, level);
    }
    let (designation, second_g0, _) = resolve_designation(Some(page), magazine, &config);
    grid.designation = designation;

    let presentation = presentation(page, magazine, &links, level);
    let default_sets = (designation.g0.set, designation.g2);
    let inputs = Inputs { page, lookup, links: &links, build, level, default_sets };
    let row_24_shown = row_24_shown(page, &config) && options.row24.is_none();
    let last_colour_row = if row_24_shown { ROWS.saturating_sub(1) } else { ROWS.saturating_sub(2) };

    // The page-level run: screen and row colours (decision E.45).
    let mut page_level = Collector::new(None, last_colour_row);
    let mut walker = Walker::new(&inputs);
    walker.walk(&mut page_level, trace);
    let mut report = walker.report;
    let (screen, row_colours) = page_colours(&page_level, &presentation);

    let control = page.control.0;
    let rows = Rows {
        inputs: &inputs,
        config: &config,
        options,
        presentation: &presentation,
        row_colours,
        last_colour_row,
        with_events: page_level.event_rows,
        second_g0,
        level_1_g0: designation.g0,
        boxed_page: control & (C5_NEWSFLASH | C6_SUBTITLE) != 0,
    };
    rebuild_rows(grid, &rows, row_24_shown, &mut report);

    store_presentation(grid, &presentation, screen, row_colours);
    report
}

/// The page's presentation (REQ-L25-030, REQ-L25-031): from the page-level
/// data of the page and its magazine.
fn presentation(
    page: &StoredPage,
    magazine: &MagazineData,
    links: &EnhancementLinks,
    level: PresentationLevel,
) -> PagePresentation {
    let page_fields = page.page_data.map(|data| data.fields);
    let page_fields_35 = page.page_data_level35.map(|data| data.fields);
    let sources = PresentationSources {
        page: page_fields.as_ref(),
        page_level_35: page_fields_35.as_ref(),
        magazine: magazine.data.as_ref(),
        magazine_level_35: magazine.level35.as_ref(),
        fallback: links.fallback,
    };
    colour::resolve(&sources, level, &ColourMap::DEFAULT)
}

/// The screen colour and the row colours the page-level run set (decision
/// E.45), with the presentation's defaults where it set none.
fn page_colours(page_level: &Collector, presentation: &PagePresentation) -> (ColourIndex, [ColourIndex; ROWS]) {
    let screen = page_level.screen.unwrap_or(presentation.default_screen);
    let fallback_row = if page_level.screen.is_some() { screen } else { presentation.default_row };
    let mut row_colours = [fallback_row; ROWS];
    for (colour, explicit) in row_colours.iter_mut().zip(page_level.row_colours) {
        *colour = explicit.unwrap_or(fallback_row);
    }
    (screen, row_colours)
}

/// Stores the page's colours in the grid, and marks the rows the rebuilt
/// cells make double height.
fn store_presentation(
    grid: &mut Grid,
    presentation: &PagePresentation,
    screen: ColourIndex,
    row_colours: [ColourIndex; ROWS],
) {
    grid.colour_map = presentation.colour_map;
    grid.screen_colour = screen;
    grid.row_colours = row_colours;
    grid.double_height_rows = 0;
    for (row, cells) in grid.cells.iter().enumerate() {
        if cells.iter().any(|cell| cell.size == Size::DoubleTop) {
            grid.double_height_rows |= 1u32.checked_shl(row as u32).unwrap_or(0);
        }
    }
}

/// Rebuilds the rows the page shows, top to bottom, so that each row knows
/// the row above it (REQ-L25-025, REQ-L25-026).
fn rebuild_rows(grid: &mut Grid, rows: &Rows<'_, '_>, row_24_shown: bool, report: &mut EnhancementReport) {
    let page = rows.inputs.page;
    let control = page.control.0;
    let inhibit = control & C10_INHIBIT_DISPLAY != 0 || !page.is_text_page();
    let receiver_area: [Cell; RECEIVER_COLUMNS] = core::array::from_fn(|column| {
        grid.cells.first().and_then(|row| row.get(column)).copied().unwrap_or(Cell::BLANK)
    });
    let mut cells = RowCells::new();
    let mut lower = false;
    for row in 0..ROWS {
        let shown = match row {
            0 => control & C7_SUPPRESS_HEADER == 0,
            _ if row == ROWS.saturating_sub(1) => !inhibit && row_24_shown,
            _ => !inhibit,
        };
        if !shown {
            cells.forget();
            lower = false;
            continue;
        }
        cells.next_row();
        // REQ-L25-026: the row below spacing double height shows, where no
        // enhancement character is written, the background of the cell
        // above and no foreground.
        let fill: [Cell; COLUMNS] = core::array::from_fn(|column| {
            let upper = cells.above.get(column).copied().flatten().unwrap_or(Cell::BLANK);
            Cell { foreground: upper.foreground, background: upper.background, boxed: upper.boxed, ..Cell::BLANK }
        });
        let spacing_double = rows.row(row, lower, &fill, &mut cells, report);
        for (column, cell) in cells.cells.iter().enumerate() {
            let mut cell = cell.unwrap_or_else(|| fill.get(column).copied().unwrap_or(Cell::BLANK));
            // REQ-L25-050: row 0 columns 0 to 7 stay the receiver's.
            if row == 0
                && let Some(receiver) = receiver_area.get(column)
            {
                cell = *receiver;
            }
            if let Some(target) = grid.cell_mut(row, column) {
                *target = cell;
            }
        }
        for (column, slot) in cells.above.iter_mut().enumerate() {
            *slot = grid.cell(row, column).copied();
        }
        lower = spacing_double && !lower;
    }
}
