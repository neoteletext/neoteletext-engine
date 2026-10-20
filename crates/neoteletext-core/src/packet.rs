//! Packet decoding (EN 300 706 clause 9, area PKT; layer L1).
//!
//! A [`Packet`] is the internal form of REQ-ARCH-002: the 42 bytes that
//! follow the framing code, in transmission order with the first transmitted
//! bit as bit 0, parity and Hamming bits retained. [`decode`] turns one packet
//! into typed fields and knows nothing about pages, time or characters.
//!
//! The packets of Levels 2.5 and 3.5 are decoded here too: the typed triplet
//! functions of Tables 28 and 29 ([`TripletFunction`], REQ-PKT-010), the
//! compositional links of X/27/4 and X/27/5 ([`CompositionalLink`]), the
//! X/28 and M/29 packets ([`PagePacket`], [`MagazinePacket`]), and, from the
//! stored bytes of enhancement pages, object page packets ([`ObjectPacket`])
//! and the Magazine Organization Table ([`mot_position`] and the `mot_*`
//! functions).

use crate::consts::{
    ADDRESS_BYTE_0, ADDRESS_BYTE_1, COMPOSITIONAL_LINKS, DATA_BYTES, DATA_START, DESIGNATION_CODES,
    DESIGNATION_DOWNLOAD_FUNCTION, DESIGNATION_LEVEL_35_DATA, DESIGNATION_LINKS_LEVEL_25, DESIGNATION_LINKS_LEVEL_35,
    DESIGNATION_PAGE_DATA, HEADER_DISPLAY_BYTES, HEADER_DISPLAY_START, LINK_NULL_PAGE, LINK_NULL_SUBCODE,
    MAGAZINE_MASK, PACKET_BYTES, PACKET_ENHANCEMENT, PACKET_FIRST_ROW, PACKET_FUNCTION_MASK, PACKET_HEADER,
    PACKET_LINKS, PACKET_MAGAZINE_ENHANCEMENT, PACKET_NUMBER_LOW_BIT, PACKET_PAGE_ENHANCEMENT, PACKET_ROW_25,
    PACKET_SERVICE, SERVICE_MAGAZINE, STATUS_MESSAGE_BYTES, TRIPLET_ADDRESS_BITS, TRIPLET_ADDRESS_MASK, TRIPLET_BYTES,
    TRIPLET_DATA_FIELD_MASK, TRIPLET_MODE_BITS, TRIPLET_MODE_MASK, TRIPLETS_PER_PACKET,
};
use crate::ecc::{hamming84, hamming2418, reverse_bits};
use crate::types::{ControlBits, Magazine, PageNumber, PresentationFields, SubCode};

mod function;
mod links;
mod objects;
mod page_data;

pub use function::{
    ColourData, DisplayAttributes, FontStyleData, ObjectKind, ObjectNumber, ObjectSource, ObjectUsage, TripletFunction,
};
pub use links::CompositionalLink;
pub use objects::{
    DefaultObject, MotObjectLink, MotPageEntry, MotPosition, MotTotals, ObjectPacket, TripletLocation, mot_fallback,
    mot_object_link, mot_page_entry, mot_position, mot_totals, triplet_location,
};
pub use page_data::{
    PageEnhancement, PageFormat, TripletValues, page_enhancement, page_function, presentation_fields, stream_bits,
};

/// One packet in the internal form (REQ-ARCH-002).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Packet {
    /// Packet bytes 4 to 45.
    pub bytes: [u8; PACKET_BYTES],
}

impl Packet {
    /// The 40 data bytes, packet bytes 6 to 45.
    #[must_use]
    pub fn data(&self) -> [u8; DATA_BYTES] {
        let mut data = [0; DATA_BYTES];
        if let Some(source) = self.bytes.get(DATA_START..) {
            data.copy_from_slice(source);
        }
        data
    }

    /// Byte `index` of the internal form, or 0 when out of range.
    #[must_use]
    pub fn byte(&self, index: usize) -> u8 {
        self.bytes.get(index).copied().unwrap_or(0)
    }
}

/// Where a packet came from, for statistics and diagnostics (REQ-ARCH-003).
/// Unknown values are `None`, never zero.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PacketMeta {
    /// Television line number.
    pub line: Option<u16>,
    /// Field: `Some(false)` first, `Some(true)` second.
    pub second_field: Option<bool>,
    /// Time stamp in microseconds.
    pub timestamp_us: Option<u64>,
    /// Caller-assigned input source number.
    pub source: Option<u8>,
    /// Slicer quality, when the packet came from raw samples.
    pub quality: Option<SliceQuality>,
}

/// Quality measures of a sliced line (REQ-SLC-012).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SliceQuality {
    /// Data amplitude as a percentage of nominal.
    pub amplitude_percent: u16,
    /// Run-in match quality, 0 to 100.
    pub match_percent: u8,
    /// Position of the run-in in samples.
    pub position: u32,
    /// True when the framing code needed a correction (decision E.24).
    pub framing_corrected: bool,
}

/// Why a packet was not used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PacketError {
    /// A packet address byte was uncorrectable (decision E.7).
    BadAddress,
    /// The designation code of an enhancement or link packet was
    /// uncorrectable (decision E.9).
    BadDesignation,
}

/// The magazine and packet number of a packet.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PacketAddress {
    /// The magazine.
    pub magazine: Magazine,
    /// The packet number, 0 to 31.
    pub number: u8,
}

/// Decodes the packet address bytes (REQ-PKT-001).
pub fn decode_address(packet: &Packet) -> Result<PacketAddress, PacketError> {
    let first = hamming84(packet.byte(ADDRESS_BYTE_0)).ok_or(PacketError::BadAddress)?;
    let second = hamming84(packet.byte(ADDRESS_BYTE_1)).ok_or(PacketError::BadAddress)?;
    let number = ((first & PACKET_NUMBER_LOW_BIT) >> 3) | (second << 1);
    Ok(PacketAddress { magazine: Magazine::from_bits(first & MAGAZINE_MASK), number })
}

/// A decoded page header (REQ-PKT-002).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Header {
    /// The magazine.
    pub magazine: Magazine,
    /// The page number, `None` when either digit was uncorrectable.
    pub page: Option<PageNumber>,
    /// The sub-code, `None` when any of its bytes was uncorrectable.
    pub subcode: Option<SubCode>,
    /// Control bits C4 to C14, `None` when any carrying byte was
    /// uncorrectable.
    pub control: Option<ControlBits>,
    /// Packet bytes 14 to 45, raw with parity.
    pub display: [u8; HEADER_DISPLAY_BYTES],
}

impl Header {
    /// True when page number, sub-code and control bits are all valid, so
    /// the header may start a page (decision E.7).
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.page.is_some() && self.subcode.is_some() && self.control.is_some()
    }

    /// True when the header is in serial mode (C11), if known.
    #[must_use]
    pub fn serial(&self) -> Option<bool> {
        self.control.map(|control| control.has(crate::consts::C11_SERIAL))
    }
}

/// A display row packet X/1 to X/25 (REQ-PKT-003).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Row {
    /// The magazine.
    pub magazine: Magazine,
    /// The row, 1 to 25.
    pub row: u8,
    /// Packet bytes 6 to 45, raw with parity.
    pub bytes: [u8; DATA_BYTES],
}

/// An enhancement triplet (clause 12.3.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Triplet {
    /// The 6-bit address: 0 to 39 a column, 40 to 63 a row.
    pub address: u8,
    /// The 5-bit mode.
    pub mode: u8,
    /// The 7-bit data.
    pub data: u8,
}

impl Triplet {
    /// Splits an 18-bit triplet value into its fields (clause 12.3.1).
    #[must_use]
    pub const fn from_value(value: u32) -> Self {
        Self {
            address: (value & TRIPLET_ADDRESS_MASK) as u8,
            mode: ((value >> TRIPLET_ADDRESS_BITS) & TRIPLET_MODE_MASK) as u8,
            data: ((value >> (TRIPLET_ADDRESS_BITS + TRIPLET_MODE_BITS)) & TRIPLET_DATA_FIELD_MASK) as u8,
        }
    }

