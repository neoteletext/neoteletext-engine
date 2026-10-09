//! Every teletext constant, defined once (REQ-GEN-014).
//!
//! No other module may restate, round, bit-reverse or re-derive a teletext
//! value. Where a value exists in two forms, both are defined here side by
//! side with the conversion stated, and a test asserts that they agree.
//!
//! Sources are cited by clause or table of ETSI EN 300 706 V1.2.1 unless
//! another standard is named.

// ---------------------------------------------------------------------------
// Signal and packet framing (clauses 5 to 7)
// ---------------------------------------------------------------------------

/// Bit rate of System B teletext, 444 × line frequency (clause 5.3).
pub const BIT_RATE_HZ: u32 = 6_937_500;
/// Clock run-in byte, 10101010 in transmission order (clause 6.1).
pub const CLOCK_RUN_IN: u8 = 0x55;
/// Framing code 11100100 in transmission order, first bit as bit 0
/// (clause 6.2). This is the T42 form.
pub const FRAMING_CODE: u8 = 0x27;
/// The framing code in the bit order of EN 300 472 clause 4.4, which is the
/// bit-reversal of [`FRAMING_CODE`].
pub const FRAMING_CODE_DVB: u8 = 0xE4;
/// The framing code of inverted teletext data units (EN 300 472 Table 3).
pub const FRAMING_CODE_DVB_INVERTED: u8 = 0x1B;
/// Bits in a packet after the framing code (clause 7.1): 42 bytes.
pub const PACKET_BITS: usize = 336;
/// Bytes of a packet after the framing code: bytes 4 to 45 (clause 7.1).
pub const PACKET_BYTES: usize = 42;
/// Bytes per packet with the framing code in front (REQ-INP-003).
pub const PACKET_BYTES_WITH_FRAMING: usize = 43;
/// Bytes per packet with clock run-in and framing code (REQ-INP-003).
pub const PACKET_BYTES_WITH_RUN_IN: usize = 45;
/// The transmitted byte number of the first byte of the internal packet
/// form (REQ-ARCH-002): internal index = packet byte number − 4.
pub const PACKET_FIRST_BYTE_NUMBER: usize = 4;
/// Index of the first packet address byte (packet byte 4).
pub const ADDRESS_BYTE_0: usize = 0;
/// Index of the second packet address byte (packet byte 5).
pub const ADDRESS_BYTE_1: usize = 1;
/// Index of the first data byte, packet byte 6.
pub const DATA_START: usize = 2;
/// Data bytes in a packet: bytes 6 to 45 (clause 7.1.3).
pub const DATA_BYTES: usize = 40;

// ---------------------------------------------------------------------------
// Packet numbers and magazines (clause 7.1.2, Table 1)
// ---------------------------------------------------------------------------

/// Number of magazines.
pub const MAGAZINES: usize = 8;
/// Magazine number transmitted as 0 means magazine 8 (clause 7.1.2).
pub const MAGAZINE_ZERO_MEANS: u8 = 8;
/// Mask of the magazine bits in the first address nibble.
pub const MAGAZINE_MASK: u8 = 0x07;
/// Packet number of the page header.
pub const PACKET_HEADER: u8 = 0;
/// First display row packet.
pub const PACKET_FIRST_ROW: u8 = 1;
/// Row 24, used for Fastext prompts (Annex H).
pub const PACKET_ROW_24: u8 = 24;
/// Packet X/25: key-word search labels, stored but not displayed (Part G).
pub const PACKET_ROW_25: u8 = 25;
/// Page enhancement data packet X/26 (clause 9.4.1).
pub const PACKET_ENHANCEMENT: u8 = 26;
/// Editorial linking packet X/27 (clause 9.6.1).
pub const PACKET_LINKS: u8 = 27;
/// Page-related enhancement packet X/28 (clause 9.4.2).
pub const PACKET_PAGE_ENHANCEMENT: u8 = 28;
/// Magazine-related enhancement packet M/29 (clause 9.5).
pub const PACKET_MAGAZINE_ENHANCEMENT: u8 = 29;
/// Broadcast service data packet 8/30 (clause 9.8).
pub const PACKET_SERVICE: u8 = 30;
/// Independent data packet 31 (clause 9.7), out of scope.
pub const PACKET_INDEPENDENT_DATA: u8 = 31;
/// Magazine that carries broadcast service data.
pub const SERVICE_MAGAZINE: u8 = 8;
/// Packet number bit carried in the first address nibble.
pub const PACKET_NUMBER_LOW_BIT: u8 = 0x08;

