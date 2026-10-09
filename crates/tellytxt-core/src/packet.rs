//! Packet decoding (EN 300 706 clause 9, area PKT; layer L1).
//!
//! A [`Packet`] is the internal form of REQ-ARCH-002: the 42 bytes that
//! follow the framing code, in transmission order with the first transmitted
//! bit as bit 0, parity and Hamming bits retained. [`decode`] turns one packet
//! into typed fields and knows nothing about pages, time or characters.

use crate::consts::{
    ADDRESS_BYTE_0, ADDRESS_BYTE_1, DATA_BYTES, DATA_START, DESIGNATION_CODES,
    HEADER_DISPLAY_BYTES, HEADER_DISPLAY_START, LINK_NULL_PAGE, LINK_NULL_SUBCODE, MAGAZINE_MASK,
    PACKET_BYTES, PACKET_ENHANCEMENT, PACKET_FIRST_ROW, PACKET_HEADER,
    PACKET_LINKS, PACKET_MAGAZINE_ENHANCEMENT, PACKET_NUMBER_LOW_BIT,
    PACKET_PAGE_ENHANCEMENT, PACKET_ROW_25, PACKET_SERVICE, SERVICE_MAGAZINE,
    STATUS_MESSAGE_BYTES, TRIPLET_ADDRESS_BITS, TRIPLET_ADDRESS_MASK, TRIPLET_BYTES,
    TRIPLET_DATA_FIELD_MASK, TRIPLET_MODE_BITS, TRIPLET_MODE_MASK, TRIPLETS_PER_PACKET,
};
use crate::ecc::{hamming84, hamming2418, reverse_bits};
use crate::types::{ControlBits, Magazine, PageNumber, SubCode};

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
            data: ((value >> (TRIPLET_ADDRESS_BITS + TRIPLET_MODE_BITS)) & TRIPLET_DATA_FIELD_MASK)
                as u8,
        }
    }

    /// The 18-bit value of the fields.
    #[must_use]
    pub const fn value(self) -> u32 {
        (self.address as u32 & TRIPLET_ADDRESS_MASK)
            | ((self.mode as u32 & TRIPLET_MODE_MASK) << TRIPLET_ADDRESS_BITS)
            | ((self.data as u32 & TRIPLET_DATA_FIELD_MASK)
                << (TRIPLET_ADDRESS_BITS + TRIPLET_MODE_BITS))
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

/// Page or magazine enhancement data: X/28/0 format 1 or M/29/0
/// (REQ-PKT-006 and REQ-PKT-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EnhancementData {
    /// Page function, triplet 1 bits 1 to 4 (Table 3).
    pub page_function: u8,
    /// Page coding, triplet 1 bits 5 to 7 (Table 3).
    pub page_coding: u8,
    /// Default G0 and G2 designation, triplet 1 bits 8 to 14 (Table 4).
    pub default_designation: u8,
    /// Second G0 designation, triplet 1 bits 15 to 18 and triplet 2 bits 1
    /// to 3 (Table 4).
    pub second_designation: u8,
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
    /// Packet X/28/0 format 1.
    PageData {
        /// The magazine.
        magazine: Magazine,
        /// The decoded data, `None` when triplet 1 or 2 was uncorrectable.
        data: Option<EnhancementData>,
    },
    /// Packet X/28/1, stored without interpretation (decision E.4).
    PageDataRaw {
        /// The magazine.
        magazine: Magazine,
        /// Packet bytes 6 to 45.
        bytes: [u8; DATA_BYTES],
    },
    /// Packet M/29/0 with packet function 0.
    Magazine {
        /// The magazine.
        magazine: Magazine,
        /// The decoded data, `None` when uncorrectable.
        data: Option<EnhancementData>,
    },
    /// Packet M/29/1, stored without interpretation.
    MagazineRaw {
        /// The magazine.
        magazine: Magazine,
        /// Packet bytes 6 to 45.
        bytes: [u8; DATA_BYTES],
    },
    /// Packet 8/30.
    Service(ServicePacket),
    /// Anything counted and ignored: X/27/4 and above, other X/28 and M/29
    /// designations, packets 30 outside magazine 8, packets 31.
    Ignored(PacketAddress),
}

