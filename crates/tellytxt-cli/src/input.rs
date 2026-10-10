//! Reading inputs into a session: files and standard input through the
//! format readers (area INP), raw video through the slicer, live feeds
//! through the web module.

use crate::commands::{ConvertTarget, Failure};
use crate::session::Session;
use crate::view::Source;
use crate::{Common, ParityChoice, exit};
use clap::ValueEnum;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use tellytxt_core::config::{Config, ParityPolicy};
use tellytxt_core::fsm::PageState;
use tellytxt_core::types::PageAddress;
use tellytxt_formats::Item;
use tellytxt_formats::detect::{AutoConfig, AutoReader, Format};
use tellytxt_formats::item::Reader;
use tellytxt_formats::sliced::ByteOrder;
use tellytxt_formats::t42::T42Variant;
use tellytxt_formats::transport::{TsConfig, TsPacketSize};

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

/// Bytes read from the input at a time.
const CHUNK: usize = 64 * 1024;

/// The configuration from the profile and the overrides (REQ-PRF-005).
pub fn config(common: &Common) -> Result<Config, Failure> {
    let mut config = Config::profile(&common.profile)
        .ok_or_else(|| (exit::USAGE, format!("unknown profile {:?} (en300706, saa5050, saa5246a)", common.profile)))?;
    if let Some(parity) = common.parity {
        config.parity_policy = match parity {
            ParityChoice::KeepGood => ParityPolicy::KeepLastGood,
            ParityChoice::Space => ParityPolicy::AlwaysSpace,
            ParityChoice::Marker => ParityPolicy::ErrorMarker,
        };
    }
    if let Some(region) = common.region {
        config.default_region = region;
    }
    if common.level1_only {
        config.level15 = false;
    }
    if common.no_rounding {
        config.rounding = false;
    }
    if common.single_start_box {
        config.double_start_box = false;
    }
    config.trace = common.verbose >= 2;
    Ok(config)
}

/// True for inputs that are live feeds or web services.
pub fn is_live(input: &str) -> bool {
    input.starts_with("zxnet:")
        || input.starts_with("wss://")
        || input.starts_with("http://")
        || input.starts_with("https://")
}

