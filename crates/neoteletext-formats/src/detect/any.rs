//! Any of the readers compiled in (decision E.61), and the stand-in for a
//! reader that is not. The variants of [`AnyReader`] exist per feature, so
//! their `cfg` attributes stay on the items: the one enum with two forms.

#[cfg(feature = "dump")]
use crate::dump::DumpReader;
#[cfg(feature = "edittf")]
use crate::edittf::EditTfReader;
#[cfg(feature = "ep1")]
use crate::ep1::Ep1Reader;
use crate::item::{Diagnostic, DiagnosticKind, Item, Reader};
#[cfg(feature = "transport")]
use crate::pes::PesReader;
#[cfg(feature = "sliced")]
use crate::sliced::SlicedReader;
use crate::t42::T42Reader;
#[cfg(feature = "transport")]
use crate::transport::{TsConfig, TsReader};
#[cfg(feature = "tti")]
use crate::tti::TtiReader;

use super::{AutoConfig, Format};
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
