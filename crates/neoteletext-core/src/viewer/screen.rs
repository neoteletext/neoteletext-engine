//! Composing the screen the viewer shows: the page at its level, or what
//! stands in for it.

use super::{ComposeContext, SecurityIndicator, Viewer, ViewerState};
use crate::acq::EnhancementReport;
use crate::acq::StoredPage;
use crate::compose::compose_level;
use crate::level1::{DisplayOptions, Grid, build_grid};

impl Viewer {
    /// Builds the grid the viewer shows (REQ-ARCH-006): Level 1, then the
    /// pass of the presentation level, with the receiver's status area,
    /// rolling header and row 24. While holding it leaves the grid alone.
    ///
    /// At Levels 2.5 and 3.5 the result carries the enhancement processor's
    /// report. The caller records it with `Decoder::record_enhancement`
    /// once `context` no longer borrows the decoder (REQ-L25-051); the
    /// decoder counts each build once however often the page is redrawn.
    #[must_use]
    pub fn compose<const PANEL_ROWS: usize>(
        &self,
        grid: &mut Grid<PANEL_ROWS>,
        page: Option<&StoredPage>,
        context: &ComposeContext<'_>,
    ) -> Composed {
        let ComposeContext { magazine, config, rolling_header, store, row24, lookup } = *context;
        // REQ-NAV-006: while held, new receptions do not reach the screen.
        if self.state == ViewerState::Holding {
            return Composed::Held;
        }
        let searching = page.is_none() || self.state == ViewerState::Searching;
        let options = DisplayOptions {
            status: self.status(store),
            searching,
            rolling_header: if page.is_none() { Some(*rolling_header) } else { None },
            row24,
            security_indicator: self.indicator != SecurityIndicator::None,
            mix: self.mix,
            boxed: false,
        };
        build_grid(grid, page, magazine, config, &options);
        Composed::Built(page.and_then(|page| compose_level(grid, page, magazine, &lookup, config, &options)))
    }
}

/// What [`Viewer::compose`] did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Composed {
    /// The viewer is holding, and the grid was left alone (REQ-NAV-006).
    Held,
    /// The grid was built; at Levels 2.5 and 3.5, with the report of the
    /// faults the enhancement processor met, for the decoder's statistics.
    Built(Option<EnhancementReport>),
}
