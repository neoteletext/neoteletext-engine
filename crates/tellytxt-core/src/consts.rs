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
/// Bit periods in one television line: the bit rate is 444 × the line
/// frequency (clause 5.3).
pub const BITS_PER_LINE: u32 = 444;
/// The "1" level as a percentage of black-to-peak-white (clause 5.2:
/// 66 ± 6 percent).
pub const ONE_LEVEL_PERCENT_OF_WHITE: u32 = 66;
/// Leading run-in bits that may be "absent or reduced in amplitude"
/// (clause 6.1, note).
pub const RUN_IN_UNRELIABLE_BITS: usize = 2;
/// Nominal time from the half-amplitude point of the line synchronising
/// pulse's leading edge to the centre of the timing reference bit, in
/// nanoseconds (clause 6.3, Figure 4: 12.0 µs).
pub const TIMING_REFERENCE_NS: u32 = 12_000;
/// How much earlier than nominal the timing reference may fall (Figure 4:
/// −1.0 µs).
pub const TIMING_EARLY_NS: u32 = 1_000;
/// How much later than nominal the timing reference may fall (Figure 4:
/// +0.4 µs).
pub const TIMING_LATE_NS: u32 = 400;
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
/// Row mode 00000: Full Screen Colour (Table 28).
pub const MODE_FULL_SCREEN_COLOUR: u8 = 0x00;
/// Row modes 01000 to 01101: Programme Delivery Control (Table 28, Part G).
pub const MODE_ROW_PDC_FIRST: u8 = 0x08;
/// The last Programme Delivery Control row mode.
pub const MODE_ROW_PDC_LAST: u8 = 0x0D;
/// Row mode 10000: Origin Modifier (Table 28).
pub const MODE_ORIGIN_MODIFIER: u8 = 0x10;
/// Row modes 10001 to 10011: Active, Adaptive and Passive Object
/// Invocation (Table 28).
pub const MODE_OBJECT_INVOCATION_FIRST: u8 = 0x11;
/// The last Object Invocation row mode.
pub const MODE_OBJECT_INVOCATION_LAST: u8 = 0x13;
/// Row modes 10101 to 10111: Active, Adaptive and Passive Object
/// Definition (Table 28).
pub const MODE_OBJECT_DEFINITION_FIRST: u8 = 0x15;
/// The last Object Definition row mode.
pub const MODE_OBJECT_DEFINITION_LAST: u8 = 0x17;
/// Row mode 11000: DRCS Mode (Table 28).
pub const MODE_DRCS_MODE: u8 = 0x18;
/// The low two bits of an object mode, M1 M0: 01 Active, 10 Adaptive,
/// 11 Passive (Table 28).
pub const MODE_OBJECT_TYPE_MASK: u8 = 0x03;
/// Column mode 00000: Foreground Colour (Table 29).
pub const MODE_FOREGROUND_COLOUR: u8 = 0x00;
/// Column mode 00001: G1 block mosaic character (Table 29).
pub const MODE_G1: u8 = 0x01;
/// Column mode 00011: Background Colour (Table 29).
pub const MODE_BACKGROUND_COLOUR: u8 = 0x03;
/// Column mode 00111: Additional Flash Functions (Table 29).
pub const MODE_ADDITIONAL_FLASH: u8 = 0x07;
/// Column mode 01000: Modified G0 and G2 Character Set Designation
/// (Table 29).
pub const MODE_MODIFIED_DESIGNATION: u8 = 0x08;
/// Column mode 01001: G0 character at Levels 2.5 and 3.5 (Table 29).
pub const MODE_G0_LEVEL_25: u8 = 0x09;
/// Column mode 01100: Display Attributes (Table 29).
pub const MODE_DISPLAY_ATTRIBUTES: u8 = 0x0C;
/// Column mode 01101: DRCS Character Invocation (Table 29).
pub const MODE_DRCS_CHARACTER: u8 = 0x0D;
/// Column mode 01110: Font Style, Level 3.5 only (Table 29).
pub const MODE_FONT_STYLE: u8 = 0x0E;
/// The colour index of a colour field: D4 to D0, CLUT in D4 D3 and entry
/// in D2 to D0 (Tables 28 and 29, decision E.43).
pub const TRIPLET_COLOUR_INDEX_MASK: u8 = 0x1F;
/// Shift of D6 D5 above a colour index: zero except in reserved values,
/// and S1 S0 in Full Row Colour and Address Display Row 0 (Table 28).
pub const TRIPLET_COLOUR_HIGH_SHIFT: u32 = 5;
/// S1 S0 = 00 of Full Row Colour: this row only (Table 28).
pub const ROW_COLOUR_THIS_ROW: u8 = 0b00;
/// S1 S0 = 11 of Full Row Colour: this row and every row below (Table 28).
pub const ROW_COLOUR_DOWN_TO_BOTTOM: u8 = 0b11;
/// The reserved address that the encoder writes in triplets whose address
/// field is reserved but must still be a row address (Full Screen Colour
/// and DRCS Mode): 41, which Annex A.5 note 1 recommends for Level 1.5
/// compatibility.
pub const ROW_ADDRESS_RESERVED_DEFAULT: u8 = 41;
/// Shift of S1 S0 in the address of an Object Invocation or Definition:
/// address bits 6 to 1 are 1, S1, S0, X, N8, N7 (Table 28).
pub const OBJECT_SOURCE_SHIFT: u32 = 3;
/// Mask of S1 S0 after the shift.
pub const OBJECT_SOURCE_MASK: u8 = 0x03;
/// Address bit 6, always 1 in an Object Invocation or Definition.
pub const OBJECT_ADDRESS_BIT6: u8 = 0x20;
/// Address bits N8 N7 of an Object Invocation or Definition.
pub const OBJECT_ADDRESS_N8_N7_MASK: u8 = 0x03;
/// Bits of an object number, N8 to N0 (Table 28).
pub const OBJECT_NUMBER_BITS: u32 = 9;
/// Shift of N8 N7 within an object number: N6 to N0 are the data field.
pub const OBJECT_NUMBER_HIGH_SHIFT: u32 = 7;
/// Data bit D6 of DRCS Mode and DRCS Character Invocation: 1 normal DRCS,
/// 0 global DRCS (Tables 28 and 29).
pub const DRCS_NORMAL_BIT: u8 = 0x40;
/// Shift of L1 L0, the DRCS level bits, in the DRCS Mode data field.
pub const DRCS_LEVEL_SHIFT: u32 = 4;
/// Mask of L1 L0 after the shift.
pub const DRCS_LEVEL_MASK: u8 = 0x03;
/// Mask of T3 to T0, the DRCS sub-table, in the DRCS Mode data field.
pub const DRCS_SUBTABLE_MASK: u8 = 0x0F;
/// The Level 2.5 bit of a two-bit level field: S0 of an Object Definition's
/// usage, L0 of DRCS Mode, and the low bit of an X/27/4 page validity
/// (Tables 15 and 28: 01 Level 2.5, 10 Level 3.5, 11 both).
pub const LEVEL_BIT_25: u8 = 0b01;
/// The Level 3.5 bit of a two-bit level field.
pub const LEVEL_BIT_35: u8 = 0b10;
/// Mask of the character number D5 to D0 of a DRCS Character Invocation.
pub const DRCS_CHARACTER_MASK: u8 = 0x3F;
/// Rows of a DRCS character: the 12 by 10 matrix (clause 14.2).
pub const DRCS_PATTERN_ROWS: usize = 10;
/// Dots across a DRCS character.
pub const DRCS_PATTERN_DOTS: usize = 12;
/// Bit planes of a DRCS character: four in modes 2 and 3 (Table 31).
pub const DRCS_PLANES: usize = 4;
/// Characters in a DRCS sub-table: numbers 0 to 47 (Table 29, clause 14.1).
pub const DRCS_CHARACTERS: u8 = 48;
/// Mask of the Termination Marker type, data bits D2 to D0 (Table 28).
pub const TERMINATION_TYPE_MASK: u8 = 0x07;
/// Mask of the flash mode, D1 D0 of Additional Flash Functions (Table 29).
pub const FLASH_MODE_MASK: u8 = 0x03;
/// Shift of the flash rate and phase, D4 to D2 (Table 29).
pub const FLASH_RATE_SHIFT: u32 = 2;
/// Mask of the flash rate and phase after the shift.
pub const FLASH_RATE_MASK: u8 = 0x07;
/// Display Attributes data bit D0: double height (Table 29).
pub const ATTRIBUTE_DOUBLE_HEIGHT: u8 = 0x01;
/// Display Attributes D1: boxing or window.
pub const ATTRIBUTE_BOXING: u8 = 0x02;
/// Display Attributes D2: conceal.
pub const ATTRIBUTE_CONCEAL: u8 = 0x04;
/// Display Attributes D4: invert.
pub const ATTRIBUTE_INVERT: u8 = 0x10;
/// Display Attributes D5: underline or separated mosaics.
pub const ATTRIBUTE_UNDERLINE: u8 = 0x20;
/// Display Attributes D6: double width.
pub const ATTRIBUTE_DOUBLE_WIDTH: u8 = 0x40;
/// Font Style data bit D0: proportional spacing (Table 29).
pub const FONT_PROPORTIONAL: u8 = 0x01;
/// Font Style D1: bold.
pub const FONT_BOLD: u8 = 0x02;
/// Font Style D2: italics.
pub const FONT_ITALIC: u8 = 0x04;
/// Shift of the Font Style row count R2 to R0, D6 to D4.
pub const FONT_ROWS_SHIFT: u32 = 4;
/// Mask of the row count after the shift.
pub const FONT_ROWS_MASK: u8 = 0x07;
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
// Page-related and magazine-related enhancement packets (clauses 9.4.2 to
// 9.5.3, 9.6.2; Tables 3, 4 and 8 to 16)
// ---------------------------------------------------------------------------