/// Decodes one packet (area PKT).
pub fn decode(packet: &Packet) -> Result<Decoded, PacketError> {
    let address = decode_address(packet)?;
    let magazine = address.magazine;
    let decoded = match address.number {
        PACKET_HEADER => Decoded::Header(decode_header(packet, magazine)),
        PACKET_FIRST_ROW..=PACKET_ROW_25 => {
            Decoded::Row(Row { magazine, row: address.number, bytes: packet.data() })
        }
        PACKET_ENHANCEMENT => {
            let designation = designation_code(packet)?;
            Decoded::Enhancement(Enhancement { magazine, designation, triplets: triplets(packet) })
        }
        PACKET_LINKS => match designation_code(packet)? {
            0 => Decoded::Links(decode_links(packet, magazine)),
            designation @ 1..=3 => {
                Decoded::LinksRaw { magazine, designation, bytes: packet.data() }
            }
            // Designation codes 4 to 15 are Level 2.5 compositional links.
            _ => Decoded::Ignored(address),
        },
        PACKET_PAGE_ENHANCEMENT => match designation_code(packet)? {
            0 => Decoded::PageData { magazine, data: enhancement_data(packet) },
            1 => Decoded::PageDataRaw { magazine, bytes: packet.data() },
            _ => Decoded::Ignored(address),
        },
        PACKET_MAGAZINE_ENHANCEMENT => match designation_code(packet)? {
            0 => match enhancement_data(packet) {
                // Table 12: packet function 0000000 is the only defined one,
                // and it shares bits 1 to 4 with the X/28 page function.
                Some(data) if data.page_function == 0 && data.page_coding == 0 => {
                    Decoded::Magazine { magazine, data: Some(data) }
                }
                Some(_) => Decoded::Ignored(address),
                None => Decoded::Magazine { magazine, data: None },
            },
            1 => Decoded::MagazineRaw { magazine, bytes: packet.data() },
            _ => Decoded::Ignored(address),
        },
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

/// The thirteen triplets of packet bytes 7 to 45.
fn triplets(packet: &Packet) -> Triplets {
    let mut result = [None; TRIPLETS_PER_PACKET];
    let body = packet.bytes.get(DATA_START.saturating_add(1)..).unwrap_or(&[]);
    let (chunks, _) = body.as_chunks::<TRIPLET_BYTES>();
    for (slot, chunk) in result.iter_mut().zip(chunks) {
        *slot = hamming2418(*chunk).map(Triplet::from_value);
    }
    result
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
/// Data bit 4 of the link control byte: display row 24 (Table 14).
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
    let display_row_24 =
        nibble(packet, LINK_CONTROL).is_some_and(|control| control & LINK_CONTROL_ROW_24 != 0);
    let checksum_bytes = [packet.byte(LINK_CHECKSUM), packet.byte(LINK_CHECKSUM.saturating_add(1))];
    Links { magazine, links, display_row_24, checksum_bytes }
}

/// Triplet 1 and 2 of X/28/0 format 1 or M/29/0 (Table 4).
fn enhancement_data(packet: &Packet) -> Option<EnhancementData> {
    let all = triplets(packet);
    let first = all.first().copied().flatten()?.value();
    let second = all.get(1).copied().flatten()?.value();
    Some(EnhancementData {
        page_function: (first & 0xF) as u8,
        page_coding: ((first >> 4) & 0x7) as u8,
        default_designation: ((first >> 7) & 0x7F) as u8,
        second_designation: (((first >> 14) & 0xF) | ((second & 0x7) << 4)) as u8,
    })
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
    use super::{
        HEADER_C7_C10, HEADER_C11_C14, HEADER_S1, HEADER_S2_C4, HEADER_S3, HEADER_S4_C5_C6,
        HEADER_TENS, HEADER_UNITS, LINK_BYTES, LINK_CHECKSUM, LINK_CONTROL, LINK_CONTROL_ROW_24,
        LINK_START, Packet, Triplet,
    };
    use crate::consts::{
        ADDRESS_BYTE_0, ADDRESS_BYTE_1, DATA_BYTES, DATA_START, HEADER_DISPLAY_BYTES,
        HEADER_DISPLAY_START, MAGAZINE_MASK, NIBBLE_MASK, PACKET_BYTES, TRIPLET_BYTES,
        TRIPLETS_PER_PACKET,
    };
    use crate::ecc::{hamming84_encode, hamming2418_encode};
    use crate::types::{ControlBits, Magazine, PageNumber, SubCode};

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
            let start = DATA_START
                .saturating_add(1)
                .saturating_add(index.saturating_mul(TRIPLET_BYTES));
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
        let first = (u32::from(default_designation & 0x7F) << 7)
            | (u32::from(second_designation & 0xF) << 14);
        let second = u32::from((second_designation >> 4) & 0x7);
        [Triplet::from_value(first), Triplet::from_value(second)]
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
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::consts::{C4_ERASE, C8_UPDATE, C11_SERIAL, C12, C14};
    use crate::ecc::with_odd_parity;

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
        let Ok(Decoded::PageData { data: Some(data), .. }) = decode(&packet) else { panic!() };
        assert_eq!((data.page_function, data.page_coding), (0, 0));
        assert_eq!(data.default_designation, 0x2B);
        assert_eq!(data.second_designation, 0x55);
        let packet = encode::triplet_packet(mag(2), 29, 0, &triplets);
        assert!(matches!(decode(&packet), Ok(Decoded::Magazine { data: Some(_), .. })));
    }

    #[test]
    fn packets_30_outside_magazine_8_and_31_are_ignored() {
        assert!(matches!(decode(&encode::address(mag(1), 30)), Ok(Decoded::Ignored(_))));
        assert!(matches!(decode(&encode::address(mag(4), 31)), Ok(Decoded::Ignored(_))));
    }
}
