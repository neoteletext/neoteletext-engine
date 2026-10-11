//! Constants of the container and file formats this crate reads, each
//! defined once (implementation decision 6). Teletext values themselves come
//! only from `neoteletext_core::consts` (REQ-GEN-014).
//!
//! Sources: ISO/IEC 13818-1 (MPEG-2 systems), ETSI EN 300 472 (teletext in
//! DVB), ETSI EN 300 468 (DVB service information), the Linux V4L2 sliced
//! VBI interface, and the page-file notes of `research/notes-formats.md`.

use neoteletext_core::consts::PACKET_BYTES;

// ---------------------------------------------------------------------------
// MPEG-2 transport stream (ISO/IEC 13818-1 clause 2.4.3)
// ---------------------------------------------------------------------------

/// The transport packet sync byte (ISO/IEC 13818-1 clause 2.4.3.3).
pub const TS_SYNC_BYTE: u8 = 0x47;
/// Bytes of a transport packet (ISO/IEC 13818-1 clause 2.4.3.2).
pub const TS_PACKET_BYTES: usize = 188;
/// Bytes per packet of an M2TS stream: a 4-byte time stamp prefix before
/// each 188-byte packet (REQ-INP-006).
pub const TS_M2TS_PACKET_BYTES: usize = 192;
/// Bytes per packet with 16 bytes of Reed-Solomon parity after each packet
/// (EN 300 421 clause 4.4.2, RS(204,188)).
pub const TS_FEC_PACKET_BYTES: usize = 204;
/// Bytes of the fixed transport packet header (clause 2.4.3.2).
pub const TS_HEADER_BYTES: usize = 4;
/// `transport_error_indicator`, byte 1 bit 7.
pub const TS_TRANSPORT_ERROR: u8 = 0x80;
/// `payload_unit_start_indicator`, byte 1 bit 6.
pub const TS_PAYLOAD_UNIT_START: u8 = 0x40;
/// The five PID bits of byte 1.
pub const TS_PID_HIGH_MASK: u8 = 0x1F;
/// Shift of `adaptation_field_control` in byte 3.
pub const TS_ADAPTATION_SHIFT: u32 = 4;
/// `adaptation_field_control` bit: an adaptation field is present (Table 2-5).
pub const TS_ADAPTATION_PRESENT: u8 = 0x2;
/// `adaptation_field_control` bit: a payload is present (Table 2-5).
pub const TS_PAYLOAD_PRESENT: u8 = 0x1;
/// Mask of the two `adaptation_field_control` bits.
pub const TS_ADAPTATION_MASK: u8 = 0x3;
/// Mask of `continuity_counter`, byte 3 bits 0 to 3.
pub const TS_CONTINUITY_MASK: u8 = 0x0F;
/// `discontinuity_indicator`, the first flag of the adaptation field
/// (clause 2.4.3.4).
pub const TS_DISCONTINUITY: u8 = 0x80;
/// PID of the Program Association Table (Table 2-3).
pub const PID_PAT: u16 = 0x0000;
/// PID of null packets (Table 2-3).
pub const PID_NULL: u16 = 0x1FFF;
/// Mask of a 13-bit PID.
pub const PID_MASK: u16 = 0x1FFF;

// ---------------------------------------------------------------------------
// Program-specific information (ISO/IEC 13818-1 clause 2.4.4)
// ---------------------------------------------------------------------------

