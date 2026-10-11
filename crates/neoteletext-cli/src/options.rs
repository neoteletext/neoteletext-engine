//! The options every command accepts, turned into the application layer's
//! plain options (REQ-CLI-002).

use crate::{Common, LevelChoice, ParityChoice};
use clap::ValueEnum;
use neoteletext_app::input::Options;
use neoteletext_core::config::{ParityPolicy, PresentationLevel};
use neoteletext_formats::detect::Format;
use neoteletext_formats::sliced::ByteOrder;
use neoteletext_formats::t42::T42Variant;
use neoteletext_formats::transport::TsPacketSize;

/// Formats that can be named with `--format` (REQ-INP-005).
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatChoice {
    /// 42-byte packets.
    T42,
    /// 42-byte packets, bit-reversed.
    T42Reversed,
    /// 43-byte packets with the framing code.
    T42Framed,
    /// 45-byte packets with run-in and framing code.
    T42RunIn,
    /// MPEG transport stream, 188-byte packets.
    Ts,
    /// 192-byte transport stream packets.
    M2ts,
    /// 204-byte transport stream packets.
    Ts204,
    /// Raw PES packets.
    Pes,
    /// Linux sliced VBI records, little-endian.
    Sliced,
    /// Linux sliced VBI records, big-endian.
    SlicedBe,
    /// TTI page files.
    Tti,
    /// EP1 page files.
    Ep1,
    /// edit.tf or zxnet page addresses.
    Edittf,
    /// Raw screen dumps.
    Dump,
}

impl FormatChoice {
    const fn format(self) -> Format {
        match self {
            Self::T42 => Format::T42(T42Variant::Plain),
            Self::T42Reversed => Format::T42(T42Variant::Reversed),
            Self::T42Framed => Format::T42(T42Variant::Framed),
            Self::T42RunIn => Format::T42(T42Variant::RunIn),
            Self::Ts => Format::TransportStream(TsPacketSize::Standard),
            Self::M2ts => Format::TransportStream(TsPacketSize::M2ts),
            Self::Ts204 => Format::TransportStream(TsPacketSize::Fec),
            Self::Pes => Format::Pes,
            Self::Sliced => Format::Sliced(ByteOrder::Little),
            Self::SlicedBe => Format::Sliced(ByteOrder::Big),
            Self::Tti => Format::Tti,
            Self::Ep1 => Format::Ep1,
            Self::Edittf => Format::EditTf,
            Self::Dump => Format::ScreenDump,
        }
    }
}

impl Common {
    /// The application layer's options for these arguments.
    pub fn options(&self) -> Options {
        Options {
            input: self.input.clone(),
            format: self.format.map(FormatChoice::format),
            profile: self.profile.clone(),
            pid: self.pid,
            duration: self.duration,
            pages: self.pages,
            region: self.region,
            parity: self.parity.map(|parity| match parity {
                ParityChoice::KeepGood => ParityPolicy::KeepLastGood,
                ParityChoice::Space => ParityPolicy::AlwaysSpace,
                ParityChoice::Marker => ParityPolicy::ErrorMarker,
            }),
            level: self.level.map(|level| match level {
                LevelChoice::Level1 => PresentationLevel::Level1,
                LevelChoice::Level15 => PresentationLevel::Level15,
                LevelChoice::Level25 => PresentationLevel::Level25,
                LevelChoice::Level35 => PresentationLevel::Level35,
            }),
            no_rounding: self.no_rounding,
            single_start_box: self.single_start_box,
            verbose: self.verbose,
            no_sandbox: self.no_sandbox,
            allow_local: self.allow_local.clone(),
        }
    }
}