    /// The 18-bit value of the fields.
    #[must_use]
    pub const fn value(self) -> u32 {
        (self.address as u32 & TRIPLET_ADDRESS_MASK)
            | ((self.mode as u32 & TRIPLET_MODE_MASK) << TRIPLET_ADDRESS_BITS)
            | ((self.data as u32 & TRIPLET_DATA_FIELD_MASK) << (TRIPLET_ADDRESS_BITS + TRIPLET_MODE_BITS))
    }

    /// True for a row address triplet (address 40 to 63).
    #[must_use]
    pub const fn is_row_address(self) -> bool {
        self.address >= crate::consts::ROW_ADDRESS_BASE
    }
}

/// Thirteen triplets, each `None` when its Hamming 24/18 check failed.
pub type Triplets = [Option<Triplet>; TRIPLETS_PER_PACKET];

/// An enhancement packet X/26 (REQ-PKT-004).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Enhancement {
    /// The magazine.
    pub magazine: Magazine,
    /// The designation code, 0 to 15.
    pub designation: u8,
    /// The triplets.
    pub triplets: Triplets,
}

/// One editorial link of packet X/27/0 (clause 9.6.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Link {
    /// The linked magazine, after applying the relative bits M1 to M3.
    pub magazine: Magazine,
    /// The linked page number.
    pub page: PageNumber,
    /// The linked sub-code; 3F7F means any sub-page.
    pub subcode: SubCode,
}

/// Packet X/27/0: six links, the link control byte and the page checksum
/// (REQ-PKT-005).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Links {
    /// The magazine.
    pub magazine: Magazine,
    /// Links 0 to 5. `None` for a null link or an uncorrectable byte
    /// (decision E.7).
    pub links: [Option<Link>; 6],
    /// Data bit 4 of the link control byte: display row 24 (Table 14).
    pub display_row_24: bool,
    /// The transmitted checksum, bytes 44 and 45 as received.
    pub checksum_bytes: [u8; 2],
}

/// A page-related enhancement packet X/28 (clauses 9.4.2 to 9.4.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PagePacket {
    /// X/28/0, read as format 1 (Table 4; decision E.44).
    Format1(PageEnhancement),
    /// X/28/3, of which only the page function and coding are read (Table
    /// 9, decision E.95).
    PageFunction(Option<PageFormat>),
    /// X/28/4, Level 3.5 page data (Table 10).
    Level35(PageEnhancement),
}

/// A magazine-related enhancement packet M/29 with packet function 0000000
/// where one is defined (clauses 9.5.1 to 9.5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MagazinePacket {
    /// M/29/0 (Table 11).
    Data(PresentationFields),
    /// M/29/4, Level 3.5 magazine data (Table 13).
    Level35(PresentationFields),
}

/// Broadcast service data, packet 8/30 (REQ-PKT/SVC-001).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServicePacket {
    /// The designation code, 0 to 3.
    pub designation: u8,
    /// The initial page magazine, page and sub-code, `None` if uncorrectable.
    pub initial_page: Option<(Magazine, PageNumber, SubCode)>,
    /// Format 1 network identification, most significant bit first.
    pub network_id: u16,
    /// Format 1 time offset byte, raw.
    pub time_offset: u8,
    /// Format 1 Modified Julian Date and UTC bytes 16 to 21, raw.
    pub date_time: [u8; 6],
    /// Format 2 Programme Delivery Control bytes 13 to 25, raw (Part G).
    pub pdc: [u8; 13],
    /// Status message bytes 26 to 45, raw with parity.
    pub status: [u8; STATUS_MESSAGE_BYTES],
}

/// A decoded packet (layer L1 output).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Decoded {
    /// Packet X/0.
    Header(Header),
    /// Packets X/1 to X/25.
    Row(Row),
    /// Packet X/26.
    Enhancement(Enhancement),
    /// Packet X/27/0.
    Links(Links),
    /// Packets X/27/1 to X/27/3, stored without interpretation.
    LinksRaw {
        /// The magazine.
        magazine: Magazine,
        /// The designation code 1 to 3.
        designation: u8,
        /// Packet bytes 6 to 45.
        bytes: [u8; DATA_BYTES],
    },
    /// Packets X/27/4 and X/27/5: six compositional links each (REQ-PKT-011).
    CompositionalLinks {
        /// The magazine.
        magazine: Magazine,
        /// The designation code, 4 or 5.
        designation: u8,
        /// The links, each `None` when either of its triplets was
        /// uncorrectable.
        links: [Option<CompositionalLink>; COMPOSITIONAL_LINKS],
    },
    /// Packets X/28/0, X/28/1, X/28/3 and X/28/4.
    PageData {
        /// The magazine.
        magazine: Magazine,
        /// The decoded packet.
        packet: PagePacket,
    },
    /// Packets M/29/0 and M/29/4 with packet function 0, and M/29/1.
    Magazine {
        /// The magazine.
        magazine: Magazine,
        /// The decoded packet.
        packet: MagazinePacket,
    },
    /// Packet 8/30.
    Service(ServicePacket),
    /// Anything counted and ignored: X/27/6 and above, other X/28 and M/29
    /// designations, M/29/0 and M/29/4 whose packet function is not 0 or
    /// whose triplet 1 was uncorrectable, packets 30 outside magazine 8,
    /// packets 31.
    Ignored(PacketAddress),
}

/// Decodes one packet (area PKT).
pub fn decode(packet: &Packet) -> Result<Decoded, PacketError> {
    let address = decode_address(packet)?;
    let magazine = address.magazine;
    let decoded = match address.number {
        PACKET_HEADER => Decoded::Header(decode_header(packet, magazine)),
        PACKET_FIRST_ROW..=PACKET_ROW_25 => Decoded::Row(Row { magazine, row: address.number, bytes: packet.data() }),
        PACKET_ENHANCEMENT => {
            let designation = designation_code(packet)?;
            Decoded::Enhancement(Enhancement { magazine, designation, triplets: triplets(packet) })
        }
        PACKET_LINKS => match designation_code(packet)? {
            0 => Decoded::Links(decode_links(packet, magazine)),
            // Bytes 43 to 45 are reserved here (clause 9.6.1); stored
            // uninterpreted, so they are ignored (REQ-PKT-009).
            designation @ 1..=3 => Decoded::LinksRaw { magazine, designation, bytes: packet.data() },
            designation @ (DESIGNATION_LINKS_LEVEL_25 | DESIGNATION_LINKS_LEVEL_35) => {
                Decoded::CompositionalLinks { magazine, designation, links: compositional_links(packet, magazine) }
            }
            // Designation codes 6 to 15 are ignored (REQ-PKT-005).
            _ => Decoded::Ignored(address),
        },
        PACKET_PAGE_ENHANCEMENT => {
            let designation = designation_code(packet)?;
            let values = triplet_values(packet);
            let packet = match designation {
                DESIGNATION_PAGE_DATA => PagePacket::Format1(page_enhancement(&values)),
                DESIGNATION_DOWNLOAD_FUNCTION => PagePacket::PageFunction(page_function(&values)),
                DESIGNATION_LEVEL_35_DATA => PagePacket::Level35(page_enhancement(&values)),
                _ => return Ok(Decoded::Ignored(address)),
            };
            Decoded::PageData { magazine, packet }
        }
        PACKET_MAGAZINE_ENHANCEMENT => {
            let designation = designation_code(packet)?;
            let values = triplet_values(packet);
            // Tables 11 and 13: packet function 0000000 is the only defined
            // one; reserved values are ignored (REQ-PKT-016). An
            // uncorrectable triplet 1 leaves the function unknown, so the
            // packet is ignored too.
            let function_zero =
                values.first().copied().flatten().is_some_and(|first| first & PACKET_FUNCTION_MASK == 0);
            let packet = match designation {
                DESIGNATION_PAGE_DATA if function_zero => MagazinePacket::Data(presentation_fields(&values)),
                DESIGNATION_LEVEL_35_DATA if function_zero => MagazinePacket::Level35(presentation_fields(&values)),
                _ => return Ok(Decoded::Ignored(address)),
            };
            Decoded::Magazine { magazine, packet }
        }
        PACKET_SERVICE if magazine.number() == SERVICE_MAGAZINE => {
            let designation = designation_code(packet)?;
            Decoded::Service(decode_service(packet, designation))
        }
        // Packets 30 outside magazine 8 and packets 31 (REQ-PKT-008).
        _ => Decoded::Ignored(address),
    };
    Ok(decoded)
}

