//! Pieces of interaction that every viewer shares: the form on the page
//! being shown, the submission it makes (REQ-WEB-011), and the messages
//! the receiver draws in row 24.

use crate::pipe::Submission;
use neoteletext_core::consts::{ALPHA_YELLOW, COLUMNS, SPACE};
use neoteletext_core::forms::{FormModel, Method};
use neoteletext_core::types::{PageAddress, SubCode};

/// A form on the page being shown.
pub struct ActiveForm {
    /// The page it is on.
    pub page: PageAddress,
    /// The sub-page it is on.
    pub subcode: SubCode,
    /// The form's state.
    pub model: FormModel,
    /// The address the page came from: the base and origin of the form.
    pub url: String,
    /// The form has the keys.
    pub focused: bool,
}

/// A receiver-drawn row 24 message, in yellow, as 7-bit codes.
#[must_use]
pub fn message_row(text: &str) -> [u8; COLUMNS] {
    let mut row = [SPACE; COLUMNS];
    row[0] = ALPHA_YELLOW;
    for (slot, byte) in row.iter_mut().skip(1).zip(text.bytes()) {
        *slot = if byte.is_ascii() && byte >= SPACE { byte } else { SPACE };
    }
    row
}

/// The origin part of an address, for showing where a form submits.
#[must_use]
pub fn origin_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split('/').next().unwrap_or(rest);
    let scheme = url.split_once("://").map_or("", |(scheme, _)| scheme);
    format!("{scheme}://{host}")
}

/// The submission for a form and the button pressed (REQ-WEB-011).
#[must_use]
pub fn submission(form: &ActiveForm, button: Option<usize>) -> Submission {
    let spec = form.model.spec();
    let mut fields = Vec::new();
    form.model.pairs(&mut |name, value| {
        fields.push((String::from_utf8_lossy(name).into_owned(), value.iter().collect::<String>()));
    });
    let action = button
        .and_then(|index| spec.fields.as_slice().get(index))
        .map(|field| String::from_utf8_lossy(field.action.as_slice()).into_owned())
        .filter(|action| !action.is_empty());
    let token = String::from_utf8_lossy(spec.token.as_slice()).into_owned();
    Submission {
        page_url: form.url.clone(),
        post: spec.method == Method::Post,
        target: String::from_utf8_lossy(spec.target.as_slice()).into_owned(),
        token: (!token.is_empty()).then_some(token),
        action,
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_yellow_and_printable_and_origins_drop_the_path() {
        let row = message_row("Sent.\u{7}");
        assert_eq!(row[0], ALPHA_YELLOW);
        assert_eq!(&row[1..6], b"Sent.");
        assert_eq!(row[6], SPACE, "control characters are not drawn");
        assert_eq!(origin_of("https://forms.example:8443/contact?x=1"), "https://forms.example:8443");
    }
}
