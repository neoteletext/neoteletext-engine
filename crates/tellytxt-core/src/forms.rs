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

use crate::charset::{CharRef, CharSet, G0, to_unicode};
use crate::consts::{CODE_MASK_7BIT, COLUMNS, ROWS, SPACE};
use crate::level1::{Cell, Glyph, Grid};
use crate::types::{FixedVec, Instant, PageAddress};

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

    fn cells(&self) -> impl Iterator<Item = (usize, usize)> {
        self.cells_at(self.column)
    }

    /// The cells the field would occupy starting at `column`: a RADIO
    /// choice's marker has the field's size at the choice's own column.
    fn cells_at(&self, column: u8) -> impl Iterator<Item = (usize, usize)> {
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

/// Why validation failed (Appendix C.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Invalid {
    /// A required field is empty.
    Required,
    /// Longer than `maxlen`.
    TooLong,
    /// Does not match the pattern.
    Pattern,
    /// Not a whole number, out of range or off-step.
    Number,
    /// Not a valid date or time in range.
    DateTime,
    /// Not an email address.
    Email,
    /// Not a telephone number.
    Tel,
}

/// Keys a form understands (REQ-NAV-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormKey {
    /// Tab: next field.
    Tab,
    /// Shift-Tab: previous field.
    BackTab,
    /// A typed character.
    Char(char),
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow: previous choice.
    Up,
    /// Down arrow: next choice.
    Down,
    /// Space bar: toggle a checkbox, or type a space.
    Space,
    /// Enter: press the focused button.
    Enter,
    /// Backspace.
    Backspace,
    /// A digit key on a remote control.
    RemoteDigit(u8),
    /// Red: previous field.
    Red,
    /// Green: next field.
    Green,
    /// Yellow: change the focused field.
    Yellow,
    /// Cyan: press the focused button or submit.
    Cyan,
    /// Index: leave the form.
    Index,
}

/// What a key did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormAction {
    /// Nothing more to do.
    None,
    /// The character cannot be shown by the page's set and was refused.
    Refused,
    /// Submit, naming the button pressed, if any.
    Submit(Option<usize>),
    /// Validation failed at a field; focus moved there.
    Invalid(usize, Invalid),
    /// Leave the form without submitting.
    Leave,
}

/// Remote text entry mode (REQ-NAV-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryMode {
    /// Digit keys give letters, telephone style.
    Letters,
    /// Digit keys give digits.
    Digits,
}

/// ITU E.161 keypad letters (decision 12 of IMPLEMENTATION-LOG.md).
const KEYPAD: [&[u8]; 10] =
    [b" 0", b".,-@_1", b"ABC2", b"DEF3", b"GHI4", b"JKL5", b"MNO6", b"PQRS7", b"TUV8", b"WXYZ9"];

