//! Spacing attributes (clause 12.2, Table 26), their TTI aliases, and the
//! mosaic codes of Table 47 (REQ-CHR-005).

/// Codes 0/0 to 1/F are spacing attributes.
pub const ATTRIBUTE_LAST: u8 = 0x1F;
/// Alpha Black, no function at Level 1 (decision E.18).
pub const ALPHA_BLACK: u8 = 0x00;
/// Alpha Red; Alpha colours are 0/1 to 0/7.
pub const ALPHA_RED: u8 = 0x01;
/// Alpha Yellow.
pub const ALPHA_YELLOW: u8 = 0x03;
/// Alpha White.
pub const ALPHA_WHITE: u8 = 0x07;
/// Flash.
pub const FLASH: u8 = 0x08;
/// Steady.
pub const STEADY: u8 = 0x09;
/// End Box.
pub const END_BOX: u8 = 0x0A;
/// Start Box.
pub const START_BOX: u8 = 0x0B;
/// Normal Size.
pub const NORMAL_SIZE: u8 = 0x0C;
/// Double Height.
pub const DOUBLE_HEIGHT: u8 = 0x0D;
/// Double Width, no function at Level 1 (decision E.18).
pub const DOUBLE_WIDTH: u8 = 0x0E;
/// Double Size, no function at Level 1 (decision E.18).
pub const DOUBLE_SIZE: u8 = 0x0F;
/// Mosaic Black, no function at Level 1 (decision E.18).
pub const MOSAIC_BLACK: u8 = 0x10;
/// Mosaic Red; Mosaic colours are 1/1 to 1/7.
pub const MOSAIC_RED: u8 = 0x11;
/// Mosaic White.
pub const MOSAIC_WHITE: u8 = 0x17;
/// Conceal.
pub const CONCEAL: u8 = 0x18;
/// Contiguous Mosaics.
pub const CONTIGUOUS_MOSAICS: u8 = 0x19;
/// Separated Mosaics.
pub const SEPARATED_MOSAICS: u8 = 0x1A;
/// ESC (switch G0 set).
pub const ESC: u8 = 0x1B;
/// Black Background.
pub const BLACK_BACKGROUND: u8 = 0x1C;
/// New Background.
pub const NEW_BACKGROUND: u8 = 0x1D;
/// Hold Mosaics.
pub const HOLD_MOSAICS: u8 = 0x1E;
/// Release Mosaics.
pub const RELEASE_MOSAICS: u8 = 0x1F;
/// Mask of the colour number in a colour attribute.
pub const COLOUR_ATTRIBUTE_MASK: u8 = 0x07;
/// Bit distinguishing mosaic colour attributes from alpha colours.
pub const MOSAIC_ATTRIBUTE_BIT: u8 = 0x10;
/// The space character.
pub const SPACE: u8 = 0x20;
/// In MRG TTI files, 0x10 in OL text means Double Height (REQ-INP-013).
pub const TTI_DOUBLE_HEIGHT_ALIAS: u8 = 0x10;
/// Offset added to a control code in the TTI escape form.
pub const TTI_ESCAPE_OFFSET: u8 = 0x40;

// ---------------------------------------------------------------------------
// Mosaics (Table 47, REQ-CHR-005)
// ---------------------------------------------------------------------------

/// Mosaic codes with bit 6 set (columns 2, 3, 6, 7) are block mosaics.
pub const MOSAIC_BIT6: u8 = 0x40;
/// Bit 5 of a code; columns 4 and 5 have bit 6 set and bit 5 clear and are
/// blast-through capitals.
pub const MOSAIC_BIT5: u8 = 0x20;
/// The low five bits of a mosaic code.
pub const MOSAIC_LOW_BITS: u8 = 0x1F;
/// Number of block mosaic patterns.
pub const MOSAIC_PATTERNS: usize = 64;
/// The full-block pattern.
pub const MOSAIC_FULL: u8 = 0x3F;
