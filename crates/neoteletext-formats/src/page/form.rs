//! The records of the TTI Form Extension (Appendix C).

use neoteletext_core::consts::COLUMNS;
use neoteletext_core::types::{FixedVec, PageAddress};

use crate::text::Text;

/// Form fields per page.
pub const FORM_FIELDS: usize = 32;
/// Form options per page, over all SELECT and RADIO fields.
pub const FORM_OPTIONS: usize = 64;
/// Bytes of a field name or option value.
pub const FIELD_NAME_BYTES: usize = 32;
/// Bytes of a field's default, value, label, pattern or option text: a
/// whole row.
pub const FIELD_TEXT_BYTES: usize = COLUMNS;
/// Bytes of a field's minimum, maximum or step.
pub const FIELD_LIMIT_BYTES: usize = 16;
/// Bytes of a form submission address.
pub const FORM_ADDRESS_BYTES: usize = 128;
/// Bytes of a form token.
pub const FORM_TOKEN_BYTES: usize = 64;

/// The field types of the Form Extension (Appendix C.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FieldKind {
    /// TEXT.
    Text,
    /// EMAIL.
    Email,
    /// TEL.
    Telephone,
    /// NUMBER.
    Number,
    /// DATE.
    Date,
    /// TIME.
    Time,
    /// SELECT.
    Select,
    /// RADIO.
    Radio,
    /// CHECKBOX.
    Checkbox,
    /// TEXTAREA.
    TextArea,
    /// SUBMIT.
    Submit,
    /// RESET.
    Reset,
}

impl FieldKind {
    /// Every kind with its keyword.
    pub const KEYWORDS: [(Self, &'static [u8]); 12] = [
        (Self::Text, b"TEXT"),
        (Self::Email, b"EMAIL"),
        (Self::Telephone, b"TEL"),
        (Self::Number, b"NUMBER"),
        (Self::Date, b"DATE"),
        (Self::Time, b"TIME"),
        (Self::Select, b"SELECT"),
        (Self::Radio, b"RADIO"),
        (Self::Checkbox, b"CHECKBOX"),
        (Self::TextArea, b"TEXTAREA"),
        (Self::Submit, b"SUBMIT"),
        (Self::Reset, b"RESET"),
    ];

    /// The kind named by a keyword, ignoring case.
    #[must_use]
    pub fn from_keyword(keyword: &[u8]) -> Option<Self> {
        Self::KEYWORDS.iter().find(|(_, name)| name.eq_ignore_ascii_case(keyword)).map(|&(kind, _)| kind)
    }

    /// The keyword written in `FD` lines.
    #[must_use]
    pub fn keyword(self) -> &'static [u8] {
        Self::KEYWORDS.iter().find(|(kind, _)| *kind == self).map_or(&[], |&(_, name)| name)
    }

    /// True for the kinds that take `OPTION` lines.
    #[must_use]
    pub const fn has_options(self) -> bool {
        matches!(self, Self::Select | Self::Radio)
    }
}

/// One field of a form (Appendix C.2 and C.3). Text members that a kind does
/// not use are empty.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormField {
    /// The type.
    pub kind: FieldKind,
    /// The name submitted.
    pub name: Text<FIELD_NAME_BYTES>,
    /// The row of the most recent `OL` line.
    pub row: u8,
    /// The first column.
    pub column: u8,
    /// Columns occupied.
    pub width: u8,
    /// Rows occupied: TEXTAREA's `rows`, otherwise 1.
    pub rows: u8,
    /// `required` was `Y`.
    pub required: bool,
    /// `maxlen`, where given.
    pub max_length: Option<u16>,
    /// TEXT and TEL `pattern`.
    pub pattern: Text<FIELD_TEXT_BYTES>,
    /// The default value of the text-like kinds.
    pub default: Text<FIELD_TEXT_BYTES>,
    /// NUMBER, DATE and TIME minimum.
    pub minimum: Text<FIELD_LIMIT_BYTES>,
    /// NUMBER, DATE and TIME maximum.
    pub maximum: Text<FIELD_LIMIT_BYTES>,
    /// NUMBER step.
    pub step: Text<FIELD_LIMIT_BYTES>,
    /// CHECKBOX value.
    pub value: Text<FIELD_TEXT_BYTES>,
    /// CHECKBOX `checked` was `Y`.
    pub checked: bool,
    /// SUBMIT and RESET label.
    pub label: Text<FIELD_TEXT_BYTES>,
    /// SUBMIT action, sent as `ttx_action`.
    pub action: Text<FIELD_NAME_BYTES>,
    /// The line of the file.
    pub line: u32,
}

impl FormField {
    /// An empty field of a kind.
    #[must_use]
    pub const fn new(kind: FieldKind) -> Self {
        Self {
            kind,
            name: Text::new(),
            row: 0,
            column: 0,
            width: 0,
            rows: 1,
            required: false,
            max_length: None,
            pattern: Text::new(),
            default: Text::new(),
            minimum: Text::new(),
            maximum: Text::new(),
            step: Text::new(),
            value: Text::new(),
            checked: false,
            label: Text::new(),
            action: Text::new(),
            line: 0,
        }
    }
}

/// One `OPTION` line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormOption {
    /// Index of its field in [`Form::fields`].
    pub field: u8,
    /// The value submitted.
    pub value: Text<FIELD_NAME_BYTES>,
    /// The text shown by SELECT.
    pub display: Text<FIELD_TEXT_BYTES>,
    /// `selected` was `Y`.
    pub selected: bool,
    /// RADIO marker column for the second and later choices.
    pub column: Option<u8>,
    /// The line of the file.
    pub line: u32,
}

/// The HTTP method of a form submission.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FormMethod {
    /// POST.
    Post,
    /// GET.
    Get,
}

/// A form: the declarative description of Appendix C. The receiver runs it;
/// this crate only reads it.
#[derive(Clone, Copy, Debug)]
pub struct Form {
    /// The fields in file order.
    pub fields: FixedVec<FormField, FORM_FIELDS>,
    /// The options in file order.
    pub options: FixedVec<FormOption, FORM_OPTIONS>,
    /// `FS`: method and address.
    pub submission: Option<(FormMethod, Text<FORM_ADDRESS_BYTES>)>,
    /// `FP`: the success and error pages.
    pub result_pages: Option<(PageAddress, PageAddress)>,
    /// `FT`: the token.
    pub token: Text<FORM_TOKEN_BYTES>,
    /// True when the form may be made interactive: it has fields and no
    /// form problem was found (Appendix C.2 and C.6).
    pub interactive: bool,
}

impl Form {
    /// No form.
    #[must_use]
    pub const fn new() -> Self {
        let option =
            FormOption { field: 0, value: Text::new(), display: Text::new(), selected: false, column: None, line: 0 };
        Self {
            fields: FixedVec::new(FormField::new(FieldKind::Text)),
            options: FixedVec::new(option),
            submission: None,
            result_pages: None,
            token: Text::new(),
            interactive: false,
        }
    }

    /// True when the page has any form command.
    #[must_use]
    pub fn is_present(&self) -> bool {
        !self.fields.is_empty() || self.submission.is_some()
    }
}

// `FixedVec`'s derived equality also compares its unused slots, which hold
// whatever an earlier page left; pages compare by what they hold.
impl PartialEq for Form {
    fn eq(&self, other: &Self) -> bool {
        self.fields.as_slice() == other.fields.as_slice()
            && self.options.as_slice() == other.options.as_slice()
            && self.submission == other.submission
            && self.result_pages == other.result_pages
            && self.token == other.token
            && self.interactive == other.interactive
    }
}

impl Eq for Form {}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}
