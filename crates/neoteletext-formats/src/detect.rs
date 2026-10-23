//! Format detection (REQ-INP-005) and the [`AutoReader`] that detects the
//! format and then reads with the matching reader.
//!
//! Detection recognises only the formats compiled into the build (decision
//! E.61, [`Format::is_compiled`]); a format asked for by the configuration
//! that is not compiled in is reported as
//! [`DiagnosticKind::FormatNotCompiled`](crate::item::DiagnosticKind::FormatNotCompiled).

use neoteletext_core::consts::PACKET_BYTES_WITH_RUN_IN;

use crate::sliced::ByteOrder;
use crate::t42::T42Variant;
use crate::transport::{TsConfig, TsPacketSize};

mod any;
mod auto;
mod scoring;

pub use any::{AnyReader, NotCompiledReader};
pub use auto::AutoReader;
pub use scoring::detect;

/// Packets examined when choosing among the headerless binary layouts:
/// "the first several hundred" (REQ-INP-005).
pub const DETECTION_PACKETS: usize = 300;
/// Bytes held for detection: that many packets of the longest layout.
pub const DETECTION_BYTES: usize = DETECTION_PACKETS * PACKET_BYTES_WITH_RUN_IN;

/// An input format.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Format {
    /// A headerless packet stream (REQ-INP-002 to 004).
    T42(T42Variant),
    /// An MPEG-2 transport stream (REQ-INP-006).
    TransportStream(TsPacketSize),
    /// A raw file of PES packets (REQ-INP-009).
    Pes,
    /// Linux sliced VBI records (REQ-INP-010).
    Sliced(ByteOrder),
    /// A TTI page file (REQ-INP-013).
    Tti,
    /// An EP1 page file (REQ-INP-015).
    Ep1,
    /// edit.tf or zxnet page addresses (REQ-INP-016).
    EditTf,
    /// A raw screen dump (REQ-INP-017).
    ScreenDump,
}

impl Format {
    /// True when this build has the format's reader (decision E.61). T42 is
    /// always compiled; each other format needs its feature.
    #[must_use]
    pub const fn is_compiled(self) -> bool {
        match self {
            Self::T42(_) => true,
            Self::TransportStream(_) | Self::Pes => cfg!(feature = "transport"),
            Self::Sliced(_) => cfg!(feature = "sliced"),
            Self::Tti => cfg!(feature = "tti"),
            Self::Ep1 => cfg!(feature = "ep1"),
            Self::EditTf => cfg!(feature = "edittf"),
            Self::ScreenDump => cfg!(feature = "dump"),
        }
    }
}

/// What detection decided.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detection {
    /// The format, and the bytes to discard before the first whole packet.
    Found {
        /// The format.
        format: Format,
        /// Bytes to skip at the start.
        skip: usize,
    },
    /// More input is needed.
    NeedMore,
    /// No format matched.
    Unknown,
}

/// Configuration of the [`AutoReader`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct AutoConfig {
    /// An explicit format, which overrides detection (REQ-INP-005).
    pub format: Option<Format>,
    /// Follow each page item with the packets of the encoded page
    /// (REQ-ARCH-004). Pages come only from the page-file readers, each of
    /// which brings in the `pages` feature that encodes them, so a build
    /// without it has no pages to encode.
    pub encode_pages: bool,
    /// Transport stream selection.
    pub transport: TsConfig,
    /// The source number written into packet metadata.
    pub source: Option<u8>,
}

#[cfg(test)]
mod tests;