/// `table_id` of the Program Association Table (Table 2-31).
pub const TABLE_ID_PAT: u8 = 0x00;
/// `table_id` of a Program Map Table (Table 2-31).
pub const TABLE_ID_PMT: u8 = 0x02;
/// Bytes before `section_length` ends: `table_id` and the two length bytes.
pub const SECTION_HEADER_BYTES: usize = 3;
/// The largest PSI section: `section_length` is at most 1021 (clause
/// 2.4.4.11), plus the three header bytes.
pub const SECTION_MAX_BYTES: usize = 1024;
/// Mask of the 12-bit `section_length` and the info length fields.
pub const SECTION_LENGTH_MASK: u16 = 0x0FFF;
/// Bytes of the `CRC_32` that ends every long-form section.
pub const SECTION_CRC_BYTES: usize = 4;
/// Generator polynomial of the MPEG-2 `CRC_32` (Annex A).
pub const CRC32_POLYNOMIAL: u32 = 0x04C1_1DB7;
/// Initial value of the MPEG-2 `CRC_32` register (Annex A).
pub const CRC32_INITIAL: u32 = 0xFFFF_FFFF;
/// Filler byte after the last section in a packet (clause 2.4.4).
pub const SECTION_STUFFING: u8 = 0xFF;
/// Offset of the first program entry in a PAT section (clause 2.4.4.3).
pub const PAT_ENTRIES_START: usize = 8;
/// Bytes per PAT program entry.
pub const PAT_ENTRY_BYTES: usize = 4;
/// Offset of `program_info_length` in a PMT section (clause 2.4.4.8).
pub const PMT_INFO_LENGTH: usize = 10;
/// Offset of the first byte after `program_info_length`.
pub const PMT_DESCRIPTORS_START: usize = 12;
/// Bytes of an elementary stream entry header in a PMT.
pub const PMT_STREAM_HEADER_BYTES: usize = 5;
/// Offset of `version_number`'s byte in a long-form section.
pub const SECTION_VERSION_BYTE: usize = 5;
/// Shift and mask of `version_number` in that byte.
pub const SECTION_VERSION_SHIFT: u32 = 1;
/// Mask of the 5-bit `version_number`.
pub const SECTION_VERSION_MASK: u8 = 0x1F;
/// `stream_type` of PES packets carrying private data (Table 2-34), used for
/// teletext (EN 300 472 clause 4.1).
pub const STREAM_TYPE_PRIVATE_PES: u8 = 0x06;

// ---------------------------------------------------------------------------
// Descriptors (EN 300 468 clause 6)
// ---------------------------------------------------------------------------

/// Bytes of a descriptor header: tag and length.
pub const DESCRIPTOR_HEADER_BYTES: usize = 2;
/// `teletext_descriptor` (EN 300 468 clause 6.2.43).
pub const DESCRIPTOR_TELETEXT: u8 = 0x56;
/// `VBI_teletext_descriptor` (EN 300 468 clause 6.2.48).
pub const DESCRIPTOR_VBI_TELETEXT: u8 = 0x46;
/// `VBI_data_descriptor` (EN 300 468 clause 6.2.47).
pub const DESCRIPTOR_VBI_DATA: u8 = 0x45;
/// Bytes per teletext descriptor entry: language, type and magazine, page.
pub const TELETEXT_ENTRY_BYTES: usize = 5;
/// Shift of `teletext_type` in the fourth entry byte (EN 300 468 6.2.43).
pub const TELETEXT_TYPE_SHIFT: u32 = 3;
/// Mask of `teletext_magazine_number` in that byte.
pub const TELETEXT_MAGAZINE_MASK: u8 = 0x07;
/// `data_service_id` of EBU teletext in a VBI data descriptor (EN 300 468
/// Table 100).
pub const DATA_SERVICE_EBU_TELETEXT: u8 = 0x01;
/// `data_service_id` of inverted teletext (EN 300 468 Table 100).
pub const DATA_SERVICE_INVERTED_TELETEXT: u8 = 0x02;

// ---------------------------------------------------------------------------
// Packetised elementary streams (ISO/IEC 13818-1 clause 2.4.3.6)
// ---------------------------------------------------------------------------

/// `packet_start_code_prefix` 0x000001 as three bytes.
pub const PES_START_CODE: [u8; 3] = [0x00, 0x00, 0x01];
/// `stream_id` of `private_stream_1` (Table 2-22), used for teletext
/// (EN 300 472 clause 4.2).
pub const STREAM_ID_PRIVATE_1: u8 = 0xBD;
/// Bytes up to and including `PES_packet_length`.
pub const PES_FIXED_HEADER_BYTES: usize = 6;
/// Bytes of the optional header before `PES_header_data_length`'s data:
/// the two flag bytes and `PES_header_data_length` itself.
pub const PES_OPTIONAL_HEADER_BYTES: usize = 3;
/// `PTS_DTS_flags` bit meaning a PTS is present, in the second flag byte.
pub const PES_PTS_PRESENT: u8 = 0x80;
/// Bytes of a coded PTS.
pub const PES_PTS_BYTES: usize = 5;
/// The PTS counts a 90 kHz clock (clause 2.4.3.7): microseconds are
/// PTS × 100 / 9.
pub const PTS_TO_US_NUMERATOR: u64 = 100;
/// Denominator of the PTS conversion.
pub const PTS_TO_US_DENOMINATOR: u64 = 9;

