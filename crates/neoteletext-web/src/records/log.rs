//! The LOG record (`neoteletext-common/INPUT.md` section 7.6, REQ-INP-026,
//! decision E.109 as amended for DEC-1459): a log line for the channel's
//! author, as a TTI `LG` line holds one, checked by the same rules.
//!
//! ```text
//! byte 0       level: E, W or I
//! bytes 1-2    page, big-endian: 0x100 to 0x8FF, or 0 for none
//! bytes 3-end  text: UTF-8, 1 to 200 bytes, no control or bidirectional
//!              formatting character
//! ```

use neoteletext_core::packet::Packet;
use neoteletext_core::types::PageAddress;
use neoteletext_formats::log::{LogLevel, LogLine, MAX_LOG_TEXT_BYTES, check};

use super::{RecordFault, TYPE_LOG, take_u16, write_packets};

pub use neoteletext_formats::log::MAX_LOG_LINES_PER_PAGE;

/// The level's byte and the page's two before the text.
const FIXED: usize = 1 + 2;
/// The largest body: the fixed fields and the longest text.
pub const MAX_BODY: usize = FIXED + MAX_LOG_TEXT_BYTES;
/// The page field's value for a line that concerns no page.
const NO_PAGE: u16 = 0;

/// A LOG record as read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LogRecord<'a> {
    /// The level and the checked text, as a TTI `LG` line gives them.
    pub line: LogLine<'a>,
    /// The page the line concerns, if any.
    pub page: Option<PageAddress>,
}

const fn level_of(letter: u8) -> Option<LogLevel> {
    match letter {
        b'E' => Some(LogLevel::Error),
        b'W' => Some(LogLevel::Warning),
        b'I' => Some(LogLevel::Information),
        _ => None,
    }
}

/// Reads a LOG record's body.
///
/// # Errors
/// A level other than `E`, `W` or `I`, a page neither 0 nor a page
/// number, or text that breaks REQ-INP-026.
pub fn read(body: &[u8]) -> Result<LogRecord<'_>, RecordFault> {
    let (&letter, rest) = body.split_first().ok_or(RecordFault::Length)?;
    let level = level_of(letter).ok_or(RecordFault::BadValue("log level"))?;
    let (page, text) = take_u16(rest)?;
    let page = match page {
        NO_PAGE => None,
        mpp => Some(PageAddress::from_mpp(mpp).ok_or(RecordFault::BadValue("log page"))?),
    };
    let text = check(text).map_err(|_| RecordFault::BadValue("log text"))?;
    Ok(LogRecord { line: LogLine { level, text }, page })
}

/// Writes a LOG record as packets 4/31 of audio service `service`, record
/// id `record_id`. Returns the packets written.
///
/// # Errors
/// What the reader would refuse: text that breaks REQ-INP-026, or a page
/// that is not a page number.
pub fn encode(
    level: LogLevel,
    page: Option<PageAddress>,
    text: &str,
    service: u8,
    record_id: u8,
    out: &mut dyn FnMut(Packet),
) -> Result<usize, RecordFault> {
    let mut body = [0_u8; MAX_BODY];
    let length = FIXED.checked_add(text.len()).filter(|&length| length <= MAX_BODY).ok_or(RecordFault::Length)?;
    let [high, low] = page.map_or(NO_PAGE, PageAddress::mpp).to_be_bytes();
    let target = body.get_mut(..length).ok_or(RecordFault::Length)?;
    let (fixed, rest) = target.split_at_mut(FIXED);
    fixed.copy_from_slice(&[level.letter(), high, low]);
    rest.copy_from_slice(text.as_bytes());
    let body = body.get(..length).ok_or(RecordFault::Length)?;
    // The writer checks with the reader, so the two cannot disagree.
    let back = read(body)?;
    if back.line.text != text || back.page != page {
        return Err(RecordFault::BadValue("does not read back"));
    }
    write_packets(TYPE_LOG, body, service, record_id, out).ok_or(RecordFault::Length)
}
