//! The name and value pairs a form submits (REQ-WEB-011).

use super::spec::{FieldKind, FieldSpec, OptionSpec};
use super::{FormModel, VALUE_CHARS};
use crate::charset::{CharRef, CharSet, to_unicode};

impl FormModel {
    /// The chosen option of a SELECT or RADIO field. Only the field's own
    /// options count: one with none has no choice (Appendix C.3).
    pub(super) fn chosen_option(&self, index: usize, field: &FieldSpec) -> Option<&OptionSpec> {
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
}
