//! MPEG-2 transport streams (REQ-INP-006) and finding teletext in them
//! through the Program Association and Program Map Tables (REQ-INP-007).
//! The teletext PES packets are read by `pes::PesParser` (REQ-INP-008).
//!
//! The types here are always compiled; the reader needs the `transport`
//! feature (decision E.61).

use neoteletext_core::types::PageAddress;

use crate::consts::{TS_FEC_PACKET_BYTES, TS_M2TS_PACKET_BYTES, TS_PACKET_BYTES};

#[cfg(feature = "transport")]
mod reader;
#[cfg(feature = "transport")]
pub use reader::{TsReader, crc32};

/// Programs whose Program Map Tables the reader follows. A broadcast
/// multiplex carries about ten; the excess is reported.
pub const MAX_PROGRAMS: usize = 16;

/// The three transport packet layouts (REQ-INP-006).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TsPacketSize {
    /// 188-byte packets.
    Standard,
    /// 192-byte packets with a 4-byte prefix ("M2TS").
    M2ts,
    /// 204-byte packets with 16 bytes of error-correction data after each.
    Fec,
}

impl TsPacketSize {
    /// Every size, in the order detection tries them.
    pub const ALL: [Self; 3] = [Self::Standard, Self::M2ts, Self::Fec];

    /// Bytes from one sync byte to the next.
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::Standard => TS_PACKET_BYTES,
            Self::M2ts => TS_M2TS_PACKET_BYTES,
            Self::Fec => TS_FEC_PACKET_BYTES,
        }
    }
}

/// Transport stream reader configuration (REQ-INP-007).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TsConfig {
    /// The packet size, detected when `None`.
    pub packet_size: Option<TsPacketSize>,
    /// Decode this PID as teletext without waiting for the tables.
    pub pid: Option<u16>,
    /// Decode the first teletext stream of this program.
    pub program: Option<u16>,
    /// The source number written into packet metadata.
    pub source: Option<u8>,
}

/// `teletext_type` of a descriptor entry (EN 300 468 Table 94).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TeletextType {
    /// Initial teletext page.
    Initial,
    /// Teletext subtitle page.
    Subtitle,
    /// Additional information page.
    AdditionalInformation,
    /// Programme schedule page.
    ProgrammeSchedule,
    /// Teletext subtitle page for hearing-impaired people.
    HearingImpairedSubtitle,
    /// A reserved value.
    Reserved(u8),
}

impl TeletextType {
    /// Interprets the 5-bit field (EN 300 468 Table 94).
    #[must_use]
    pub const fn from_bits(value: u8) -> Self {
        match value {
            0x01 => Self::Initial,
            0x02 => Self::Subtitle,
            0x03 => Self::AdditionalInformation,
            0x04 => Self::ProgrammeSchedule,
            0x05 => Self::HearingImpairedSubtitle,
            other => Self::Reserved(other),
        }
    }

    /// True for both kinds of subtitle page.
    #[must_use]
    pub const fn is_subtitle(self) -> bool {
        matches!(self, Self::Subtitle | Self::HearingImpairedSubtitle)
    }
}

/// One entry of a teletext descriptor (EN 300 468 clause 6.2.43).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct TeletextEntry {
    /// The ISO 639-2 language code.
    pub language: [u8; 3],
    /// The kind of page.
    pub kind: TeletextType,
    /// The page: magazine (transmitted 0 meaning 8) and page number.
    pub page: PageAddress,
    /// The descriptor tag the entry came from, 0x56 or 0x46.
    pub descriptor: u8,
}

#[cfg(all(test, feature = "transport"))]
pub(crate) mod tests;