/// X/28 and M/29 designation code 0: X/28/0 format 1 and M/29/0.
pub const DESIGNATION_PAGE_DATA: u8 = 0;
/// X/28 and M/29 designation code 1: the DRCS colour lookup tables.
pub const DESIGNATION_DRCS_COLOURS: u8 = 1;
/// X/28 designation code 3: DRCS mode identification (Table 9).
pub const DESIGNATION_DRCS_MODES: u8 = 3;
/// X/28 and M/29 designation code 4: Level 3.5 data (Tables 10 and 13).
pub const DESIGNATION_LEVEL_35_DATA: u8 = 4;
/// X/27 designation code 4: compositional links, Level 2.5 (clause 9.6.2).
pub const DESIGNATION_LINKS_LEVEL_25: u8 = 4;
/// X/27 designation code 5: compositional links, Level 3.5.
pub const DESIGNATION_LINKS_LEVEL_35: u8 = 5;
/// Mask of the page function, triplet 1 bits 1 to 4 (Table 3).
pub const PAGE_FUNCTION_MASK: u32 = 0x0F;
/// Page function 0000: basic Level 1 page (Table 3).
pub const PAGE_FUNCTION_BASIC: u8 = 0x0;
/// Page function 0010: Global Public Object Page (Table 3).
pub const PAGE_FUNCTION_GPOP: u8 = 0x2;
/// Page function 0011: Public Object Page.
pub const PAGE_FUNCTION_POP: u8 = 0x3;
/// Page function 0100: Global DRCS page.
pub const PAGE_FUNCTION_GDRCS: u8 = 0x4;
/// Page function 0101: normal DRCS page.
pub const PAGE_FUNCTION_DRCS: u8 = 0x5;
/// Page function 0110: Magazine Organization Table.
pub const PAGE_FUNCTION_MOT: u8 = 0x6;
/// Page coding 000: 7 data bits with odd parity (Table 3).
pub const PAGE_CODING_PARITY: u8 = 0x0;
/// Mask of the M/29 packet function, triplet 1 bits 1 to 7; only 0000000
/// is defined (Tables 11 and 13).
pub const PACKET_FUNCTION_MASK: u32 = 0x7F;
/// Shift of the default G0 and G2 designation, triplet 1 bits 8 to 14
/// (Table 4).
pub const DESIGNATION_DEFAULT_SHIFT: u32 = 7;
/// Mask of a 7-bit designation (Tables 32 and 33).
pub const DESIGNATION_MASK: u32 = 0x7F;
/// Shift of the low four bits of the second G0 designation, triplet 1
/// bits 15 to 18 (Table 4).
pub const DESIGNATION_SECOND_LOW_SHIFT: u32 = 14;
/// Bits of the second G0 designation in triplet 1.
pub const DESIGNATION_SECOND_LOW_BITS: u32 = 4;
/// Mask of the high three bits of the second G0 designation, triplet 2
/// bits 1 to 3 (Table 4).
pub const DESIGNATION_SECOND_HIGH_MASK: u32 = 0x07;
/// Triplet 2 bit 4: left side panel (Table 4).
pub const SIDE_PANEL_LEFT_SHIFT: u32 = 3;
/// Triplet 2 bit 5: right side panel.
pub const SIDE_PANEL_RIGHT_SHIFT: u32 = 4;
/// Triplet 2 bit 6: Side Panel Status Flag.
pub const SIDE_PANEL_STATUS_SHIFT: u32 = 5;
/// Triplet 2 bits 7 to 10: number of columns of the left panel.
pub const SIDE_PANEL_COLUMNS_SHIFT: u32 = 6;
/// Triplet 13 bits 5 to 9: default screen colour (Table 4).
pub const DEFAULT_SCREEN_COLOUR_SHIFT: u32 = 4;
/// Triplet 13 bits 10 to 14: default row colour.
pub const DEFAULT_ROW_COLOUR_SHIFT: u32 = 9;
/// Triplet 13 bit 15: Black Background Colour Substitution.
pub const BLACK_SUBSTITUTION_SHIFT: u32 = 14;
/// Triplet 13 bits 16 to 18: Colour Table Re-mapping.
pub const REMAPPING_SHIFT: u32 = 15;
/// Mask of the 3-bit re-mapping value.
pub const REMAPPING_MASK: u32 = 0x07;
/// Colour words in X/28/0 format 1, X/28/4, M/29/0 and M/29/4: two CLUTs
/// of eight (Tables 4 and 10).
pub const COLOUR_WORDS: usize = 16;
/// Bits of one colour word: four each of red, green and blue.
pub const COLOUR_WORD_BITS: usize = 12;
/// Bits of one primary in a colour word.
pub const COLOUR_PRIMARY_BITS: u32 = 4;
/// Stream position of the first colour word bit, triplet 2 bit 11, where
/// stream position 18 × t + b − 1 is bit b of triplet t + 1 (Table 4).
pub const COLOUR_WORDS_START: usize = 28;
/// Bits of a DRCS colour lookup table entry: a Colour Map index (Table 8).
pub const DRCS_CLUT_ENTRY_BITS: usize = 5;
/// Entries of the DRCS colour lookup tables for 12 by 10 by 2 characters.
pub const DRCS_CLUT_4: usize = 4;
/// Entries of the DRCS colour lookup tables for 12 by 10 by 4 and 6 by 5
/// by 4 characters.
pub const DRCS_CLUT_16: usize = 16;
/// Stream position of the first DRCS colour table bit, triplet 2 bit 1
/// (Table 8).
pub const DRCS_CLUTS_START: usize = 18;
/// Pattern Transfer Units in a DRCS sub-page (clause 14.1).
pub const DRCS_PTUS: usize = 48;
/// Bits of one DRCS mode identification code (Table 9).
pub const DRCS_MODE_BITS: usize = 4;
/// Stream position of the first mode identification bit, triplet 2 bit 1
/// (Table 9, decision E.54).
pub const DRCS_MODES_START: usize = 18;
/// Mask of the link function, X/27/4 triplet 1 bits 1 and 2 (Table 15).
pub const LINK_FUNCTION_MASK: u32 = 0x03;
/// Shift of the page validity, triplet 1 bits 3 and 4 (Table 15).
pub const LINK_VALIDITY_SHIFT: u32 = 2;
/// Mask of the page validity after the shift.
pub const LINK_VALIDITY_MASK: u32 = 0x03;
/// Shift of the page units, triplet 1 bits 7 to 10.
pub const LINK_UNITS_SHIFT: u32 = 6;
/// Shift of the Format 1 marker, triplet 1 bit 11, which is 1.
pub const LINK_MARKER_FIRST_SHIFT: u32 = 10;
/// Shift of the relative magazine, triplet 1 bits 12 to 14.
pub const LINK_MAGAZINE_SHIFT: u32 = 11;
/// Shift of the page tens, triplet 1 bits 15 to 18.
pub const LINK_TENS_SHIFT: u32 = 14;
/// Mask of the Format 1 marker of triplet 2, bit 1, which is 0.
pub const LINK_MARKER_SECOND_MASK: u32 = 0x01;
/// Shift of the sixteen sub-page flags, triplet 2 bits 3 to 18.
pub const LINK_SUBPAGES_SHIFT: u32 = 2;
/// Mask of the sub-page flags after the shift.
pub const LINK_SUBPAGES_MASK: u32 = 0xFFFF;
/// Links in an X/27/4 or X/27/5 packet (Table 15).
pub const COMPOSITIONAL_LINKS: usize = 6;

