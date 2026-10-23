//! Pages and sub-codes (clause 9.3.1, Table 1, Annex A.1): the page
//! geometry, the receiver's columns, sub-codes, links and the header control
//! bits.

/// Display rows of a page: 0 to 24.
pub const ROWS: usize = 25;
/// Columns of a page.
pub const COLUMNS: usize = 40;
/// Stored rows including X/25: 0 to 25.
pub const STORED_ROWS: usize = 26;
/// Displayable bytes of a header: packet bytes 14 to 45.
pub const HEADER_DISPLAY_BYTES: usize = 32;
/// Index of the first displayable header byte (packet byte 14).
pub const HEADER_DISPLAY_START: usize = 10;
/// Column at which header display bytes appear in row 0.
pub const HEADER_DISPLAY_COLUMN: usize = 8;
/// Receiver-owned status columns of row 0: columns 0 to 6 (decision E.10).
pub const STATUS_COLUMNS: usize = 7;
/// Column of the receiver-generated colour attribute in row 0 (E.10).
pub const STATUS_ATTRIBUTE_COLUMN: usize = 7;
/// Columns 0 to 7 of row 0 belong to the receiver (REQ-SEC-005).
pub const RECEIVER_COLUMNS: usize = 8;
/// First clock column of row 0 (decision E.11).
pub const CLOCK_COLUMN: usize = 32;
/// Page number of time-filling headers (clause 9.3.1.1).
pub const PAGE_TIME_FILLING: u8 = 0xFF;
/// Page number tens or units digits above 9 are hexadecimal (Table 1 note).
pub const DECIMAL_DIGIT_MAX: u8 = 9;
/// Sub-code meaning "any sub-page" (Annex A.1, REQ-NAV-003).
pub const SUBCODE_ANY: u16 = 0x3F7F;
/// The bits a sub-code can carry: S1 4 bits, S2 3, S3 4, S4 2.
pub const SUBCODE_MASK: u16 = 0x3F7F;
/// Page number of a null link (clause 9.6.1).
pub const LINK_NULL_PAGE: u8 = 0xFF;
/// Sub-code of a null link (clause 9.6.1).
pub const LINK_NULL_SUBCODE: u16 = 0x3F7F;
/// Default initial page when no 8/30 packet is received (Annex D.3).
pub const DEFAULT_INITIAL_PAGE: u16 = 0x100;
/// Lowest and highest page numbers that can be entered (REQ-NAV-001).
pub const FIRST_PAGE: u16 = 0x100;
/// Highest page number; 899 wraps to 100 (REQ-NAV-002).
pub const LAST_PAGE: u16 = 0x899;

// Header control bits (clause 9.3.1.3). Bit n of a control word is Cn.

/// Erase page.
pub const C4_ERASE: u16 = 1 << 4;
/// Newsflash.
pub const C5_NEWSFLASH: u16 = 1 << 5;
/// Subtitle.
pub const C6_SUBTITLE: u16 = 1 << 6;
/// Suppress header.
pub const C7_SUPPRESS_HEADER: u16 = 1 << 7;
/// Update indicator.
pub const C8_UPDATE: u16 = 1 << 8;
/// Interrupted sequence.
pub const C9_INTERRUPTED: u16 = 1 << 9;
/// Inhibit display.
pub const C10_INHIBIT_DISPLAY: u16 = 1 << 10;
/// Magazine serial (1) or parallel (0) mode.
pub const C11_SERIAL: u16 = 1 << 11;
/// National option character subset bit C12.
pub const C12: u16 = 1 << 12;
/// National option character subset bit C13.
pub const C13: u16 = 1 << 13;
/// National option character subset bit C14.
pub const C14: u16 = 1 << 14;
/// All control bits C4 to C14.
pub const CONTROL_MASK: u16 = 0x7FF0;
/// Shift of the national option bits in a control word.
pub const NATIONAL_SHIFT: u32 = 12;
/// Mask of the national option index after it has been formed (0 to 7).
pub const NATIONAL_INDEX_MASK: u8 = 0x07;
