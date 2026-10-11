//! The rows the receiver draws itself, as 7-bit teletext codes: row 0 on
//! every page (REQ-SEC-025, `docs/web-native-concept.md` section 4), and whole pages
//! for the tuning screen, the start page and error pages. The program
//! renders them with the same renderer as broadcast pages, but they never
//! pass through a decoder process, so no page can forge them.

use neoteletext_core::charset::is_national_position;
use neoteletext_core::consts::{
    ALPHA_RED, ALPHA_WHITE, ALPHA_YELLOW, COLUMNS, DOUBLE_HEIGHT, HEADER_DISPLAY_COLUMN, ROWS, SPACE, STATUS_COLUMNS,
};

/// One row of 7-bit codes.
pub type Row = [u8; COLUMNS];
/// A page of rows; row 0 is the header.
pub type Page = [Row; ROWS];

/// Alpha Green: the colour attributes are Alpha Black plus the colour
/// number (EN 300 706 Table 26).
pub const ALPHA_GREEN: u8 = ALPHA_RED + 1;
/// Alpha Cyan.
pub const ALPHA_CYAN: u8 = ALPHA_WHITE - 1;
/// What stands in for a character the receiver cannot draw the same
/// everywhere.
const UNSHOWABLE: u8 = b'?';
/// The last printable 7-bit code.
const PRINTABLE_LAST: u8 = 0x7E;
/// The mark at the left of a shortened host: the rest is to the left.
const SHORTENED: u8 = b'<';
/// Columns of the clock in row 0.
const CLOCK_COLUMNS: usize = 8;
/// The first column of the clock.
const CLOCK_COLUMN: usize = COLUMNS - CLOCK_COLUMNS;
/// Columns for an address on the tuning screen after its attribute.
const TEXT_COLUMNS: usize = COLUMNS - 1;
/// Lines of the full address on the tuning screen.
const ADDRESS_LINES: usize = 4;
/// The most of a channel's last folder segment row 0 shows before cutting
/// it too (REQ-SEC-025).
const LAST_SEGMENT_COLUMNS: usize = 20;
/// What stands for the folder segments between the host and the last.
const SKIPPED_SEGMENTS: &str = "/.../";

/// Text as codes the receiver can draw the same under every national
/// option: printable ASCII outside the positions where the options differ
/// (EN 300 706 Table 36); anything else becomes `?`.
#[must_use]
pub fn showable(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match u8::try_from(c) {
            Ok(byte) if (SPACE..=PRINTABLE_LAST).contains(&byte) && !is_national_position(byte) => byte,
            _ => UNSHOWABLE,
        })
        .collect()
}

/// A blank row.
#[must_use]
pub const fn blank() -> Row {
    [SPACE; COLUMNS]
}

/// A row starting with `attributes`, then `text` drawn from the next
/// column, cut at the end of the row.
#[must_use]
pub fn line(attributes: &[u8], text: &str) -> Row {
    let mut row = blank();
    let codes = showable(text);
    for (slot, &code) in row.iter_mut().zip(attributes.iter().chain(codes.iter())) {
        *slot = code;
    }
    row
}

/// `text` shortened from the left to `width` columns, marked with `<` when
/// cut, so that the end of a host, which names the site, always shows.
#[must_use]
pub fn keep_right(text: &str, width: usize) -> Vec<u8> {
    let codes = showable(text);
    if codes.len() <= width {
        return codes;
    }
    let mut kept = vec![SHORTENED];
    kept.extend_from_slice(&codes[codes.len() - (width - 1)..]);
    kept
}

/// A channel's name, `host/folder`, in at most `width` columns
/// (REQ-SEC-025): whole when it fits; otherwise the folder's last segment
/// whole (its own start cut and marked `<` when it alone is longer than
/// [`LAST_SEGMENT_COLUMNS`]), the segments before it as `/.../`, and the
/// end of the host, cut from the left and marked `<`.
#[must_use]
pub fn channel_name(label: &str, width: usize) -> Vec<u8> {
    let codes = showable(label);
    if codes.len() <= width {
        return codes;
    }
    let Some((host, path)) = label.split_once('/') else { return keep_right(label, width) };
    let segments: Vec<&str> = path.split('/').collect();
    let last = segments.last().copied().unwrap_or("");
    let last = keep_right(last, LAST_SEGMENT_COLUMNS);
    let joiner = if segments.len() > 1 { SKIPPED_SEGMENTS } else { "/" };
    let room = width.saturating_sub(last.len() + joiner.len());
    let mut name = keep_right(host, room);
    name.extend_from_slice(joiner.as_bytes());
    name.extend_from_slice(&last);
    name
}