/// Byte 6, the designation code, Hamming 8/4.
fn designation_code(packet: &Packet) -> Result<u8, PacketError> {
    let value = hamming84(packet.byte(DATA_START)).ok_or(PacketError::BadDesignation)?;
    if usize::from(value) < DESIGNATION_CODES { Ok(value) } else { Err(PacketError::BadDesignation) }
}

/// Hamming 8/4 value of internal byte `index`.
fn nibble(packet: &Packet, index: usize) -> Option<u8> {
    hamming84(packet.byte(index))
}

/// Indices of the header's Hamming bytes, packet bytes 6 to 13.
const HEADER_UNITS: usize = 2;
const HEADER_TENS: usize = 3;
const HEADER_S1: usize = 4;
const HEADER_S2_C4: usize = 5;
const HEADER_S3: usize = 6;
const HEADER_S4_C5_C6: usize = 7;
const HEADER_C7_C10: usize = 8;
const HEADER_C11_C14: usize = 9;

/// Decodes a page header (clause 9.3.1, REQ-PKT-002).
fn decode_header(packet: &Packet, magazine: Magazine) -> Header {
    let units = nibble(packet, HEADER_UNITS);
    let tens = nibble(packet, HEADER_TENS);
    let page = match (units, tens) {
        (Some(units), Some(tens)) => Some(PageNumber((tens << 4) | units)),
        _ => None,
    };
    let s1 = nibble(packet, HEADER_S1);
    let s2c4 = nibble(packet, HEADER_S2_C4);
    let s3 = nibble(packet, HEADER_S3);
    let s4c5c6 = nibble(packet, HEADER_S4_C5_C6);
    let c7c10 = nibble(packet, HEADER_C7_C10);
    let c11c14 = nibble(packet, HEADER_C11_C14);
    let subcode = match (s1, s2c4, s3, s4c5c6) {
        (Some(s1), Some(s2), Some(s3), Some(s4)) => Some(SubCode::new(
            u16::from(s1) | (u16::from(s2 & 0x7) << 4) | (u16::from(s3) << 8) | (u16::from(s4 & 0x3) << 12),
        )),
        _ => None,
    };
    let control = match (s2c4, s4c5c6, c7c10, c11c14) {
        (Some(s2c4), Some(s4c5c6), Some(c7c10), Some(c11c14)) => {
            // Clause 9.3.1.3: C4 is data bit 4 of byte 9; C5 and C6 bits 3
            // and 4 of byte 11; C7 to C10 byte 12; C11 to C14 byte 13. Bit n
            // of the control word is Cn.
            Some(ControlBits(
                (u16::from(s2c4 >> 3) << 4)
                    | (u16::from(s4c5c6 >> 2) << 5)
                    | (u16::from(c7c10) << 7)
                    | (u16::from(c11c14) << 11),
            ))
        }
        _ => None,
    };
    let mut display = [0; HEADER_DISPLAY_BYTES];
    if let Some(source) = packet.bytes.get(HEADER_DISPLAY_START..) {
        display.copy_from_slice(source);
    }
    Header { magazine, page, subcode, control, display }
}

/// The thirteen triplet values of bytes that follow byte 6.
fn values_of(body: &[u8]) -> TripletValues {
    let mut result = [None; TRIPLETS_PER_PACKET];
    let (chunks, _) = body.as_chunks::<TRIPLET_BYTES>();
    for (slot, chunk) in result.iter_mut().zip(chunks) {
        *slot = hamming2418(*chunk);
    }
    result
}

/// The thirteen triplets of bytes that follow byte 6.
fn triplets_of(body: &[u8]) -> Triplets {
    values_of(body).map(|value| value.map(Triplet::from_value))
}

/// Packet bytes 7 to 45.
fn triplet_body(packet: &Packet) -> &[u8] {
    packet.bytes.get(DATA_START.saturating_add(1)..).unwrap_or(&[])
}

/// The thirteen triplets of packet bytes 7 to 45.
fn triplets(packet: &Packet) -> Triplets {
    triplets_of(triplet_body(packet))
}

/// The thirteen triplet values of packet bytes 7 to 45.
fn triplet_values(packet: &Packet) -> TripletValues {
    values_of(triplet_body(packet))
}

/// The six links of X/27/4 or X/27/5: link k in triplets 2k + 1 and
/// 2k + 2 (Figure 14).
fn compositional_links(packet: &Packet, magazine: Magazine) -> [Option<CompositionalLink>; COMPOSITIONAL_LINKS] {
    let values = triplet_values(packet);
    let mut links = [None; COMPOSITIONAL_LINKS];
    for (link, pair) in links.iter_mut().zip(values.as_chunks::<2>().0) {
        if let [Some(first), Some(second)] = *pair {
            *link = Some(CompositionalLink::from_values(magazine, first, second));
        }
    }
    links
}

/// Decodes a 6-byte page reference: units, tens, S1, S2 with M1, S3, S4 with
/// M2 and M3 (Figure 12). Returns page, sub-code and the three magazine bits.
fn page_reference(packet: &Packet, start: usize) -> Option<(PageNumber, SubCode, u8)> {
    let mut values = [0u8; 6];
    for (offset, value) in values.iter_mut().enumerate() {
        *value = nibble(packet, start.saturating_add(offset))?;
    }
    let [units, tens, s1, s2m1, s3, s4m23] = values;
    let page = PageNumber((tens << 4) | units);
    let subcode = SubCode::new(
        u16::from(s1) | (u16::from(s2m1 & 0x7) << 4) | (u16::from(s3) << 8) | (u16::from(s4m23 & 0x3) << 12),
    );
    // M1 is data bit 4 of the S2 byte; M2 and M3 are data bits 3 and 4 of
    // the S4 byte (Figure 12).
    let magazine_bits = (s2m1 >> 3) | ((s4m23 >> 1) & 0x6);
    Some((page, subcode, magazine_bits & MAGAZINE_MASK))
}

/// Index of link 0 (packet byte 7) and the bytes per link.
const LINK_START: usize = 3;
const LINK_BYTES: usize = 6;
/// Index of the link control byte (packet byte 43).
const LINK_CONTROL: usize = 39;
/// Data bit 4 of the link control byte: display row 24 (Table 14). Only
/// this bit is read; the reserved bits beside it are ignored (REQ-PKT-009).
const LINK_CONTROL_ROW_24: u8 = 0x8;
/// Index of the first checksum byte (packet byte 44).
const LINK_CHECKSUM: usize = 40;

/// Decodes packet X/27/0 (REQ-PKT-005).
fn decode_links(packet: &Packet, magazine: Magazine) -> Links {
    let mut links = [None; 6];
    for (index, slot) in links.iter_mut().enumerate() {
        let start = LINK_START.saturating_add(index.saturating_mul(LINK_BYTES));
        *slot = page_reference(packet, start).and_then(|(page, subcode, relative)| {
            if page.0 == LINK_NULL_PAGE && subcode.value() == LINK_NULL_SUBCODE {
                return None;
            }
            // Clause 9.6.1: set bits complement the packet's magazine bits.
            let linked = Magazine::from_bits(magazine.bits() ^ relative);
            Some(Link { magazine: linked, page, subcode })
        });
    }
    let display_row_24 = nibble(packet, LINK_CONTROL).is_some_and(|control| control & LINK_CONTROL_ROW_24 != 0);
    let checksum_bytes = [packet.byte(LINK_CHECKSUM), packet.byte(LINK_CHECKSUM.saturating_add(1))];
    Links { magazine, links, display_row_24, checksum_bytes }
}

/// Indices within 8/30 (clause 9.8.1, Table 18): initial page bytes 7 to
/// 12, network identification 13 and 14, time offset 15, date and time 16 to
/// 21, Programme Delivery Control 13 to 25, status 26 to 45.
const SERVICE_INITIAL_PAGE: usize = 3;
const SERVICE_NETWORK_ID: usize = 9;
const SERVICE_TIME_OFFSET: usize = 11;
const SERVICE_DATE_TIME: usize = 12;
const SERVICE_PDC: usize = 9;
const SERVICE_STATUS: usize = 22;

fn copy_bytes<const N: usize>(packet: &Packet, start: usize) -> [u8; N] {
    let mut out = [0; N];
    if let Some(source) = packet.bytes.get(start..start.saturating_add(N)) {
        out.copy_from_slice(source);
    }
    out
}

