//! Format detection (REQ-INP-005) and the [`AutoReader`] that detects the
//! format and then reads with the matching reader.
//!
//! Detection recognises only the formats compiled into the build (decision
//! E.61, [`Format::is_compiled`]); a format asked for by the configuration
//! that is not compiled in is reported as
//! [`DiagnosticKind::FormatNotCompiled`].

use neoteletext_core::consts::{ADDRESS_BYTE_0, ADDRESS_BYTE_1, PACKET_BYTES_WITH_RUN_IN};
use neoteletext_core::ecc::{Status, hamming84_decode};
#[cfg(feature = "pages")]
use neoteletext_core::packet::PacketMeta;

use crate::consts::{
    BASE64URL_ALPHABET, DUMP_BYTES_24, DUMP_BYTES_25, EP1_SIGNATURE, PES_START_CODE, SLICED_HEADER_BYTES,
    SLICED_RECORD_BYTES, SLICED_TELETEXT_B, STREAM_ID_PRIVATE_1, TS_SYNC_BYTE,
};
#[cfg(feature = "dump")]
use crate::dump::DumpReader;
#[cfg(feature = "edittf")]
use crate::edittf::EditTfReader;
#[cfg(feature = "pages")]
use crate::encode::encode_page;
#[cfg(feature = "ep1")]
use crate::ep1::Ep1Reader;
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader};
#[cfg(feature = "transport")]
use crate::pes::PesReader;
use crate::sliced::ByteOrder;
#[cfg(feature = "sliced")]
use crate::sliced::SlicedReader;
use crate::t42::{T42Reader, T42Variant};
use crate::text::copy_prefix;
#[cfg(feature = "transport")]
use crate::transport::TsReader;
use crate::transport::{TsConfig, TsPacketSize};
#[cfg(feature = "tti")]
use crate::tti::TtiReader;

/// Packets examined when choosing among the headerless binary layouts:
/// "the first several hundred" (REQ-INP-005).
pub const DETECTION_PACKETS: usize = 300;
/// Bytes held for detection: that many packets of the longest layout.
pub const DETECTION_BYTES: usize = DETECTION_PACKETS * PACKET_BYTES_WITH_RUN_IN;
/// Sync bytes that must agree before a transport stream is recognised.
const TS_SYNCS: usize = 4;
/// The UTF-8 byte order mark some editors write at the start of text files.
const BYTE_ORDER_MARK: &[u8] = &[0xEF, 0xBB, 0xBF];
/// A binary layout is accepted when at least this fraction of its packets
/// have decodable addresses: two in three. Random bytes give about one in
/// three.
const ACCEPT_NUMERATOR: usize = 2;
const ACCEPT_DENOMINATOR: usize = 3;

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

fn without_bom(prefix: &[u8]) -> &[u8] {
    prefix.strip_prefix(BYTE_ORDER_MARK).unwrap_or(prefix)
}

/// True when the text begins with an edit.tf address: a hexadecimal digit, a
/// colon and base64url data, possibly after a web address ending in `#`.
fn looks_like_edittf(text: &[u8]) -> bool {
    let line = text.split(|&byte| byte == b'\n').next().unwrap_or(&[]).trim_ascii();
    let address = line
        .iter()
        .rposition(|&byte| byte == b'#')
        .map_or(line, |index| line.get(index.saturating_add(1)..).unwrap_or(&[]));
    matches!(address, [digit, b':', first, ..] if digit.is_ascii_hexdigit() && BASE64URL_ALPHABET.contains(first))
}

/// True when the text begins with a TTI command: two capital letters and a
/// comma.
fn looks_like_tti(text: &[u8]) -> bool {
    let text = text.trim_ascii_start();
    matches!(text, [first, second, b',', ..] if first.is_ascii_uppercase() && second.is_ascii_uppercase())
}

/// The first offset at which a transport stream of `size` keeps its sync
/// bytes.
fn transport_offset(prefix: &[u8], size: usize, complete: bool) -> Option<usize> {
    (0..size).find(|&offset| {
        let mut syncs = 0usize;
        let mut position = offset;
        while let Some(&byte) = prefix.get(position) {
            if byte != TS_SYNC_BYTE {
                return false;
            }
            syncs = syncs.saturating_add(1);
            if syncs == TS_SYNCS {
                return true;
            }
            position = position.saturating_add(size);
        }
        complete && syncs > 0
    })
}

/// True when both packet address bytes can be decoded, and whether both
/// were error-free.
fn address_quality(first: u8, second: u8) -> (bool, bool) {
    let (_, a) = hamming84_decode(first);
    let (_, b) = hamming84_decode(second);
    (a.is_usable() && b.is_usable(), a == Status::Clean && b == Status::Clean)
}