/// What row 0's status area shows (the receiver's columns 0 to 6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activity {
    /// A page, by its three digits as the viewer shows them (digits, or
    /// `-` while being typed), over an encrypted connection or not.
    Page {
        /// The digits.
        digits: [u8; 3],
        /// The page came over plain HTTP: the red marker of REQ-SEC-005.
        unencrypted: bool,
    },
    /// A form has the keys.
    Form,
    /// Page catching: a link is selected, and the header shows where it
    /// leads (concept decision 11).
    Link,
    /// The tuning screen is waiting for confirmation.
    Tuning,
    /// The channel's contents are shown (REQ-BRW-009).
    Contents,
    /// The receiver's own page: the start page or an error.
    Receiver,
}

/// The digits a decoder reported in its status area, if they are what
/// the viewer writes there: `P` and three hexadecimal digits or `-`.
/// Anything else is replaced by `---`, so a decoder cannot write words
/// into the receiver's columns.
#[must_use]
pub fn reported_digits(status: &[u8]) -> [u8; 3] {
    let valid = |byte: &u8| byte.is_ascii_hexdigit() || *byte == b'-';
    match status.get(1..4) {
        Some([a, b, c]) if valid(a) && valid(b) && valid(c) => [*a, *b, *c],
        _ => [b'-'; 3],
    }
}

/// The receiver's status area for an activity.
#[must_use]
pub fn status_area(activity: Activity) -> [u8; STATUS_COLUMNS] {
    let mut status = [SPACE; STATUS_COLUMNS];
    let fill = |status: &mut [u8; STATUS_COLUMNS], codes: &[u8]| {
        for (slot, &code) in status.iter_mut().zip(codes) {
            *slot = code;
        }
    };
    match activity {
        Activity::Page { digits, unencrypted: false } => fill(&mut status, &[b'P', digits[0], digits[1], digits[2]]),
        Activity::Page { digits, unencrypted: true } => {
            fill(&mut status, &[ALPHA_RED, digits[0], digits[1], digits[2], SPACE, b'!']);
        }
        Activity::Form => fill(&mut status, &[ALPHA_YELLOW, b'F', b'O', b'R', b'M']),
        Activity::Link => fill(&mut status, &[ALPHA_YELLOW, b'L', b'I', b'N', b'K']),
        Activity::Tuning => fill(&mut status, &[ALPHA_YELLOW, b'G', b'O', b'?']),
        Activity::Contents => fill(&mut status, &[ALPHA_YELLOW, b'L', b'I', b'S', b'T']),
        Activity::Receiver => fill(&mut status, b"P---"),
    }
    status
}

/// Row 0 columns 8 to 39: the site's name, and the clock while the name
/// fits in columns 8 to 31; a longer name takes the clock's columns too
/// (concept decision 10).
#[must_use]
pub fn header_text(label: &str, clock: Option<&str>) -> [u8; COLUMNS - HEADER_DISPLAY_COLUMN] {
    let mut out = [SPACE; COLUMNS - HEADER_DISPLAY_COLUMN];
    let short = CLOCK_COLUMN - HEADER_DISPLAY_COLUMN;
    let codes = showable(label);
    let (name, with_clock) = if codes.len() < short {
        (codes, true)
    } else {
        (channel_name(label, COLUMNS - HEADER_DISPLAY_COLUMN - 1), false)
    };
    // Column 8 is the space after the status area; the name follows it.
    for (slot, &code) in out.iter_mut().skip(1).zip(&name) {
        *slot = code;
    }
    if with_clock && let Some(clock) = clock {
        for (slot, &code) in out.iter_mut().skip(CLOCK_COLUMN - HEADER_DISPLAY_COLUMN).zip(showable(clock).iter()) {
            *slot = code;
        }
    }
    out
}

/// A whole row 0: the status area, column 7, and the header text.
#[must_use]
pub fn row0(activity: Activity, label: &str, clock: Option<&str>) -> Row {
    let mut row = blank();
    row[..STATUS_COLUMNS].copy_from_slice(&status_area(activity));
    row[STATUS_COLUMNS] = ALPHA_WHITE;
    row[HEADER_DISPLAY_COLUMN..].copy_from_slice(&header_text(label, clock));
    row
}