/// Decodes packet 8/30 (REQ-SVC-001 and REQ-SVC-003).
fn decode_service(packet: &Packet, designation: u8) -> ServicePacket {
    // The initial page magazine is absolute here (Table 18), not relative.
    let initial_page = page_reference(packet, SERVICE_INITIAL_PAGE)
        .map(|(page, subcode, bits)| (Magazine::from_bits(bits), page, subcode));
    // The network identification is sent most significant bit first, so each
    // byte is reversed to read it (Table 18).
    let network_id = (u16::from(reverse_bits(packet.byte(SERVICE_NETWORK_ID))) << 8)
        | u16::from(reverse_bits(packet.byte(SERVICE_NETWORK_ID.saturating_add(1))));
    ServicePacket {
        designation,
        initial_page,
        network_id,
        time_offset: packet.byte(SERVICE_TIME_OFFSET),
        date_time: copy_bytes(packet, SERVICE_DATE_TIME),
        pdc: copy_bytes(packet, SERVICE_PDC),
        status: copy_bytes(packet, SERVICE_STATUS),
    }
}

/// Builders used by the page encoder and by tests: the inverse of
/// [`decode`].
pub mod encode {
    use super::page_data::encode::presentation_values;
    use super::{
        CompositionalLink, HEADER_C7_C10, HEADER_C11_C14, HEADER_S1, HEADER_S2_C4, HEADER_S3, HEADER_S4_C5_C6,
        HEADER_TENS, HEADER_UNITS, LINK_BYTES, LINK_CHECKSUM, LINK_CONTROL, LINK_CONTROL_ROW_24, LINK_START, Packet,
        PageFormat, Triplet,
    };
    use crate::consts::{
        ADDRESS_BYTE_0, ADDRESS_BYTE_1, COMPOSITIONAL_LINKS, DATA_BYTES, DATA_START, DESIGNATION_DOWNLOAD_FUNCTION,
        HEADER_DISPLAY_BYTES, HEADER_DISPLAY_START, MAGAZINE_MASK, NIBBLE_MASK, OBJECT_POINTER_BITS,
        OBJECT_POINTER_UNUSED, PACKET_BYTES, PACKET_MAGAZINE_ENHANCEMENT, PACKET_PAGE_ENHANCEMENT, TRIPLET_BYTES,
        TRIPLETS_PER_PACKET,
    };
    use crate::ecc::{hamming84_encode, hamming2418_encode};
    use crate::types::{ControlBits, Magazine, PageNumber, PresentationFields, SubCode};

    pub use super::objects::encode::{mot_object_link, mot_page_entry, mot_totals};

    fn set(packet: &mut Packet, index: usize, value: u8) {
        if let Some(byte) = packet.bytes.get_mut(index) {
            *byte = value;
        }
    }

    /// A packet with the given address and all data bytes zero.
    #[must_use]
    pub fn address(magazine: Magazine, number: u8) -> Packet {
        let mut packet = Packet { bytes: [0; PACKET_BYTES] };
        let first = (magazine.bits() & MAGAZINE_MASK) | ((number & 1) << 3);
        set(&mut packet, ADDRESS_BYTE_0, hamming84_encode(first));
        set(&mut packet, ADDRESS_BYTE_1, hamming84_encode((number >> 1) & NIBBLE_MASK));
        packet
    }

    /// A page header. `display` is 32 bytes already carrying parity.
    #[must_use]
    pub fn header(
        magazine: Magazine,
        page: PageNumber,
        subcode: SubCode,
        control: ControlBits,
        display: &[u8; HEADER_DISPLAY_BYTES],
    ) -> Packet {
        let mut packet = address(magazine, 0);
        let sub = subcode.value();
        let c = control.0;
        let nibbles = [
            (HEADER_UNITS, page.units()),
            (HEADER_TENS, page.tens()),
            (HEADER_S1, (sub & 0xF) as u8),
            (HEADER_S2_C4, (((sub >> 4) & 0x7) as u8) | ((((c >> 4) & 1) as u8) << 3)),
            (HEADER_S3, ((sub >> 8) & 0xF) as u8),
            (HEADER_S4_C5_C6, (((sub >> 12) & 0x3) as u8) | ((((c >> 5) & 0x3) as u8) << 2)),
            (HEADER_C7_C10, ((c >> 7) & 0xF) as u8),
            (HEADER_C11_C14, ((c >> 11) & 0xF) as u8),
        ];
        for (index, value) in nibbles {
            set(&mut packet, index, hamming84_encode(value));
        }
        if let Some(target) = packet.bytes.get_mut(HEADER_DISPLAY_START..) {
            target.copy_from_slice(display);
        }
        packet
    }

    /// A display row. `bytes` are 40 bytes already carrying parity.
    #[must_use]
    pub fn row(magazine: Magazine, row: u8, bytes: &[u8; DATA_BYTES]) -> Packet {
        let mut packet = address(magazine, row);
        if let Some(target) = packet.bytes.get_mut(DATA_START..) {
            target.copy_from_slice(bytes);
        }
        packet
    }

    /// A packet carrying a designation code and thirteen triplets; `None`
    /// triplets are written as the Termination Marker's neighbour, an empty
    /// value 0.
    #[must_use]
    pub fn triplet_packet(
        magazine: Magazine,
        number: u8,
        designation: u8,
        triplets: &[Triplet; TRIPLETS_PER_PACKET],
    ) -> Packet {
        let mut packet = address(magazine, number);
        set(&mut packet, DATA_START, hamming84_encode(designation & NIBBLE_MASK));
        for (index, triplet) in triplets.iter().enumerate() {
            let start = DATA_START.saturating_add(1).saturating_add(index.saturating_mul(TRIPLET_BYTES));
            let [a, b, c] = hamming2418_encode(triplet.value());
            set(&mut packet, start, a);
            set(&mut packet, start.saturating_add(1), b);
            set(&mut packet, start.saturating_add(2), c);
        }
        packet
    }

    /// Writes a 6-byte page reference with three magazine bits.
    fn page_reference(packet: &mut Packet, start: usize, page: PageNumber, subcode: SubCode, magazine_bits: u8) {
        let sub = subcode.value();
        let values = [
            page.units(),
            page.tens(),
            (sub & 0xF) as u8,
            (((sub >> 4) & 0x7) as u8) | ((magazine_bits & 1) << 3),
            ((sub >> 8) & 0xF) as u8,
            (((sub >> 12) & 0x3) as u8) | ((magazine_bits & 0x6) << 1),
        ];
        for (offset, value) in values.iter().enumerate() {
            set(packet, start.saturating_add(offset), hamming84_encode(*value));
        }
    }

    /// Packet X/27/0. Each link is `(magazine, page, subcode)` or `None` for
    /// a null link.
    #[must_use]
    pub fn links(
        magazine: Magazine,
        links: &[Option<(Magazine, PageNumber, SubCode)>; 6],
        display_row_24: bool,
        checksum: u16,
    ) -> Packet {
        let mut packet = address(magazine, crate::consts::PACKET_LINKS);
        set(&mut packet, DATA_START, hamming84_encode(0));
        for (index, link) in links.iter().enumerate() {
            let start = LINK_START.saturating_add(index.saturating_mul(LINK_BYTES));
            match link {
                Some((target, page, subcode)) => {
                    let relative = magazine.bits() ^ target.bits();
                    page_reference(&mut packet, start, *page, *subcode, relative);
                }
                None => page_reference(
                    &mut packet,
                    start,
                    PageNumber(crate::consts::LINK_NULL_PAGE),
                    SubCode::new(crate::consts::LINK_NULL_SUBCODE),
                    0,
                ),
            }
        }
        // Table 14: bits 1 to 3 should be 1 without a local code of practice.
        let control = 0x7 | if display_row_24 { LINK_CONTROL_ROW_24 } else { 0 };
        set(&mut packet, LINK_CONTROL, hamming84_encode(control));
        let [high, low] = checksum.to_be_bytes();
        set(&mut packet, LINK_CHECKSUM, high);
        set(&mut packet, LINK_CHECKSUM.saturating_add(1), low);
        packet
    }