/// Collects a live source for `--duration` seconds, or until `wanted` is
/// stored and complete (REQ-CLI-002).
fn load_live(common: &Common, wanted: Option<PageAddress>) -> Result<Session, Failure> {
    let config = config(common)?;
    let mut session = Session::new(config, common.pages);
    let mut source = crate::live::open(&common.input, common.no_sandbox, &common.allow_local)?;
    let start = std::time::Instant::now();
    let limit = std::time::Duration::from_secs(common.duration);
    if let Some(page) = wanted {
        source.request(page, tellytxt_core::types::SubCode::ANY);
    }
    let mut asked_initial = wanted.is_some();
    while start.elapsed() < limit {
        let now = tellytxt_core::types::Instant(u64::try_from(start.elapsed().as_micros()).unwrap_or(u64::MAX));
        let alive = source.poll(&mut session, now);
        for notice in source.notices() {
            match notice {
                crate::pipe::Notice::InitialPage(mpp) if !asked_initial => {
                    if let Some(page) = PageAddress::from_mpp(mpp) {
                        source.request(page, tellytxt_core::types::SubCode::ANY);
                        asked_initial = true;
                    }
                }
                crate::pipe::Notice::Message(text) => eprintln!("ttx: {text}"),
                _ => {}
            }
        }
        let done = wanted
            .and_then(|page| session.page(page, tellytxt_core::types::SubCode::ANY))
            .is_some_and(|page| page.state == PageState::Complete);
        if done || !alive {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    session.finish();
    Ok(session)
}

/// Opens the input: a file, or standard input for `-`.
fn open(common: &Common) -> Result<Box<dyn Read>, Failure> {
    if is_live(&common.input) {
        return Err((exit::USAGE, "this command needs a file or standard input, not a live feed".into()));
    }
    if common.input == "-" {
        return Ok(Box::new(std::io::stdin().lock()));
    }
    File::open(&common.input)
        .map(|file| Box::new(file) as Box<dyn Read>)
        .map_err(|error| (exit::INPUT, format!("{}: {error}", common.input)))
}

/// Feeds every chunk of `input` to `reader`.
fn pump(input: &mut dyn Read, reader: &mut dyn Reader, out: &mut dyn FnMut(Item<'_>)) -> Result<(), Failure> {
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = input.read(&mut buffer).map_err(|error| (exit::INPUT, error.to_string()))?;
        if read == 0 {
            break;
        }
        reader.feed(&buffer[..read], out);
    }
    reader.finish(out);
    Ok(())
}

fn reader(common: &Common, encode_pages: bool) -> Box<AutoReader> {
    Box::new(AutoReader::new(AutoConfig {
        format: common.format.map(FormatChoice::format),
        encode_pages,
        transport: TsConfig { pid: common.pid, ..TsConfig::default() },
        source: None,
    }))
}

/// Reads the whole input into a new session.
pub fn load(common: &Common) -> Result<Session, Failure> {
    load_for(common, None)
}

/// Reads the whole input into a new session; for live feeds, asks for and
/// waits for `wanted`.
pub fn load_for(common: &Common, wanted: Option<PageAddress>) -> Result<Session, Failure> {
    if is_live(&common.input) {
        return load_live(common, wanted);
    }
    let config = config(common)?;
    let mut session = Session::new(config, common.pages);
    let mut input = open(common)?;
    let mut reader = reader(common, true);
    let mut packets = 0u64;
    let mut detected = None;
    let mut languages: Vec<(u16, [u8; 3])> = Vec::new();
    let mut selected_pid = None;
    let verbose = common.verbose;
    pump(input.as_mut(), reader.as_mut(), &mut |item| match item {
        Item::Packet { packet, meta } => {
            packets += 1;
            session.push(&packet, &meta);
        }
        Item::Detected(format) => {
            detected = Some(format);
            if verbose > 0 {
                eprintln!("ttx: detected format {format:?}");
            }
        }
        Item::Diagnostic(diagnostic) if verbose > 0 => {
            eprintln!("ttx: input byte {}: {:?}", diagnostic.offset, diagnostic.kind);
        }
        Item::StreamSelected { program, pid } => {
            selected_pid = Some(pid);
            if verbose > 0 {
                eprintln!("ttx: using teletext stream PID {pid:#06x}, program {program:?}");
            }
        }
        Item::TeletextEntry { pid, entry, .. } => languages.push((pid, entry.language)),
        _ => {}
    })?;
    session.finish();
    if common.region.is_none()
        && let Some(&(_, language)) = languages.iter().find(|(pid, _)| Some(*pid) == selected_pid)
    {
        let mut config = *session.decoder().config();
        config.default_region = region_for_language(language);
        if verbose > 0 {
            eprintln!("ttx: region {} from language {}", config.default_region, String::from_utf8_lossy(&language));
        }
        session.decoder_mut().set_config(config);
    }
    if verbose >= 2 {
        for event in &session.trace {
            eprintln!("ttx: {event:?}");
        }
    }
    if packets == 0 {
        let reason = if detected.is_none() { "the input format was not recognised" } else { "no teletext found" };
        return Err((exit::INPUT, format!("{}: {reason}", common.input)));
    }
    Ok(session)
}

/// The Table 32 group for an ISO 639-2 language code (decision 25 of
/// IMPLEMENTATION-LOG.md).
fn region_for_language(language: [u8; 3]) -> u8 {
    let mut lower = language;
    lower.make_ascii_lowercase();
    match &lower {
        b"pol" => 1,
        b"tur" => 2,
        b"srp" | b"scc" | b"hrv" | b"scr" | b"slv" | b"ron" | b"rum" => 3,
        b"rus" | b"bul" | b"ukr" | b"est" | b"lav" | b"lit" => 4,
        b"gre" | b"ell" => 6,
        b"ara" => 8,
        b"heb" => 10,
        _ => 0,
    }
}

/// A source with nothing more to give: the input was read at start.
struct Finished;

impl Source for Finished {
    fn poll(&mut self, _session: &mut Session, _now: tellytxt_core::types::Instant) -> bool {
        false
    }
}

/// Opens an input for `ttx view`: a file read whole, or a live feed.
pub fn open_live(common: &Common) -> Result<(Session, Box<dyn Source>), Failure> {
    if is_live(&common.input) {
        let config = config(common)?;
        let session = Session::new(config, common.pages);
        let source = crate::live::open(&common.input, common.no_sandbox, &common.allow_local)?;
        return Ok((session, source));
    }
    Ok((load(common)?, Box::new(Finished)))
}

/// `ttx services`: the teletext streams of a transport stream (REQ-INP-007).
pub fn services(common: &Common) -> Result<(), Failure> {
    let mut input = open(common)?;
    let mut reader = reader(common, false);
    let mut found = 0u32;
    let mut out = std::io::stdout().lock();
    pump(input.as_mut(), reader.as_mut(), &mut |item| match item {
        Item::TeletextEntry { program, pid, entry } => {
            found += 1;
            let _ = writeln!(
                out,
                "program {program:>5}  PID {pid:#06x}  language {}  {:?}  page {}",
                String::from_utf8_lossy(&entry.language),
                entry.kind,
                String::from_utf8_lossy(&entry.page.ascii())
            );
        }
        Item::SubtitlePage(page) => {
            let _ = writeln!(out, "default subtitle page: {}", String::from_utf8_lossy(&page.ascii()));
        }
        _ => {}
    })?;
    if found == 0 {
        return Err((exit::NOT_FOUND, "no teletext descriptors found".into()));
    }
    Ok(())
}

/// `ttx convert` (REQ-INP-014, REQ-INP-018).
pub fn convert(common: &Common, output: &Path, to: ConvertTarget, filter: Option<PageAddress>) -> Result<(), Failure> {
    let file = File::create(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    match to {
        ConvertTarget::T42 => {
            let filter = tellytxt_formats::write::PacketFilter {
                magazine: filter.map(|address| address.magazine),
                page: filter.map(|address| address.page),
            };
            let mut writer = tellytxt_formats::write::T42Writer::with_filter(std::io::BufWriter::new(file), filter);
            let mut input = open(common)?;
            let mut reader = reader(common, true);
            let mut failure = None;
            pump(input.as_mut(), reader.as_mut(), &mut |item| {
                if let Item::Packet { packet, .. } = item
                    && let Err(error) = writer.write_packet(&packet)
                {
                    failure = Some(error);
                }
            })?;
            if let Some(error) = failure {
                return Err((exit::INPUT, error.to_string()));
            }
            writer.into_inner().flush().map_err(|error| (exit::INPUT, error.to_string()))
        }
        ConvertTarget::Tti => {
            let session = load(common)?;
            let pages: Vec<tellytxt_formats::page::PageFile> = session
                .pages()
                .into_iter()
                .filter(|page| page.state != PageState::Empty)
                .filter(|page| filter.is_none_or(|address| page.key.address == address))
                .map(crate::convert::page_file)
                .collect();
            let references: Vec<&tellytxt_formats::page::PageFile> = pages.iter().collect();
            let mut out = std::io::BufWriter::new(file);
            tellytxt_formats::write::write_tti(&references, &mut out)
                .map_err(|error| (exit::INPUT, error.to_string()))?;
            out.flush().map_err(|error| (exit::INPUT, error.to_string()))
        }
    }
}

/// `ttx slice`: raw sampled video to T42 (REQ-INP-011, area SLC).
pub fn slice(common: &Common, output: &Path, preset: &str) -> Result<(), Failure> {
    use tellytxt_slicer::{Preset, RawGeometry, RawReader};
    let preset = match preset {
        "bt8x8" => Preset::Bt8x8,
        "cx88" => Preset::Cx88,
        "saa7131" => Preset::Saa7131,
        "tbc" => Preset::Tbc,
        "tbc-vbi" => Preset::TbcVbi,
        other => return Err((exit::USAGE, format!("unknown preset {other:?} (bt8x8, cx88, saa7131, tbc, tbc-vbi)"))),
    };
    let mut config = tellytxt_slicer::Config::default();
    config.framing_tolerance = self::config(common)?.framing_tolerance;
    let mut raw =
        RawReader::new(RawGeometry::preset(preset), config).map_err(|error| (exit::USAGE, format!("{error:?}")))?;
    let mut input = open(common)?;
    let file = File::create(output).map_err(|error| (exit::INPUT, format!("{}: {error}", output.display())))?;
    let mut writer = tellytxt_formats::write::T42Writer::new(std::io::BufWriter::new(file));
    let mut buffer = vec![0u8; CHUNK];
    let mut failure = None;
    loop {
        let read = input.read(&mut buffer).map_err(|error| (exit::INPUT, error.to_string()))?;
        if read == 0 {
            break;
        }
        raw.feed(&buffer[..read], |packet, _meta| {
            if let Err(error) = writer.write_packet(&packet) {
                failure = Some(error);
            }
        });
    }
    if let Some(error) = failure {
        return Err((exit::INPUT, error.to_string()));
    }
    writer.into_inner().flush().map_err(|error| (exit::INPUT, error.to_string()))?;
    let counters = raw.slicer().counters();
    eprintln!("{counters:?}");
    Ok(())
}

/// `ttx check`: the page checker (REQ-CLI-006). Exit status 4 on errors.
pub fn check(files: &[std::path::PathBuf], descriptor: Option<&Path>) -> Result<(), Failure> {
    use tellytxt_formats::check::{CheckOptions, Severity, check_page};
    let services: Vec<Vec<u8>> = match descriptor {
        Some(path) => crate::live::descriptor_service_names(path)?,
        None => Vec::new(),
    };
    let names: Vec<&[u8]> = services.iter().map(Vec::as_slice).collect();
    let options = CheckOptions { services: descriptor.map(|_| names.as_slice()) };
    let mut errors = 0u32;
    for file in files {
        let mut input = File::open(file).map_err(|error| (exit::INPUT, format!("{}: {error}", file.display())))?;
        let mut reader = tellytxt_formats::tti::TtiReader::default();
        pump(&mut input, &mut reader, &mut |item| {
            if let Item::Page(page) = item {
                check_page(page, &options, &mut |finding| {
                    let severity = match finding.severity {
                        Severity::Error => {
                            errors += 1;
                            "error"
                        }
                        Severity::Warning => "warning",
                    };
                    println!(
                        "{}:{}: page {} sub-page {}: {severity}: {:?}",
                        file.display(),
                        finding.line,
                        String::from_utf8_lossy(&finding.page.ascii()),
                        finding.subpage,
                        finding.problem
                    );
                });
            }
        })?;
    }
    if errors > 0 { Err((exit::CHECK_ERRORS, format!("{errors} error(s)"))) } else { Ok(()) }
}
