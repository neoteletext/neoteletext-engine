//! The interaction lines of a page: the cursor record (`CU`, REQ-INT-006)
//! and the input modes (`IM`, REQ-INT-114).

/// A `CU` line (REQ-INT-006): the page's cursor record of the input
/// profile, as written. The fields are only read as numbers here; the
/// input profile (`neoteletext-input`) checks what they mean.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct CursorLine {
    /// The cursor's row, or 255 for no cursor.
    pub row: u8,
    /// Its column.
    pub column: u8,
    /// The record's flags.
    pub flags: u8,
    /// The input count acted on (`neoteletext-common/INPUT.md` section
    /// 7.1), or 0.
    pub acked: u16,
    /// The line of the file.
    pub line: u32,
}
