//! The standard's Arabic set of Table 44, which codes each joined form of a
//! letter apart: the writer gives each letter its form from its neighbours
//! in reading order, as the receiver does for NeoTeletext's plain-letter
//! sets (REQ-2190, REQ-84), and the encoder then finds the form's code.

use alloc::string::String;
use alloc::vec::Vec;
use neoteletext_core::charset::{JoinForm, JoiningType, joined_form, joining_type};

/// Lam, alef, and the lam-alef ligature in its two forms.
const LAM: char = '\u{0644}';
const ALEF: char = '\u{0627}';
const LAM_ALEF_ISOLATED: char = '\u{FEFB}';
const LAM_ALEF_FINAL: char = '\u{FEFC}';

/// One letter of the text in reading order: the letter, and how it joins.
#[derive(Clone, Copy)]
struct Letter {
    character: char,
    joining: Option<JoiningType>,
}

/// The forms tried for a letter, in order, when the sets lack the one its
/// neighbours call for: Table 44 and its G2 set (Table 45) hold only some
/// forms of many letters, so each form falls back to the nearest the
/// tables have, a form joining on the same side first.
const fn fallbacks(form: JoinForm) -> [JoinForm; 4] {
    match form {
        JoinForm::Medial => [JoinForm::Medial, JoinForm::Initial, JoinForm::Final, JoinForm::Isolated],
        JoinForm::Initial => [JoinForm::Initial, JoinForm::Medial, JoinForm::Isolated, JoinForm::Final],
        JoinForm::Final => [JoinForm::Final, JoinForm::Isolated, JoinForm::Medial, JoinForm::Initial],
        JoinForm::Isolated => [JoinForm::Isolated, JoinForm::Final, JoinForm::Initial, JoinForm::Medial],
    }
}

/// `text`, in reading order, with every Arabic letter in its joined form,
/// the first of [`fallbacks`] that `holds` (the page's G0 or G2 set has a
/// code for it):
/// lam then alef as the lam-alef ligature.
#[must_use]
pub(super) fn join(text: &str, holds: impl Fn(char) -> bool) -> String {
    let mut letters: Vec<Letter> = Vec::with_capacity(text.len());
    for character in text.chars() {
        if character == ALEF && letters.last().is_some_and(|last| last.character == LAM) {
            if let Some(last) = letters.last_mut() {
                // The ligature joins the letter before it, as alef does.
                *last = Letter { character: LAM_ALEF_ISOLATED, joining: Some(JoiningType::Right) };
            }
            continue;
        }
        letters.push(Letter { character, joining: joining_type(character) });
    }
    let mut out = String::with_capacity(text.len());
    for (index, letter) in letters.iter().enumerate() {
        let Some(joining) = letter.joining else {
            out.push(letter.character);
            continue;
        };
        let previous = index.checked_sub(1).and_then(|before| letters.get(before));
        let joins_previous =
            joining.joins_previous() && previous.and_then(|before| before.joining).is_some_and(JoiningType::joins_next);
        let joins_next = joining.joins_next()
            && letters
                .get(index.saturating_add(1))
                .and_then(|next| next.joining)
                .is_some_and(JoiningType::joins_previous);
        let form = match (joins_previous, joins_next) {
            (false, false) => JoinForm::Isolated,
            (true, false) => JoinForm::Final,
            (false, true) => JoinForm::Initial,
            (true, true) => JoinForm::Medial,
        };
        let shaped = if letter.character == LAM_ALEF_ISOLATED {
            if joins_previous && holds(LAM_ALEF_FINAL) { LAM_ALEF_FINAL } else { LAM_ALEF_ISOLATED }
        } else {
            fallbacks(form)
                .into_iter()
                .map(|candidate| joined_form(letter.character, candidate))
                .find(|&candidate| holds(candidate))
                .unwrap_or(letter.character)
        };
        out.push(shaped);
    }
    out
}