// ---------------------------------------------------------------------------
// Teletext in DVB (EN 300 472 clause 4)
// ---------------------------------------------------------------------------

/// First `data_identifier` of EBU data (EN 300 472 Table 2).
pub const DATA_IDENTIFIER_EBU_FIRST: u8 = 0x10;
/// Last `data_identifier` of EBU data (EN 300 472 Table 2).
pub const DATA_IDENTIFIER_EBU_LAST: u8 = 0x1F;
/// First `data_identifier` of EN 301 775 VBI data that may carry teletext.
pub const DATA_IDENTIFIER_VBI_FIRST: u8 = 0x99;
/// Last `data_identifier` of EN 301 775 VBI data that may carry teletext.
pub const DATA_IDENTIFIER_VBI_LAST: u8 = 0x9B;
/// `data_unit_id` of EBU teletext non-subtitle data (EN 300 472 Table 3).
pub const DATA_UNIT_TELETEXT: u8 = 0x02;
/// `data_unit_id` of EBU teletext subtitle data (EN 300 472 Table 3).
pub const DATA_UNIT_SUBTITLE: u8 = 0x03;
/// `data_unit_id` of inverted teletext (EN 301 775 Table 4).
pub const DATA_UNIT_INVERTED: u8 = 0xC0;
/// `data_unit_id` of stuffing (EN 300 472 Table 3).
pub const DATA_UNIT_STUFFING: u8 = 0xFF;
/// Bytes of a data unit header: `data_unit_id` and `data_unit_length`.
pub const DATA_UNIT_HEADER_BYTES: usize = 2;
/// `data_unit_length` of a teletext data unit: the field and line byte, the
/// framing code and the packet (EN 300 472 clause 4.4, value 0x2C).
pub const DATA_UNIT_TELETEXT_LENGTH: usize = 2 + PACKET_BYTES;
/// `field_parity`, bit 5 of the first data byte: 1 is the first field.
pub const DATA_UNIT_FIELD_PARITY: u8 = 0x20;
/// `line_offset`, bits 0 to 4 of the first data byte; 0 means unknown.
pub const DATA_UNIT_LINE_MASK: u8 = 0x1F;

// ---------------------------------------------------------------------------
// Linux sliced VBI records (V4L2 `struct v4l2_sliced_vbi_data`)
// ---------------------------------------------------------------------------

/// Bytes per record: four 32-bit fields and 48 data bytes.
pub const SLICED_RECORD_BYTES: usize = 64;
/// Bytes of the four 32-bit fields.
pub const SLICED_HEADER_BYTES: usize = 16;
/// `V4L2_SLICED_TELETEXT_B`.
pub const SLICED_TELETEXT_B: u32 = 0x0001;
/// Offsets of the field and line words.
pub const SLICED_FIELD_OFFSET: usize = 4;
/// Offset of the line word.
pub const SLICED_LINE_OFFSET: usize = 8;

// ---------------------------------------------------------------------------
// TTI page files (MRG specification and vbit2; notes-formats.md section 5)
// ---------------------------------------------------------------------------

