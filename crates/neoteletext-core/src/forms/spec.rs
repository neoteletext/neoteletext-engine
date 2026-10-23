//! A form's declaration (Appendix C.2 and C.3): its fields, options,
//! method and target.

use super::{MAX_FIELDS, MAX_OPTIONS, Name, Short, TARGET_BYTES, TOKEN_BYTES, Value};
use crate::types::{FixedVec, PageAddress};

/// Field types (Appendix C.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldKind {
    /// Free text.
    Text,
    /// An email address.
    Email,
    /// A telephone number.
    Tel,
    /// A whole number.
    Number,
    /// A date, year-month-day.
    Date,
    /// A time, hours and minutes.
    Time,
    /// One choice shown at a time.
    Select,
    /// One choice of several markers.
    Radio,
    /// One marker cell.
    Checkbox,
    /// Several rows of text.
    TextArea,
    /// A submit button.
    Submit,
    /// A reset button.
    Reset,
}

/// One field definition (`FD`, Appendix C.2 and C.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FieldSpec {
    /// The type.
    pub kind: FieldKind,
    /// The name sent on submission.
    pub name: Name,
    /// The row of the most recent `OL` line.
    pub row: u8,
    /// First column.
    pub column: u8,
    /// Columns occupied.
    pub width: u8,
    /// Rows occupied (TEXTAREA), otherwise 1.
    pub rows: u8,
    /// Must be filled.
    pub required: bool,
    /// Maximum length, 0 for none.
    pub maxlen: u16,
    /// Simple pattern: `9` digit, `A` letter, `*` any.
    pub pattern: Short,
    /// NUMBER minimum, maximum and step.
    pub min: i32,
    /// NUMBER maximum.
    pub max: i32,
    /// NUMBER step, 0 for none.
    pub step: i32,
    /// DATE or TIME minimum, as text.
    pub min_text: Short,
    /// DATE or TIME maximum, as text.
    pub max_text: Short,
    /// Default value, as G0 codes.
    pub default: Value,
    /// CHECKBOX default.
    pub checked: bool,
    /// CHECKBOX value sent when checked.
    pub value: Name,
    /// SUBMIT or RESET label.
    pub label: Short,
    /// SUBMIT action sent as `ttx_action`.
    pub action: Name,
    /// First option index for SELECT and RADIO.
    pub first_option: u8,
    /// Number of options.
    pub option_count: u8,
}

impl FieldSpec {
    /// An empty TEXT field at a position, for building specs.
    #[must_use]
    pub const fn new(kind: FieldKind, row: u8, column: u8, width: u8) -> Self {
        Self {
            kind,
            name: FixedVec::new(0),
            row,
            column,
            width,
            rows: 1,
            required: false,
            maxlen: 0,
            pattern: FixedVec::new(0),
            min: i32::MIN,
            max: i32::MAX,
            step: 0,
            min_text: FixedVec::new(0),
            max_text: FixedVec::new(0),
            default: FixedVec::new(0),
            checked: false,
            value: FixedVec::new(0),
            label: FixedVec::new(0),
            action: FixedVec::new(0),
            first_option: 0,
            option_count: 0,
        }
    }

    /// True when the field takes typed characters.
    #[must_use]
    pub const fn is_textual(&self) -> bool {
        matches!(
            self.kind,
            FieldKind::Text
                | FieldKind::Email
                | FieldKind::Tel
                | FieldKind::Number
                | FieldKind::Date
                | FieldKind::Time
                | FieldKind::TextArea
        )
    }

    pub(super) fn cells(&self) -> impl Iterator<Item = (usize, usize)> {
        self.cells_at(self.column)
    }

    /// The cells the field would occupy starting at `column`: a RADIO
    /// choice's marker has the field's size at the choice's own column.
    pub(super) fn cells_at(&self, column: u8) -> impl Iterator<Item = (usize, usize)> {
        let (row, width, rows) = (self.row, self.width, self.rows.max(1));
        (0..rows).flat_map(move |r| {
            (0..width).map(move |c| (usize::from(row.saturating_add(r)), usize::from(column.saturating_add(c))))
        })
    }
}

/// One choice (`OPTION`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OptionSpec {
    /// Value sent.
    pub value: Name,
    /// Text shown for SELECT, as G0 codes.
    pub display: Short,
    /// Selected by default.
    pub selected: bool,
    /// RADIO marker column for choices after the first.
    pub column: Option<u8>,
}

/// Submission method (`FS`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    /// POST.
    Post,
    /// GET.
    Get,
}

/// A whole form (Appendix C).
#[derive(Clone, Copy, Debug)]
pub struct FormSpec {
    /// The fields in page order.
    pub fields: FixedVec<FieldSpec, MAX_FIELDS>,
    /// The options of all SELECT and RADIO fields.
    pub options: FixedVec<OptionSpec, MAX_OPTIONS>,
    /// Submission method.
    pub method: Method,
    /// Submission address as written (`FS`).
    pub target: FixedVec<u8, TARGET_BYTES>,
    /// Success page (`FP`).
    pub success: Option<PageAddress>,
    /// Error page (`FP`).
    pub error: Option<PageAddress>,
    /// Form token (`FT`).
    pub token: FixedVec<u8, TOKEN_BYTES>,
}

/// Why a form could not be made interactive (Appendix C.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormError {
    /// Two fields overlap.
    Overlap,
    /// A field lies outside the page.
    OutOfPage,
    /// A field's options are out of range.
    BadOptions,
}
