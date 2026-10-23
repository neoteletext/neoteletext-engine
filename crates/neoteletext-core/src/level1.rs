//! Level 1.0 presentation (area L1, layer L3): from a stored page to a grid
//! of 25 by 40 cells, through the row attribute machine of REQ-FSM-006.
//!
//! [`build_grid`] is a pure function of the stored page, the magazine data,
//! the configuration and the display options (REQ-ARCH-006). Every feature
//! that crosses rows is resolved here, so rows are independent from here on
//! (decision E.29).
//!
//! The [`Cell`] and [`Grid`] also hold what Levels 2.5 and 3.5 add
//! (decision E.42, REQ-L25-006): Colour Map colours, size in two parts,
//! underline, invert, flash modes, which source wrote each
//! character, the side-panel area, the Colour Map, and the screen and row
//! colours. The Level 1 decoding leaves all of them at their defaults.

mod cell;
mod designation;
mod grid;
mod page;
mod row;

pub use cell::{Cell, CellAttributes, CharSource, FlashMode, FlashRate, Glyph, Size, Width};
pub use designation::{DesignationReport, DesignationSource, NationalSource, designation_report, resolve_designation};
pub use grid::{DisplayOptions, Grid, Zoom};
pub use page::{build_grid, row_24_shown, row_codes};
pub use row::RowState;

#[cfg(test)]
mod tests;
