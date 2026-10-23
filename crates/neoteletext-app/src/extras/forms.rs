//! The core form model's specification of a TTI form (Appendix C), from
//! the page file's form lines.

use neoteletext_core::forms::{FieldKind, FieldSpec, FormSpec, Method, OptionSpec};
use neoteletext_core::types::FixedVec;
use neoteletext_formats::page;

fn fixed<const N: usize>(bytes: &[u8]) -> FixedVec<u8, N> {
    let mut out = FixedVec::new(0);
    for &byte in bytes {
        if !out.push(byte & 0x7F) {
            break;
        }
    }
    out
}

fn number(text: &[u8], default: i32) -> i32 {
    std::str::from_utf8(text).ok().and_then(|text| text.trim().parse().ok()).unwrap_or(default)
}

/// The core form model's specification of a TTI form (Appendix C).
#[must_use]
pub fn form_spec(form: &page::Form) -> Option<FormSpec> {
    let (method, target) = form.submission.as_ref()?;
    let mut spec = FormSpec {
        fields: FixedVec::new(FieldSpec::new(FieldKind::Text, 0, 0, 1)),
        options: FixedVec::new(OptionSpec {
            value: FixedVec::new(0),
            display: FixedVec::new(0),
            selected: false,
            column: None,
        }),
        method: match method {
            page::FormMethod::Post => Method::Post,
            page::FormMethod::Get => Method::Get,
        },
        target: fixed(target.as_bytes()),
        success: form.result_pages.map(|(success, _)| success),
        error: form.result_pages.map(|(_, error)| error),
        token: fixed(form.token.as_bytes()),
    };
    for (index, field) in form.fields.as_slice().iter().enumerate() {
        let kind = match field.kind {
            page::FieldKind::Text => FieldKind::Text,
            page::FieldKind::Email => FieldKind::Email,
            page::FieldKind::Telephone => FieldKind::Tel,
            page::FieldKind::Number => FieldKind::Number,
            page::FieldKind::Date => FieldKind::Date,
            page::FieldKind::Time => FieldKind::Time,
            page::FieldKind::Select => FieldKind::Select,
            page::FieldKind::Radio => FieldKind::Radio,
            page::FieldKind::Checkbox => FieldKind::Checkbox,
            page::FieldKind::TextArea => FieldKind::TextArea,
            page::FieldKind::Submit => FieldKind::Submit,
            page::FieldKind::Reset => FieldKind::Reset,
        };
        let mut spec_field = FieldSpec::new(kind, field.row, field.column, field.width);
        spec_field.name = fixed(field.name.as_bytes());
        spec_field.rows = field.rows.max(1);
        spec_field.required = field.required;
        spec_field.maxlen = field.max_length.unwrap_or(0);
        spec_field.pattern = fixed(field.pattern.as_bytes());
        spec_field.default = fixed(field.default.as_bytes());
        spec_field.checked = field.checked;
        spec_field.value = fixed(field.value.as_bytes());
        spec_field.label = fixed(field.label.as_bytes());
        spec_field.action = fixed(field.action.as_bytes());
        if kind == FieldKind::Number {
            spec_field.min = number(field.minimum.as_bytes(), i32::MIN);
            spec_field.max = number(field.maximum.as_bytes(), i32::MAX);
            spec_field.step = number(field.step.as_bytes(), 0);
        } else {
            spec_field.min_text = fixed(field.minimum.as_bytes());
            spec_field.max_text = fixed(field.maximum.as_bytes());
        }
        let first = spec.options.len();
        for option in form.options.as_slice().iter().filter(|option| usize::from(option.field) == index) {
            let _ = spec.options.push(OptionSpec {
                value: fixed(option.value.as_bytes()),
                display: fixed(option.display.as_bytes()),
                selected: option.selected,
                column: option.column,
            });
        }
        spec_field.first_option = u8::try_from(first).unwrap_or(0);
        spec_field.option_count = u8::try_from(spec.options.len().saturating_sub(first)).unwrap_or(0);
        if !spec.fields.push(spec_field) {
            break;
        }
    }
    Some(spec)
}