/// Bytes of a TTI command name.
pub const TTI_COMMAND_BYTES: usize = 2;
/// Characters of a packed enhancement line after the designation code:
/// thirteen triplets of three characters.
pub const TTI_PACKED_CHARACTERS: usize = 39;
/// Bits carried by each character of a packed triplet.
pub const TTI_TRIPLET_CHARACTER_BITS: u32 = 6;
/// Mask of those bits.
pub const TTI_TRIPLET_CHARACTER_MASK: u32 = 0x3F;
/// Bit 6, set on every packed character so that it is printable.
pub const TTI_PACKED_CHARACTER_BASE: u8 = 0x40;
/// FL value 0 means no link (MRG section 2.7).
pub const TTI_FL_NO_LINK: u16 = 0x000;
/// FL value 8FF also means no link (vbit2).
pub const TTI_FL_NULL_LINK: u16 = 0x8FF;
/// Number of FL links.
pub const TTI_FL_LINKS: usize = 6;
/// PS bit with no control-bit meaning: "transmit page" (MRG section 2.4).
pub const TTI_PS_TRANSMIT: u16 = 0x8000;
/// PS bit of C4, erase page (decision E.21).
pub const TTI_PS_C4: u16 = 0x4000;
/// PS bits 0 to 6 are C5 to C11 in order (decision E.21).
pub const TTI_PS_C5_TO_C11: u16 = 0x007F;
/// Shift from PS bits 0 to 6 to control bits 5 to 11.
pub const TTI_PS_C5_SHIFT: u32 = 5;
/// PS bit 7 is C12, bit 8 C13 and bit 9 C14 (decision E.21).
pub const TTI_PS_C12_BIT: u32 = 7;

// ---------------------------------------------------------------------------
// Pages of Levels 2.5 and 3.5 in files and streams (REQ-INP-023 to 025)
// ---------------------------------------------------------------------------

/// Page coding 010 (Table 3): per packet one Hamming 8/4 byte, then
/// thirteen Hamming 24/18 triplets. Object pages use it.
pub const PAGE_CODING_TRIPLETS: u8 = 0x2;
/// Page coding 011 (Table 3): every byte Hamming 8/4. The Magazine
/// Organization Table uses it (clause 10.6.1).
pub const PAGE_CODING_NIBBLES: u8 = 0x3;
/// Table 6: for a page function other than basic Level 1 and data
/// broadcasting, bits 8 to 18 of X/28/0 triplet 1 are "11111111100". The
/// string is read in the order the table names the bits, bit 8 first, so
/// bits 8 to 16 are 1 and bits 17 and 18 are 0. Either reading makes bits 9
/// to 14 non-zero, which is what keeps EN 300 708 decoders away (ETS 300
/// 708 clause 5.4.1, Table 6).
pub const NOT_TEXT_PAGE_MARKER: u32 = 0x1FF << 7;
/// Termination Marker types, data bits D2 D1 D0 (Table 28, mode 11111):
/// intermediate object sub-page, end of an object, more follow.
pub const TERMINATION_INTERMEDIATE_MORE: u8 = 0b000;
/// Intermediate object sub-page, end of the last object.
pub const TERMINATION_INTERMEDIATE_LAST: u8 = 0b001;
/// Last object sub-page, end of an object, more follow.
pub const TERMINATION_FINAL_MORE: u8 = 0b010;
/// Last object sub-page, end of the last object.
pub const TERMINATION_FINAL_LAST: u8 = 0b011;
/// Local Object definitions, end of an object, more follow.
pub const TERMINATION_LOCAL_OBJECT_MORE: u8 = 0b100;
/// Local Object definitions, end of the last object.
pub const TERMINATION_LOCAL_OBJECT_LAST: u8 = 0b101;
/// Local enhancement data ends; Local Object definitions follow.
pub const TERMINATION_LOCAL_DATA_OBJECTS_FOLLOW: u8 = 0b110;
/// Local enhancement data ends; no Local Object definitions follow.
pub const TERMINATION_LOCAL_DATA_END: u8 = 0b111;
/// Objects of one type in a pointer packet: two in each of the four
/// pointer triplets of that type (clause 10.5.1.2).
pub const OBJECTS_PER_POINTER_PACKET: usize = 8;
/// Objects of one type on one object sub-page: packets 1 to 4
/// (clause 10.5.1.1).
pub const OBJECTS_PER_TYPE: usize = 32;
/// Shift of the sub-code's S3 digit; S4 S3 together hold the last packet
/// of an enhancement page as six bits (Annex A.1).
pub const SUBCODE_LAST_PACKET_SHIFT: u32 = 8;
/// Shift of the sub-code's S2 digit, the update indicator (Annex A.1).
pub const SUBCODE_UPDATE_SHIFT: u32 = 4;
/// Mask of the three bits of S2.
pub const SUBCODE_UPDATE_MASK: u16 = 0x7;
/// The last packet number of an enhancement page sent up to X/26 with
/// designation code d is this plus d (Annex A.1).
pub const LAST_PACKET_ENHANCEMENT_BASE: u8 = 26;
/// Packets X/1 to X/24 of a DRCS page carry two PTUs each
/// (clause 14.1, Figure 27).
pub const DRCS_PTUS_PER_PACKET: usize = 2;
/// D-bytes of one PTU (clause 14.1).
pub const DRCS_PTU_BYTES: usize = 20;
/// Pattern bits in one D-byte; bit 6 of the 7-bit code is always 1
/// (clause 14.1).
pub const DRCS_DOTS_PER_BYTE: usize = 6;
/// Bit 6 of every D-byte: its codes are 4/0 to 7/F (clause 14.1).
pub const DRCS_BYTE_BASE: u8 = 0x40;
/// Rows and columns of a mode 3 character (Table 31): 6 by 5 pixels.
pub const DRCS_MODE3_COLUMNS: usize = 6;
/// Rows of a mode 3 character.
pub const DRCS_MODE3_ROWS: usize = 5;
/// X/28/3 mode identification codes (Table 9): a further PTU of a mode 1
/// or mode 2 character.
pub const DRCS_MODE_SUBSEQUENT: u8 = 0xE;
/// X/28/3 mode identification code: no data for this PTU.
pub const DRCS_MODE_NO_DATA: u8 = 0xF;
/// The PTUs of a mode 1 (12 by 10 by 2) character.
pub const DRCS_MODE1_PTUS: usize = 2;
/// The PTUs of a mode 2 (12 by 10 by 4) character.
pub const DRCS_MODE2_PTUS: usize = 4;
/// Highest packet of a DRCS page; X/25 is not used (Figure 27).
pub const DRCS_LAST_PACKET: u8 = 24;

