//! `neottx`, the neoteletext command-line program (area CLI).
//!
//! Exit status (REQ-CLI-005): 0 success, 1 page not found, 2 invalid usage,
//! 3 input unreadable or unrecognised, 4 page checker errors.

mod commands;
mod contents;
mod font;
mod options;
mod output;
mod terminal;
#[cfg(feature = "view")]
mod view;
mod write_text;

use clap::{Args, Parser, Subcommand, ValueEnum};
use neoteletext_app::{exit, sandbox};
use std::path::PathBuf;
use std::process::ExitCode;

/// neoteletext: a software teletext receiver (ETSI EN 300 706 Levels 1.0, 1.5, 2.5 and 3.5).
#[derive(Parser, Debug)]
#[command(name = "neottx", version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Options every command accepts (REQ-CLI-002).
// Each flag is a separate command-line switch.
#[allow(clippy::struct_excessive_bools)]
#[derive(Args, Debug, Clone)]
pub struct Common {
    /// Input: a file, `-` for standard input, or `zxnet:<name>` for a live feed.
    pub input: String,
    /// Input format, overriding detection.
    #[arg(long, value_enum)]
    pub format: Option<options::FormatChoice>,
    /// Compatibility profile.
    #[arg(long, default_value = "en300706")]
    pub profile: String,
    /// Transport stream packet identifier of the teletext stream to use.
    #[arg(long)]
    pub pid: Option<u16>,
    /// Seconds to collect pages from a live feed before acting (REQ-CLI-002).
    #[arg(long, default_value_t = 20)]
    pub duration: u64,
    /// Pages and sub-pages to keep in memory.
    #[arg(long, default_value_t = neoteletext_app::session::DEFAULT_PAGES)]
    pub pages: usize,
    /// Character set region, the Table 32 group 0 to 15 (decision E.4).
    /// By default it follows the language a transport stream declares.
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..16))]
    pub region: Option<u8>,
    /// Parity failure policy (decision E.8).
    #[arg(long, value_enum)]
    pub parity: Option<ParityChoice>,
    /// Presentation level, overriding the profile's (REQ-CLI-007).
    #[arg(long, value_enum)]
    pub level: Option<LevelChoice>,
    /// Turn character rounding off (REQ-RND-012).
    #[arg(long)]
    pub no_rounding: bool,
    /// Accept a single Start Box (decision E.17).
    #[arg(long)]
    pub single_start_box: bool,
    /// Print diagnostics, and with -vv the state machine trace (REQ-FSM-010).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Run without the operating-system sandbox (REQ-SEC-017).
    #[arg(long)]
    pub no_sandbox: bool,
    /// Allow a web service on this local-network address, for example a
    /// test server on 127.0.0.1 (REQ-SEC-006 is otherwise strict).
    #[arg(long, value_name = "IP")]
    pub allow_local: Vec<std::net::IpAddr>,
}

/// Parity policy names (decision E.8).
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum ParityChoice {
    /// Keep the last character that passed.
    KeepGood,
    /// Show failed characters as spaces.
    Space,
    /// Show a visible error marker.
    Marker,
}

/// Presentation levels (REQ-CLI-007, decision E.40).
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelChoice {
    /// Level 1.0.
    #[value(name = "1")]
    Level1,
    /// Level 1.5.
    #[value(name = "1.5")]
    Level15,
    /// Level 2.5.
    #[value(name = "2.5")]
    Level25,
    /// Level 3.5.
    #[value(name = "3.5")]
    Level35,
}

/// Terminal display choices (REQ-RND-029).
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphicsChoice {
    /// Detect.
    Auto,
    /// iTerm2 inline images.
    Iterm2,
    /// Characters with ANSI colour.
    Text,
}

