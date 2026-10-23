//! The Form Extension (Appendix C): `FD` field definitions, `OPTION`
//! lines, `FS` submission, and the checks that end a page's form.

use neoteletext_core::consts::{COLUMNS, ROWS};

use crate::item::{Capacity, DiagnosticKind, Item};
use crate::page::{FieldKind, FormField, FormMethod, FormOption, IssueKind};
use crate::text::Text;

use super::TtiReader;
use super::fields::{Args, decimal, flag};

impl TtiReader {
    fn set_text<const N: usize>(&self, target: &mut Text<N>, value: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        if !target.set(value) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Text }, out);
        }
    }

    pub(super) fn field_definition(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let Some(kind) = FieldKind::from_keyword(fields.next().trim_ascii()) else {
            self.issue(IssueKind::BadField, out);
            return;
        };
        let name = fields.next();
        let column = decimal(fields.next()).and_then(|value| u8::try_from(value).ok());
        let width = decimal(fields.next()).and_then(|value| u8::try_from(value).ok());
        let columns = u8::try_from(COLUMNS).unwrap_or(u8::MAX);
        let (Some(column), Some(width)) = (column, width) else {
            // Version 2.0 put type parameters where version 3 puts the
            // column and width (Appendix C.6).
            self.issue(IssueKind::FormNeedsConversion, out);
            return;
        };
        if column >= columns || width == 0 || width > columns {
            self.issue(IssueKind::FormNeedsConversion, out);
            return;
        }
        let Some(row) = self.last_row else {
            self.issue(IssueKind::FieldWithoutRow, out);
            return;
        };
        let mut field = FormField::new(kind);
        self.set_text(&mut field.name, name, out);
        field.row = row;
        field.column = column;
        field.width = width;
        field.line = self.line_number;
        let readable = self.field_parameters(&mut field, &mut fields, out);
        if !readable {
            self.issue(IssueKind::BadField, out);
        }
        let rows = u8::try_from(ROWS).unwrap_or(u8::MAX);
        if row == 0 || column.saturating_add(width) > columns || row.saturating_add(field.rows) > rows {
            self.issue(IssueKind::FieldOutsideRow, out);
        }
        let index = u8::try_from(self.page.form.fields.len()).ok();
        if !self.page.form.fields.push(field) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Fields }, out);
            return;
        }
        if kind.has_options() {
            self.last_choice_field = index;
        }
    }

    /// Reads the parameters after `column,width` (Appendix C.3). Returns
    /// false when one could not be read.
    fn field_parameters(&self, field: &mut FormField, fields: &mut Args<'_>, out: &mut dyn FnMut(Item<'_>)) -> bool {
        let mut ok = true;
        let max_length = |text: &[u8]| -> (Option<u16>, bool) {
            if text.trim_ascii().is_empty() {
                return (None, true);
            }
            let value = decimal(text).and_then(|value| u16::try_from(value).ok());
            (value, value.is_some())
        };
        match field.kind {
            FieldKind::Text | FieldKind::Telephone | FieldKind::Email => {
                let (length, length_ok) = max_length(fields.next());
                field.max_length = length;
                ok &= length_ok;
                let required = flag(fields.next());
                if field.kind != FieldKind::Email {
                    self.set_text(&mut field.pattern, fields.next(), out);
                }
                self.set_text(&mut field.default, fields.rest(), out);
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
            }
            FieldKind::Number | FieldKind::Date | FieldKind::Time => {
                self.set_text(&mut field.minimum, fields.next(), out);
                self.set_text(&mut field.maximum, fields.next(), out);
                if field.kind == FieldKind::Number {
                    self.set_text(&mut field.step, fields.next(), out);
                }
                let required = flag(fields.next());
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
                self.set_text(&mut field.default, fields.rest(), out);
            }
            FieldKind::Select | FieldKind::Radio => {
                let required = flag(fields.next());
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
            }
            FieldKind::Checkbox => {
                self.set_text(&mut field.value, fields.next(), out);
                let required = flag(fields.next());
                let checked = flag(fields.next());
                field.required = required.unwrap_or(false);
                field.checked = checked.unwrap_or(false);
                ok &= required.is_some() && checked.is_some();
            }
            FieldKind::TextArea => {
                let rows = decimal(fields.next()).and_then(|value| u8::try_from(value).ok()).filter(|&rows| rows > 0);
                ok &= rows.is_some();
                field.rows = rows.unwrap_or(1);
                let (length, length_ok) = max_length(fields.next());
                field.max_length = length;
                ok &= length_ok;
                let required = flag(fields.next());
                field.required = required.unwrap_or(false);
                ok &= required.is_some();
                self.set_text(&mut field.default, fields.rest(), out);
            }
            FieldKind::Submit => {
                self.set_text(&mut field.label, fields.next(), out);
                self.set_text(&mut field.action, fields.next(), out);
            }
            FieldKind::Reset => self.set_text(&mut field.label, fields.next(), out),
        }
        ok
    }

    pub(super) fn option_line(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let Some(field) = self.last_choice_field else {
            self.issue(IssueKind::OptionWithoutField, out);
            return;
        };
        let mut fields = Args::new(args);
        let mut option = FormOption {
            field,
            value: Text::new(),
            display: Text::new(),
            selected: false,
            column: None,
            line: self.line_number,
        };
        self.set_text(&mut option.value, fields.next(), out);
        self.set_text(&mut option.display, fields.next(), out);
        let selected = flag(fields.next());
        let column_text = fields.next();
        let column = decimal(column_text).and_then(|value| u8::try_from(value).ok());
        let columns = u8::try_from(COLUMNS).unwrap_or(u8::MAX);
        let column_ok = column_text.trim_ascii().is_empty() || column.is_some_and(|column| column < columns);
        option.selected = selected.unwrap_or(false);
        option.column = column.filter(|&column| column < columns);
        if selected.is_none() || !column_ok {
            self.issue(IssueKind::BadField, out);
        }
        if !self.page.form.options.push(option) {
            self.diagnostic(DiagnosticKind::CapacityExceeded { what: Capacity::Options }, out);
        }
    }

    pub(super) fn form_submission(&mut self, args: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut fields = Args::new(args);
        let method = match fields.next().trim_ascii() {
            method if method.eq_ignore_ascii_case(b"POST") => Some(FormMethod::Post),
            method if method.eq_ignore_ascii_case(b"GET") => Some(FormMethod::Get),
            _ => None,
        };
        let Some(method) = method else {
            self.diagnostic(DiagnosticKind::BadArgument { command: *b"FS" }, out);
            return;
        };
        let mut address = Text::new();
        self.set_text(&mut address, fields.rest().trim_ascii(), out);
        self.page.form.submission = Some((method, address));
    }

    /// Checks the form for overlapping fields (Appendix C.2) and decides
    /// whether it may be interactive.
    pub(super) fn finish_form(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        let mut cells = [0u64; ROWS];
        let mut overlap: Option<u32> = None;
        let form = &self.page.form;
        for (index, field) in form.fields.as_slice().iter().enumerate() {
            let mut overlaps = mark_cells(&mut cells, field.row, field.rows, field.column, field.width);
            if field.kind == FieldKind::Radio {
                // The first choice's marker is the field's column; the others
                // give their own (Appendix C.3).
                let options = form.options.as_slice().iter().filter(|option| usize::from(option.field) == index);
                for column in options.skip(1).filter_map(|option| option.column) {
                    overlaps |= mark_cells(&mut cells, field.row, field.rows, column, field.width);
                }
            }
            if overlaps {
                overlap = overlap.or(Some(field.line));
            }
        }
        if let Some(line) = overlap {
            self.issue_at(IssueKind::FieldOverlap, line, out);
        }
        let has_fields = !self.page.form.fields.is_empty();
        let blocked = self.page.issues.as_slice().iter().any(|issue| issue.kind.affects_form());
        self.page.form.interactive = has_fields && !blocked;
    }
}

/// Marks a field's cells, clipped to the page. Returns true when any cell
/// was already marked.
fn mark_cells(cells: &mut [u64; ROWS], row: u8, rows: u8, column: u8, width: u8) -> bool {
    let mut overlapped = false;
    let end_column = usize::from(column).saturating_add(usize::from(width)).min(COLUMNS);
    let mut mask = 0u64;
    for bit in usize::from(column)..end_column {
        mask |= 1u64.checked_shl(u32::try_from(bit).unwrap_or(u32::MAX)).unwrap_or(0);
    }
    let end_row = usize::from(row).saturating_add(usize::from(rows));
    for slot in cells.iter_mut().take(end_row).skip(usize::from(row)) {
        overlapped |= *slot & mask != 0;
        *slot |= mask;
    }
    overlapped
}
