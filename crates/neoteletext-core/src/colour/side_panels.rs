//! Side panels (clause 12.5, decision E.49): the widths of the left and
//! right panels.

use crate::config::PresentationLevel;
use crate::consts::{COLUMNS, SIDE_PANEL_AREA_COLUMNS, SIDE_PANEL_MAX_COLUMNS};
use crate::types::SidePanelFields;

/// The widths of the side panels (decision E.49).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub struct SidePanels {
    /// Columns of the left panel, 0 to 16: columns 72 − left to 71 of the
    /// panel area.
    pub left: u8,
    /// Columns of the right panel, 0 to 16: columns 40 to 40 + right − 1.
    pub right: u8,
}

impl SidePanels {
    /// No side panels.
    pub const NONE: Self = Self { left: 0, right: 0 };

    /// The widths that the fields give, by the width table of decision
    /// E.49. At Level 2.5 panels are shown only when the Side Panel Status
    /// Flag is 1; at Level 3.5 whenever they are enabled; at Levels 1 and
    /// 1.5 never.
    #[must_use]
    pub const fn from_fields(fields: SidePanelFields, level: PresentationLevel) -> Self {
        let shown = match level {
            PresentationLevel::Level1 | PresentationLevel::Level15 => false,
            PresentationLevel::Level25 => fields.status_flag,
            PresentationLevel::Level35 => true,
        };
        if !shown {
            return Self::NONE;
        }
        let n = if fields.left_columns > SIDE_PANEL_MAX_COLUMNS { SIDE_PANEL_MAX_COLUMNS } else { fields.left_columns };
        let rest = SIDE_PANEL_MAX_COLUMNS.saturating_sub(n);
        match (fields.left, fields.right) {
            (false, false) => Self::NONE,
            (true, false) => Self { left: if n == 0 { SIDE_PANEL_MAX_COLUMNS } else { n }, right: 0 },
            (false, true) => Self { left: 0, right: if n == 0 { SIDE_PANEL_MAX_COLUMNS } else { rest } },
            (true, true) => Self { left: n, right: rest },
        }
    }

    /// The first panel-area column of the left panel: 72 − left width.
    #[must_use]
    pub const fn left_first_column(self) -> usize {
        (COLUMNS + SIDE_PANEL_AREA_COLUMNS).saturating_sub(self.left as usize)
    }

    /// The width of the display in columns: 40 plus both panels.
    #[must_use]
    pub const fn display_columns(self) -> usize {
        COLUMNS.saturating_add(self.left as usize).saturating_add(self.right as usize)
    }
}