/// Raster options shared by `show`, `render` and `view`.
#[derive(Args, Debug, Clone)]
pub struct RasterArgs {
    /// Whole-number scale factor; 0 fits the terminal.
    #[arg(long, default_value_t = 0)]
    pub scale: usize,
    /// Stretch height by 1.2 for 4:3 proportions (REQ-RND-004).
    #[arg(long)]
    pub aspect: bool,
    /// Show concealed text.
    #[arg(long)]
    pub reveal: bool,
    /// Draw text in this loaded font, a 12 x 10 or 12 x 20 BDF file
    /// (REQ-FNT-002, REQ-FNT-008), instead of Bedstead.
    #[arg(long)]
    pub font: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List the pages and sub-pages found, with counts and error statistics.
    List {
        #[command(flatten)]
        common: Common,
    },
    /// Display one page in the terminal.
    Show {
        #[command(flatten)]
        common: Common,
        /// Page, e.g. 100 or 100:0003.
        page: String,
        #[command(flatten)]
        raster: RasterArgs,
        /// Graphics protocol.
        #[arg(long, value_enum, default_value = "auto")]
        graphics: GraphicsChoice,
        /// Write separated mosaics with contiguous characters, for older fonts.
        #[arg(long)]
        legacy_mosaics: bool,
        /// Use 8 colours instead of 24-bit colour.
        #[arg(long)]
        colours8: bool,
    },
    /// Browse pages interactively in the terminal.
    View {
        #[command(flatten)]
        common: Common,
        #[command(flatten)]
        raster: RasterArgs,
        /// Graphics protocol.
        #[arg(long, value_enum, default_value = "auto")]
        graphics: GraphicsChoice,
        /// Write separated mosaics with contiguous characters, for older fonts.
        #[arg(long)]
        legacy_mosaics: bool,
        /// Use 8 colours instead of 24-bit colour.
        #[arg(long)]
        colours8: bool,
    },
    /// Write one page, or all, as image files.
    Render {
        #[command(flatten)]
        common: Common,
        /// Page, e.g. 100, 100:0003, or `all`.
        page: String,
        /// Output file, or directory for `all`.
        #[arg(short, long)]
        output: PathBuf,
        #[command(flatten)]
        raster: RasterArgs,
        /// Image format.
        #[arg(long, value_enum, default_value = "png")]
        image: commands::ImageFormat,
    },
    /// Write one page, or all, as UTF-8 text.
    Text {
        #[command(flatten)]
        common: Common,
        /// Page, e.g. 100, 100:0003, or `all`.
        page: String,
        /// Show concealed text.
        #[arg(long)]
        reveal: bool,
        /// Write separated mosaics with contiguous characters, for older fonts.
        #[arg(long)]
        legacy_mosaics: bool,
        /// Write every cell with its colours and attributes instead: the
        /// test interface of the zvbi oracle (REQ-TST-023).
        #[arg(long, hide = true)]
        cells: bool,
    },
    /// Convert between formats.
    Convert {
        #[command(flatten)]
        common: Common,
        /// Output file.
        #[arg(short, long)]
        output: PathBuf,
        /// Output format.
        #[arg(long, value_enum)]
        to: commands::ConvertTarget,
        /// Only this magazine and page, e.g. 100.
        #[arg(long)]
        page: Option<String>,
    },
    /// Convert raw sampled video to T42 with quality statistics.
    Slice {
        #[command(flatten)]
        common: Common,
        /// Output T42 file.
        #[arg(short, long)]
        output: PathBuf,
        /// Capture geometry preset.
        #[arg(long, default_value = "bt8x8")]
        preset: String,
    },
    /// List the teletext streams, languages and subtitle pages of a transport stream.
    Services {
        #[command(flatten)]
        common: Common,
    },
    /// Print the broadcast service data of packet 8/30.
    ServiceInfo {
        #[command(flatten)]
        common: Common,
    },
    /// Apply the sandbox, then try to open a file, start a program and open
    /// a network connection; all three must fail (REQ-SEC-017 verification).
    #[command(hide = true)]
    SandboxTest,
    /// The fetcher process of live mode (REQ-SEC-018); started by `view`.
    #[command(hide = true)]
    Fetch {
        /// The live source.
        address: String,
        /// Run without the sandbox.
        #[arg(long)]
        no_sandbox: bool,
        /// Local-network addresses allowed.
        #[arg(long)]
        allow_local: Vec<std::net::IpAddr>,
    },
    /// Write a channel's contents.txt, to standard output, from a folder
    /// of page files (REQ-CLI-008).
    Contents {
        /// Common options; the input is the channel's folder.
        #[command(flatten)]
        common: Common,
    },
    /// Check TTI page files for authoring problems.
    Check {
        /// TTI files.
        files: Vec<PathBuf>,
        /// Service descriptor to resolve link targets against.
        #[arg(long)]
        descriptor: Option<PathBuf>,
    },
    /// Write a TTI page from a UTF-8 text file in one of neoteletext's own
    /// character sets (REQ-CLI-010, REQ-CHR-012).
    WriteText {
        /// The set: hausa-fula, west-african, ewe, kabyle-tamazight,
        /// azerbaijani, kanuri-mossi, yoruba-igbo, vietnamese, ukrainian,
        /// belarusian, macedonian, serbian, kyrgyz-mongolian, tatar, tajik,
        /// georgian, armenian or tifinagh.
        #[arg(long)]
        set: String,
        /// The page number, 100 to 8FF.
        #[arg(long)]
        page: String,
        /// The UTF-8 text.
        input: PathBuf,
        /// The TTI file to write.
        output: PathBuf,
        /// Run without the operating-system sandbox (REQ-SEC-017).
        #[arg(long)]
        no_sandbox: bool,
    },
    /// Work with the fonts the browser loads (area FNT).
    Font {
        #[command(subcommand)]
        action: FontCommand,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() { ExitCode::from(exit::USAGE) } else { ExitCode::SUCCESS };
        }
    };
    let result = match cli.command {
        Command::List { common } => commands::list(&common.options()),
        Command::Show { common, page, raster, graphics, legacy_mosaics, colours8 } => {
            commands::show(&common.options(), &page, &raster, graphics, legacy_mosaics, colours8)
        }
        Command::View { common, raster, graphics, legacy_mosaics, colours8 } => {
            commands::view(&common.options(), &raster, graphics, legacy_mosaics, colours8)
        }
        Command::Render { common, page, output, raster, image } => {
            commands::render(&common.options(), &page, &output, &raster, image)
        }
        Command::Text { common, page, reveal, legacy_mosaics, cells } => {
            commands::text(&common.options(), &page, reveal, legacy_mosaics, cells)
        }
        Command::Convert { common, output, to, page } => {
            commands::convert(&common.options(), &output, to, page.as_deref())
        }
        Command::Slice { common, output, preset } => commands::slice(&common.options(), &output, &preset),
        Command::Services { common } => commands::services(&common.options()),
        Command::ServiceInfo { common } => commands::service_info(&common.options()),
        Command::Check { files, descriptor } => commands::check(&files, descriptor.as_deref()),
        Command::Font { action: FontCommand::Convert { input, output, map, no_sandbox } } => {
            font::convert_font(&input, &output, map.as_deref(), no_sandbox)
        }
        Command::Font { action: FontCommand::Sets { font, library_line, no_sandbox } } => {
            font::font_sets(&font, library_line.as_deref(), no_sandbox)
        }
        Command::Contents { common } => contents::contents(&common.options()),
        Command::WriteText { set, page, input, output, no_sandbox } => {
            write_text::write_text(&set, &page, &input, &output, no_sandbox)
        }
        Command::SandboxTest => sandbox::self_test(),
        Command::Fetch { address, no_sandbox, allow_local } => commands::fetch(&address, no_sandbox, &allow_local),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("neottx: {message}");
            ExitCode::from(code)
        }
    }
}

