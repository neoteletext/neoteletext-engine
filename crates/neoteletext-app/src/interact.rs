//! Pieces of interaction that every viewer shares: the messages the
//! receiver draws in row 24.

use neoteletext_core::consts::{ALPHA_YELLOW, COLUMNS, SPACE};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_yellow_and_printable() {
        let row = message_row("Ready\u{7}");
        assert_eq!(row[0], ALPHA_YELLOW);
        assert_eq!(&row[1..6], b"Ready");
        assert_eq!(row[6], SPACE, "control characters are not drawn");
    }
}
