//! The scrolling origin of a form in the status area (REQ-SEC-009).

use neoteletext_core::consts::{ALPHA_YELLOW, SPACE, STATUS_COLUMNS};
use neoteletext_core::level1::Grid;
use neoteletext_core::types::Instant;

/// Microseconds per step of the scrolling form origin (REQ-SEC-009).
const TICKER_STEP_US: u64 = 300_000;

/// Writes the form's origin into the status area as a ticker (REQ-SEC-009:
/// while a form has focus, the origin it submits to is shown there).
pub(super) fn origin_ticker(grid: &mut Grid, origin: &str, now: Instant) {
    let text: Vec<u8> = format!("> {origin}   ").bytes().filter(|b| b.is_ascii_graphic() || *b == b' ').collect();
    if text.is_empty() {
        return;
    }
    let offset = usize::try_from(now.0 / TICKER_STEP_US).unwrap_or(0).checked_rem(text.len()).unwrap_or(0);
    let reference = grid.cells[0][STATUS_COLUMNS];
    let g0 = grid.designation.g0;
    for column in 0..STATUS_COLUMNS {
        let byte =
            offset.saturating_add(column).checked_rem(text.len()).and_then(|at| text.get(at)).copied().unwrap_or(SPACE);
        if let Some(cell) = grid.cell_mut(0, column) {
            *cell = neoteletext_core::level1::Cell {
                glyph: if byte == SPACE {
                    neoteletext_core::level1::Glyph::Space
                } else {
                    neoteletext_core::level1::Glyph::Char(neoteletext_core::charset::CharRef::new(
                        neoteletext_core::charset::CharSet::G0(g0),
                        byte,
                    ))
                },
                foreground: neoteletext_core::consts::COLOUR_BLACK,
                // The colour of the Alpha Yellow attribute.
                background: ALPHA_YELLOW & neoteletext_core::consts::COLOUR_ATTRIBUTE_MASK,
                ..reference
            };
        }
    }
}