/// The font commands (REQ-FNT-010).
#[derive(Subcommand, Debug)]
enum FontCommand {
    /// Convert a BDF font of another cell size to the 12 x 10 form, only
    /// where the conversion is exact: 5 x 10 and 6 x 10 with every pixel
    /// doubled in width, 5 x 9 and 6 x 9 the same below a blank line, and
    /// 12 x 20 with identical line pairs halved.
    Convert {
        /// The source font.
        input: PathBuf,
        /// The 12 x 10 font to write.
        output: PathBuf,
        /// For a font not in ISO10646: one `code<TAB>U+XXXX` line per
        /// character, the code as the font's ENCODING writes it.
        #[arg(long)]
        map: Option<PathBuf>,
        /// Run without the operating-system sandbox (REQ-SEC-017).
        #[arg(long)]
        no_sandbox: bool,
    },
    /// Report the character sets a loaded font conforms for (REQ-FNT-011):
    /// for each set, whether it conforms, the positions it
    /// draws, the SAA5050 cell profile, and the first rule that fails.
    Sets {
        /// The loaded font, 12 x 10 or 12 x 20.
        font: PathBuf,
        /// Print only the font library's `c` line (REQ-FNT-004) for the
        /// font under this file name.
        #[arg(long, value_name = "NAME")]
        library_line: Option<String>,
        /// Run without the operating-system sandbox (REQ-SEC-017).
        #[arg(long)]
        no_sandbox: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(["neottx"].iter().chain(arguments))
    }

    fn level(arguments: &[&str]) -> Option<LevelChoice> {
        match parse(arguments).map(|cli| cli.command) {
            Ok(Command::List { common }) => common.level,
            other => panic!("not a list command: {other:?}"),
        }
    }

    /// REQ-CLI-007: each level is accepted, any other value is invalid
    /// usage, and the former `--level1-only` option is gone.
    #[test]
    fn level_option_takes_the_four_levels() {
        assert_eq!(level(&["list", "in.t42"]), None);
        for (text, expected) in [
            ("1", LevelChoice::Level1),
            ("1.5", LevelChoice::Level15),
            ("2.5", LevelChoice::Level25),
            ("3.5", LevelChoice::Level35),
        ] {
            assert_eq!(level(&["list", "in.t42", "--level", text]), Some(expected), "{text}");
        }
        for invalid in ["2", "1.0", "4", "level1"] {
            let error = parse(&["list", "in.t42", "--level", invalid]).unwrap_err();
            assert!(error.use_stderr(), "{invalid} is invalid usage (exit status {})", exit::USAGE);
        }
        assert!(parse(&["list", "in.t42", "--level1-only"]).is_err());
    }
}
