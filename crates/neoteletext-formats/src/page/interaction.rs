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
    /// How far local echo may run.
    pub width: u8,
    /// The record's flags.
    pub flags: u8,
    /// The input count acted on (`neoteletext-common/INPUT.md` section
    /// 7.1), or 0.
    pub acked: u16,
    /// The line of the file.
    pub line: u32,
}

/// The input mode bits a page may ask for: INPUT.md section 6.3's eight.
/// The command-line program asserts that `neoteletext-input` agrees.
pub const INPUT_MODE_BITS: u32 = 0xFF;

/// An `IM` line (REQ-INT-114): the input modes the page asks for while it
/// is open, as written.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct ModesLine {
    /// The mode bits, within [`INPUT_MODE_BITS`]; 0 asks for none.
    pub modes: u32,
    /// The line of the file.
    pub line: u32,
}