// ---------------------------------------------------------------------------
// EP1 page files (notes-formats.md section 6.1)
// ---------------------------------------------------------------------------

/// The two signature bytes FE 01.
pub const EP1_SIGNATURE: [u8; 2] = [0xFE, 0x01];
/// Bytes of the file header.
pub const EP1_HEADER_BYTES: usize = 6;
/// Offset of the language byte.
pub const EP1_LANGUAGE: usize = 2;
/// Offset of the enhancement flag.
pub const EP1_FLAG: usize = 3;
/// The enhancement flag value meaning a block is present.
pub const EP1_ENHANCED: u8 = 0xCA;
/// The two bytes that open the enhancement block.
pub const EP1_BLOCK_MARKER: [u8; 2] = [0xC2, 0x00];
/// Offset of the little-endian offset to the Level 1 data.
pub const EP1_OFFSET: usize = 4;
/// Bytes of the enhancement block header (C2 00 and a 2-byte length).
pub const EP1_ENHANCEMENT_HEADER_BYTES: usize = 4;
/// Bytes per stored enhancement packet: designation code and 13 triplets.
pub const EP1_PACKET_BYTES: usize = 40;
/// The most enhancement packets a file can hold.
pub const EP1_MAX_PACKETS: usize = 16;
/// Address byte that marks the terminator in place of 63.
pub const EP1_TERMINATOR_ADDRESS: u8 = 0x7F;
/// Rows of Level 1 data, 0 to 23, then one further row.
pub const EP1_ROWS: usize = 24;
/// Bytes of the trailing terminator.
pub const EP1_TERMINATOR_BYTES: usize = 2;

/// EP1 language codes (notes-formats.md section 6.1), each with the Table 32
/// designation value it selects: group in bits 3 to 6, national option index
/// in bits 0 to 2. Languages with no Table 32 entry are absent.
pub const EP1_LANGUAGES: [(u8, u8); 12] = [
    (0x09, 0x00), // English
    (0x0D, 0x01), // German
    (0x18, 0x02), // Swedish and Finnish
    (0x11, 0x03), // Italian
    (0x0B, 0x04), // French
    (0x17, 0x05), // Portuguese and Spanish
    (0x07, 0x06), // Czech and Slovak
    (0x14, 0x08), // Polish
    (0x1C, 0x16), // Turkish
    (0x1E, 0x1D), // Serbian, Croatian and Slovenian
    (0x16, 0x1F), // Rumanian
    (0x0E, 0x37), // Greek
];