// ---------------------------------------------------------------------------
// Object pages (clause 10.5.1, Figure 19)
// ---------------------------------------------------------------------------

/// Bit B1 of byte 6 of packets 1 to 4 of an object page: 1 for pointer
/// data, 0 for object definitions (clause 10.5.1.2).
pub const OBJECT_POINTER_DATA_BIT: u8 = 0x01;
/// The first pointer packet of an object page.
pub const OBJECT_POINTER_FIRST_PACKET: u8 = 1;
/// The last packet that may carry pointers.
pub const OBJECT_POINTER_LAST_PACKET: u8 = 4;
/// The first packet of the definition area: absolute triplet 0 is
/// triplet 0 of packet 3 (clause 10.5.1.2).
pub const OBJECT_DEFINITION_FIRST_PACKET: u8 = 3;
/// Bits of a pointer (clause 10.5.1.2).
pub const OBJECT_POINTER_BITS: u32 = 9;
/// A pointer of all ones: "pointer not used".
pub const OBJECT_POINTER_UNUSED: u16 = 0x1FF;
/// The highest absolute triplet number, triplet 12 of X/26/15
/// (clause 10.5.1.2).
pub const OBJECT_TRIPLET_LAST: u16 = 506;
/// Absolute triplet numbers in packets 3 to 25; X/26/0 starts at this
/// number (clause 10.5.1.2).
pub const OBJECT_TRIPLETS_IN_ROWS: u16 =
    (PACKET_ROW_25 - OBJECT_DEFINITION_FIRST_PACKET + 1) as u16 * TRIPLETS_PER_PACKET as u16;