    /// Triplets 1 and 2 of X/28/0 format 1 or M/29/0 for a basic Level 1
    /// page: page function 0, page coding 0 (Table 4).
    #[must_use]
    pub fn designation_triplets(default_designation: u8, second_designation: u8) -> [Triplet; 2] {
        let first = (u32::from(default_designation & 0x7F) << 7) | (u32::from(second_designation & 0xF) << 14);
        let second = u32::from((second_designation >> 4) & 0x7);
        [Triplet::from_value(first), Triplet::from_value(second)]
    }

    /// A packet carrying a designation code and thirteen 18-bit triplet
    /// values.
    fn value_packet(magazine: Magazine, number: u8, designation: u8, values: &[u32; TRIPLETS_PER_PACKET]) -> Packet {
        triplet_packet(magazine, number, designation, &values.map(Triplet::from_value))
    }

    /// Packet X/27/4 or X/27/5 (Table 15); triplet 13 is reserved and 0.
    #[must_use]
    pub fn compositional_links(
        magazine: Magazine,
        designation: u8,
        links: &[CompositionalLink; COMPOSITIONAL_LINKS],
    ) -> Packet {
        let mut values = [0u32; TRIPLETS_PER_PACKET];
        for (pair, link) in values.as_chunks_mut::<2>().0.iter_mut().zip(links) {
            let (first, second) = link.to_values(magazine);
            *pair = [first, second];
        }
        value_packet(magazine, crate::consts::PACKET_LINKS, designation, &values)
    }

    /// Packet X/28/0 format 1 (designation 0) or X/28/4 (designation 4)
    /// (Tables 4 and 10). Absent field groups are written as 0.
    #[must_use]
    pub fn page_enhancement(
        magazine: Magazine,
        designation: u8,
        format: PageFormat,
        fields: &PresentationFields,
    ) -> Packet {
        value_packet(magazine, PACKET_PAGE_ENHANCEMENT, designation, &presentation_values(format.bits(), fields))
    }

    /// Packet M/29/0 (designation 0) or M/29/4 (designation 4) with packet
    /// function 0000000 (Tables 11 and 13).
    #[must_use]
    pub fn magazine_enhancement(magazine: Magazine, designation: u8, fields: &PresentationFields) -> Packet {
        value_packet(magazine, PACKET_MAGAZINE_ENHANCEMENT, designation, &presentation_values(0, fields))
    }

    /// Packet X/28/3 with the page function and coding only (Table 9), as
    /// a pattern download page sends it.
    #[must_use]
    pub fn page_function(magazine: Magazine, format: PageFormat) -> Packet {
        let mut values = [0; TRIPLETS_PER_PACKET];
        if let Some(first) = values.first_mut() {
            *first = format.bits();
        }
        value_packet(magazine, PACKET_PAGE_ENHANCEMENT, DESIGNATION_DOWNLOAD_FUNCTION, &values)
    }

    /// A pointer triplet of an object page: `even` in bits 1 to 9 and `odd`
    /// in bits 10 to 18 (clause 10.5.1.2).
    #[must_use]
    pub const fn pointer_triplet(even: u16, odd: u16) -> Triplet {
        let mask = OBJECT_POINTER_UNUSED as u32;
        Triplet::from_value((even as u32 & mask) | (odd as u32 & mask) << OBJECT_POINTER_BITS)
    }

    /// A packet whose forty data bytes are Hamming 8/4 coded, such as a
    /// packet of the Magazine Organization Table (clause 10.6.1). `values`
    /// are the 4-bit values before coding.
    #[must_use]
    pub fn nibble_packet(magazine: Magazine, number: u8, values: &[u8; DATA_BYTES]) -> Packet {
        let mut packet = address(magazine, number);
        for (index, &value) in values.iter().enumerate() {
            set(&mut packet, DATA_START.saturating_add(index), hamming84_encode(value & NIBBLE_MASK));
        }
        packet
    }

    /// The 40 raw display bytes of a packet, for building raw packets.
    #[must_use]
    pub fn raw(magazine: Magazine, number: u8, designation: u8, body: &[u8; DATA_BYTES - 1]) -> Packet {
        let mut packet = address(magazine, number);
        set(&mut packet, DATA_START, hamming84_encode(designation & NIBBLE_MASK));
        if let Some(target) = packet.bytes.get_mut(DATA_START.saturating_add(1)..) {
            target.copy_from_slice(body);
        }
        packet
    }
}