// ---------------------------------------------------------------------------
// edit.tf and zxnet addresses (notes-formats.md section 7)
// ---------------------------------------------------------------------------

/// Bits per character code in the packed data.
pub const EDITTF_CODE_BITS: usize = 7;
/// Bits per base64 character.
pub const BASE64_BITS: usize = 6;
/// Characters of the 25-row form: ceil(25 × 40 × 7 / 6).
pub const EDITTF_CHARACTERS_25: usize = 1167;
/// Characters of the older 24-row form: 24 × 40 × 7 / 6.
pub const EDITTF_CHARACTERS_24: usize = 1120;
/// Characters of the zxnet `X25` row: ceil(40 × 7 / 6).
pub const EDITTF_ROW_CHARACTERS: usize = 47;
/// Base64 characters per triplet in the zxnet `X26` field.
pub const EDITTF_TRIPLET_CHARACTERS: usize = 3;
/// Hexadecimal characters per `X270` link: page and sub-code.
pub const EDITTF_LINK_CHARACTERS: usize = 7;
/// The URL-safe base64 alphabet of RFC 4648 section 5.
pub const BASE64URL_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
/// edit.tf metadata values 0 to 4 name the national options English,
/// German, Swedish, Italian and Belgian (French), which are national option
/// indices 0 to 4; 5 to 7 are edit.tf's own sets.
pub const EDITTF_LAST_NATIONAL: u8 = 4;

// ---------------------------------------------------------------------------
// Raw screen dumps (REQ-INP-017)
// ---------------------------------------------------------------------------

/// Bytes of a 25-row dump.
pub const DUMP_BYTES_25: usize = 1000;
/// Bytes of a 24-row dump.
pub const DUMP_BYTES_24: usize = 960;

#[cfg(test)]
mod tests {
    use super::*;
    use neoteletext_core::consts::{COLUMNS, ROWS};

    #[test]
    fn derived_sizes_agree() {
        assert_eq!(DATA_UNIT_TELETEXT_LENGTH, 0x2C);
        assert_eq!(EDITTF_CHARACTERS_25, (ROWS * COLUMNS * EDITTF_CODE_BITS).div_ceil(BASE64_BITS));
        assert_eq!(EDITTF_CHARACTERS_24, (24 * COLUMNS * EDITTF_CODE_BITS) / BASE64_BITS);
        assert_eq!(EDITTF_ROW_CHARACTERS, (COLUMNS * EDITTF_CODE_BITS).div_ceil(BASE64_BITS));
        assert_eq!(DUMP_BYTES_25, ROWS * COLUMNS);
        assert_eq!(SLICED_HEADER_BYTES + 48, SLICED_RECORD_BYTES);
        assert_eq!(TS_M2TS_PACKET_BYTES - TS_PACKET_BYTES, 4);
        assert_eq!(1 + 13 * 3, EP1_PACKET_BYTES);
    }

    /// The layouts of object pages and DRCS pages agree with the core's
    /// constants (clauses 10.5.1.2 and 14.1).
    #[test]
    fn enhancement_page_layouts_agree() {
        use neoteletext_core::consts::{DATA_BYTES, DRCS_PATTERN_DOTS, DRCS_PATTERN_ROWS, DRCS_PTUS};
        assert_eq!(OBJECTS_PER_POINTER_PACKET * 4, OBJECTS_PER_TYPE);
        assert_eq!(DRCS_PTU_BYTES * DRCS_PTUS_PER_PACKET, DATA_BYTES);
        assert_eq!(DRCS_PTUS, DRCS_PTUS_PER_PACKET * usize::from(DRCS_LAST_PACKET));
        assert_eq!(DRCS_PTU_BYTES * DRCS_DOTS_PER_BYTE, DRCS_PATTERN_ROWS * DRCS_PATTERN_DOTS);
        assert_eq!(DRCS_MODE3_ROWS * DRCS_MODE3_COLUMNS * 4, DRCS_PTU_BYTES * DRCS_DOTS_PER_BYTE);
        assert_eq!(NOT_TEXT_PAGE_MARKER >> 7, 0b1_1111_1111);
    }
}