/// Scores one binary layout at one offset: (units with usable addresses,
/// units with clean addresses, units examined).
fn score_t42(prefix: &[u8], variant: T42Variant, offset: usize) -> (usize, usize, usize) {
    let mut usable = 0usize;
    let mut clean = 0usize;
    let mut examined = 0usize;
    let units = prefix.get(offset..).unwrap_or(&[]).chunks_exact(variant.unit_bytes()).take(DETECTION_PACKETS);
    for unit in units {
        examined = examined.saturating_add(1);
        let Some(packet) = variant.packet(unit) else { continue };
        let (ok, perfect) = address_quality(packet.byte(ADDRESS_BYTE_0), packet.byte(ADDRESS_BYTE_1));
        usable = usable.saturating_add(usize::from(ok));
        clean = clean.saturating_add(usize::from(perfect));
    }
    (usable, clean, examined)
}

/// Scores sliced records: teletext records whose packet addresses decode,
/// against records with service 0 or teletext.
fn score_sliced(prefix: &[u8], order: ByteOrder) -> (usize, usize, usize) {
    let mut usable = 0usize;
    let mut clean = 0usize;
    let mut examined = 0usize;
    for record in prefix.as_chunks::<SLICED_RECORD_BYTES>().0.iter().take(DETECTION_PACKETS) {
        examined = examined.saturating_add(1);
        let service = order.word(record, 0).unwrap_or(u32::MAX);
        if service != SLICED_TELETEXT_B {
            // Empty records count for the layout without adding evidence.
            if service == 0 {
                usable = usable.saturating_add(1);
            }
            continue;
        }
        let first = record.get(SLICED_HEADER_BYTES).copied().unwrap_or(0);
        let second = record.get(SLICED_HEADER_BYTES.saturating_add(1)).copied().unwrap_or(0);
        let (ok, perfect) = address_quality(first, second);
        usable = usable.saturating_add(usize::from(ok));
        clean = clean.saturating_add(usize::from(perfect));
    }
    (usable, clean, examined)
}

/// Chooses among the headerless binary layouts the one whose packet
/// addresses decode most often (REQ-INP-005 step 2).
fn detect_binary(prefix: &[u8]) -> Option<Detection> {
    let mut best: Option<(Format, usize, (usize, usize, usize))> = None;
    let mut consider = |format: Format, skip: usize, score: (usize, usize, usize)| {
        let (usable, clean, examined) = score;
        if examined == 0 || usable.saturating_mul(ACCEPT_DENOMINATOR) < examined.saturating_mul(ACCEPT_NUMERATOR) {
            return;
        }
        let better = best.is_none_or(|(_, _, (best_usable, best_clean, best_examined))| {
            // Compare usable/examined, then clean/examined, by
            // cross-multiplication.
            let left = usable.saturating_mul(best_examined);
            let right = best_usable.saturating_mul(examined);
            left > right || (left == right && clean.saturating_mul(best_examined) > best_clean.saturating_mul(examined))
        });
        if better {
            best = Some((format, skip, score));
        }
    };
    for variant in T42Variant::ALL {
        for offset in 0..variant.unit_bytes() {
            consider(Format::T42(variant), offset, score_t42(prefix, variant, offset));
        }
    }
    for order in [ByteOrder::Little, ByteOrder::Big] {
        if !Format::Sliced(order).is_compiled() {
            continue;
        }
        let (usable, clean, examined) = score_sliced(prefix, order);
        // Sliced records need at least one teletext record as evidence.
        if clean > 0 || usable > 0 && examined > 0 && order.word(prefix, 0) == Some(SLICED_TELETEXT_B) {
            consider(Format::Sliced(order), 0, (usable, clean, examined));
        }
    }
    best.map(|(format, skip, _)| Detection::Found { format, skip })
}