// ---------------------------------------------------------------------------
// Pages and sub-codes (clause 9.3.1, Table 1, Annex A.1)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Byte coding (clause 8)
// ---------------------------------------------------------------------------

/// Mask of the seven data bits of an odd-parity byte (clause 8.1).
pub const CODE_MASK_7BIT: u8 = 0x7F;
/// The parity bit of an odd-parity byte.
pub const PARITY_BIT: u8 = 0x80;
/// Mask of a 4-bit Hamming 8/4 value.
pub const NIBBLE_MASK: u8 = 0x0F;
/// Bits in a Hamming 24/18 triplet.
pub const TRIPLET_BITS: u32 = 24;
/// Data bits in a Hamming 24/18 triplet.
pub const TRIPLET_DATA_BITS: u32 = 18;
/// Mask of the 18 data bits of a triplet.
pub const TRIPLET_DATA_MASK: u32 = (1 << TRIPLET_DATA_BITS) - 1;
/// Bytes in a triplet.
pub const TRIPLET_BYTES: usize = 3;
/// Triplets in packets X/26, X/28 and M/29 (clause 9.4.1).
pub const TRIPLETS_PER_PACKET: usize = 13;
/// Designation codes 0 to 15.
pub const DESIGNATION_CODES: usize = 16;

// ---------------------------------------------------------------------------
// Enhancement triplets (clause 12.3.1, Tables 27 to 29)
// ---------------------------------------------------------------------------

/// Bits of the address field (D1 to D6).
pub const TRIPLET_ADDRESS_BITS: u32 = 6;
/// Bits of the mode field (D7 to D11).
pub const TRIPLET_MODE_BITS: u32 = 5;
/// Bits of the data field (D12 to D18).
pub const TRIPLET_DATA_FIELD_BITS: u32 = 7;
/// Mask of a 6-bit address.
pub const TRIPLET_ADDRESS_MASK: u32 = (1 << TRIPLET_ADDRESS_BITS) - 1;
/// Mask of a 5-bit mode.
pub const TRIPLET_MODE_MASK: u32 = (1 << TRIPLET_MODE_BITS) - 1;
/// Mask of a 7-bit data field.
pub const TRIPLET_DATA_FIELD_MASK: u32 = (1 << TRIPLET_DATA_FIELD_BITS) - 1;
/// Addresses 0 to 39 are column addresses; 40 to 63 row addresses.
pub const ROW_ADDRESS_BASE: u8 = 40;
/// Row address 40 means row 24 (clause 12.3.2).
pub const ROW_ADDRESS_ROW_24: u8 = 40;
/// The highest row address: Termination Marker and Address Display Row 0.
pub const ROW_ADDRESS_LAST: u8 = 63;
/// Row mode 00001: Full Row Colour.
pub const MODE_FULL_ROW_COLOUR: u8 = 0x01;
/// Row mode 00100: Set Active Position.
pub const MODE_SET_ACTIVE_POSITION: u8 = 0x04;
/// Row mode 00111: Address Display Row 0.
pub const MODE_ADDRESS_ROW_0: u8 = 0x07;
/// Row mode 11111 with address 63: Termination Marker.
pub const MODE_TERMINATION: u8 = 0x1F;
/// Column mode 00010: G3 character at Level 1.5.
pub const MODE_G3_LEVEL_15: u8 = 0x02;
/// Column modes 00100 and 00101: reserved (clause 12.3.4).
pub const MODE_COLUMN_RESERVED_1: u8 = 0x04;
/// Column mode 00101: reserved.
pub const MODE_COLUMN_RESERVED_2: u8 = 0x05;
/// Column mode 00110: Programme Delivery Control.
pub const MODE_COLUMN_PDC: u8 = 0x06;
/// Column mode 01010: reserved.
pub const MODE_COLUMN_RESERVED_3: u8 = 0x0A;
/// Column mode 01011: G3 character at Level 2.5, ignored at 1.5.
pub const MODE_G3_LEVEL_25: u8 = 0x0B;
/// Column mode 01111: G2 character.
pub const MODE_G2: u8 = 0x0F;
/// Column mode 10000: G0 character without diacritical mark.
pub const MODE_G0_NO_DIACRITIC: u8 = 0x10;
/// Low four mode bits of modes 10001 to 11111 number the diacritic.
pub const MODE_DIACRITIC_MASK: u8 = 0x0F;
/// Data values below 2/0 write nothing (REQ-L15-006).
pub const TRIPLET_FIRST_CHARACTER: u8 = 0x20;
/// In mode 10000, data 2/A writes "@" (Table 29 note).
pub const TRIPLET_AT_SIGN_CODE: u8 = 0x2A;
/// The Latin G0 code of "@" in Table 35.
pub const LATIN_AT_SIGN: u8 = 0x40;
/// No diacritical mark.
pub const DIACRITIC_NONE: u8 = 0;

