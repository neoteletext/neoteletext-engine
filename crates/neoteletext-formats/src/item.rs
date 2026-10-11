//! What readers report: the [`Item`] callback type, diagnostics and the
//! common [`Reader`] interface (REQ-INP-001 and REQ-INP-020).

use neoteletext_core::packet::{Packet, PacketMeta};
use neoteletext_core::types::PageAddress;

use crate::detect::Format;
use crate::page::{IssueKind, PageFile};
use crate::transport::TeletextEntry;

/// One thing a reader found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item<'a> {
    /// A packet in the internal form, with its metadata (REQ-ARCH-002 and
    /// REQ-ARCH-003).
    Packet {
        /// The packet.
        packet: Packet,
        /// Where it came from.
        meta: PacketMeta,
    },
    /// A complete page or sub-page read from a page file. Convert it with
    /// `encode::encode_page` (feature `pages`) to pass it to the decoder
    /// (REQ-ARCH-004).
    Page(&'a PageFile),
    /// The format chosen by detection (REQ-INP-005).
    Detected(Format),
    /// One entry of a teletext descriptor in a Program Map Table
    /// (REQ-INP-007).
    TeletextEntry {
        /// The `program_number`.
        program: u16,
        /// The elementary stream's PID.
        pid: u16,
        /// The entry.
        entry: TeletextEntry,
    },
    /// The teletext stream the reader decodes, chosen by the configuration
    /// or as the first found (REQ-INP-007).
    StreamSelected {
        /// The `program_number`, `None` when the PID was given directly and no
        /// table has named its program yet.
        program: Option<u16>,
        /// The PID.
        pid: u16,
    },
    /// The page that the selected stream's descriptor names as a subtitle
    /// page, offered as the default for subtitle display (REQ-INP-007).
    SubtitlePage(PageAddress),
    /// Malformed input (REQ-INP-020).
    Diagnostic(Diagnostic),
}

/// A problem in the input and where it was found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    /// Byte offset from the start of the input.
    pub offset: u64,
    /// Line number, starting at 1, for text formats.
    pub line: Option<u32>,
    /// What was wrong.
    pub kind: DiagnosticKind,
}

impl Diagnostic {
    /// A diagnostic at a byte offset.
    #[must_use]
    pub const fn at(offset: u64, kind: DiagnosticKind) -> Self {
        Self { offset, line: None, kind }
    }

    /// A diagnostic at a byte offset and line.
    #[must_use]
    pub const fn at_line(offset: u64, line: u32, kind: DiagnosticKind) -> Self {
        Self { offset, line: Some(line), kind }
    }
}

/// The kinds of malformed input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiagnosticKind {
    /// The input ended inside a packet, record or file of fixed size.
    Truncated {
        /// Bytes of the incomplete unit.
        bytes: u32,
    },
    /// Bytes were skipped to regain synchronisation; the offset is where
    /// skipping began.
    SkippedBytes {
        /// Number of bytes skipped.
        count: u32,
    },
    /// A transport packet with `transport_error_indicator` set was discarded.
    TransportError {
        /// Its PID.
        pid: u16,
    },
    /// A continuity counter did not follow on (ISO/IEC 13818-1 2.4.3.3).
    ContinuityBreak {
        /// The PID.
        pid: u16,
        /// The counter value expected.
        expected: u8,
        /// The counter value found.
        found: u8,
    },
    /// `adaptation_field_control` was the reserved value 00.
    ReservedAdaptationControl {
        /// The PID.
        pid: u16,
    },
    /// An adaptation field longer than its packet.
    BadAdaptationField {
        /// The PID.
        pid: u16,
    },
    /// A table section failed its `CRC_32`.
    SectionCrc {
        /// The PID.
        pid: u16,
    },
    /// A table section was malformed.
    BadSection {
        /// The PID.
        pid: u16,
    },
    /// More programs than the reader can follow.
    TooManyPrograms,
    /// The input ended without a teletext stream being found.
    NoTeletextStream,
    /// A PES packet with a `stream_id` other than `private_stream_1` was skipped.
    NotPrivateStream {
        /// The `stream_id`.
        stream_id: u8,
    },
    /// A PES header was malformed.
    BadPesHeader,
    /// A `data_identifier` outside 0x10 to 0x1F and 0x99 to 0x9B
    /// (REQ-INP-008); the rest of the PES packet was skipped.
    BadDataIdentifier {
        /// The value.
        value: u8,
    },
    /// A teletext data unit had the wrong framing code and was skipped.
    BadFramingCode {
        /// The byte found.
        value: u8,
    },
    /// A teletext data unit was too short to hold a packet.
    ShortDataUnit {
        /// `data_unit_id`.
        id: u8,
        /// `data_unit_length`.
        length: u8,
    },
    /// A PES packet with `PES_packet_length` 0, whose end is not known.
    UnboundedPes,
    /// A text line longer than the reader's line buffer; the rest of the line
    /// was ignored.
    LineTooLong,
    /// An unknown page file command, ignored (REQ-INP-013).
    UnknownCommand {
        /// The two command letters.
        command: [u8; 2],
    },
    /// A command whose arguments could not be read; it was ignored.
    BadArgument {
        /// The two command letters.
        command: [u8; 2],
    },
    /// An output line with more than 40 characters; the rest was ignored.
    RowTooLong {
        /// The row number.
        row: u8,
    },
    /// An escape byte at the end of a line, with no code after it.
    BadEscape,
    /// A fixed capacity was exceeded and the excess ignored.
    CapacityExceeded {
        /// Which capacity.
        what: Capacity,
    },
    /// Page content arrived before any page number; the page was given the
    /// default address.
    MissingPageNumber,
    /// A page-level problem recorded on the page (Appendix C and E rules).
    Page(IssueKind),
    /// A file did not begin with its format's signature.
    BadSignature,
    /// A file of fixed layout had the wrong size.
    BadSize {
        /// The size found, saturated.
        size: u32,
    },
    /// A character outside the base64url alphabet.
    BadBase64,
    /// A character-set code with no Table 32 equivalent; the default set is
    /// used.
    UnmappedLanguage {
        /// The code.
        code: u8,
    },
    /// The format could not be detected.
    UnknownFormat,
    /// The format was asked for but its reader is not compiled into this
    /// build (decision E.61); the input was ignored.
    FormatNotCompiled {
        /// The format asked for.
        format: Format,
    },
}

/// The fixed capacities a page or reader can run out of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Capacity {
    /// Programs followed in a transport stream.
    Programs,
    /// X/26 packets of a page.
    Enhancements,
    /// Other packed enhancement lines of a page.
    RawPackets,
    /// Link Extension links.
    Links,
    /// Form fields.
    Fields,
    /// Form options.
    Options,
    /// Recorded page issues.
    Issues,
    /// A text field.
    Text,
}

/// The common interface of every streaming reader (REQ-INP-001).
pub trait Reader {
    /// Reads the next piece of input, of any length, reporting what it
    /// completes.
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>));

    /// Signals the end of the input and reports anything still pending, such
    /// as the last page of a page file or a truncated packet.
    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>));
}

/// Saturating conversion of a length to the `u32` diagnostics carry.
#[must_use]
pub(crate) fn saturate_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Adds an index within the current piece to the piece's start offset.
#[must_use]
pub(crate) fn offset_of(base: u64, index: usize) -> u64 {
    base.saturating_add(u64::try_from(index).unwrap_or(u64::MAX))
}