/// Pointers in one pointer packet: two in each of triplets 1 to 12.
pub const OBJECT_POINTERS_PER_PACKET: usize = 24;

/// Packets an enhancement sub-page can hold: X/1 to X/25 and X/26/0 to
/// X/26/15 (clause 10.5.1.1).
pub const ENHANCEMENT_SUBPAGE_PACKETS: usize = PACKET_ROW_25 as usize + DESIGNATION_CODES;
/// The most packets of DRCS and object pages over all magazines at Level
/// 2.5 (Annex E.8); the documented default capacity of REQ-ACQ-019.
pub const ENHANCEMENT_PACKETS_LEVEL_25: usize = 500;
/// The same total at Level 3.5 (Annex E.8).
pub const ENHANCEMENT_PACKETS_LEVEL_35: usize = 1200;

// ---------------------------------------------------------------------------
// The Magazine Organization Table (clause 10.6, Figure 20, Tables 20 to 23)
// ---------------------------------------------------------------------------

/// Page number of the Magazine Organization Table in every magazine.
pub const PAGE_MOT: u8 = 0xFE;
/// The first packet of the byte pairs for decimal pages x00 to x99.
pub const MOT_DECIMAL_FIRST_PACKET: u8 = 1;
/// The first packet of the byte pairs for tens A to F, units 0 to 9.
pub const MOT_HEX_TENS_FIRST_PACKET: u8 = 6;
/// The first packet of the byte pairs for units A to F.
pub const MOT_HEX_UNITS_FIRST_PACKET: u8 = 9;
/// Byte pairs per packet for pages with decimal units (Figure 20).
pub const MOT_PAIRS_DECIMAL_UNITS: usize = 20;
/// Byte pairs per packet for pages with units A to F (Figure 20).
pub const MOT_PAIRS_HEX_UNITS: usize = 18;
/// Units digits 0 to 9, and hexadecimal units A to F (Figure 20).
pub const MOT_DECIMAL_DIGITS: u8 = 10;
/// Hexadecimal units digits A to F in each tens row (Figure 20).
pub const MOT_HEX_DIGITS: u8 = 6;
/// Packets of the Level 2.5 object links: GPOP and POP links 1 to 3, then
/// POP links 4 to 7 (Table 20).
pub const MOT_OBJECT_LINKS_LEVEL_25: [u8; 2] = [19, 20];
/// Packet of the Level 2.5 DRCS links (Table 22).
pub const MOT_DRCS_LINKS_LEVEL_25: u8 = 21;
/// Packets of the Level 3.5 object links (Table 20).
pub const MOT_OBJECT_LINKS_LEVEL_35: [u8; 2] = [22, 23];
/// Packet of the Level 3.5 DRCS links (Table 22).
pub const MOT_DRCS_LINKS_LEVEL_35: u8 = 24;
/// Packet byte of the first object link (Table 20).
pub const MOT_OBJECT_LINK_FIRST_BYTE: usize = 6;
/// Bytes of an object link (Table 21).
pub const MOT_OBJECT_LINK_BYTES: usize = 10;
/// Object links in one packet (Table 20).
pub const MOT_OBJECT_LINKS_PER_PACKET: usize = 4;
/// Packet byte of the first DRCS link (Table 22).
pub const MOT_DRCS_LINK_FIRST_BYTE: usize = 6;
/// Bytes of a DRCS link (Table 23).
pub const MOT_DRCS_LINK_BYTES: usize = 4;
/// DRCS links in one packet: the GDRCS link and DRCS links 1 to 7.
pub const MOT_DRCS_LINKS: usize = 8;
/// Packets X/1 to X/24 of a Magazine Organization Table (clause 10.6.1).
pub const MOT_PACKETS: usize = 24;
/// Packet byte of the totals of enhancement pages (clause 10.6.6).
pub const MOT_TOTALS_BYTE: usize = 40;
/// Data bit 4 of a link's first byte: the X/27/4,5 flag (Table 21).
pub const MOT_LINK_X27_FLAG: u8 = 0x08;
/// Data bit 4 of a byte pair's bytes: the global object or DRCS flag
/// (clause 10.6.2).
pub const MOT_GLOBAL_FLAG: u8 = 0x08;
/// Data bits 3 to 1 of a byte pair's bytes: the link number 1 to 7, or 0.
pub const MOT_LINK_NUMBER_MASK: u8 = 0x07;
/// Fallback byte N+4 bit 1: 1 means no side panel and no substitution
/// (Table 21).
pub const MOT_FALLBACK_NONE: u8 = 0x01;
/// Shift of the default side panel, bits 3 and 2 of byte N+4.
pub const MOT_FALLBACK_PANELS_SHIFT: u32 = 1;
/// Mask of the default side panel after the shift: 00 none, 01 left 16,
/// 10 right 16, 11 both 8.
pub const MOT_FALLBACK_PANELS_MASK: u8 = 0x03;
/// Byte N+4 bit 4: black background replaced by the full row colour.
pub const MOT_FALLBACK_SUBSTITUTION: u8 = 0x08;
/// The full width of one side panel (clause 12.5.2).
pub const SIDE_PANEL_MAX_COLUMNS: u8 = 16;
/// The width of each panel when both are used by the MOT fallback.
pub const SIDE_PANEL_HALF_COLUMNS: u8 = 8;
/// Shift of the second default object's type, bits 4 and 3 of byte N+5
/// (clause 10.6.4.1).
pub const MOT_DEFAULT_SECOND_SHIFT: u32 = 2;
// ---------------------------------------------------------------------------