// ---------------------------------------------------------------------------
// Spacing attributes (clause 12.2, Table 26)
// ---------------------------------------------------------------------------

/// Codes 0/0 to 1/F are spacing attributes.
pub const ATTRIBUTE_LAST: u8 = 0x1F;
/// Alpha Black, no function at Level 1 (decision E.18).
pub const ALPHA_BLACK: u8 = 0x00;
/// Alpha Red; Alpha colours are 0/1 to 0/7.
pub const ALPHA_RED: u8 = 0x01;
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

// ---------------------------------------------------------------------------
// Colours (Table 30, CLUT 0)
// ---------------------------------------------------------------------------

/// Colour number of black.
pub const COLOUR_BLACK: u8 = 0;
/// Colour number of green.
pub const COLOUR_GREEN: u8 = 2;
/// Colour number of white.
pub const COLOUR_WHITE: u8 = 7;
/// Number of Level 1 colours.
pub const COLOURS: usize = 8;
/// Bit of the red primary in a colour number (REQ-L1-005).
pub const COLOUR_RED_BIT: u8 = 0x01;
/// Bit of the green primary.
pub const COLOUR_GREEN_BIT: u8 = 0x02;
/// Bit of the blue primary.
pub const COLOUR_BLUE_BIT: u8 = 0x04;
/// Full intensity of a primary in 8-bit output (REQ-RND-005).
pub const PRIMARY_FULL: u8 = 0xFF;

// ---------------------------------------------------------------------------
// Rendering geometry (Part C, decisions E.13 and E.14, REQ-RND-002/003)
// ---------------------------------------------------------------------------

/// Dots across a glyph.
pub const GLYPH_DOTS: usize = 5;
/// Lines of a glyph.
pub const GLYPH_LINES: usize = 9;
/// Dot columns of a cell: the glyph plus one blank column on the left.
pub const CELL_DOTS: usize = 6;
/// Lines of a cell in one field: the glyph plus one blank line.
pub const CELL_FIELD_LINES: usize = 10;
/// Half-dots across a cell in the 12-by-20 mode.
pub const CELL_WIDTH: usize = 12;
/// Lines of a cell in the 12-by-20 mode (both fields).
pub const CELL_HEIGHT: usize = 20;
/// Lines of a cell in the 12-by-10 mode.
pub const CELL_HEIGHT_SMALL: usize = 10;
/// Page width in pixels: 40 cells of 12.
pub const PAGE_WIDTH: usize = COLUMNS * CELL_WIDTH;
/// Page height in the 12-by-20 mode: 25 rows of 20 lines.
pub const PAGE_HEIGHT: usize = ROWS * CELL_HEIGHT;
/// Contiguous mosaic block rows: 3/10, 4/10 and 3/10 of the cell (E.14).
pub const MOSAIC_ROW_TENTHS: [usize; 3] = [3, 4, 3];
/// Pixels are about 1.2 times taller than wide on a 4:3 display
/// (REQ-RND-004), expressed as a ratio of whole numbers: 6 / 5.
pub const ASPECT_NUMERATOR: usize = 6;
/// Denominator of the aspect factor.
pub const ASPECT_DENOMINATOR: usize = 5;

