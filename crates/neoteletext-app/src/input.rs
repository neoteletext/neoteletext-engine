//! Reading inputs into a session: files and standard input through the
//! format readers (area INP), live feeds through the web module.

use crate::session::Session;
use crate::{Failure, exit};
use neoteletext_core::config::{Config, ParityPolicy, PresentationLevel};
use neoteletext_core::types::PageAddress;
use neoteletext_formats::Item;
use neoteletext_formats::detect::{AutoConfig, AutoReader, Format};
use neoteletext_formats::item::Reader;
use neoteletext_formats::transport::TsConfig;
use std::fs::File;
use std::io::Read;
use std::net::IpAddr;

mod feed;

pub use feed::{is_live, open_live};

/// What to read and how to decode it: the options every `neottx` command
/// accepts (REQ-CLI-002), free of any argument parser.
// Each flag is a separate user choice.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct Options {
    /// A file, `-` for standard input, or a live feed or web service.
    pub input: String,
    /// The input format, overriding detection.
    pub format: Option<Format>,
    /// The compatibility profile's name.
    pub profile: String,
    /// The transport stream packet identifier of the teletext stream.
    pub pid: Option<u16>,
    /// Seconds to collect pages from a live feed before acting.
    pub duration: u64,
    /// Pages and sub-pages to keep in memory.
    pub pages: usize,
    /// The Table 32 group 0 to 15 (decision E.4), overriding the
    /// language a transport stream declares.
    pub region: Option<u8>,
    /// The parity failure policy (decision E.8), overriding the profile's.
    pub parity: Option<ParityPolicy>,
    /// The presentation level, overriding the profile's (REQ-CLI-007).
    pub level: Option<PresentationLevel>,
    /// Turn character rounding off (REQ-RND-012).
    pub no_rounding: bool,
    /// Accept a single Start Box (decision E.17).
    pub single_start_box: bool,
    /// Diagnostics: 1 prints them, 2 adds the state machine trace
    /// (REQ-FSM-010).
    pub verbose: u8,
    /// Run without the operating-system sandbox (REQ-SEC-017).
    pub no_sandbox: bool,
    /// Local-network addresses a web service may use (REQ-SEC-006).
    pub allow_local: Vec<IpAddr>,
    /// The proxy every connection goes through, `http://host:port` or
    /// `socks5://host:port` (REQ-WEB-032).
    pub proxy: Option<String>,
}

/// Bytes read from the input at a time.
pub const CHUNK: usize = 64 * 1024;

/// The configuration from the profile and the overrides (REQ-PRF-005).
pub fn config(options: &Options) -> Result<Config, Failure> {
    let mut config = Config::profile(&options.profile)
        .ok_or_else(|| (exit::USAGE, format!("unknown profile {:?} (en300706, saa5050, saa5246a)", options.profile)))?;
    if let Some(parity) = options.parity {
        config.parity_policy = parity;
    }
    if let Some(region) = options.region {
        config.default_region = region;
    }
    if let Some(level) = options.level {
        config.level = level;
    }
    if options.no_rounding {
        config.rounding = false;
    }
    if options.single_start_box {
        config.double_start_box = false;
    }
    config.trace = options.verbose >= 2;
    Ok(config)
}

/// Opens the input: a file, or standard input for `-`.
pub fn open(options: &Options) -> Result<Box<dyn Read>, Failure> {
    if is_live(&options.input) {
        return Err((exit::USAGE, "this command needs a file or standard input, not a live feed".into()));
    }
    if options.input == "-" {
        return Ok(Box::new(std::io::stdin().lock()));
    }
    File::open(&options.input)
        .map(|file| Box::new(file) as Box<dyn Read>)
        .map_err(|error| (exit::INPUT, format!("{}: {error}", options.input)))
}

/// Feeds every chunk of `input` to `reader`.
pub fn pump(input: &mut dyn Read, reader: &mut dyn Reader, out: &mut dyn FnMut(Item<'_>)) -> Result<(), Failure> {
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = input.read(&mut buffer).map_err(|error| (exit::INPUT, error.to_string()))?;
        if read == 0 {
            break;
        }
        // A reader that reports more than the buffer holds is broken.
        let chunk = buffer
            .get(..read)
            .ok_or((exit::INPUT, "the input reported more bytes than it was given room for".to_string()))?;
        reader.feed(chunk, out);
    }
    reader.finish(out);
    Ok(())
}

