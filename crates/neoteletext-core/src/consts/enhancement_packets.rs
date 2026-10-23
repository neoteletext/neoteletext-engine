//! Page-related and magazine-related enhancement packets and compositional
//! links (clauses 9.4.2 to 9.5.3, 9.6.2; Tables 3, 4 and 8 to 16): the
//! designation codes, page functions, the fields of Table 4, the link
//! fields and functions, and the designation shifts of Tables 32 and 33.

/// X/28 and M/29 designation code 0: X/28/0 format 1 and M/29/0.
pub const DESIGNATION_PAGE_DATA: u8 = 0;
/// X/28 designation code 3: the data of a pattern download page, of which
/// only the page function is read (Table 9, decision E.95).
pub const DESIGNATION_DOWNLOAD_FUNCTION: u8 = 3;
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
/// Page function 0100: Global DRCS page. Its patterns are not read; the
/// code is kept so that such a page is never shown as text (decision E.95).
pub const PAGE_FUNCTION_GDRCS: u8 = 0x4;
/// Page function 0101: normal DRCS page, kept for the same reason.
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

/// Link functions of X/27/4 and X/27/5 links (Table 15): GPOP, then POP.
/// Links 0 to 3 of X/27/4 carry the four functions in order (Table 16);
/// the last two, for DRCS pages, are not used (decision E.95).
pub const LINK_FUNCTION_GPOP: u8 = 0b00;
/// The POP link function (Table 15).
pub const LINK_FUNCTION_POP: u8 = 0b01;
/// X/27/4 links 0 to 3 have fixed roles; links 4 and 5 of X/27/4 and 0 and
/// 1 of X/27/5 take their role from their function bits at Level 3.5
/// (Table 16).
pub const LINKS_FIXED_ROLES: usize = 4;
/// The links of X/27/5 that Level 3.5 uses (Table 16); the others are
/// reserved.
pub const LINKS_X27_5_USED: usize = 2;

/// Shift of the group in a 7-bit Table 32 designation value: the group is
/// bits 4 to 7, the national option bits 1 to 3 (Table 32).
pub const DESIGNATION_GROUP_SHIFT: u32 = 3;
/// Table 33 value meaning "no second G0 set required".
pub const NO_SECOND_G0: u8 = 0x7F;
/// Table 33 value of Latin G0 with the English option: group 0000,
/// option 000, the second set of neoteletext's own sets (REQ-CHR-012).
pub const DESIGNATION_LATIN_ENGLISH: u8 = 0x00;
/// Shift of the page coding within triplet 1 of X/28/0 format 1: bits 5 to
/// 7 (Table 3).
pub const PAGE_CODING_SHIFT: u32 = 4;
/// Mask of the 3-bit page coding.
pub const PAGE_CODING_MASK: u32 = 0x7;
