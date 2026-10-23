//! The form model (Appendix C, REQ-NAV-009, layer L6): fields, focus,
//! editing from a keyboard or a remote control, validation, and drawing the
//! values over the page. Submission itself belongs to the web module; the
//! two halves together implement the forms of REQ-WEB-010.
//!
//! A form is declarative data: field types, positions and validation rules
//! chosen from a fixed set. Nothing in it is ever run as code (REQ-SEC-004).
//!
//! Values are held as 7-bit codes of the page's G0 set, so every value can
//! be drawn; characters the set cannot show are refused at entry (decision
//! E.31 item 5).

use crate::charset::G0;
use crate::consts::{COLUMNS, ROWS};
use crate::types::{FixedVec, Instant};

/// Fields per form.
pub const MAX_FIELDS: usize = 32;
/// Options per form, over all SELECT and RADIO fields.
pub const MAX_OPTIONS: usize = 64;
/// Bytes of a field name, option value or button action.
pub const NAME_BYTES: usize = 32;
/// Characters of a field value.
pub const VALUE_CHARS: usize = 256;
/// Characters of a pattern, label or limit.
pub const SHORT_CHARS: usize = 40;
/// Bytes of the submission address.
pub const TARGET_BYTES: usize = 256;
/// Bytes of the form token.
pub const TOKEN_BYTES: usize = 128;
/// Microseconds within which a repeated digit cycles letters (decision 12
/// of IMPLEMENTATION-LOG.md).
pub const MULTITAP_US: u64 = 1_000_000;

/// A field name, option value or action.
pub type Name = FixedVec<u8, NAME_BYTES>;
/// A field value as G0 codes.
pub type Value = FixedVec<u8, VALUE_CHARS>;
/// A short text: pattern, label, limit.
pub type Short = FixedVec<u8, SHORT_CHARS>;

mod draw;
mod editing;
mod spec;
mod submission;
mod validation;

pub use editing::{EntryMode, FormAction, FormKey};
pub use spec::{FieldKind, FieldSpec, FormError, FormSpec, Method, OptionSpec};
pub use validation::Invalid;

/// A form in progress.
#[derive(Clone, Copy, Debug)]
pub struct FormModel {
    spec: FormSpec,
    values: [Value; MAX_FIELDS],
    choice: [u8; MAX_FIELDS],
    checked: u64,
    focus: usize,
    cursor: usize,
    mode: EntryMode,
    multitap: Option<(u8, usize, Instant)>,
    g0: G0,
}

impl FormModel {
    /// Makes a form interactive. Overlapping or out-of-page fields refuse it
    /// (Appendix C.2), and the page is shown as an ordinary page.
    pub fn new(spec: FormSpec, g0: G0) -> Result<Self, FormError> {
        let fields = spec.fields.as_slice();
        for (index, field) in fields.iter().enumerate() {
            let end_column = usize::from(field.column).saturating_add(usize::from(field.width));
            let end_row = usize::from(field.row).saturating_add(usize::from(field.rows.max(1)));
            if end_column > COLUMNS || end_row > ROWS || field.width == 0 {
                return Err(FormError::OutOfPage);
            }
            let first = usize::from(field.first_option);
            if first.saturating_add(usize::from(field.option_count)) > spec.options.len() {
                return Err(FormError::BadOptions);
            }
            for other in fields.iter().skip(index.saturating_add(1)) {
                if field.cells().any(|cell| other.cells().any(|o| o == cell)) {
                    return Err(FormError::Overlap);
                }
            }
        }
        let mut model = Self {
            spec,
            values: [FixedVec::new(0); MAX_FIELDS],
            choice: [0; MAX_FIELDS],
            checked: 0,
            focus: 0,
            cursor: 0,
            mode: EntryMode::Letters,
            multitap: None,
            g0,
        };
        model.reset();
        Ok(model)
    }

    /// The form definition.
    #[must_use]
    pub const fn spec(&self) -> &FormSpec {
        &self.spec
    }

    /// The focused field.
    #[must_use]
    pub const fn focus(&self) -> usize {
        self.focus
    }

    /// The remote entry mode, shown in the status area.
    #[must_use]
    pub const fn mode(&self) -> EntryMode {
        self.mode
    }

    /// Restores every field to its default (RESET).
    pub fn reset(&mut self) {
        let fields = self.spec.fields;
        for (index, field) in fields.as_slice().iter().enumerate() {
            if let Some(value) = self.values.get_mut(index) {
                *value = field.default;
            }
            let first = usize::from(field.first_option);
            let options = self.spec.options.as_slice();
            let selected = (0..usize::from(field.option_count))
                .find(|&o| options.get(first.saturating_add(o)).is_some_and(|option| option.selected))
                .unwrap_or(0);
            if let Some(choice) = self.choice.get_mut(index) {
                *choice = selected as u8;
            }
            let bit = 1u64.checked_shl(index as u32).unwrap_or(0);
            if field.checked { self.checked |= bit } else { self.checked &= !bit }
        }
        self.cursor = 0;
        self.multitap = None;
    }

    fn field(&self) -> Option<&FieldSpec> {
        self.spec.fields.as_slice().get(self.focus)
    }

    /// The value of a field, as G0 codes.
    #[must_use]
    pub fn value(&self, index: usize) -> &[u8] {
        self.values.get(index).map_or(&[], FixedVec::as_slice)
    }
}

#[cfg(test)]
mod tests;