// ---------------------------------------------------------------------------
// Palette indices (REQ-RND-023)
// ---------------------------------------------------------------------------

/// Index of transparent ("picture").
pub const PALETTE_TRANSPARENT: u8 = 8;
/// First index of flashing foreground pixels: 16 + 8 × f + b.
pub const PALETTE_FLASH_BASE: u8 = 16;
/// First index of concealed foreground pixels: 80 + 8 × f + b.
pub const PALETTE_CONCEAL_BASE: u8 = 80;
/// Number of palette entries.
pub const PALETTE_SIZE: usize = 144;

// ---------------------------------------------------------------------------
// Timing (decisions E.12 and E.19, REQ-NAV-007)
// ---------------------------------------------------------------------------

/// Microseconds per field at 50 fields per second.
pub const FIELD_PERIOD_US: u64 = 20_000;
/// Fields a flashing character is shown (decision E.12).
pub const FLASH_FIELDS_SHOWN: u64 = 48;
/// Fields a flashing character is hidden (decision E.12).
pub const FLASH_FIELDS_HIDDEN: u64 = 16;
/// Page completion timeout (decision E.19, Annex B.9).
pub const PAGE_COMPLETION_TIMEOUT_US: u64 = 100_000;
/// Momentary reveal timeout (REQ-NAV-007).
pub const MOMENTARY_REVEAL_US: u64 = 160_000;

// ---------------------------------------------------------------------------
// Broadcast service data (clause 9.8.1)
// ---------------------------------------------------------------------------

/// Modified Julian Date 45000 is 31 January 1982 (REQ-SVC-002).
pub const MJD_REFERENCE: u32 = 45_000;
/// Every MJD and UTC digit is transmitted plus one (clause 9.8.1).
pub const SERVICE_DIGIT_OFFSET: u8 = 1;
/// Characters of the 8/30 status message.
pub const STATUS_MESSAGE_BYTES: usize = 20;
/// Designation code bit 1: multiplexed (0) or full-field (1).
pub const SERVICE_FULL_FIELD_BIT: u8 = 0x01;
/// Designation codes 2 and 3 are format 2.
pub const SERVICE_FORMAT_2_BIT: u8 = 0x02;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_code_forms_agree() {
        assert_eq!(FRAMING_CODE.reverse_bits(), FRAMING_CODE_DVB);
    }

    #[test]
    fn page_geometry_is_consistent() {
        assert_eq!(PAGE_WIDTH, 480);
        assert_eq!(PAGE_HEIGHT, 500);
        assert_eq!(CELL_WIDTH, 2 * CELL_DOTS);
        assert_eq!(CELL_HEIGHT, 2 * CELL_FIELD_LINES);
        assert_eq!(MOSAIC_ROW_TENTHS.iter().sum::<usize>(), 10);
        assert_eq!(PACKET_BITS, PACKET_BYTES * 8);
    }

    #[test]
    fn palette_ranges_do_not_overlap() {
        let flash_end = usize::from(PALETTE_FLASH_BASE) + COLOURS * COLOURS;
        assert!(flash_end <= usize::from(PALETTE_CONCEAL_BASE));
        assert_eq!(usize::from(PALETTE_CONCEAL_BASE) + COLOURS * COLOURS, PALETTE_SIZE);
    }
}
