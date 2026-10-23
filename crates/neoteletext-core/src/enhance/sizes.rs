//! The size rules of Annex C.3 (REQ-L25-025): double height, width and
//! size placed along a row, and the lower halves they claim in the next.

use super::RowCells;

use crate::consts::SCREEN_COLUMNS;
use crate::level1::{Cell, Size, Width};

impl RowCells {
    pub(super) const fn new() -> Self {
        Self {
            resolved: [None; SCREEN_COLUMNS],
            cells: [None; SCREEN_COLUMNS],
            above: [None; SCREEN_COLUMNS],
            claimed: [false; SCREEN_COLUMNS],
            claims: [false; SCREEN_COLUMNS],
        }
    }

    /// Starts a row: the claims of the previous row become this row's.
    pub(super) fn next_row(&mut self) {
        self.claimed = self.claims;
        self.claims = [false; SCREEN_COLUMNS];
        self.resolved = [None; SCREEN_COLUMNS];
        self.cells = [None; SCREEN_COLUMNS];
    }

    /// Forgets the row above, for a row that is not rebuilt.
    pub(super) fn forget(&mut self) {
        self.above = [None; SCREEN_COLUMNS];
        self.claims = [false; SCREEN_COLUMNS];
    }

    /// Applies the size rules of Annex C.3 to the resolved cells of a
    /// segment and places them (REQ-L25-025). Cells that the row above
    /// covers with lower halves show them.
    pub(super) fn place(&mut self, first: usize, end: usize, double_allowed: bool) {
        let mut column = first;
        while column < end {
            let right = column.saturating_add(1);
            // Rule 2: earlier size attributes in scan order win, so a lower
            // half from the row above hides this cell.
            if self.claimed.get(column).copied().unwrap_or(false) {
                if let (Some(slot), Some(Some(upper))) = (self.cells.get_mut(column), self.above.get(column)) {
                    *slot = Some(Cell { size: Size::DoubleBottom, ..*upper });
                }
                column = right;
                continue;
            }
            let Some(Some(resolved)) = self.resolved.get(column).copied() else {
                column = right;
                continue;
            };
            // Rule 1: no doubling where it cannot be shown: rows 0, 23 and
            // 24, the last column of an area, or over a lower half. Rule 3:
            // a double size character keeps whichever doubling fits.
            let height = resolved.double_height && double_allowed;
            let width = resolved.double_width && right < end && !self.claimed.get(right).copied().unwrap_or(true);
            let mut cell = resolved.cell;
            cell.size = if height { Size::DoubleTop } else { Size::Normal };
            if width {
                cell.attributes = cell.attributes.with_width(Width::LeftHalf);
            }
            let parts = if width { 2 } else { 1 };
            for (part, target) in [column, right].into_iter().enumerate().take(parts) {
                // Rule 4: the origin's attributes apply to every part.
                let part_cell = if part == 0 {
                    cell
                } else {
                    Cell { attributes: cell.attributes.with_width(Width::RightHalf), ..cell }
                };
                if let Some(slot) = self.cells.get_mut(target) {
                    *slot = Some(part_cell);
                }
                if height && let Some(claim) = self.claims.get_mut(target) {
                    *claim = true;
                }
            }
            column = if width { right.saturating_add(1) } else { right };
        }
    }
}
