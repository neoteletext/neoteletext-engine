//! Text as the input system sends it: in Unicode Normalization Form C
//! (`neoteletext-common/INPUT.md` section 5.7 rule 3, REQ-INT-116), so `é`
//! typed on any layout or input method arrives as U+00E9 and a channel
//! compares what it receives without normalizing it again. A paste is
//! data and goes as it was copied.

use std::borrow::Cow;

use neoteletext_input::event::{Event, Item, Key};
use unicode_normalization::{IsNormalized, UnicodeNormalization, is_nfc_quick};

/// `text` in Normalization Form C, borrowed when it already is.
fn nfc(text: &str) -> Cow<'_, str> {
    if is_nfc_quick(text.chars()) == IsNormalized::Yes { Cow::Borrowed(text) } else { Cow::Owned(text.nfc().collect()) }
}

/// Gives `send` the item with its text, and a KEY's text, normalized.
pub fn with_nfc<R>(item: &Item<'_>, send: impl FnOnce(&Item<'_>) -> R) -> R {
    match item {
        Item::Text(text) => send(&Item::Text(&nfc(text))),
        Item::Event(Event::Key(key @ Key { text: Some(text), .. })) => {
            let text = nfc(text);
            send(&Item::Event(Event::Key(Key { text: Some(&text), ..*key })))
        }
        other @ Item::Event(_) => send(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neoteletext_input::event::{Action, KeyCode, Stamp};

    /// REQ-INT-116: a decomposed `e` and combining acute accent go as
    /// U+00E9, in text and in a KEY's text; a paste goes as copied.
    #[test]
    fn text_goes_in_normalization_form_c() {
        let decomposed = "e\u{301}";
        assert!(with_nfc(&Item::Text(decomposed), |item| *item == Item::Text("\u{e9}")));
        let on = Stamp { page: 0x100, sub: 0 };
        let key = Key {
            on,
            action: Action::Press,
            key: KeyCode::Character('e'),
            mods: 0,
            text: Some(decomposed),
            base: None,
            lost: false,
        };
        let sent = with_nfc(&Item::Event(Event::Key(key)), |item| match item {
            Item::Event(Event::Key(key)) => key.text.map(str::to_owned),
            _ => None,
        });
        assert_eq!(sent.as_deref(), Some("\u{e9}"));
        let paste = Item::Event(Event::Paste { on, text: decomposed });
        assert!(with_nfc(&paste, |item| *item == paste));
        assert!(matches!(nfc("plain"), Cow::Borrowed("plain")), "already normal text is not copied");
        assert_eq!(nfc("\u{212B}"), "\u{C5}", "the Ångström sign is the letter Å");
    }
}