/// Shift of the group in a 7-bit Table 32 designation value: the group is
/// bits 4 to 7, the national option bits 1 to 3 (Table 32).
pub const DESIGNATION_GROUP_SHIFT: u32 = 3;
/// Table 33 value meaning "no second G0 set required".
pub const NO_SECOND_G0: u8 = 0x7F;
/// Shift of the page coding within triplet 1 of X/28/0 format 1: bits 5 to
/// 7 (Table 3).
pub const PAGE_CODING_SHIFT: u32 = 4;
/// Mask of the 3-bit page coding.
pub const PAGE_CODING_MASK: u32 = 0x7;
/// The 13 national option positions of the Latin G0 set (Table 36).
pub const NATIONAL_POSITIONS: [u8; 13] = [0x23, 0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x7B, 0x7C, 0x7D, 0x7E];
/// The right guillemet "»" in the Latin G2 set (Table 37, position 3/B),
/// the receiver's mark for a link leaving the service (REQ-NAV-011).
pub const G2_RIGHT_GUILLEMET: u8 = 0x3B;

// ---------------------------------------------------------------------------
// Spacing attributes (clause 12.2, Table 26)
// ---------------------------------------------------------------------------

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
// The Colour Map of Levels 2.5 and 3.5 (Table 30, decision E.43)
// ---------------------------------------------------------------------------