/// Writes `text` wrapped over rows starting at `first`, at most `lines`
/// of them, each after `attribute`. A cut is marked by `?` at the end.
pub fn wrap(page: &mut Page, first: usize, lines: usize, attribute: u8, text: &str) {
    let codes = showable(text);
    let chunks: Vec<&[u8]> = codes.chunks(TEXT_COLUMNS).collect();
    for (index, chunk) in chunks.iter().take(lines).enumerate() {
        let Some(row) = page.get_mut(first + index) else { return };
        *row = blank();
        row[0] = attribute;
        row[1..=chunk.len()].copy_from_slice(chunk);
        if index + 1 == lines && chunks.len() > lines {
            row[COLUMNS - 1] = UNSHOWABLE;
        }
    }
}

/// A page with a big title in double height on rows 1 and 2.
#[must_use]
pub fn titled(title: &str, colour: u8) -> Page {
    let mut page = [blank(); ROWS];
    page[1] = line(&[colour, DOUBLE_HEIGHT], title);
    page
}

/// The rows of the full address, for the tuning screen.
pub const ADDRESS_ROWS: usize = ADDRESS_LINES;

#[cfg(test)]
mod tests {
    use super::*;

    /// REQ-SEC-025: a long channel name keeps its last folder and the end
    /// of its host.
    #[test]
    fn channel_names_keep_their_ends() {
        let text = |codes: Vec<u8>| String::from_utf8(codes).unwrap();
        assert_eq!(text(channel_name("user.github.io/ceefax", 31)), "user.github.io/ceefax");
        let long = "raw.githubusercontent.com/spark-teletext/spark-teletext/master";
        assert_eq!(text(channel_name(long, 31)), "<thubusercontent.com/.../master");
        let one = "a-very-long-host-name-for-a-test.example/news";
        assert_eq!(text(channel_name(one, 31)), "<t-name-for-a-test.example/news");
        let deep = "h.example/a/an-extremely-long-folder-name-indeed";
        let shown = text(channel_name(deep, 31));
        assert!(shown.ends_with("/.../<-folder-name-indeed") && shown.len() <= 31, "{shown}");
        assert_eq!(text(channel_name("example.org", 31)), "example.org");
    }

    #[test]
    fn text_avoids_the_national_positions_and_control_codes() {
        assert_eq!(showable("a#b\u{1b}c€_"), b"a?b?c??");
        assert_eq!(line(&[ALPHA_YELLOW], "Hi")[..3], [ALPHA_YELLOW, b'H', b'i']);
    }

    #[test]
    fn row0_keeps_the_end_of_the_host() {
        let row = row0(Activity::Page { digits: *b"140", unencrypted: false }, "example.org", Some("12:34:56"));
        assert_eq!(&row[..4], b"P140");
        assert_eq!(row[STATUS_COLUMNS], ALPHA_WHITE);
        assert_eq!(&row[9..20], b"example.org");
        assert_eq!(&row[32..], b"12:34:56");
        let long = "a-very-long-subdomain.of.something.example.co.uk";
        let row = row0(Activity::Tuning, long, Some("12:34:56"));
        assert_eq!(&row[..4], &[ALPHA_YELLOW, b'G', b'O', b'?']);
        let text = String::from_utf8_lossy(&row[9..]).into_owned();
        assert!(text.starts_with('<') && text.ends_with("example.co.uk"), "{text}");
        assert!(!text.contains("12:34"), "a long host takes the clock's columns");
    }

    #[test]
    fn a_decoder_cannot_write_words_in_the_status_area() {
        assert_eq!(reported_digits(b"P14-    "), *b"14-");
        assert_eq!(reported_digits(b"\x01100 ! "), *b"100");
        assert_eq!(reported_digits(b"GO? bank"), *b"---");
        assert_eq!(reported_digits(b""), *b"---");
        let red = status_area(Activity::Page { digits: *b"100", unencrypted: true });
        assert_eq!(&red[..6], &[ALPHA_RED, b'1', b'0', b'0', SPACE, b'!']);
    }

    #[test]
    fn wrapping_marks_a_cut() {
        let mut page = [blank(); ROWS];
        let address = "https://example.org/".to_string() + &"x".repeat(200);
        wrap(&mut page, 8, 2, ALPHA_WHITE, &address);
        assert_eq!(page[8][0], ALPHA_WHITE);
        assert_eq!(&page[8][1..21], b"https://example.org/");
        assert_eq!(page[9][COLUMNS - 1], b'?');
        assert_eq!(page[10], blank());
    }
}
