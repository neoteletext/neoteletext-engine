//! The input system's limits and its one page check (`INPUT.md` sections
//! 5 to 8): every bound a message, a record or a session is held to.

use crate::{control, wire};

/// The protocol version HELLO and WELCOME carry (section 6.1).
pub const VERSION: u32 = 5;
/// The most bytes in one binary message (section 5.1, REQ-INT-101): room
/// for a SELECT with the longest page parameters (DEC-1715).
pub const MAX_MESSAGE: usize = 8192;
/// The most bytes of a SELECT's page parameters (section 5.4, DEC-1715): no
/// address has more, so every address the map accepts can be selected.
pub const MAX_PAGE_PARAMETERS: usize = neoteletext_core::limits::MAX_URL_LENGTH;
/// The most bytes of a PASTE's text (section 5.4, REQ-INT-103).
pub const MAX_PASTE: usize = 4000;
/// The most bytes of a COMPOSE preedit (section 5.4).
pub const MAX_COMPOSE: usize = 256;
/// The most bytes of a KEY's text (section 5.5).
pub const MAX_KEY_TEXT: usize = 32;
/// The most bytes of HELLO's user agent string (section 5.4).
pub const MAX_AGENT: usize = 64;
/// Cells across a page (section 5.4): always 40 (DEC-1636, REQ-1643).
pub const MAX_COLUMNS: u32 = neoteletext_core::consts::COLUMNS as u32;
/// Rows of a page: header, 23 rows and the Fastext row (section 5.4).
pub const MAX_ROWS: u32 = 25;
/// The most input units a second (section 8, REQ-INT-109): qrkeyboard's
/// relay limit.
pub const MAX_PER_SECOND: u32 = 120;
/// How long a receiver waits for WELCOME (section 6.1).
pub const WELCOME_TIMEOUT_MS: u32 = 10_000;
/// The most contacts on the page at once (section 5.4, REQ-RCH-043): ten
/// fingers. The first is MOUSE's, the others TOUCH's.
pub const MAX_CONTACTS: usize = 10;
/// The highest TOUCH `id`: contact 0 is MOUSE's.
pub const MAX_TOUCH_ID: u8 = (MAX_CONTACTS - 1) as u8;
/// The pointer's moves a second (REQ-RCH-031): at most one in each period
/// of 1000/60 ms, the display's refresh (the owner's decision of 28 October
/// 2026). The period is not a whole number of microseconds, so the pacer
/// counts it exactly ([`crate::pace`]).
pub const MOVES_PER_SECOND: u64 = 60;
/// The bytes of a session's token (section 6.4, REQ-RCH-013): 128 bits from
/// the channel's cryptographic generator, which no one can guess within
/// the session's hold.
pub const TOKEN_BYTES: usize = 16;
/// How long a channel that set RESUME holds a session whose connection was
/// lost, at least (section 6.4): longer than the push connection's first
/// five retries, 1, 2, 4, 8 and 16 s (the engine's `PUSH_RETRY_FIRST_US`,
/// doubling), so a reconnection after a short break resumes.
pub const RESUME_HOLD_MS: u32 = 32_000;
/// The bytes of unacknowledged messages a resumable receiver keeps to send
/// again (section 6.4): four pastes and more. It is not four of
/// [`MAX_MESSAGE`], because only SELECT reaches that, and it is never kept.
pub const RESEND_BYTES: usize = 2 * MAX_MESSAGE;

// A TOUCH id fits its byte, and every id from 1 is a contact.
const _: () = assert!(MAX_CONTACTS > 1 && MAX_CONTACTS <= 1 + u8::MAX as usize);
// A PASTE, its event byte, type, stamp and length, fits a message.
const _: () = assert!(MAX_PASTE + 2 + crate::event::STAMP_BYTES + wire::MAX_VARINT <= MAX_MESSAGE);
// A SELECT, its event byte, type and length, its page and sub-code and the
// longest parameters, fits a message.
const _: () = assert!(MAX_PAGE_PARAMETERS + 2 + 3 * wire::MAX_VARINT <= MAX_MESSAGE);
// Section 7.2: a control message is at most 26 bytes, so its record, with
// the frame's 8, is the 34 bytes of one fragment.
const _: () = assert!(control::MAX_BODY == 26);

/// The highest page number a record may name: magazine 8 (written 0 on
/// air), page FF. Pages with hexadecimal digits are addressable though no
/// viewer types them (EN 300 706 clause 7.1.2), so a channel may use them.
pub const LAST_ADDRESSABLE_PAGE: u16 = neoteletext_core::limits::LAST_PAGE_ADDRESS;

/// Whether `page` is a page number in the form records carry: magazine 1
/// to 8 then two hexadecimal page digits.
#[must_use]
pub const fn is_page(page: u16) -> bool {
    page >= neoteletext_core::consts::FIRST_PAGE && page <= LAST_ADDRESSABLE_PAGE
}