/// Entries of one colour lookup table (CLUT). The eight Level 1 colours are
/// CLUT 0, so existing colour numbers keep their meaning (decision E.43).
pub const CLUT_ENTRIES: usize = COLOURS;
/// Colour lookup tables in the Colour Map, CLUTs 0 to 3.
pub const CLUTS: usize = 4;
/// Entries of the Colour Map: Colour Map index = 8 × CLUT + entry.
pub const COLOUR_MAP_ENTRIES: usize = CLUTS * CLUT_ENTRIES;
/// Shift of the CLUT number in a Colour Map index.
pub const CLUT_SHIFT: u32 = 3;
/// Mask of the entry number in a Colour Map index.
pub const CLUT_ENTRY_MASK: u8 = 0x07;
/// Colour Map index 8, CLUT 1 entry 0: transparent at every level, and
/// never redefined (Table 30). It is also the renderer's transparent
/// palette index ([`PALETTE_TRANSPARENT`]); a test asserts they agree.
pub const COLOUR_TRANSPARENT: u8 = 8;
/// The first CLUT that X/28/4 and M/29/4 redefine: CLUTs 0 and 1 (Table 10,
/// decision E.43).
pub const CLUT_LEVEL_35_FIRST: u8 = 0;
/// The first CLUT that X/28/0 format 1 and M/29/0 redefine: CLUTs 2 and 3
/// (Table 4).
pub const CLUT_LEVEL_25_FIRST: u8 = 2;
/// The default Colour Map of Table 30, verified on the page image (p.89):
/// each entry `0xRGB`, four bits per primary. CLUT 1 entry 5 is 7, 0, 7 and
/// entry 6 is 0, 7, 7 (decision E.43, where zvbi 0.2.44 differs). CLUT 1
/// entry 0 is transparent; its value here is never shown.
pub const COLOUR_MAP_DEFAULTS: [u16; COLOUR_MAP_ENTRIES] = [
    // CLUT 0: black, red, green, yellow, blue, magenta, cyan, white.
    0x000, 0xF00, 0x0F0, 0xFF0, 0x00F, 0xF0F, 0x0FF, 0xFFF, //
    // CLUT 1: transparent, then the half-intensity colours and grey.
    0x000, 0x700, 0x070, 0x770, 0x007, 0x707, 0x077, 0x777, //
    // CLUT 2.
    0xF05, 0xF70, 0x0F7, 0xFFB, 0x0CA, 0x500, 0x652, 0xC77, //
    // CLUT 3.
    0x333, 0xF77, 0x7F7, 0xFF7, 0x77F, 0xF7F, 0x7FF, 0xDDD,
];
/// A 4-bit primary times 17 is its 8-bit value, so that 15 gives 255
/// (REQ-RND-030: 16 equal steps).
pub const PRIMARY_4_TO_8: u8 = 17;
/// Colour Table Re-mapping, Table 4 triplet 13 bits 16 to 18: the
/// foreground and background CLUTs of the Level 1 spacing colours for each
/// value 0 to 7.
pub const REMAPPING_CLUTS: [[u8; 2]; 8] = [[0, 0], [0, 1], [0, 2], [1, 1], [1, 2], [2, 1], [2, 2], [2, 3]];