/// The code drawn in a checked CHECKBOX and at the chosen RADIO choice:
/// "*" at 2/A, which is not a national option position, as Appendix C.4
/// recommends for markers.
const MARKER: u8 = b'*';

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

    fn move_focus(&mut self, forward: bool) {
        let count = self.spec.fields.len();
        if count == 0 {
            return;
        }
        self.focus = if forward {
            self.focus.saturating_add(1).checked_rem(count).unwrap_or(0)
        } else {
            self.focus.checked_sub(1).unwrap_or(count.saturating_sub(1))
        };
        self.cursor = self.value(self.focus).len();
        self.multitap = None;
    }

    /// The G0 code that shows `c`, if the page's set has one.
    fn code_for(&self, c: char) -> Option<u8> {
        (SPACE..CODE_MASK_7BIT).find(|&code| {
            let unicode = to_unicode(CharRef::new(CharSet::G0(self.g0), code));
            unicode.base == c && unicode.combining.is_none()
        })
    }

    fn insert(&mut self, c: char) -> FormAction {
        let Some(field) = self.field().copied() else { return FormAction::None };
        if !field.is_textual() {
            return FormAction::None;
        }
        let Some(code) = self.code_for(c) else { return FormAction::Refused };
        let limit = if field.maxlen == 0 { VALUE_CHARS } else { usize::from(field.maxlen).min(VALUE_CHARS) };
        let cursor = self.cursor;
        let Some(value) = self.values.get_mut(self.focus) else { return FormAction::None };
        if value.len() >= limit {
            return FormAction::Refused;
        }
        if value.push(code) {
            let slice = value.as_mut_slice();
            let len = slice.len();
            if let Some(tail) = slice.get_mut(cursor.min(len.saturating_sub(1))..) {
                tail.rotate_right(1);
            }
            self.cursor = cursor.saturating_add(1);
        }
        FormAction::None
    }

    fn backspace(&mut self) {
        let Some(at) = self.cursor.checked_sub(1) else { return };
        if let Some(value) = self.values.get_mut(self.focus) {
            let _ = value.remove(at);
            self.cursor = at;
        }
    }

    fn step_choice(&mut self, forward: bool) {
        let Some(field) = self.field().copied() else { return };
        let count = field.option_count;
        if count == 0 {
            return;
        }
        if let Some(choice) = self.choice.get_mut(self.focus) {
            *choice = if forward {
                choice.saturating_add(1).checked_rem(count).unwrap_or(0)
            } else {
                choice.checked_sub(1).unwrap_or(count.saturating_sub(1))
            };
        }
    }

    fn toggle(&mut self) {
        let bit = 1u64.checked_shl(self.focus as u32).unwrap_or(0);
        self.checked ^= bit;
    }

    fn press(&mut self) -> FormAction {
        match self.field().map(|field| field.kind) {
            Some(FieldKind::Submit) => self.submit(Some(self.focus)),
            Some(FieldKind::Reset) => {
                self.reset();
                FormAction::None
            }
            _ => self.submit(None),
        }
    }

    fn submit(&mut self, button: Option<usize>) -> FormAction {
        match self.validate() {
            Ok(()) => FormAction::Submit(button),
            Err((index, reason)) => {
                self.focus = index;
                self.cursor = self.value(index).len();
                FormAction::Invalid(index, reason)
            }
        }
    }

    /// Handles a key (REQ-NAV-009).
    pub fn key(&mut self, key: FormKey, now: Instant) -> FormAction {
        if !matches!(key, FormKey::RemoteDigit(_)) {
            self.multitap = None;
        }
        let kind = self.field().map(|field| field.kind);
        match key {
            FormKey::Tab | FormKey::Green => {
                self.move_focus(true);
                FormAction::None
            }
            FormKey::BackTab | FormKey::Red => {
                self.move_focus(false);
                FormAction::None
            }
            FormKey::Index => FormAction::Leave,
            FormKey::Cyan | FormKey::Enter => self.press(),
            FormKey::Char(c) => self.insert(c),
            FormKey::Space => match kind {
                Some(FieldKind::Checkbox) => {
                    self.toggle();
                    FormAction::None
                }
                _ => self.insert(' '),
            },
            FormKey::Backspace => {
                self.backspace();
                FormAction::None
            }
            FormKey::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                FormAction::None
            }
            FormKey::Right => {
                self.cursor = self.cursor.saturating_add(1).min(self.value(self.focus).len());
                FormAction::None
            }
            FormKey::Up | FormKey::Down => {
                self.step_choice(key == FormKey::Down);
                FormAction::None
            }
            FormKey::Yellow => {
                match kind {
                    Some(FieldKind::Select | FieldKind::Radio) => self.step_choice(true),
                    Some(FieldKind::Checkbox) => self.toggle(),
                    _ => {
                        self.mode = match self.mode {
                            EntryMode::Letters => EntryMode::Digits,
                            EntryMode::Digits => EntryMode::Letters,
                        };
                    }
                }
                FormAction::None
            }
            FormKey::RemoteDigit(digit) => self.remote_digit(digit, now),
        }
    }

    fn remote_digit(&mut self, digit: u8, now: Instant) -> FormAction {
        let Some(letters) = KEYPAD.get(usize::from(digit)) else { return FormAction::None };
        if self.mode == EntryMode::Digits || !self.field().is_some_and(FieldSpec::is_textual) {
            self.multitap = None;
            return self.insert(char::from(b'0'.saturating_add(digit)));
        }
        match self.multitap {
            Some((last, index, at)) if last == digit && now.since(at) < MULTITAP_US => {
                // Cycle: replace the character just typed with the next.
                let next = index.saturating_add(1).checked_rem(letters.len()).unwrap_or(0);
                self.backspace();
                let c = letters.get(next).copied().map_or(' ', char::from);
                self.multitap = Some((digit, next, now));
                self.insert(c)
            }
            _ => {
                let c = letters.first().copied().map_or(' ', char::from);
                self.multitap = Some((digit, 0, now));
                self.insert(c)
            }
        }
    }

    /// Checks every field (Appendix C.5). The first failure is returned.
    pub fn validate(&self) -> Result<(), (usize, Invalid)> {
        for (index, field) in self.spec.fields.as_slice().iter().enumerate() {
            let value = self.value(index);
            let text = |out: &mut [u8; VALUE_CHARS]| -> usize {
                let mut len = 0usize;
                for (slot, &code) in out.iter_mut().zip(value) {
                    *slot = ascii_of(code, self.g0);
                    len = len.saturating_add(1);
                }
                len
            };
            let mut buffer = [0u8; VALUE_CHARS];
            let len = text(&mut buffer);
            let ascii = buffer.get(..len).unwrap_or(&[]);
            if !field.is_textual() {
                continue;
            }
            if ascii.is_empty() {
                if field.required {
                    return Err((index, Invalid::Required));
                }
                continue;
            }
            if field.maxlen != 0 && ascii.len() > usize::from(field.maxlen) {
                return Err((index, Invalid::TooLong));
            }
            let pattern = field.pattern.as_slice();
            if !pattern.is_empty() && !matches_pattern(ascii, pattern) {
                return Err((index, Invalid::Pattern));
            }
            let check = match field.kind {
                FieldKind::Email => valid_email(ascii).then_some(()).ok_or(Invalid::Email),
                FieldKind::Tel => (pattern.is_empty() && !valid_tel(ascii)).then_some(Invalid::Tel).map_or(Ok(()), Err),
                FieldKind::Number => valid_number(ascii, field).then_some(()).ok_or(Invalid::Number),
                FieldKind::Date => valid_date(ascii, field).then_some(()).ok_or(Invalid::DateTime),
                FieldKind::Time => valid_time(ascii, field).then_some(()).ok_or(Invalid::DateTime),
                _ => Ok(()),
            };
            check.map_err(|reason| (index, reason))?;
        }
        Ok(())
    }

    /// The chosen option of a SELECT or RADIO field. Only the field's own
    /// options count: one with none has no choice (Appendix C.3).
    fn chosen_option(&self, index: usize, field: &FieldSpec) -> Option<&OptionSpec> {
        let choice = self.choice.get(index).copied().filter(|&choice| choice < field.option_count)?;
        self.spec.options.as_slice().get(usize::from(field.first_option).saturating_add(usize::from(choice)))
    }

    /// The name and value pairs to submit (REQ-WEB-011), values as Unicode
    /// characters. The web module adds `ttx_token` and `ttx_action`.
    pub fn pairs(&self, out: &mut dyn FnMut(&[u8], &[char])) {
        let mut chars = ['\0'; VALUE_CHARS];
        for (index, field) in self.spec.fields.as_slice().iter().enumerate() {
            match field.kind {
                FieldKind::Submit | FieldKind::Reset => {}
                FieldKind::Checkbox => {
                    let checked = self.checked.checked_shr(index as u32).is_some_and(|bits| bits & 1 == 1);
                    if checked {
                        let mut len = 0usize;
                        for (slot, &byte) in chars.iter_mut().zip(field.value.as_slice()) {
                            *slot = char::from(byte);
                            len = len.saturating_add(1);
                        }
                        out(field.name.as_slice(), chars.get(..len).unwrap_or(&[]));
                    }
                }
                FieldKind::Select | FieldKind::Radio => {
                    if let Some(option) = self.chosen_option(index, field) {
                        let mut len = 0usize;
                        for (slot, &byte) in chars.iter_mut().zip(option.value.as_slice()) {
                            *slot = char::from(byte);
                            len = len.saturating_add(1);
                        }
                        out(field.name.as_slice(), chars.get(..len).unwrap_or(&[]));
                    }
                }
                _ => {
                    let mut len = 0usize;
                    for (slot, &code) in chars.iter_mut().zip(self.value(index)) {
                        *slot = to_unicode(CharRef::new(CharSet::G0(self.g0), code)).base;
                        len = len.saturating_add(1);
                    }
                    out(field.name.as_slice(), chars.get(..len).unwrap_or(&[]));
                }
            }
        }
    }

    /// Draws the values over the page in the attributes already in force at
    /// those cells (Appendix C.4). The focused field is shown in inverse
    /// video, with the cursor cell flashing.
    pub fn draw(&self, grid: &mut Grid) {
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
    fn draw_radio(&self, index: usize, field: &FieldSpec, focused: bool, grid: &mut Grid) {
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

/// The ASCII form of a G0 code for validation: digits, letters, `@`, `.`
/// and so on map to themselves; anything else becomes `?`.
fn ascii_of(code: u8, g0: G0) -> u8 {
    let c = to_unicode(CharRef::new(CharSet::G0(g0), code)).base;
    if c.is_ascii() { c as u8 } else { b'?' }
}

fn matches_pattern(value: &[u8], pattern: &[u8]) -> bool {
    value.len() <= pattern.len()
        && value.iter().zip(pattern).all(|(&v, &p)| match p {
            b'9' => v.is_ascii_digit(),
            b'A' => v.is_ascii_alphabetic(),
            _ => true,
        })
}

fn valid_email(value: &[u8]) -> bool {
    // Exactly one "@": its first and last positions coincide.
    let first = value.iter().position(|&b| b == b'@');
    let last = value.iter().rposition(|&b| b == b'@');
    first == last && first.is_some_and(|p| p > 0 && p.saturating_add(1) < value.len())
}

fn valid_tel(value: &[u8]) -> bool {
    value.iter().all(|&b| b.is_ascii_digit() || b == b' ' || b == b'+')
}

fn parse_int(value: &[u8]) -> Option<i32> {
    let (negative, digits) = match value.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, value),
    };
    if digits.is_empty() {
        return None;
    }
    let magnitude = digits.iter().try_fold(0i32, |acc, &b| {
        if b.is_ascii_digit() { acc.checked_mul(10)?.checked_add(i32::from(b.wrapping_sub(b'0'))) } else { None }
    })?;
    Some(if negative { magnitude.checked_neg()? } else { magnitude })
}

