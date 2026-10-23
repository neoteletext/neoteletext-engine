//! Object pages (clause 10.5.1, Figure 19), the Magazine Organization Table
//! (clause 10.6, Figure 20, Tables 20 to 23), the enhancement-page
//! capacities of Annex E.8 and the object limits of clause 13 and Annex A.8.

use super::coding::{DESIGNATION_CODES, TRIPLETS_PER_PACKET};
use super::packets::PACKET_ROW_25;

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
/// The most packets of enhancement pages over all magazines at Level 2.5,
/// as Annex E.8 gives it; the documented default capacity of REQ-ACQ-019.
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
/// Packets of the Level 3.5 object links (Table 20).
pub const MOT_OBJECT_LINKS_LEVEL_35: [u8; 2] = [22, 23];
/// Packet X/24, the last of the Level 3.5 part of the table, which a Level
/// 3.5 decoder needs present to use that part (clause 10.6.1). It carries
/// DRCS links (Table 22), which are not read (decision E.95).
pub const MOT_LEVEL_35_LAST_PACKET: u8 = 24;
/// Packet byte of the first object link (Table 20).
pub const MOT_OBJECT_LINK_FIRST_BYTE: usize = 6;
/// Bytes of an object link (Table 21).
pub const MOT_OBJECT_LINK_BYTES: usize = 10;
/// Object links in one packet (Table 20).
pub const MOT_OBJECT_LINKS_PER_PACKET: usize = 4;
/// Packets X/1 to X/24 of a Magazine Organization Table (clause 10.6.1).
pub const MOT_PACKETS: usize = 24;
/// Packet byte of the totals of enhancement pages (clause 10.6.6).
pub const MOT_TOTALS_BYTE: usize = 40;
/// Data bit 4 of a link's first byte: the X/27/4,5 flag (Table 21).
pub const MOT_LINK_X27_FLAG: u8 = 0x08;
/// Data bit 4 of a byte pair's first byte: the global object flag (clause
/// 10.6.2).
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

/// The triplets of a page's packets X/26: sixteen packets of thirteen.
pub const LOCAL_TRIPLETS: u16 = (DESIGNATION_CODES * TRIPLETS_PER_PACKET) as u16;
/// An object is at most 507 triplets long (clause 13.1): the whole
/// definition area of an object page, absolute triplets 0 to 506.
pub const OBJECT_BODY_TRIPLETS: u16 = OBJECT_TRIPLET_LAST + 1;
/// At most 127 invocations of each object type on one display page, side
/// panels and default objects included (Annex A.8 item 1).
pub const OBJECT_INVOCATIONS_PER_TYPE: u16 = 127;
/// At most 15 objects open at one time (Annex A.8 item 2).
pub const OBJECTS_OPEN_AT_ONCE: usize = 15;
/// At most 15 objects opened in succession without a displayable character
/// or attribute written in between (Annex A.8 item 3).
pub const OBJECTS_OPENED_IN_SUCCESSION: u8 = 15;
/// The object types: Active, Adaptive and Passive (Table 28).
pub const OBJECT_TYPES: usize = 3;
/// Default objects of a page: two through the GPOP link and two through the
/// POP link of the Magazine Organization Table (clause 13.8).
pub const DEFAULT_OBJECTS: usize = 4;
/// The deepest walk: the local enhancement data, then an Active, an
/// Adaptive and a Passive object, the only nesting clauses 13.3 to 13.5
/// allow (decision E.51).
pub const WALK_DEPTH: usize = 1 + OBJECT_TYPES;
/// The pointer triplets of one object group of a pointer packet, one for
/// each type, so that the pointer triplet is 1 + 3 × group + type
/// (clause 10.5.1.2, Table 28).
pub const OBJECT_POINTER_GROUP_TRIPLETS: u8 = OBJECT_TYPES as u8;
