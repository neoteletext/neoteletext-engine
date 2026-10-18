//! Composing a page at the configured presentation level (REQ-PRF-006,
//! layer L3): the pass of the level applied to the page's Level 1 grid.
//! It is separate from the viewer because a port without the viewer still
//! composes pages (decision E.61). A level that is not compiled in falls
//! back to the highest compiled level below it.

use crate::acq::{EnhancementLookup, EnhancementReport, MagazineData, StoredPage};
use crate::config::{Config, PresentationLevel};
use crate::level1::{DisplayOptions, Grid};
use crate::trace::TraceSink;

/// Applies the pass of the presentation level to a page's Level 1 grid
/// (REQ-PRF-006): nothing at Level 1, the Level 1.5 pass at Level 1.5, and
/// the enhancement processor at Levels 2.5 and 3.5 (REQ-L25-001). The
/// level is read on every build, so changing it never re-acquires a page.
/// At Levels 2.5 and 3.5 it returns the processor's report of the faults it
/// met, for the caller's statistics and trace (REQ-L25-051).
pub fn compose_level<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &mut Grid<DRCS, PANEL_ROWS>,
    page: &StoredPage,
    magazine: &MagazineData,
    lookup: &EnhancementLookup<'_>,
    config: &Config,
    options: &DisplayOptions,
) -> Option<EnhancementReport> {
    let _ = (&grid, page, magazine, lookup, options);
    let report = match config.level.available() {
        PresentationLevel::Level1 => None,
        #[cfg(feature = "level15")]
        PresentationLevel::Level15 => {
            crate::level15::apply(grid, &page.enhancements, page.enhancements_present, config, true);
            None
        }
        #[cfg(feature = "level25")]
        PresentationLevel::Level25 | PresentationLevel::Level35 => {
            Some(crate::enhance::apply(grid, page, magazine, lookup, config, options))
        }
        // `available` returns only compiled levels.
        #[allow(unreachable_patterns)]
        _ => None,
    };
    // REQ-CHR-015: letters are joined from what the cells finally hold.
    #[cfg(feature = "extended-sets")]
    crate::joining::join(grid);
    report
}

/// As [`compose_level`], and when `Config::trace` is on, reports the
/// transitions of the walk of the level to `trace` (REQ-FSM-010): the
/// Level 1.5 triplet walk (REQ-FSM-007), or the page-level run of the
/// enhancement walk (REQ-FSM-011).
pub fn compose_level_traced<const DRCS: usize, const PANEL_ROWS: usize>(
    grid: &mut Grid<DRCS, PANEL_ROWS>,
    page: &StoredPage,
    magazine: &MagazineData,
    lookup: &EnhancementLookup<'_>,
    config: &Config,
    options: &DisplayOptions,
    trace: &mut TraceSink<'_>,
) -> Option<EnhancementReport> {
    let _ = (&grid, page, magazine, lookup, options, &trace);
    let report = match config.level.available() {
        PresentationLevel::Level1 => None,
        #[cfg(feature = "level15")]
        PresentationLevel::Level15 => {
            crate::level15::apply_traced(grid, &page.enhancements, page.enhancements_present, config, true, trace);
            None
        }
        #[cfg(feature = "level25")]
        PresentationLevel::Level25 | PresentationLevel::Level35 => {
            Some(crate::enhance::apply_traced(grid, page, magazine, lookup, config, options, trace))
        }
        // `available` returns only compiled levels.
        #[allow(unreachable_patterns)]
        _ => None,
    };
    #[cfg(feature = "extended-sets")]
    crate::joining::join(grid);
    report
}
