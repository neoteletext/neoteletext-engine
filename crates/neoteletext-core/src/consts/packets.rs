//! Packet numbers and magazines (clause 7.1.2, Table 1), and the fields of
//! the broadcast service data packet (clause 9.8.1).

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