/// Detects the format of an input from its first bytes (REQ-INP-005).
/// `complete` is true when `prefix` is the whole input. Signatures are
/// tried first; then the headerless binary layouts are scored, which needs
/// [`DETECTION_BYTES`] or the whole input. Only the formats compiled in are
/// recognised ([`Format::is_compiled`]).
#[must_use]
pub fn detect(prefix: &[u8], complete: bool) -> Detection {
    // Every decision waits until the bytes it rests on have all arrived, so
    // that it does not depend on how the input is split (REQ-INP-001).
    let enough = complete || prefix.len() >= DETECTION_BYTES;
    let text = without_bom(prefix);
    let found = |format: Format| Detection::Found { format, skip: prefix.len().saturating_sub(text.len()) };
    if enough || text.contains(&b'\n') {
        if Format::EditTf.is_compiled() && looks_like_edittf(text) {
            return found(Format::EditTf);
        }
        if Format::Tti.is_compiled() && looks_like_tti(text) {
            return found(Format::Tti);
        }
    }
    if enough || prefix.len() > PES_START_CODE.len() {
        if Format::Ep1.is_compiled() && prefix.starts_with(&EP1_SIGNATURE) {
            return Detection::Found { format: Format::Ep1, skip: 0 };
        }
        if Format::Pes.is_compiled()
            && prefix.starts_with(&PES_START_CODE)
            && prefix.get(PES_START_CODE.len()) == Some(&STREAM_ID_PRIVATE_1)
        {
            return Detection::Found { format: Format::Pes, skip: 0 };
        }
    }
    if !enough {
        return Detection::NeedMore;
    }
    for size in TsPacketSize::ALL {
        // The reader finds the first sync byte itself.
        if Format::TransportStream(size).is_compiled() && transport_offset(prefix, size.bytes(), complete).is_some() {
            return Detection::Found { format: Format::TransportStream(size), skip: 0 };
        }
    }
    if Format::ScreenDump.is_compiled() && complete && (prefix.len() == DUMP_BYTES_25 || prefix.len() == DUMP_BYTES_24)
    {
        return Detection::Found { format: Format::ScreenDump, skip: 0 };
    }
    detect_binary(prefix).unwrap_or(Detection::Unknown)
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

/// The stand-in for a reader that is not compiled in (decision E.61): it
/// reports [`DiagnosticKind::FormatNotCompiled`] once and ignores the input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NotCompiledReader {
    format: Format,
    reported: bool,
}

impl NotCompiledReader {
    /// The stand-in for a format.
    #[must_use]
    pub const fn new(format: Format) -> Self {
        Self { format, reported: false }
    }

    fn report(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if !self.reported {
            self.reported = true;
            out(Item::Diagnostic(Diagnostic::at(0, DiagnosticKind::FormatNotCompiled { format: self.format })));
        }
    }
}

impl Reader for NotCompiledReader {
    fn feed(&mut self, _bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        self.report(out);
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.report(out);
    }
}

/// Any of the readers compiled in. The variants depend on the features, so
/// the enumeration is not exhaustive (decision E.61).
#[derive(Clone, Debug)]
// The variants differ greatly in size by design: each holds its own fixed
// buffers, and boxing is not available without allocation.
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum AnyReader {
    /// Headerless packets.
    T42(T42Reader),
    /// Transport streams.
    #[cfg(feature = "transport")]
    Transport(TsReader),
    /// Raw PES files.
    #[cfg(feature = "transport")]
    Pes(PesReader),
    /// Sliced records.
    #[cfg(feature = "sliced")]
    Sliced(SlicedReader),
    /// TTI files.
    #[cfg(feature = "tti")]
    Tti(TtiReader),
    /// EP1 files.
    #[cfg(feature = "ep1")]
    Ep1(Ep1Reader),
    /// edit.tf addresses.
    #[cfg(feature = "edittf")]
    EditTf(EditTfReader),
    /// Screen dumps.
    #[cfg(feature = "dump")]
    Dump(DumpReader),
    /// A format whose reader is not compiled in.
    NotCompiled(NotCompiledReader),
}

impl AnyReader {
    /// The reader for a format, or the [`NotCompiledReader`] when the
    /// format's reader is not compiled in.
    #[must_use]
    pub fn new(format: Format, skip: usize, config: &AutoConfig) -> Self {
        // One `if` per format, compiled with its reader, so that a build
        // without a format names nothing of its reader; a match would need
        // a wildcard arm that is unreachable when every format is compiled.
        if let Format::T42(variant) = format {
            return Self::T42(T42Reader::with_skip(variant, config.source, skip));
        }
        #[cfg(feature = "transport")]
        if let Format::TransportStream(size) = format {
            let transport = TsConfig { packet_size: Some(size), source: config.source, ..config.transport };
            return Self::Transport(TsReader::new(transport));
        }
        #[cfg(feature = "transport")]
        if format == Format::Pes {
            return Self::Pes(PesReader::new(config.source));
        }
        #[cfg(feature = "sliced")]
        if let Format::Sliced(order) = format {
            return Self::Sliced(SlicedReader::new(order, config.source));
        }
        #[cfg(feature = "tti")]
        if format == Format::Tti {
            return Self::Tti(TtiReader::new());
        }
        #[cfg(feature = "ep1")]
        if format == Format::Ep1 {
            return Self::Ep1(Ep1Reader::new());
        }
        #[cfg(feature = "edittf")]
        if format == Format::EditTf {
            return Self::EditTf(EditTfReader::new());
        }
        #[cfg(feature = "dump")]
        if format == Format::ScreenDump {
            return Self::Dump(DumpReader::new());
        }
        Self::NotCompiled(NotCompiledReader::new(format))
    }

