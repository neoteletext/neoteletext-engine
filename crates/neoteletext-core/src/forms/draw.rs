//! Drawing a form's values over the page (Appendix C.4).

use super::editing::MARKER;
use super::spec::{FieldKind, FieldSpec};
use super::{FormModel, VALUE_CHARS};
use crate::charset::{CharRef, CharSet};
use crate::consts::{CODE_MASK_7BIT, SPACE};
use crate::level1::{Cell, Glyph, Grid};

impl FormModel {
    /// Draws the values over the page in the attributes already in force at
    /// those cells (Appendix C.4). The focused field is shown in inverse
    /// video, with the cursor cell flashing.
    pub fn draw<const PANEL_ROWS: usize>(&self, grid: &mut Grid<PANEL_ROWS>) {
        for (index, field) in self.spec.fields.as_slice().iter().enumerate() {
            let focused = index == self.focus;
            if field.kind == FieldKind::Radio {
                self.draw_radio(index, field, focused, grid);
                continue;
            }
            let scroll = self.scroll(index, field);
            let mut codes = [SPACE; VALUE_CHARS];
            let shown = self.shown_codes(index, field, scroll, &mut codes);
            for (position, (row, column)) in field.cells().enumerate() {
                let Some(cell) = grid.cell_mut(row, column) else { continue };
                self.paint(cell, shown.get(position).copied().unwrap_or(SPACE), focused);
                if focused {
                    cell.flash = field.is_textual() && position.saturating_add(scroll) == self.cursor;
                }
            }
        }
    }

    /// Draws a RADIO field: the field's column is the first choice's marker
    /// and each further choice gives its own column (Appendix C.3), so the
    /// marker shows at the chosen choice and every other is cleared.
    fn draw_radio<const PANEL_ROWS: usize>(
        &self,
        index: usize,
        field: &FieldSpec,
        focused: bool,
        grid: &mut Grid<PANEL_ROWS>,
    ) {
        let chosen = usize::from(self.choice.get(index).copied().unwrap_or(0));
        let first = usize::from(field.first_option);
        let options = self.spec.options.as_slice();
        for choice in 0..usize::from(field.option_count) {
            let column = if choice == 0 {
                Some(field.column)
            } else {
                options.get(first.saturating_add(choice)).and_then(|option| option.column)
            };
            let Some(column) = column else { continue };
            for (position, (row, column)) in field.cells_at(column).enumerate() {
                let Some(cell) = grid.cell_mut(row, column) else { continue };
                let code = if position == 0 && choice == chosen { MARKER } else { SPACE };
                self.paint(cell, code, focused);
                if focused {
                    cell.flash = false;
                }
            }
        }
    }

    /// Puts a code in a cell in the attributes already there, revealed, and
    /// in inverse video when its field has focus.
    fn paint(&self, cell: &mut Cell, code: u8, focused: bool) {
        cell.glyph = if code == SPACE { Glyph::Space } else { Glyph::Char(CharRef::new(CharSet::G0(self.g0), code)) };
        cell.conceal = false;
        if focused {
            core::mem::swap(&mut cell.foreground, &mut cell.background);
        }
    }

    /// Characters of a value scrolled out of the field to the left: enough
    /// to keep the cursor, or for a field without focus the end of the
    /// value, within the field (Appendix C.4).
    fn scroll(&self, index: usize, field: &FieldSpec) -> usize {
        if !field.is_textual() {
            return 0;
        }
        let capacity = usize::from(field.width).saturating_mul(usize::from(field.rows.max(1)));
        let end = if index == self.focus { self.cursor } else { self.value(index).len() };
        end.saturating_sub(capacity.saturating_sub(1))
    }

    /// The codes a field shows, from `scroll` characters into its value.
    fn shown_codes<'a>(
        &self,
        index: usize,
        field: &FieldSpec,
        scroll: usize,
        out: &'a mut [u8; VALUE_CHARS],
    ) -> &'a [u8] {
        let capacity = usize::from(field.width).saturating_mul(usize::from(field.rows.max(1)));
        let len = match field.kind {
            FieldKind::Checkbox => {
                let on = self.checked.checked_shr(index as u32).is_some_and(|bits| bits & 1 == 1);
                if let Some(first) = out.first_mut() {
                    *first = if on { MARKER } else { SPACE };
                }
                1
            }
            FieldKind::Select => {
                let text = self.chosen_option(index, field).map_or(&[][..], |option| option.display.as_slice());
                copy(out, text)
            }
            FieldKind::Submit | FieldKind::Reset => copy(out, field.label.as_slice()),
            _ => copy(out, self.value(index).get(scroll..).unwrap_or(&[])),
        };
        out.get(..len.min(capacity)).unwrap_or(&[])
    }
}

fn copy(out: &mut [u8; VALUE_CHARS], from: &[u8]) -> usize {
    let mut len = 0usize;
    for (slot, &byte) in out.iter_mut().zip(from) {
        *slot = byte & CODE_MASK_7BIT;
        len = len.saturating_add(1);
    }
    len
}