fn valid_number(value: &[u8], field: &FieldSpec) -> bool {
    let Some(number) = parse_int(value) else { return false };
    let in_range = number >= field.min && number <= field.max;
    let on_step =
        field.step == 0 || number.checked_sub(field.min.max(0)).and_then(|d| d.checked_rem(field.step)) == Some(0);
    in_range && on_step
}

/// `YYYY-MM-DD` with a real month and day; compared as text against limits.
fn valid_date(value: &[u8], field: &FieldSpec) -> bool {
    let well_formed = value.len() == 10
        && value.get(4) == Some(&b'-')
        && value.get(7) == Some(&b'-')
        && value.iter().enumerate().all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !well_formed {
        return false;
    }
    let year = value.get(..4).and_then(parse_int).unwrap_or(0);
    let month = value.get(5..7).and_then(parse_int).unwrap_or(0);
    let day = value.get(8..10).and_then(parse_int).unwrap_or(0);
    (1..=days_in_month(year, month)).contains(&day) && within(value, field)
}

/// Days in a month of the Gregorian calendar (ISO 8601), zero for a month
/// outside 1 to 12.
fn days_in_month(year: i32, month: i32) -> i32 {
    let divides = |divisor: i32| year.checked_rem(divisor) == Some(0);
    let leap = divides(4) && (!divides(100) || divides(400));
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    }
}

/// `HH:MM`.
fn valid_time(value: &[u8], field: &FieldSpec) -> bool {
    let well_formed = value.len() == 5
        && value.get(2) == Some(&b':')
        && value.iter().enumerate().all(|(i, b)| i == 2 || b.is_ascii_digit());
    let hour = value.get(..2).and_then(parse_int).unwrap_or(99);
    let minute = value.get(3..).and_then(parse_int).unwrap_or(99);
    well_formed && hour <= 23 && minute <= 59 && within(value, field)
}

