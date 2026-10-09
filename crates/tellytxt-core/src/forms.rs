//! The form model (Appendix C, REQ-NAV-009, layer L6): fields, focus,
//! editing from a keyboard or a remote control, validation, and drawing the
//! values over the page. Submission itself belongs to the web module.
//!
//! Values are held as 7-bit codes of the page's G0 set, so every value can
//! be drawn; characters the set cannot show are refused at entry (decision
//! E.31 item 5).

use crate::charset::{CharRef, CharSet, G0, to_unicode};
use crate::consts::{CODE_MASK_7BIT, COLUMNS, ROWS, SPACE};
use crate::level1::{Glyph, Grid};
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
        let (row, column, width, rows) = (self.row, self.column, self.width, self.rows.max(1));
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
                    let choice = usize::from(self.choice.get(index).copied().unwrap_or(0));
                    let option =
                        self.spec.options.as_slice().get(usize::from(field.first_option).saturating_add(choice));
                    if let Some(option) = option {
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
            let mut codes = [SPACE; VALUE_CHARS];
            let shown = self.shown_codes(index, field, &mut codes);
            for (position, (row, column)) in field.cells().enumerate() {
                let Some(cell) = grid.cell_mut(row, column) else { continue };
                let code = shown.get(position).copied().unwrap_or(SPACE);
                cell.glyph =
                    if code == SPACE { Glyph::Space } else { Glyph::Char(CharRef::new(CharSet::G0(self.g0), code)) };
                cell.conceal = false;
                if focused {
                    core::mem::swap(&mut cell.foreground, &mut cell.background);
                    cell.flash = field.is_textual() && position == self.cursor;
                }
            }
        }
    }

    /// The codes a field shows, scrolled so the cursor stays visible.
    fn shown_codes<'a>(&self, index: usize, field: &FieldSpec, out: &'a mut [u8; VALUE_CHARS]) -> &'a [u8] {
        let capacity = usize::from(field.width).saturating_mul(usize::from(field.rows.max(1)));
        let len = match field.kind {
            FieldKind::Checkbox | FieldKind::Radio => {
                let on = if field.kind == FieldKind::Checkbox {
                    self.checked.checked_shr(index as u32).is_some_and(|bits| bits & 1 == 1)
                } else {
                    true
                };
                if let Some(first) = out.first_mut() {
                    *first = if on { b'*' } else { SPACE };
                }
                1
            }
            FieldKind::Select => {
                let choice = usize::from(self.choice.get(index).copied().unwrap_or(0));
                let option = self.spec.options.as_slice().get(usize::from(field.first_option).saturating_add(choice));
                let text = option.map_or(&[][..], |option| option.display.as_slice());
                copy(out, text)
            }
            FieldKind::Submit | FieldKind::Reset => copy(out, field.label.as_slice()),
            _ => {
                let value = self.value(index);
                let skip = value.len().saturating_sub(capacity.saturating_sub(1));
                copy(out, value.get(skip..).unwrap_or(&[]))
            }
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
    let month = value.get(5..7).and_then(parse_int).unwrap_or(0);
    let day = value.get(8..10).and_then(parse_int).unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day) && within(value, field)
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
        assert!(valid_time(b"23:59", &date));
        assert!(!valid_time(b"24:00", &date));
        assert!(valid_email(b"a@b") && !valid_email(b"@b") && !valid_email(b"a@b@c"));
    }
}
