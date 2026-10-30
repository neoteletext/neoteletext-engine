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
    DESIGNATION_PAGE_DATA, MAGAZINE_MASK, PACKET_BYTES, PACKET_ENHANCEMENT, PACKET_FIRST_ROW, PACKET_FUNCTION_MASK,
    PACKET_HEADER, PACKET_LINKS, PACKET_MAGAZINE_ENHANCEMENT, PACKET_NUMBER_LOW_BIT, PACKET_PAGE_ENHANCEMENT,
    PACKET_ROW_25, PACKET_SERVICE, SERVICE_MAGAZINE,
};
use crate::ecc::hamming84;
use crate::types::{Magazine, PresentationFields};

mod compositional_links;
mod editorial_links;
mod function;
mod header;
mod inventory;
mod magazine_table;
mod object_pages;
mod page_data;
mod service_data;
mod triplet_fields;
mod triplets;

use compositional_links::compositional_links;
use editorial_links::decode_links;
use header::decode_header;
use service_data::decode_service;
use triplets::{triplet_values, triplets};

pub use compositional_links::CompositionalLink;
pub use editorial_links::{Link, Links};
pub use function::TripletFunction;
pub use header::Header;
pub use inventory::{InventoryFunction, PageCode, mip_position, page_code, sub_page_count};
pub use magazine_table::{
    DefaultObject, MotObjectLink, MotPageEntry, MotPosition, MotTotals, mot_fallback, mot_object_link, mot_page_entry,
    mot_position, mot_totals,
};
pub use object_pages::{ObjectPacket, TripletLocation, triplet_location};
pub use page_data::{
    PageEnhancement, PageFormat, TripletValues, page_enhancement, page_function, presentation_fields, stream_bits,
};
pub use service_data::ServicePacket;
pub use triplet_fields::{ColourData, DisplayAttributes, ObjectKind, ObjectNumber, ObjectSource, ObjectUsage};
pub use triplets::{Enhancement, Triplet, Triplets};

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

/// The magazine whose packet 31 is data channel 12, the audio profile's
/// and the channel records' (EN 300 708 clause 6.4.2): the data channel's
/// low three bits are the magazine bits, and its fourth bit the packet
/// number's low bit, set for packet 31.
const INDEPENDENT_AUDIO_MAGAZINE: u8 = crate::consts::IDL_CHANNEL_INDEPENDENT_AUDIO & MAGAZINE_MASK;
const _: () = assert!(
    crate::consts::IDL_CHANNEL_INDEPENDENT_AUDIO == INDEPENDENT_AUDIO_MAGAZINE | PACKET_NUMBER_LOW_BIT,
    "data channel 12 is magazine 4's packet 31"
);

/// Whether a packet is one a channel's T42 stream may carry (neoteletext-
/// common's REQ-1462, DEC-1459): its address decodes under Hamming 8/4
/// (EN 300 706 clause 8.2), and its kind is a page's packet X/0 to X/28,
/// the magazine's packet M/29, packet 8/30, or packet 4/31, data channel
/// 12 (EN 300 708 clause 6.4.2). Every other packet is invalid: an address
/// that does not decode, packet 30 of a magazine other than 8, and packet
/// 31 of any other data channel. The one check the push reader and a
/// server both use.
#[must_use]
pub fn is_valid_stream_packet(packet: &Packet) -> bool {
    let Ok(address) = decode_address(packet) else { return false };
    match address.number {
        number if number <= PACKET_MAGAZINE_ENHANCEMENT => true,
        PACKET_SERVICE => address.magazine.number() == SERVICE_MAGAZINE,
        crate::consts::PACKET_INDEPENDENT_DATA => address.magazine.number() == INDEPENDENT_AUDIO_MAGAZINE,
        _ => false,
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

/// Builders used by the page encoder and by tests: the inverse of
/// [`decode`].
pub mod encode;

#[cfg(test)]
mod tests;