/// Fixed-width dates and times compare correctly as text.
fn within(value: &[u8], field: &FieldSpec) -> bool {
    let min = field.min_text.as_slice();
    let max = field.max_text.as_slice();
    (min.is_empty() || value >= min) && (max.is_empty() || value <= max)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::charset::{G0Set, NationalOption};

    fn name(text: &[u8]) -> Name {
        let mut name = FixedVec::new(0);
        for &b in text {
            name.push(b);
        }
        name
    }

    fn spec() -> FormSpec {
        let mut fields = FixedVec::new(FieldSpec::new(FieldKind::Text, 0, 0, 1));
        let mut text = FieldSpec::new(FieldKind::Text, 3, 7, 10);
        text.name = name(b"name");
        text.required = true;
        fields.push(text);
        let mut email = FieldSpec::new(FieldKind::Email, 4, 7, 20);
        email.name = name(b"email");
        fields.push(email);
        let mut tick = FieldSpec::new(FieldKind::Checkbox, 5, 2, 1);
        tick.name = name(b"news");
        tick.value = name(b"yes");
        fields.push(tick);
        let mut send = FieldSpec::new(FieldKind::Submit, 8, 2, 8);
        send.action = name(b"send");
        fields.push(send);
        FormSpec {
            fields,
            options: FixedVec::new(OptionSpec {
                value: name(b""),
                display: FixedVec::new(0),
                selected: false,
                column: None,
            }),
            method: Method::Post,
            target: FixedVec::new(0),
            success: None,
            error: None,
            token: FixedVec::new(0),
        }
    }

    fn english() -> G0 {
        G0 { set: G0Set::Latin, national: Some(NationalOption::English) }
    }

    #[test]
    fn overlapping_fields_are_refused() {
        let mut bad = spec();
        bad.fields.push(FieldSpec::new(FieldKind::Text, 3, 10, 5));
        assert_eq!(FormModel::new(bad, english()).err(), Some(FormError::Overlap));
    }

    /// REQ-WEB-010: client-side validation.
    #[test]
    fn keyboard_entry_validation_and_pairs() {
        let mut form = FormModel::new(spec(), english()).unwrap();
        let now = Instant(0);
        assert_eq!(form.key(FormKey::Cyan, now), FormAction::Invalid(0, Invalid::Required));
        for c in "Ann".chars() {
            form.key(FormKey::Char(c), now);
        }
        form.key(FormKey::Tab, now);
        for c in "a@b".chars() {
            form.key(FormKey::Char(c), now);
        }
        form.key(FormKey::Tab, now);
        form.key(FormKey::Space, now);
        form.key(FormKey::Tab, now);
        assert_eq!(form.key(FormKey::Enter, now), FormAction::Submit(Some(3)));
        let mut seen = std::vec::Vec::new();
        form.pairs(&mut |name, value| seen.push((name.to_vec(), value.iter().collect::<std::string::String>())));
        assert_eq!(seen[0], (b"name".to_vec(), "Ann".into()));
        assert_eq!(seen[1], (b"email".to_vec(), "a@b".into()));
        assert_eq!(seen[2], (b"news".to_vec(), "yes".into()));
    }

    #[test]
    fn characters_the_set_cannot_show_are_refused() {
        let mut form = FormModel::new(spec(), english()).unwrap();
        // English G0 has no "#" at 2/3 (it is "£"), and no "é" at all.
        assert_eq!(form.key(FormKey::Char('é'), Instant(0)), FormAction::Refused);
        assert_eq!(form.key(FormKey::Char('£'), Instant(0)), FormAction::None);
    }

    #[test]
    fn remote_multitap_letters() {
        let mut form = FormModel::new(spec(), english()).unwrap();
        form.key(FormKey::RemoteDigit(2), Instant(0));
        form.key(FormKey::RemoteDigit(2), Instant(100_000));
        form.key(FormKey::RemoteDigit(2), Instant(200_000));
        form.key(FormKey::RemoteDigit(3), Instant(300_000));
        form.key(FormKey::RemoteDigit(3), Instant(2_000_000));
        let value: std::vec::Vec<u8> = form.value(0).to_vec();
        assert_eq!(value, b"CDD".to_vec());
        form.key(FormKey::Yellow, Instant(3_000_000));
        form.key(FormKey::RemoteDigit(7), Instant(3_100_000));
        assert_eq!(form.value(0), b"CDD7");
    }

    #[test]
    fn number_date_and_time_rules() {
        let mut field = FieldSpec::new(FieldKind::Number, 0, 0, 5);
        field.min = 0;
        field.max = 100;
        field.step = 5;
        assert!(valid_number(b"35", &field));
        assert!(!valid_number(b"36", &field));
        assert!(!valid_number(b"105", &field));
        let date = FieldSpec::new(FieldKind::Date, 0, 0, 10);
        assert!(valid_date(b"2026-10-10", &date));
        assert!(!valid_date(b"2026-13-10", &date));
        // Days per month, with the Gregorian leap years.
        assert!(!valid_date(b"2026-02-31", &date) && !valid_date(b"2026-04-31", &date));
        assert!(valid_date(b"2026-01-31", &date) && valid_date(b"2024-02-29", &date));
        assert!(!valid_date(b"2026-02-29", &date) && !valid_date(b"1900-02-29", &date));
        assert!(valid_date(b"2000-02-29", &date) && !valid_date(b"2026-02-00", &date));
        assert!(valid_time(b"23:59", &date));
        assert!(!valid_time(b"24:00", &date));
        assert!(valid_email(b"a@b") && !valid_email(b"@b") && !valid_email(b"a@b@c"));
    }

    const NOW: Instant = Instant(0);

    fn fixed<const N: usize>(text: &[u8]) -> FixedVec<u8, N> {
        let mut out = FixedVec::new(0);
        for &b in text {
            assert!(out.push(b));
        }
        out
    }

    fn choice(value: &[u8], display: &[u8], selected: bool, column: Option<u8>) -> OptionSpec {
        OptionSpec { value: fixed(value), display: fixed(display), selected, column }
    }

    fn form_spec(fields: &[FieldSpec], options: &[OptionSpec]) -> FormSpec {
        let mut spec = spec();
        spec.fields.clear();
        for &field in fields {
            assert!(spec.fields.push(field));
        }
        for &option in options {
            assert!(spec.options.push(option));
        }
        spec
    }

    fn form(fields: &[FieldSpec], options: &[OptionSpec]) -> FormModel {
        FormModel::new(form_spec(fields, options), english()).unwrap()
    }

    fn type_text(form: &mut FormModel, text: &str) {
        for c in text.chars() {
            assert_eq!(form.key(FormKey::Char(c), NOW), FormAction::None, "typing {c:?}");
        }
    }

    /// Validates `text` typed into a form holding only `field`.
    fn check(field: &FieldSpec, text: &str) -> Result<(), Invalid> {
        let mut form = form(&[*field], &[]);
        type_text(&mut form, text);
        form.validate().map_err(|(index, reason)| {
            assert_eq!(index, 0);
            reason
        })
    }

    fn submitted(form: &FormModel) -> std::vec::Vec<(std::string::String, std::string::String)> {
        let mut seen = std::vec::Vec::new();
        form.pairs(&mut |name, value| {
            seen.push((std::string::String::from_utf8(name.to_vec()).unwrap(), value.iter().collect()));
        });
        seen
    }

    fn pair(name: &str, value: &str) -> (std::string::String, std::string::String) {
        (name.into(), value.into())
    }

    /// Appendix C.2: a field must fit within the page, and its options must
    /// exist.
    #[test]
    fn fields_outside_the_page_or_options_are_refused() {
        let past_right = FieldSpec::new(FieldKind::Text, 1, 35, 6);
        let mut past_bottom = FieldSpec::new(FieldKind::TextArea, 23, 0, 10);
        past_bottom.rows = 3;
        let zero_width = FieldSpec::new(FieldKind::Text, 1, 0, 0);
        for field in [past_right, past_bottom, zero_width] {
            assert_eq!(
                FormModel::new(form_spec(&[field], &[]), english()).err(),
                Some(FormError::OutOfPage),
                "{field:?}"
            );
        }
        // The last cell of the page is inside it.
        let last = FieldSpec::new(FieldKind::Text, 24, 39, 1);
        assert!(FormModel::new(form_spec(&[last], &[]), english()).is_ok());
        let mut select = FieldSpec::new(FieldKind::Select, 2, 0, 5);
        select.option_count = 2;
        let one = [choice(b"a", b"A", false, None)];
        assert_eq!(FormModel::new(form_spec(&[select], &one), english()).err(), Some(FormError::BadOptions));
    }

    #[test]
    fn a_new_form_starts_at_the_first_field_in_letter_mode() {
        let form = FormModel::new(spec(), english()).unwrap();
        assert_eq!(form.focus(), 0);
        assert_eq!(form.mode(), EntryMode::Letters);
        assert_eq!(form.spec().fields.len(), 4);
        assert_eq!(form.spec().method, Method::Post);
    }

    /// REQ-NAV-009: Tab and Green move forward, Shift-Tab and Red back,
    /// both wrapping; Index leaves.
    #[test]
    fn focus_moves_both_ways_and_wraps() {
        let mut form = FormModel::new(spec(), english()).unwrap();
        let mut focus = std::vec::Vec::new();
        for key in [FormKey::BackTab, FormKey::Red, FormKey::Green, FormKey::Tab, FormKey::Tab] {
            assert_eq!(form.key(key, NOW), FormAction::None);
            focus.push(form.focus());
        }
        assert_eq!(focus, [3, 2, 3, 0, 1]);
        assert_eq!(form.key(FormKey::Index, NOW), FormAction::Leave);
    }

    /// A form with no fields has nothing to focus or edit, and submits.
    #[test]
    fn an_empty_form_only_submits_or_leaves() {
        let mut form = form(&[], &[]);
        for key in [FormKey::Tab, FormKey::BackTab, FormKey::Char('a'), FormKey::Up, FormKey::Backspace, FormKey::Space]
        {
            assert_eq!(form.key(key, NOW), FormAction::None, "{key:?}");
            assert_eq!(form.focus(), 0);
        }
        assert_eq!(form.key(FormKey::Enter, NOW), FormAction::Submit(None));
        assert_eq!(form.key(FormKey::Index, NOW), FormAction::Leave);
        assert_eq!(submitted(&form), []);
    }

    /// REQ-NAV-009: typing inserts at the cursor, the arrows move it within
    /// the value, Backspace deletes before it, and the space bar types a
    /// space in a text field.
    #[test]
    fn cursor_editing() {
        let mut text = FieldSpec::new(FieldKind::Text, 1, 0, 20);
        text.name = name(b"t");
        let mut form = form(&[text], &[]);
        type_text(&mut form, "ac");
        form.key(FormKey::Left, NOW);
        type_text(&mut form, "b");
        assert_eq!(form.value(0), b"abc");
        // Right stops at the end of the value; Left stops at the start.
        for _ in 0..5 {
            form.key(FormKey::Right, NOW);
        }
        assert_eq!(form.key(FormKey::Space, NOW), FormAction::None);
        type_text(&mut form, "d");
        assert_eq!(form.value(0), b"abc d");
        for _ in 0..9 {
            form.key(FormKey::Left, NOW);
        }
        assert_eq!(form.key(FormKey::Backspace, NOW), FormAction::None);
        assert_eq!(form.value(0), b"abc d", "nothing before the cursor");
        form.key(FormKey::Right, NOW);
        form.key(FormKey::Right, NOW);
        form.key(FormKey::Backspace, NOW);
        assert_eq!(form.value(0), b"ac d");
        type_text(&mut form, "B");
        assert_eq!(form.value(0), b"aBc d");
        assert_eq!(submitted(&form), [pair("t", "aBc d")]);
    }

    /// Appendix C.5: `maxlen` stops entry at the limit.
    #[test]
    fn entry_stops_at_maxlen() {
        let mut text = FieldSpec::new(FieldKind::Text, 1, 0, 20);
        text.maxlen = 3;
        let mut form = form(&[text], &[]);
        type_text(&mut form, "abc");
        assert_eq!(form.key(FormKey::Char('d'), NOW), FormAction::Refused);
        assert_eq!(form.value(0), b"abc");
    }

    /// Characters go only into fields that take text.
    #[test]
    fn typing_into_a_marker_or_button_does_nothing() {
        let tick = FieldSpec::new(FieldKind::Checkbox, 1, 0, 1);
        let mut form = form(&[tick, FieldSpec::new(FieldKind::Submit, 2, 0, 4)], &[]);
        assert_eq!(form.key(FormKey::Char('x'), NOW), FormAction::None);
        assert_eq!(form.key(FormKey::RemoteDigit(5), NOW), FormAction::None);
        assert_eq!(form.value(0), b"");
        form.key(FormKey::Tab, NOW);
        assert_eq!(form.key(FormKey::Char('x'), NOW), FormAction::None);
        assert_eq!(form.value(1), b"");
    }

    fn colours() -> ([FieldSpec; 3], [OptionSpec; 5]) {
        let mut colour = FieldSpec::new(FieldKind::Select, 2, 10, 5);
        colour.name = name(b"colour");
        colour.option_count = 3;
        let mut size = FieldSpec::new(FieldKind::Radio, 6, 10, 1);
        size.name = name(b"size");
        size.first_option = 3;
        size.option_count = 2;
        let mut tick = FieldSpec::new(FieldKind::Checkbox, 8, 2, 1);
        tick.name = name(b"news");
        tick.value = name(b"yes");
        let options = [
            choice(b"r", b"Red", false, None),
            choice(b"g", b"Green", true, None),
            choice(b"b", b"Blue", false, None),
            choice(b"s", b"Small", false, None),
            choice(b"l", b"Large", false, Some(20)),
        ];
        ([colour, size, tick], options)
    }

    /// REQ-NAV-009: the arrows and Yellow step SELECT and RADIO choices,
    /// wrapping; the default is the option marked selected, else the first.
    #[test]
    fn choices_step_and_wrap() {
        let (fields, options) = colours();
        let mut form = form(&fields, &options);
        assert_eq!(submitted(&form), [pair("colour", "g"), pair("size", "s")]);
        let mut seen = std::vec::Vec::new();
        for key in [FormKey::Down, FormKey::Down, FormKey::Up, FormKey::Up, FormKey::Up, FormKey::Yellow] {
            form.key(key, NOW);
            seen.push(submitted(&form)[0].1.clone());
        }
        assert_eq!(seen, ["b", "r", "b", "g", "r", "g"]);
        form.key(FormKey::Tab, NOW);
        form.key(FormKey::Yellow, NOW);
        assert_eq!(submitted(&form)[1], pair("size", "l"));
        form.key(FormKey::Yellow, NOW);
        assert_eq!(submitted(&form)[1], pair("size", "s"));
        form.key(FormKey::Up, NOW);
        assert_eq!(submitted(&form)[1], pair("size", "l"));
        // Arrows on a field with no choices change nothing.
        form.key(FormKey::Tab, NOW);
        form.key(FormKey::Down, NOW);
        assert_eq!(submitted(&form), [pair("colour", "g"), pair("size", "l")]);
    }

    /// REQ-NAV-009: the space bar and Yellow toggle a checkbox, which is
    /// submitted only when checked (REQ-WEB-011).
    #[test]
    fn checkbox_toggles() {
        let (fields, options) = colours();
        let mut form = form(&fields, &options);
        form.key(FormKey::BackTab, NOW);
        assert_eq!(form.focus(), 2);
        form.key(FormKey::Space, NOW);
        assert_eq!(submitted(&form)[2], pair("news", "yes"));
        form.key(FormKey::Yellow, NOW);
        assert_eq!(submitted(&form).len(), 2);
        form.key(FormKey::Yellow, NOW);
        assert_eq!(submitted(&form)[2], pair("news", "yes"));
    }

    /// Appendix C.3: RESET restores every field to its default.
    #[test]
    fn reset_restores_defaults() {
        let (fields, options) = colours();
        let mut text = FieldSpec::new(FieldKind::Text, 10, 0, 10);
        text.name = name(b"t");
        text.default = fixed(b"Hi");
        let reset = FieldSpec::new(FieldKind::Reset, 12, 0, 7);
        let mut checked = fields[2];
        checked.checked = true;
        let mut form = form(&[text, fields[0], fields[1], checked, reset], &options);
        let defaults = submitted(&form);
        assert_eq!(defaults, [pair("t", "Hi"), pair("colour", "g"), pair("size", "s"), pair("news", "yes")]);
        // Coming back to a field puts the cursor after its value.
        form.key(FormKey::BackTab, NOW);
        form.key(FormKey::Tab, NOW);
        type_text(&mut form, "!");
        assert_eq!(form.value(0), b"Hi!");
        form.key(FormKey::Tab, NOW);
        form.key(FormKey::Down, NOW);
        form.key(FormKey::Tab, NOW);
        form.key(FormKey::Down, NOW);
        form.key(FormKey::Tab, NOW);
        form.key(FormKey::Space, NOW);
        assert_ne!(submitted(&form), defaults);
        form.key(FormKey::Tab, NOW);
        assert_eq!(form.key(FormKey::Enter, NOW), FormAction::None);
        assert_eq!(submitted(&form), defaults);
        assert_eq!(form.focus(), 4);
    }

    /// REQ-NAV-009: Yellow switches text entry between letters and digits,
    /// and back.
    #[test]
    fn yellow_switches_entry_mode() {
        let mut form = form(&[FieldSpec::new(FieldKind::Text, 1, 0, 10)], &[]);
        form.key(FormKey::Yellow, NOW);
        assert_eq!(form.mode(), EntryMode::Digits);
        form.key(FormKey::RemoteDigit(4), NOW);
        form.key(FormKey::Yellow, NOW);
        assert_eq!(form.mode(), EntryMode::Letters);
        form.key(FormKey::RemoteDigit(4), Instant(1));
        assert_eq!(form.value(0), b"4G");
        // There is no digit key 10.
        assert_eq!(form.key(FormKey::RemoteDigit(10), Instant(2)), FormAction::None);
        assert_eq!(form.value(0), b"4G");
    }

    /// Appendix C.5 and REQ-WEB-011: a required field must be filled; an
    /// empty optional field passes whatever its type.
    #[test]
    fn required_and_empty_fields() {
        for kind in
            [FieldKind::Text, FieldKind::Email, FieldKind::Tel, FieldKind::Number, FieldKind::Date, FieldKind::Time]
        {
            let mut field = FieldSpec::new(kind, 1, 0, 10);
            assert_eq!(check(&field, ""), Ok(()), "{kind:?}");
            field.required = true;
            assert_eq!(check(&field, ""), Err(Invalid::Required), "{kind:?}");
        }
    }

    /// Appendix C.5: a default longer than `maxlen` fails.
    #[test]
    fn value_longer_than_maxlen_fails() {
        let mut field = FieldSpec::new(FieldKind::Text, 1, 0, 10);
        field.maxlen = 3;
        field.default = fixed(b"abcd");
        assert_eq!(form(&[field], &[]).validate(), Err((0, Invalid::TooLong)));
        field.default = fixed(b"abc");
        assert_eq!(form(&[field], &[]).validate(), Ok(()));
    }

    /// Appendix C.3: `9` a digit, `A` a letter, `*` any character; the
    /// value may be shorter than the pattern, not longer.
    #[test]
    fn simple_patterns() {
        let mut field = FieldSpec::new(FieldKind::Text, 1, 0, 10);
        field.pattern = fixed(b"99A*");
        assert_eq!(check(&field, "12b-"), Ok(()));
        assert_eq!(check(&field, "12"), Ok(()));
        assert_eq!(check(&field, "1a"), Err(Invalid::Pattern));
        assert_eq!(check(&field, "123"), Err(Invalid::Pattern));
        assert_eq!(check(&field, "12b-5"), Err(Invalid::Pattern));
    }

    /// Appendix C.3: one "@" with text on either side.
    #[test]
    fn email_rule() {
        let field = FieldSpec::new(FieldKind::Email, 1, 0, 20);
        assert_eq!(check(&field, "ann@example.com"), Ok(()));
        for bad in ["ann", "ann@", "@example", "a@b@c"] {
            assert_eq!(check(&field, bad), Err(Invalid::Email), "{bad}");
        }
    }

    /// Appendix C.3: digits, spaces and "+" only, unless a pattern says
    /// otherwise.
    #[test]
    fn telephone_rule() {
        let mut field = FieldSpec::new(FieldKind::Tel, 1, 0, 20);
        assert_eq!(check(&field, "+44 20 7946 0000"), Ok(()));
        assert_eq!(check(&field, "020-7946"), Err(Invalid::Tel));
        field.pattern = fixed(b"999*9999");
        assert_eq!(check(&field, "020-7946"), Ok(()));
        assert_eq!(check(&field, "+20-7946"), Err(Invalid::Pattern));
    }

    /// Appendix C.3 and C.5: whole numbers only, in range.
    #[test]
    fn number_rule() {
        let mut field = FieldSpec::new(FieldKind::Number, 1, 0, 10);
        field.min = -10;
        field.max = 10;
        for good in ["-10", "-5", "0", "10"] {
            assert_eq!(check(&field, good), Ok(()), "{good}");
        }
        for bad in ["-11", "11", "5a", "-", "--5", "1.5", "99999999999"] {
            assert_eq!(check(&field, bad), Err(Invalid::Number), "{bad}");
        }
    }

    /// Appendix C.3 and C.5: year-month-day, within the limits.
    #[test]
    fn date_rule() {
        let mut field = FieldSpec::new(FieldKind::Date, 1, 0, 10);
        field.min_text = fixed(b"2026-01-01");
        field.max_text = fixed(b"2026-12-31");
        for good in ["2026-01-01", "2026-06-15", "2026-12-31"] {
            assert_eq!(check(&field, good), Ok(()), "{good}");
        }
        for bad in ["2025-12-31", "2027-01-01", "2026/06/15", "2026-6-15", "2026-00-10", "2026-10-32", "2026-1a-01"] {
            assert_eq!(check(&field, bad), Err(Invalid::DateTime), "{bad}");
        }
    }

    /// Appendix C.3 and C.5: hours and minutes, within the limits.
    #[test]
    fn time_rule() {
        let mut field = FieldSpec::new(FieldKind::Time, 1, 0, 10);
        assert_eq!(check(&field, "00:00"), Ok(()));
        for bad in ["9:30", "09:60", "24:00", "0930", "09:3x"] {
            assert_eq!(check(&field, bad), Err(Invalid::DateTime), "{bad}");
        }
        field.min_text = fixed(b"09:00");
        field.max_text = fixed(b"17:00");
        assert_eq!(check(&field, "09:00"), Ok(()));
        assert_eq!(check(&field, "17:00"), Ok(()));
        assert_eq!(check(&field, "08:59"), Err(Invalid::DateTime));
        assert_eq!(check(&field, "17:01"), Err(Invalid::DateTime));
    }

    /// REQ-WEB-011 step 1: the first failing field takes the focus, with
    /// the cursor at the end of its value.
    #[test]
    fn submit_moves_to_the_first_failing_field() {
        let mut first = FieldSpec::new(FieldKind::Number, 1, 0, 10);
        first.max = 5;
        let second = FieldSpec::new(FieldKind::Email, 2, 0, 10);
        let mut form = form(&[first, second, FieldSpec::new(FieldKind::Submit, 3, 0, 4)], &[]);
        type_text(&mut form, "3");
        form.key(FormKey::Tab, NOW);
        type_text(&mut form, "nobody");
        form.key(FormKey::Tab, NOW);
        assert_eq!(form.key(FormKey::Cyan, NOW), FormAction::Invalid(1, Invalid::Email));
        assert_eq!(form.focus(), 1);
        type_text(&mut form, "@x");
        assert_eq!(form.value(1), b"nobody@x", "typing continues at the end");
        assert_eq!(form.key(FormKey::Cyan, NOW), FormAction::Submit(None));
    }

    fn shown(grid: &Grid, row: usize, columns: core::ops::Range<usize>) -> std::string::String {
        columns
            .map(|column| match grid.cell(row, column).unwrap().glyph {
                Glyph::Space => ' ',
                Glyph::Char(character) => to_unicode(character).base,
                Glyph::Mosaic { .. } => '?',
            })
            .collect()
    }

    fn flashing(grid: &Grid, row: usize, columns: core::ops::Range<usize>) -> std::vec::Vec<usize> {
        columns.filter(|&column| grid.cell(row, column).unwrap().flash).collect()
    }

    /// Appendix C.4: the value is drawn over the placeholder, the focused
    /// field in inverse video with the cursor cell flashing, and concealed
    /// placeholders are revealed.
    #[test]
    fn draw_highlights_the_focused_field() {
        use crate::consts::{COLOUR_BLACK, COLOUR_WHITE};
        let mut form = form(&[FieldSpec::new(FieldKind::Text, 2, 5, 5), FieldSpec::new(FieldKind::Text, 3, 5, 5)], &[]);
        type_text(&mut form, "AB");
        let mut grid = Grid::EMPTY;
        for column in 0..10 {
            grid.cells[2][column].conceal = true;
        }
        grid.cells[3][5].flash = true;
        form.draw(&mut grid);
        assert_eq!(shown(&grid, 2, 4..11), " AB    ");
        for column in 5..10 {
            let cell = grid.cell(2, column).unwrap();
            assert_eq!((cell.foreground, cell.background, cell.conceal), (COLOUR_BLACK, COLOUR_WHITE, false));
        }
        assert!(grid.cells[2][4].conceal, "outside the field");
        assert_eq!(flashing(&grid, 2, 0..COLUMNS), [7]);
        // The field without focus keeps the page's colours and flashing.
        let other = grid.cell(3, 5).unwrap();
        assert_eq!((other.foreground, other.background, other.flash), (COLOUR_WHITE, COLOUR_BLACK, true));
    }

    /// Appendix C.4: a value longer than the field scrolls within it, and
    /// the cursor stays visible and marked.
    #[test]
    fn draw_scrolls_to_keep_the_cursor_visible() {
        let mut form = form(&[FieldSpec::new(FieldKind::Text, 2, 0, 3)], &[]);
        type_text(&mut form, "ABCDE");
        let draw = |form: &FormModel| {
            let mut grid = Grid::EMPTY;
            form.draw(&mut grid);
            (shown(&grid, 2, 0..3), flashing(&grid, 2, 0..3))
        };
        assert_eq!(draw(&form), ("DE ".into(), std::vec![2]));
        for _ in 0..4 {
            form.key(FormKey::Left, NOW);
        }
        assert_eq!(draw(&form), ("ABC".into(), std::vec![1]));
        form.key(FormKey::Right, NOW);
        form.key(FormKey::Right, NOW);
        assert_eq!(draw(&form), ("BCD".into(), std::vec![2]));
    }

    /// Appendix C.2: a TEXTAREA continues down the following rows.
    #[test]
    fn draw_textarea_over_rows() {
        let mut area = FieldSpec::new(FieldKind::TextArea, 10, 4, 3);
        area.rows = 2;
        let mut form = form(&[area], &[]);
        type_text(&mut form, "ABCD");
        let mut grid = Grid::EMPTY;
        form.draw(&mut grid);
        assert_eq!((shown(&grid, 10, 4..7), shown(&grid, 11, 4..7)), ("ABC".into(), "D  ".into()));
        assert_eq!((flashing(&grid, 10, 0..COLUMNS), flashing(&grid, 11, 0..COLUMNS)), (std::vec![], std::vec![5]));
    }

    /// Appendix C.3 and C.4: SELECT shows the chosen option's text, cut to
    /// the field; CHECKBOX a marker when checked; buttons their label.
    #[test]
    fn draw_select_checkbox_and_buttons() {
        let (fields, options) = colours();
        let mut send = FieldSpec::new(FieldKind::Submit, 12, 2, 6);
        send.label = fixed(b"SEND");
        let mut form = form(&[fields[0], fields[2], send], &options);
        let draw = |form: &FormModel| {
            let mut grid = Grid::EMPTY;
            form.draw(&mut grid);
            [shown(&grid, 2, 10..15), shown(&grid, 8, 2..3), shown(&grid, 12, 2..8)]
        };
        assert_eq!(draw(&form), ["Green", " ", "SEND  "]);
        form.key(FormKey::Down, NOW);
        form.key(FormKey::Tab, NOW);
        form.key(FormKey::Space, NOW);
        assert_eq!(draw(&form), ["Blue ", "*", "SEND  "]);
        let mut grid = Grid::EMPTY;
        form.draw(&mut grid);
        assert!(flashing(&grid, 8, 0..COLUMNS).is_empty(), "no cursor in a marker");
    }

    /// Appendix C.3: a RADIO field's column is the first choice's marker,
    /// and each further choice gives its own; the chosen one is marked.
    #[test]
    fn draw_radio_marks_the_chosen_choice() {
        let mut urgency = FieldSpec::new(FieldKind::Radio, 6, 10, 1);
        urgency.option_count = 3;
        let options = [
            choice(b"low", b"Low", false, None),
            choice(b"medium", b"Medium", true, Some(19)),
            choice(b"high", b"High", false, Some(31)),
        ];
        let mut form = form(&[urgency, FieldSpec::new(FieldKind::Text, 8, 0, 5)], &options);
        let markers = |form: &FormModel| {
            let mut grid = Grid::EMPTY;
            form.draw(&mut grid);
            [10, 19, 31].map(|column| shown(&grid, 6, column..column.saturating_add(1)))
        };
        assert_eq!(markers(&form), [" ", "*", " "]);
        form.key(FormKey::Down, NOW);
        assert_eq!(markers(&form), [" ", " ", "*"]);
        form.key(FormKey::Down, NOW);
        assert_eq!(markers(&form), ["*", " ", " "]);
        // Without focus, the choice is still shown.
        form.key(FormKey::Tab, NOW);
        assert_eq!(markers(&form), ["*", " ", " "]);
    }

    /// Appendix C.3: a SELECT's choices are its own `OPTION` lines. One with
    /// none submits and shows nothing, never another field's option.
    #[test]
    fn a_choice_field_without_options_has_no_value() {
        let mut empty = FieldSpec::new(FieldKind::Select, 2, 0, 5);
        empty.name = name(b"empty");
        let mut size = FieldSpec::new(FieldKind::Select, 3, 0, 5);
        size.name = name(b"size");
        size.option_count = 2;
        let options = [choice(b"s", b"Small", true, None), choice(b"l", b"Large", false, None)];
        let form = form(&[empty, size], &options);
        assert_eq!(submitted(&form), [pair("size", "s")]);
        let mut grid = Grid::EMPTY;
        form.draw(&mut grid);
        assert_eq!((shown(&grid, 2, 0..5), shown(&grid, 3, 0..5)), ("     ".into(), "Small".into()));
    }
}
