//! Enhancement triplets (clause 12.3.1, Tables 27 to 29): the fields, the
//! row and column modes, their data, the flash codes and the G1 columns a
//! triplet reads as G0.

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
/// Column mode 00101, reserved in Table 29, is NeoTeletext's control
/// triplet (decision E.127): cells a page answers itself.
pub const MODE_CONTROL: u8 = MODE_COLUMN_RESERVED_2;
/// Column mode 01010, reserved in Table 29, is NeoTeletext's pointer-area
/// triplet (decision E.128): cells where every move goes to the channel.
pub const MODE_POINTER_AREA: u8 = MODE_COLUMN_RESERVED_3;
/// A control or pointer-area triplet's data bits D5 to D0: its width, 1 to
/// 40 cells; 0, in a pointer area not joined, the whole page.
pub const CONTROL_WIDTH_MASK: u8 = 0x3F;
/// A control triplet's data bit D6: the run joins the control on the row
/// above, at the same column and width.
pub const CONTROL_JOINED: u8 = 0x40;
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
/// Column mode 01101: DRCS Character Invocation (Table 29), recognised only
/// to move the active position (decision E.95).
pub const MODE_DRCS_CHARACTER: u8 = 0x0D;
/// Column mode 01110: Font Style, Level 3.5 only (Table 29), recognised
/// only to move the active position (decision E.96).
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
/// field is reserved but must still be a row address (Full Screen
/// Colour): 41, which Annex A.5 note 1 recommends for Level 1.5
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
/// The Level 2.5 bit of a two-bit level field: S0 of an Object Definition's
/// usage, and the low bit of an X/27/4 page validity
/// (Tables 15 and 28: 01 Level 2.5, 10 Level 3.5, 11 both).
pub const LEVEL_BIT_25: u8 = 0b01;
/// The Level 3.5 bit of a two-bit level field.
pub const LEVEL_BIT_35: u8 = 0b10;
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
/// The hook above, the Vietnamese G2 set's 4/C (REQ-CHR-014, decision
/// E.81). Table 29's modes number only 15 marks, so the hook is kept as a
/// sixteenth past them.
pub const DIACRITIC_HOOK_ABOVE: u8 = 16;
/// The mark number of 4/C, the low line in Latin G2 (Table 37).
pub const DIACRITIC_LOW_LINE: u8 = 0x0C;

/// Flash mode codes, Table 29 mode 00111 data bits D1 D0: steady.
pub const FLASH_MODE_STEADY: u8 = 0b00;
/// Normal flash to the background colour.
pub const FLASH_MODE_NORMAL: u8 = 0b01;
/// Flash to the background colour in inverted phase.
pub const FLASH_MODE_INVERT_PHASE: u8 = 0b10;
/// Flash to the same entry of the adjacent CLUT.
pub const FLASH_MODE_ADJACENT_CLUT: u8 = 0b11;
/// Flash rate codes, Table 29 mode 00111 data bits D4 to D2: slow, 1 hertz.
/// 110 and 111 are reserved.
pub const FLASH_RATE_SLOW: u8 = 0b000;
/// Fast, 2 hertz, phase 1.
pub const FLASH_RATE_PHASE_1: u8 = 0b001;
/// Fast, phase 2.
pub const FLASH_RATE_PHASE_2: u8 = 0b010;
/// Fast, phase 3.
pub const FLASH_RATE_PHASE_3: u8 = 0b011;
/// Fast, incremental: apparent movement to the right from phase 1.
pub const FLASH_RATE_INCREMENTAL: u8 = 0b100;
/// Fast, decremental: apparent movement to the left from phase 1.
pub const FLASH_RATE_DECREMENTAL: u8 = 0b101;

/// The G1 codes that a triplet reads as the current G0 set: columns 4 and 5
/// (Table 47 note 3).
pub const G1_G0_COLUMNS_FIRST: u8 = 0x40;
/// The last code of G1 columns 4 and 5.
pub const G1_G0_COLUMNS_LAST: u8 = 0x5F;