    fn reader(&mut self) -> &mut dyn Reader {
        match self {
            Self::T42(reader) => reader,
            #[cfg(feature = "transport")]
            Self::Transport(reader) => reader,
            #[cfg(feature = "transport")]
            Self::Pes(reader) => reader,
            #[cfg(feature = "sliced")]
            Self::Sliced(reader) => reader,
            #[cfg(feature = "tti")]
            Self::Tti(reader) => reader,
            #[cfg(feature = "ep1")]
            Self::Ep1(reader) => reader,
            #[cfg(feature = "edittf")]
            Self::EditTf(reader) => reader,
            #[cfg(feature = "dump")]
            Self::Dump(reader) => reader,
            Self::NotCompiled(reader) => reader,
        }
    }
}

impl Reader for AnyReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        self.reader().feed(bytes, out);
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        self.reader().finish(out);
    }
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // As for AnyReader.
enum State {
    Detecting,
    Active(AnyReader),
    Failed,
}

/// A reader that detects the format and then reads it (REQ-INP-005). It
/// holds up to [`DETECTION_BYTES`] while detecting, then passes them and
/// everything after to the chosen reader.
#[derive(Clone, Debug)]
pub struct AutoReader {
    config: AutoConfig,
    window: [u8; DETECTION_BYTES],
    len: usize,
    state: State,
}

impl AutoReader {
    /// A reader. With `config.format` set, detection is skipped.
    #[must_use]
    pub fn new(config: AutoConfig) -> Self {
        let state = config.format.map_or(State::Detecting, |format| State::Active(AnyReader::new(format, 0, &config)));
        Self { config, window: [0; DETECTION_BYTES], len: 0, state }
    }

    /// Feeds the active reader, encoding pages into packets when asked.
    // Without `pages` no reader produces pages, so the configuration has
    // nothing to say.
    #[cfg_attr(not(feature = "pages"), allow(unused_variables))]
    fn pass(state: &mut State, config: &AutoConfig, bytes: &[u8], finish: bool, out: &mut dyn FnMut(Item<'_>)) {
        let State::Active(reader) = state else { return };
        #[cfg(feature = "pages")]
        let mut encoding = |item: Item<'_>| {
            out(item);
            if let (true, Item::Page(page)) = (config.encode_pages, item) {
                let meta = PacketMeta { source: config.source, ..PacketMeta::default() };
                encode_page(page, &mut |packet| out(Item::Packet { packet, meta }));
            }
        };
        #[cfg(feature = "pages")]
        let out: &mut dyn FnMut(Item<'_>) = &mut encoding;
        reader.feed(bytes, out);
        if finish {
            reader.finish(out);
        }
    }

    fn decide(&mut self, complete: bool, out: &mut dyn FnMut(Item<'_>)) {
        let held = self.window.get(..self.len).unwrap_or(&[]);
        match detect(held, complete) {
            Detection::Found { format, skip } => {
                out(Item::Detected(format));
                self.state = State::Active(AnyReader::new(format, skip, &self.config));
                // A T42 reader skips its own leading bytes.
                let start = if matches!(format, Format::T42(_)) { 0 } else { skip };
                let held = self.window.get(start..self.len).unwrap_or(&[]);
                Self::pass(&mut self.state, &self.config, held, false, out);
            }
            Detection::NeedMore => {}
            Detection::Unknown => {
                out(Item::Diagnostic(Diagnostic::at(0, DiagnosticKind::UnknownFormat)));
                self.state = State::Failed;
            }
        }
    }
}

impl Reader for AutoReader {
    fn feed(&mut self, bytes: &[u8], out: &mut dyn FnMut(Item<'_>)) {
        let mut rest = bytes;
        while matches!(self.state, State::Detecting) && !rest.is_empty() {
            let take = copy_prefix(self.window.get_mut(self.len..).unwrap_or(&mut []), rest);
            self.len = self.len.saturating_add(take);
            rest = rest.get(take..).unwrap_or(&[]);
            self.decide(false, out);
            if matches!(self.state, State::Detecting) && self.len == DETECTION_BYTES {
                // A full window decides; NeedMore cannot follow it.
                self.state = State::Failed;
            }
        }
        Self::pass(&mut self.state, &self.config, rest, false, out);
    }

    fn finish(&mut self, out: &mut dyn FnMut(Item<'_>)) {
        if matches!(self.state, State::Detecting) {
            if self.len == 0 {
                return;
            }
            self.decide(true, out);
        }
        Self::pass(&mut self.state, &self.config, &[], true, out);
    }
}

#[cfg(test)]
mod tests;