#[cfg(test)]
// REQ-TST-010: headers, links and enhancement packets built by the encoder,
// every field checked; real captures are decoded by tests/corpus.rs and the
// service data packet by service.rs.
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::consts::{C4_ERASE, C8_UPDATE, C11_SERIAL, C12, C14, OBJECT_POINTER_DATA_BIT, OBJECT_POINTER_UNUSED};
    use crate::ecc::{hamming2418_encode, with_odd_parity};
    use crate::types::{DefaultColourFields, Designations, Rgb12, SidePanelFields};

    fn mag(number: u8) -> Magazine {
        Magazine::new(number).unwrap()
    }

    #[test]
    fn addresses_for_every_magazine_and_packet() {
        for magazine in 1..=8 {
            for number in 0..32 {
                let packet = encode::address(mag(magazine), number);
                let address = decode_address(&packet).unwrap();
                assert_eq!(address.magazine.number(), magazine);
                assert_eq!(address.number, number);
            }
        }
    }

    #[test]
    fn uncorrectable_address_is_rejected() {
        let mut packet = encode::address(mag(1), 1);
        packet.bytes[0] ^= 0b11;
        assert_eq!(decode(&packet), Err(PacketError::BadAddress));
    }

    #[test]
    fn header_fields_round_trip() {
        let display = [with_odd_parity(b'A'); 32];
        let control = ControlBits(C4_ERASE | C8_UPDATE | C11_SERIAL | C12 | C14);
        let packet = encode::header(mag(3), PageNumber(0x45), SubCode::new(0x2A5B), control, &display);
        let Ok(Decoded::Header(header)) = decode(&packet) else { panic!("not a header") };
        assert_eq!(header.magazine.number(), 3);
        assert_eq!(header.page, Some(PageNumber(0x45)));
        assert_eq!(header.subcode, Some(SubCode::new(0x2A5B)));
        assert_eq!(header.control, Some(control));
        assert_eq!(header.display, display);
        assert!(header.is_complete());
        assert_eq!(header.control.unwrap().national_index(), 0b101);
    }

    #[test]
    fn every_control_bit_round_trips() {
        for bit in 4..=14 {
            let control = ControlBits(1 << bit);
            let packet = encode::header(mag(1), PageNumber(0), SubCode::new(0), control, &[0x20; 32]);
            let Ok(Decoded::Header(header)) = decode(&packet) else { panic!() };
            assert_eq!(header.control, Some(control), "C{bit}");
        }
    }

    #[test]
    fn header_with_bad_page_byte_is_incomplete() {
        let mut packet = encode::header(mag(1), PageNumber(0x23), SubCode::new(0), ControlBits(0), &[0x20; 32]);
        packet.bytes[HEADER_UNITS] ^= 0b11;
        let Ok(Decoded::Header(header)) = decode(&packet) else { panic!() };
        assert!(header.page.is_none());
        assert!(!header.is_complete());
    }

    #[test]
    fn links_apply_relative_magazine_and_null() {
        let links = [
            Some((mag(2), PageNumber(0x34), SubCode::ANY)),
            Some((mag(8), PageNumber(0x00), SubCode::new(1))),
            None,
            Some((mag(1), PageNumber(0x99), SubCode::ANY)),
            None,
            Some((mag(5), PageNumber(0x50), SubCode::ANY)),
        ];
        let packet = encode::links(mag(1), &links, true, 0xBEEF);
        let Ok(Decoded::Links(decoded)) = decode(&packet) else { panic!() };
        assert!(decoded.display_row_24);
        assert_eq!(decoded.checksum_bytes, [0xBE, 0xEF]);
        for (expected, got) in links.iter().zip(decoded.links.iter()) {
            match (expected, got) {
                (None, None) => {}
                (Some((m, p, s)), Some(link)) => {
                    assert_eq!((link.magazine, link.page, link.subcode), (*m, *p, *s));
                }
                other => panic!("mismatch {other:?}"),
            }
        }
    }

    #[test]
    fn enhancement_triplets_round_trip() {
        let mut triplets = [Triplet::default(); 13];
        triplets[0] = Triplet { address: 41, mode: 4, data: 0 };
        triplets[1] = Triplet { address: 5, mode: 0x10, data: 0x41 };
        triplets[12] = Triplet { address: 63, mode: 0x1F, data: 0 };
        let packet = encode::triplet_packet(mag(1), 26, 7, &triplets);
        let Ok(Decoded::Enhancement(decoded)) = decode(&packet) else { panic!() };
        assert_eq!(decoded.designation, 7);
        for (expected, got) in triplets.iter().zip(decoded.triplets.iter()) {
            assert_eq!(Some(*expected), *got);
        }
    }

    #[test]
    fn page_enhancement_designations() {
        let [first, second] = encode::designation_triplets(0x2B, 0x55);
        let mut triplets = [Triplet::default(); 13];
        triplets[0] = first;
        triplets[1] = second;
        let packet = encode::triplet_packet(mag(2), 28, 0, &triplets);
        let Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) = decode(&packet) else { panic!() };
        assert_eq!(data.format, Some(PageFormat { function: 0, coding: 0 }));
        assert_eq!(data.fields.designations, Some(Designations { default: 0x2B, second: 0x55 }));
        let packet = encode::triplet_packet(mag(2), 29, 0, &triplets);
        assert!(matches!(decode(&packet), Ok(Decoded::Magazine { packet: MagazinePacket::Data(_), .. })));
    }

    /// Breaks triplet `index` (0 to 12) of a packet with a double error,
    /// which Hamming 24/18 detects but cannot correct.
    fn break_triplet(packet: &mut Packet, index: usize) {
        let start = DATA_START + 1 + index * TRIPLET_BYTES;
        packet.bytes[start] ^= 0b0110_0000;
        assert_eq!(triplet_values(packet)[index], None, "triplet {index} is uncorrectable");
    }

    /// The 18-bit value and on-air bytes of a worked encoding.
    fn worked(function: TripletFunction, value: u32, bytes: [u8; 3]) {
        let triplet = Triplet::from_value(value);
        assert_eq!(triplet.function(), function, "{value:#07x} decodes");
        assert_eq!(function.to_triplet().value(), value, "{function:?} encodes");
        assert_eq!(hamming2418_encode(value), bytes, "{value:#07x} on air");
    }

    /// REQ-PKT-010: decoding an encoded function gives the same function,
    /// for every one of the 2^18 triplet values.
    #[test]
    fn every_triplet_function_survives_encoding() {
        for value in 0..1u32 << 18 {
            let function = Triplet::from_value(value).function();
            assert_eq!(function.to_triplet().function(), function, "{value:#07x}");
        }
    }

    /// REQ-PKT-010: the worked encodings of research/notes-level25-objects.md
    /// section 13.2.
    #[test]
    fn worked_object_triplets() {
        let pop_c3 = ObjectNumber(0x32);
        assert_eq!((pop_c3.subpage(), pop_c3.pointer_packet_index()), (2, 0));
        assert_eq!((pop_c3.pointer_group(), pop_c3.pointer_odd()), (1, true));
        worked(
            TripletFunction::ObjectInvocation {
                source: ObjectSource::Public,
                kind: ObjectKind::Active,
                number: pop_c3,
            },
            0x19470,
            [0x81, 0x47, 0x32],
        );
        worked(
            TripletFunction::ObjectDefinition { usage: ObjectUsage(0b11), kind: ObjectKind::Active, number: pop_c3 },
            0x19578,
            [0x43, 0x57, 0x32],
        );
        let local = ObjectNumber(0x15);
        assert_eq!((local.n8(), local.designation(), local.triplet()), (false, 1, 5));
        worked(
            TripletFunction::ObjectInvocation { source: ObjectSource::Local, kind: ObjectKind::Passive, number: local },
            0x0ACE8,
            [0xC9, 0x4E, 0x15],
        );
        worked(TripletFunction::OriginModifier { row_offset: 2, column_offset: 56 }, 0x1C42A, [0xDB, 0x42, 0x38]);
        worked(TripletFunction::Termination { kind: 0b111 }, 0x03FFF, [0x74, 0x7F, 0x87]);
        worked(TripletFunction::Termination { kind: 0b010 }, 0x017FF, [0x76, 0x7F, 0x02]);
        let pointers = encode::pointer_triplet(OBJECT_POINTER_UNUSED, 30);
        assert_eq!((pointers.value(), hamming2418_encode(pointers.value())), (0x03DFF, [0xFE, 0x5F, 0x87]));
        let usage = ObjectUsage(0b01);
        assert!(usage.level_25() && !usage.level_35());
    }

    /// REQ-PKT-010: the worked encodings of research/notes-level25-drcs.md
    /// section 11. DRCS Mode reads as a reserved row triplet and DRCS
    /// Character Invocation as a column triplet not acted on, each keeping
    /// its fields as sent (decision E.95).
    #[test]
    fn worked_drcs_and_character_triplets() {
        let drcs_mode = |data| TripletFunction::ReservedRow { mode: 0b11000, address: 41, data };
        worked(drcs_mode(0x70), 0x38629, [0x4C, 0x62, 0x70]);
        worked(drcs_mode(0x13), 0x09E29, [0x4E, 0x62, 0x93]);
        let drcs_character = |column, data| TripletFunction::UnsupportedColumn { column, mode: 0b01101, data };
        worked(drcs_character(2, 0x40), 0x20342, [0x19, 0x34, 0x40]);
        worked(drcs_character(10, 47), 0x17B4A, [0x5B, 0x34, 0x2F]);
        assert_eq!(drcs_character(10, 47).active_column(), Some(10), "it moves the active position");
        assert_eq!(drcs_mode(0x70).active_column(), None);
        worked(
            TripletFunction::G3Character { column: 0, code: 0x20, level_15_form: false },
            0x102C0,
            [0x00, 0x2C, 0xA0],
        );
        worked(
            TripletFunction::G3Character { column: 5, code: 0x5B, level_15_form: true },
            0x2D885,
            [0x27, 0x08, 0xDB],
        );
        let style = FontStyleData { proportional: true, bold: true, italic: true, rows: 2 };
        worked(TripletFunction::FontStyle { column: 4, style }, 0x13B84, [0x20, 0xB8, 0x27]);
    }

    /// REQ-PKT-010: one triplet of every other mode of Tables 28 and 29,
    /// with its fields.
    #[test]
    fn every_mode_has_its_fields() {
        use TripletFunction as F;
        let t = |address: u8, mode: u8, data: u8| Triplet { address, mode, data }.function();
        let colour = |index, high| ColourData { index, high };
        assert_eq!(t(45, 0b00000, 0x1A), F::FullScreenColour(colour(0x1A, 0)));
        assert!(!colour(3, 1).is_defined());
        assert_eq!(t(50, 0b00001, 0x7F), F::FullRowColour { address: 50, colour: colour(0x1F, 0b11) });
        assert_eq!(t(41, 0b00100, 45), F::SetActivePosition { address: 41, column: 45 });
        assert_eq!(t(63, 0b00111, 0x05), F::AddressDisplayRow0 { address: 63, colour: colour(5, 0) });
        for mode in 0b01000..=0b01101 {
            assert_eq!(t(44, mode, 9), F::ProgrammeDeliveryRow { mode, address: 44, data: 9 });
        }
        for mode in [0b00010, 0b00011, 0b00101, 0b00110, 0b01110, 0b01111, 0b10100, 0b11001, 0b11110] {
            assert_eq!(t(44, mode, 9), F::ReservedRow { mode, address: 44, data: 9 }, "row mode {mode:#07b}");
        }
        assert_eq!(t(62, 0b11111, 7), F::ReservedRow { mode: 0b11111, address: 62, data: 7 });
        let gpop = t(0b11_1011, 0b10010, 0x41);
        assert_eq!(
            gpop,
            F::ObjectInvocation {
                source: ObjectSource::Global,
                kind: ObjectKind::Adaptive,
                number: ObjectNumber(0x1C1)
            }
        );
        assert_eq!(ObjectNumber(0x1C1).pointer_packet_index(), 3);
        assert_eq!(t(1, 0b00000, 0x28), F::ForegroundColour { column: 1, colour: colour(8, 1) });
        assert_eq!(t(2, 0b00001, 0x7F), F::G1Character { column: 2, code: 0x7F });
        assert_eq!(t(3, 0b00011, 0x11), F::BackgroundColour { column: 3, colour: colour(0x11, 0) });
        for mode in [0b00100, 0b00101, 0b01010] {
            assert_eq!(t(4, mode, 1), F::ReservedColumn { column: 4, mode, data: 1 });
        }
        assert_eq!(t(5, 0b00110, 3), F::ProgrammeDeliveryColumn { column: 5, data: 3 });
        assert_eq!(t(6, 0b00111, 0b011_0011), F::AdditionalFlash { column: 6, mode: 0b11, rate: 0b100, reserved: 1 });
        assert_eq!(t(7, 0b01000, 0x37), F::ModifiedDesignation { column: 7, designation: 0x37 });
        assert_eq!(t(8, 0b01001, b'A'), F::G0Character { column: 8, code: b'A' });
        let attributes = DisplayAttributes {
            double_height: true,
            boxing: false,
            conceal: true,
            invert: false,
            underline: true,
            double_width: true,
        };
        // D3 is reserved and ignored.
        assert_eq!(t(9, 0b01100, 0b110_1101), F::DisplayAttributes { column: 9, attributes });
        assert_eq!(t(10, 0b01111, 0x3B), F::G2Character { column: 10, code: 0x3B });
        assert_eq!(t(11, 0b10000, 0x2A), F::G0Diacritic { column: 11, code: 0x2A, diacritic: 0 });
        assert_eq!(t(12, 0b11000, b'e'), F::G0Diacritic { column: 12, code: b'e', diacritic: 8 });
    }

    /// REQ-PKT-011: the worked link of research/notes-level25-objects.md
    /// section 13.2, an encoded packet, and an injected error.
    #[test]
    fn compositional_links_decode_and_mark_damage() {
        let link = CompositionalLink::from_values(mag(1), 0x39CCD, 0x0000C);
        let expected = CompositionalLink {
            function: 0b01,
            validity: 0b11,
            magazine: mag(2),
            page: PageNumber(0xE3),
            subpages: 0b11,
            marker_first: true,
            marker_second: false,
        };
        assert_eq!(link, expected);
        assert!(link.is_format_1() && !link.is_null());
        assert_eq!(link.to_values(mag(1)), (0x39CCD, 0x0000C));
        assert_eq!(hamming2418_encode(0x39CCD), [0x66, 0x4C, 0xF3]);
        assert_eq!(hamming2418_encode(0x0000C), [0xEA, 0x80, 0x80]);
        let null = CompositionalLink { page: PageNumber(0xFF), subpages: 0xFFFF, ..expected };
        let mut links = [expected, null, expected, expected, expected, expected];
        links[2].magazine = mag(8);
        links[3].subpages = 0;
        let mut packet = encode::compositional_links(mag(1), 5, &links);
        let Ok(Decoded::CompositionalLinks { designation: 5, links: decoded, .. }) = decode(&packet) else { panic!() };
        assert_eq!(decoded, links.map(Some));
        assert!(decoded[1].unwrap().is_null() && decoded[3].unwrap().is_null());
        break_triplet(&mut packet, 3);
        let Ok(Decoded::CompositionalLinks { links: decoded, .. }) = decode(&packet) else { panic!() };
        assert_eq!((decoded[0], decoded[1], decoded[2]), (Some(links[0]), None, Some(links[2])));
        assert!(matches!(decode(&encode::compositional_links(mag(1), 6, &links)), Ok(Decoded::Ignored(_))));
    }

    fn sample_fields() -> PresentationFields {
        let mut words = [Rgb12::default(); 16];
        for (index, word) in words.iter_mut().enumerate() {
            let value = index as u8;
            *word = Rgb12::new(value, 15 - value, value ^ 0b1010);
        }
        PresentationFields {
            designations: Some(Designations { default: 0x55, second: 0x2A }),
            side_panels: Some(SidePanelFields { left: true, right: false, status_flag: true, left_columns: 11 }),
            colour_words: Some(words),
            colours: Some(DefaultColourFields {
                screen: 0x13,
                row: 0x1D,
                black_background_substitution: true,
                remapping: 0b101,
            }),
        }
    }

    /// REQ-PKT-012 and REQ-PKT-015: every field of X/28/0 format 1 and
    /// X/28/4, the bit order of the colour words, and an error injected
    /// into each field group in turn (decision E.44).
    #[test]
    fn page_enhancement_fields_and_field_groups() {
        let fields = sample_fields();
        let format = PageFormat { function: 0, coding: 0 };
        for designation in [0, 4] {
            let packet = encode::page_enhancement(mag(3), designation, format, &fields);
            let decoded = match decode(&packet) {
                Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) if designation == 0 => data,
                Ok(Decoded::PageData { packet: PagePacket::Level35(data), .. }) if designation == 4 => data,
                other => panic!("{other:?}"),
            };
            assert_eq!(decoded, PageEnhancement { format: Some(format), fields });
        }
        // Colour word 0's red is the first stream bit, triplet 2 bit 11;
        // word 15's blue ends at triplet 13 bit 4 (Table 4).
        let mut words = [Rgb12::default(); 16];
        words[0] = Rgb12::new(1, 0, 0);
        words[15] = Rgb12::new(0, 0, 8);
        let only_words = PresentationFields { colour_words: Some(words), ..PresentationFields::default() };
        let packet = encode::page_enhancement(mag(1), 0, format, &only_words);
        let values = triplet_values(&packet);
        assert_eq!((values[1], values[12]), (Some(1 << 10), Some(1 << 3)));
        assert!(values[2..12].iter().all(|value| *value == Some(0)));
        // Triplet 13: screen bits 5 to 9, row 10 to 14, substitution 15,
        // re-mapping 16 to 18.
        let colours = PresentationFields { colours: fields.colours, ..PresentationFields::default() };
        let values = triplet_values(&encode::page_enhancement(mag(1), 0, format, &colours));
        assert_eq!(values[12], Some(0x13 << 4 | 0x1D << 9 | 1 << 14 | 0b101 << 15));
        // Side panels: triplet 2 bits 4 to 10.
        let panels = PresentationFields { side_panels: fields.side_panels, ..PresentationFields::default() };
        let values = triplet_values(&encode::page_enhancement(mag(1), 0, format, &panels));
        assert_eq!(values[1], Some(1 << 3 | 1 << 5 | 11 << 6));
        let damaged = |index: usize| {
            let mut packet = encode::page_enhancement(mag(3), 0, format, &fields);
            break_triplet(&mut packet, index);
            let Ok(Decoded::PageData { packet: PagePacket::Format1(data), .. }) = decode(&packet) else { panic!() };
            data
        };
        let data = damaged(0);
        assert_eq!((data.format, data.fields.designations), (None, None));
        assert_eq!((data.fields.side_panels, data.fields.colours), (fields.side_panels, fields.colours));
        assert_eq!(data.fields.colour_words, fields.colour_words);
        let data = damaged(1);
        assert_eq!(data.format, Some(format));
        assert_eq!((data.fields.designations, data.fields.side_panels, data.fields.colour_words), (None, None, None));
        assert_eq!(data.fields.colours, fields.colours);
        let data = damaged(6);
        assert_eq!(data.fields, PresentationFields { colour_words: None, ..fields });
        let data = damaged(12);
        assert_eq!(data.fields, PresentationFields { colour_words: None, colours: None, ..fields });
    }

    /// REQ-PKT-016: M/29/0, M/29/1 and M/29/4, and the packet functions
    /// that are ignored.
    #[test]
    fn magazine_packets() {
        let fields = sample_fields();
        let packet = encode::magazine_enhancement(mag(4), 0, &fields);
        assert_eq!(decode(&packet), Ok(Decoded::Magazine { magazine: mag(4), packet: MagazinePacket::Data(fields) }));
        let packet = encode::magazine_enhancement(mag(4), 4, &fields);
        assert_eq!(
            decode(&packet),
            Ok(Decoded::Magazine { magazine: mag(4), packet: MagazinePacket::Level35(fields) })
        );
        // M/29/1, the DRCS colour tables, is not decoded (decision E.95).
        let tables = encode::triplet_packet(mag(4), 29, 1, &[Triplet::default(); 13]);
        assert!(matches!(decode(&tables), Ok(Decoded::Ignored(_))));
        // A reserved packet function, and an uncorrectable triplet 1.
        let mut values = [0u32; 13];
        values[0] = 1;
        let reserved = encode::triplet_packet(mag(4), 29, 0, &values.map(Triplet::from_value));
        assert!(matches!(decode(&reserved), Ok(Decoded::Ignored(_))));
        let mut broken = encode::magazine_enhancement(mag(4), 4, &fields);
        break_triplet(&mut broken, 0);
        assert!(matches!(decode(&broken), Ok(Decoded::Ignored(_))));
        let other = encode::magazine_enhancement(mag(4), 2, &fields);
        assert!(matches!(decode(&other), Ok(Decoded::Ignored(_))));
    }

    /// REQ-PKT-014, decision E.95: of X/28/3 only the page function and
    /// coding are read, so a pattern download page is still known not to be
    /// text; and X/28/1 is not decoded.
    #[test]
    fn x28_3_gives_only_the_page_function() {
        let format = PageFormat { function: 0b0101, coding: 0 };
        let mut packet = encode::page_function(mag(1), format);
        assert_eq!(
            decode(&packet),
            Ok(Decoded::PageData { magazine: mag(1), packet: PagePacket::PageFunction(Some(format)) })
        );
        break_triplet(&mut packet, 0);
        assert_eq!(decode(&packet), Ok(Decoded::PageData { magazine: mag(1), packet: PagePacket::PageFunction(None) }));
        let tables = encode::triplet_packet(mag(1), 28, 1, &[Triplet::default(); 13]);
        assert!(matches!(decode(&tables), Ok(Decoded::Ignored(_))));
    }

    /// REQ-PKT-017: object page packets, pointer tables, an uncorrectable
    /// pointer triplet, and the extreme triplet numbers.
    #[test]
    fn object_pages_and_pointers() {
        let mut triplets = [Triplet::default(); 13];
        triplets[4] = encode::pointer_triplet(OBJECT_POINTER_UNUSED, 30);
        triplets[5] = encode::pointer_triplet(507, 0);
        let mut packet = encode::triplet_packet(mag(1), 1, OBJECT_POINTER_DATA_BIT, &triplets);
        let object = ObjectPacket::new(&packet.data());
        assert!(object.carries_pointers());
        assert_eq!((object.pointer(4, false), object.pointer(4, true)), (OBJECT_POINTER_UNUSED, 30));
        assert_eq!((object.pointer(5, false), object.pointer(5, true)), (507, 0));
        assert_eq!(object.pointer(0, false), OBJECT_POINTER_UNUSED, "triplet 0 is reserved");
        assert_eq!(triplet_location(30), Some(TripletLocation::Row { packet: 5, triplet: 4 }));
        break_triplet(&mut packet, 5);
        let object = ObjectPacket::new(&packet.data());
        assert_eq!((object.pointer(5, false), object.pointer(5, true)), (OBJECT_POINTER_UNUSED, OBJECT_POINTER_UNUSED));
        let definitions = encode::triplet_packet(mag(1), 2, 0, &triplets);
        assert!(!ObjectPacket::new(&definitions.data()).carries_pointers());
        assert_eq!(triplet_location(0), Some(TripletLocation::Row { packet: 3, triplet: 0 }));
        assert_eq!(triplet_location(298), Some(TripletLocation::Row { packet: 25, triplet: 12 }));
        assert_eq!(triplet_location(299), Some(TripletLocation::Enhancement { designation: 0, triplet: 0 }));
        assert_eq!(triplet_location(506), Some(TripletLocation::Enhancement { designation: 15, triplet: 12 }));
        assert_eq!(triplet_location(507), None);
    }

    /// REQ-PKT-018: byte-pair positions of Figure 20, as
    /// research/notes-level25-objects.md sections 7.2 and 13.3 list them.
    #[test]
    fn mot_positions() {
        for (page, packet, byte) in [
            (0x00, 1, 6),
            (0x19, 1, 44),
            (0x99, 5, 44),
            (0xA0, 6, 6),
            (0xF9, 8, 44),
            (0x0A, 9, 6),
            (0x2F, 9, 40),
            (0xFE, 14, 14),
            (0xFF, 14, 16),
            (0x23, 2, 12),
            (0x4F, 10, 28),
            (0xB7, 6, 40),
        ] {
            assert_eq!(mot_position(PageNumber(page)), MotPosition { packet, index: byte - 6 }, "page {page:02X}");
        }
    }

    /// REQ-PKT-018: links, byte pairs, totals and fallback bits, the default
    /// object of research/notes-level25-objects.md section 13.4, and
    /// uncorrectable bytes.
    #[test]
    fn mot_links_and_entries() {
        let mut row = [0u8; DATA_BYTES];
        // POP link 1 (bytes 16 to 25) as nibbles: magazine 0 (8) with the
        // X/27 flag, page 1E3, 3 sub-pages, fallback right panel with
        // substitution, defaults 0x9 then objects 0x32 and 0x00.
        let link = [0x8, 0xE, 0x3, 0x2, 0b1100, 0x9, 0x2, 0x3, 0x0, 0x0];
        row[10..20].copy_from_slice(&link);
        row[0..2].copy_from_slice(&encode::mot_page_entry(MotPageEntry { global_objects: true, pop_link: 5 }));
        // The second byte names a DRCS page, which is not read (decision
        // E.95); the encoder writes it as 0.
        assert_eq!(row[1], 0);
        row[1] = 0x7;
        let packet = encode::nibble_packet(mag(2), 19, &row);
        let data = packet.data();
        let decoded = mot_object_link(&data, 1).unwrap();
        assert_eq!(decoded.magazine, mag(8));
        assert!(decoded.x27_flag);
        assert_eq!((decoded.page, decoded.subpage_count), (PageNumber(0xE3), 2));
        assert_eq!(
            decoded.fallback.side_panels,
            SidePanelFields { left: false, right: true, status_flag: true, left_columns: 0 }
        );
        assert!(decoded.fallback.black_background_substitution);
        assert_eq!(decoded.default_objects[0], Some(DefaultObject { kind: ObjectKind::Active, number: 0x32 }));
        assert_eq!(decoded.default_objects[1], Some(DefaultObject { kind: ObjectKind::Adaptive, number: 0 }));
        assert_eq!(encode::mot_object_link(&decoded), link);
        assert_eq!(mot_page_entry(&data, 0), Some(MotPageEntry { global_objects: true, pop_link: 5 }));
        assert_eq!(
            mot_fallback(0b1111).side_panels,
            SidePanelFields { status_flag: true, ..SidePanelFields::default() }
        );
        assert_eq!(mot_fallback(0b0110).side_panels.left_columns, 8);
        assert!(mot_object_link(&data, 4).is_none());
        let mut broken = packet;
        broken.bytes[DATA_START + 12] ^= 0b11;
        assert!(mot_object_link(&broken.data(), 1).is_none());
        assert!(mot_object_link(&broken.data(), 0).is_some());
        // Only the first byte of a pair is read: a damaged second byte
        // leaves the entry, a damaged first byte does not.
        let mut broken = packet;
        broken.bytes[DATA_START + 1] ^= 0b11;
        assert_eq!(mot_page_entry(&broken.data(), 0), Some(MotPageEntry { global_objects: true, pop_link: 5 }));
        broken.bytes[DATA_START] ^= 0b11;
        assert_eq!(mot_page_entry(&broken.data(), 0), None);
        let totals = MotTotals { magazines: 0b1000_0011, object_pages: 0x2A };
        let mut row = [0u8; DATA_BYTES];
        row[34..40].copy_from_slice(&encode::mot_totals(&totals));
        assert_eq!(row[38..40], [0, 0], "no DRCS pages counted");
        // A DRCS total from a service is not read.
        row[38] = 0x1;
        let data = encode::nibble_packet(mag(2), 21, &row).data();
        assert_eq!(mot_totals(&data), Some(totals));
    }

    #[test]
    fn packets_30_outside_magazine_8_and_31_are_ignored() {
        assert!(matches!(decode(&encode::address(mag(1), 30)), Ok(Decoded::Ignored(_))));
        assert!(matches!(decode(&encode::address(mag(4), 31)), Ok(Decoded::Ignored(_))));
    }
}