// ---------------------------------------------------------------------------
// Side panels (clause 12.5, decision E.49)
// ---------------------------------------------------------------------------

/// The first column of the side-panel area, where the right panel starts
/// (clause 12.5.3).
pub const SIDE_PANEL_FIRST_COLUMN: usize = COLUMNS;
/// Columns of the side-panel area, 40 to 71; the left panel ends at 71.
pub const SIDE_PANEL_AREA_COLUMNS: usize = 2 * SIDE_PANEL_MAX_COLUMNS as usize;
/// The first row of the side panels; row 0 has no panel content (clause
/// 12.5.2).
pub const SIDE_PANEL_FIRST_ROW: usize = 1;
/// Rows of the side panels, 1 to 24.
pub const SIDE_PANEL_ROWS: usize = ROWS - SIDE_PANEL_FIRST_ROW;

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
// Flash at Levels 2.5 and 3.5 (decision E.48, Table 29 mode 00111)
// ---------------------------------------------------------------------------

/// One flash cycle at Levels 2.5 and 3.5 lasts one second (decision E.48).
pub const FLASH_CYCLE_US: u64 = 1_000_000;
/// The cycle is divided into twelve units of one twelfth of a second,
/// numbered 0 to 11: twelve divides both rates and the three fast phases
/// evenly (decision E.48).
pub const FLASH_UNITS: u8 = 12;
/// Slow flash (1 hertz) is on during units 0 to 8 and off during units 9
/// to 11, the 3 to 1 ratio of decision E.12.
pub const FLASH_SLOW_ON_UNITS: u8 = 9;
/// Fast flash (2 hertz) treats each half-second of six units alike.
pub const FLASH_FAST_HALF_UNITS: u8 = FLASH_UNITS / 2;
/// The fast flash phases, 1 to 3.
pub const FLASH_PHASES: u8 = 3;
/// A fast flash character with phase p is off during the two units
/// 2(p − 1) and 2p − 1 of each half-second.
pub const FLASH_FAST_OFF_UNITS: u8 = 2;

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

    /// Decision E.48: the fast phases fill a half-second exactly, and slow
    /// flash keeps the 3 to 1 ratio that decision E.12 measured.
    #[test]
    fn flash_units_agree_with_decisions_e12_and_e48() {
        assert_eq!(FLASH_FAST_OFF_UNITS * FLASH_PHASES, FLASH_FAST_HALF_UNITS);
        let slow_off = u64::from(FLASH_UNITS - FLASH_SLOW_ON_UNITS);
        assert_eq!(u64::from(FLASH_SLOW_ON_UNITS) * FLASH_FIELDS_HIDDEN, slow_off * FLASH_FIELDS_SHOWN);
    }

    /// A DRCS character fills a 12 by 10 cell exactly (clause 14.2,
    /// REQ-RND-038).
    #[test]
    fn drcs_patterns_fill_a_cell() {
        assert_eq!(DRCS_PATTERN_DOTS, CELL_WIDTH);
        assert_eq!(DRCS_PATTERN_ROWS, CELL_HEIGHT_SMALL);
    }

    /// Decision E.43: the Colour Map's transparent entry is the renderer's
    /// transparent palette index.
    #[test]
    fn transparent_indices_agree() {
        assert_eq!(COLOUR_TRANSPARENT, PALETTE_TRANSPARENT);
        assert_eq!(usize::from(COLOUR_TRANSPARENT), CLUT_ENTRIES);
        assert_eq!(COLOUR_MAP_ENTRIES, 1 << (CLUT_SHIFT + 2));
        assert_eq!(usize::from(SIDE_PANEL_MAX_COLUMNS) * 2 + COLUMNS, 72);
    }

    #[test]
    fn palette_ranges_do_not_overlap() {
        let flash_end = usize::from(PALETTE_FLASH_BASE) + COLOURS * COLOURS;
        assert!(flash_end <= usize::from(PALETTE_CONCEAL_BASE));
        assert_eq!(usize::from(PALETTE_CONCEAL_BASE) + COLOURS * COLOURS, PALETTE_SIZE);
    }
}