/// The detecting reader for the input, with the format and stream the
/// options name. `encode_pages` turns page files into packets.
#[must_use]
pub fn reader(options: &Options, encode_pages: bool) -> Box<AutoReader> {
    Box::new(AutoReader::new(AutoConfig {
        format: options.format,
        encode_pages,
        transport: TsConfig { pid: options.pid, ..TsConfig::default() },
        source: None,
    }))
}

/// Reads the whole input into a new session.
pub fn load(options: &Options) -> Result<Session, Failure> {
    load_for(options, None)
}

/// Reads the whole input into a new session; for live feeds, asks for and
/// waits for `wanted`.
pub fn load_for(options: &Options, wanted: Option<PageAddress>) -> Result<Session, Failure> {
    if is_live(&options.input) {
        return feed::load_live(options, wanted);
    }
    let config = config(options)?;
    let mut session = Session::new(config, options.pages);
    let mut input = open(options)?;
    let mut reader = reader(options, true);
    let mut packets = 0u64;
    let mut detected = None;
    let mut languages: Vec<(u16, [u8; 3])> = Vec::new();
    let mut selected_pid = None;
    let verbose = options.verbose;
    pump(input.as_mut(), reader.as_mut(), &mut |item| match item {
        Item::Packet { packet, meta } => {
            packets = packets.saturating_add(1);
            session.push(&packet, &meta);
        }
        Item::Detected(format) => {
            detected = Some(format);
            if verbose > 0 {
                eprintln!("neottx: detected format {format:?}");
            }
        }
        Item::Diagnostic(diagnostic) if verbose > 0 => {
            eprintln!("neottx: input byte {}: {:?}", diagnostic.offset, diagnostic.kind);
        }
        Item::StreamSelected { program, pid } => {
            selected_pid = Some(pid);
            if verbose > 0 {
                eprintln!("neottx: using teletext stream PID {pid:#06x}, program {program:?}");
            }
        }
        Item::TeletextEntry { pid, entry, .. } => languages.push((pid, entry.language)),
        // REQ-INP-026: a channel's log lines are for its author, shown
        // with -v and never on a page.
        Item::Log { line, page, line_number } if verbose > 0 => {
            let page = page.map_or_else(|| "before any page".to_owned(), |page| format!("page {:03X}", page.mpp()));
            eprintln!("neottx: log {:?} at line {line_number}, {page}: {}", line.level, line.text);
        }
        _ => {}
    })?;
    session.finish();
    if options.region.is_none()
        && let Some(&(_, language)) = languages.iter().find(|(pid, _)| Some(*pid) == selected_pid)
    {
        let mut config = *session.decoder().config();
        config.default_region = region_for_language(language);
        if verbose > 0 {
            eprintln!("neottx: region {} from language {}", config.default_region, String::from_utf8_lossy(&language));
        }
        session.decoder_mut().set_config(config);
    }
    // Events are kept only with -vv (`config.trace`).
    session.print_trace();
    if packets == 0 {
        let reason = if detected.is_none() { "the input format was not recognised" } else { "no teletext found" };
        return Err((exit::INPUT, format!("{}: {reason}", options.input)));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader of bytes that says it read more than the buffer holds.
    struct Overreporting;

    impl Read for Overreporting {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            Ok(buffer.len().saturating_add(1))
        }
    }

    /// A format reader that is never reached.
    struct Unfed;

    impl Reader for Unfed {
        fn feed(&mut self, _bytes: &[u8], _out: &mut dyn FnMut(Item<'_>)) {}
        fn finish(&mut self, _out: &mut dyn FnMut(Item<'_>)) {}
    }

    /// REQ-SEC-003: a broken input is refused, not a panic.
    #[test]
    fn an_input_reporting_more_than_it_was_given_is_refused() {
        let result = pump(&mut Overreporting, &mut Unfed, &mut |_| {});
        assert!(matches!(result, Err((exit::INPUT, _))), "{result:?}");
    }
}
